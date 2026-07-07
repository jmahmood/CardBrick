"""Application-level review operations.

The rest of the app talks to ``ReviewService`` and never manipulates
FSRS internals directly. The service composes storage.py (persistence)
and scheduler.py (the py-fsrs wrapper) into the operations the spec
names:

    get_due_cards(profile, category_filter, limits)
    answer_card(card_id, rating, elapsed_ms)
    undo_last_answer()
    bury_card(card_id)
    suspend_card(card_id)

Time is injected (``now_fn``) so daily-rollover behaviour is testable.
All daily accounting is derived from the append-only review log rather
than in-memory counters, which makes it correct across undo, crashes,
and restarts.
"""

import json
from datetime import datetime, timedelta, timezone

from .scheduler import ReviewScheduler, iso, now_utc

# How far ahead of schedule an intra-session learning card may be shown.
# Matches Anki's default "learn ahead" behaviour: a card rated Again is
# due in ~1 minute and should come back within the same session.
LEARN_AHEAD = timedelta(minutes=20)

DEFAULT_LIMITS = {
    "daily_new_cards": 10,
    "daily_review_cards": 40,
    "session_card_limit": 50,
    "session_time_minutes": 15,
}


def local_day_start(now):
    """UTC datetime of local midnight for the day containing ``now``."""
    local = now.astimezone()
    start = local.replace(hour=0, minute=0, second=0, microsecond=0)
    return start.astimezone(timezone.utc)


def next_local_midnight(now):
    """UTC datetime of the next local midnight (used for burying)."""
    local = now.astimezone()
    start = local.replace(hour=0, minute=0, second=0, microsecond=0)
    return (start + timedelta(days=1)).astimezone(timezone.utc)


def card_matches_categories(tags_str, active_categories):
    """True if a card's tag string matches the active category filter.

    ``active_categories`` of None means "all categories". An explicit
    list excludes untagged cards and cards with no matching tag.
    """
    if active_categories is None:
        return True
    return bool(set(tags_str.split()) & set(active_categories))


class ReviewService:
    def __init__(self, storage, scheduler=None, now_fn=None):
        self.storage = storage
        self.scheduler = scheduler or ReviewScheduler()
        self.now_fn = now_fn or now_utc

    def now(self):
        return self.now_fn()

    # -- queue building --------------------------------------------------------

    def get_due_cards(self, profile=None, category_filter=None,
                      deck_filter=None, limits=None):
        """Build the capped review queue for a study session.

        Order: due review cards (most overdue first), then new cards.
        Suspended and buried cards never appear. Daily caps subtract
        work already logged today, so a backlog can never flood a child
        and restarting mid-day continues from where the day left off.

        ``category_filter``/``deck_filter`` of None fall back to the
        profile's ``active_categories``/``active_decks``; an explicit
        [] means "none active" (an empty queue), same as the profile
        field would.

        Returns a list of card rows (joined with review state).
        """
        limits = dict(DEFAULT_LIMITS, **(limits or {}))
        if profile:
            for key in DEFAULT_LIMITS:
                if profile.get(key) is not None:
                    limits[key] = profile[key]
            if category_filter is None:
                category_filter = profile.get("active_categories")
            if deck_filter is None:
                deck_filter = profile.get("active_decks")

        now = self.now()
        new_done, review_done, _ = self.storage.daily_counts(
            iso(local_day_start(now)))
        remaining_new = max(limits["daily_new_cards"] - new_done, 0)
        remaining_review = max(
            limits["daily_review_cards"] - review_done, 0)

        queue = []
        for row in self.storage.queue_candidates(iso(now), new_cards=False,
                                                 decks=deck_filter):
            if remaining_review <= 0:
                break
            if card_matches_categories(row["tags"], category_filter):
                queue.append(row)
                remaining_review -= 1
        for row in self.storage.queue_candidates(iso(now), new_cards=True,
                                                 decks=deck_filter):
            if remaining_new <= 0:
                break
            if card_matches_categories(row["tags"], category_filter):
                queue.append(row)
                remaining_new -= 1

        return queue[:limits["session_card_limit"]]

    def counts_for_queue(self, profile=None, category_filter=None,
                         deck_filter=None, limits=None):
        """(review, new) counts of what get_due_cards would return."""
        queue = self.get_due_cards(profile, category_filter, deck_filter,
                                   limits)
        new = sum(1 for row in queue if row["reps"] == 0)
        return len(queue) - new, new

    # -- answering ---------------------------------------------------------------

    def answer_card(self, card_id, rating, elapsed_ms=None, session_id=None):
        """Apply a 1-4 rating; returns (new_state, came_back_soon).

        The previous state is snapshotted into the append-only review
        log *in the same transaction* as the state mutation, so undo is
        an exact restore, never a guess. ``came_back_soon`` tells the
        session to requeue the card (learning steps due within
        LEARN_AHEAD).
        """
        now = self.now()
        state_row = self.storage.get_review_state(card_id)
        if state_row is None:
            raise KeyError(f"card {card_id} has no review state")
        previous = dict(state_row)
        was_new = previous["reps"] == 0

        new_state = self.scheduler.review(state_row, rating, now=now)

        self.storage.append_review_log(
            card_id=card_id, reviewed_at=iso(now), rating=int(rating),
            elapsed_ms=elapsed_ms, was_new=was_new,
            previous_state_json=json.dumps(previous),
            new_state_json=json.dumps(new_state),
            session_id=session_id)
        # save_review_state commits, making the log append and the state
        # mutation one durable unit (auto-save after every answer).
        self.storage.save_review_state(new_state, last_reviewed_at=iso(now))

        due = datetime.fromisoformat(new_state["due"])
        return new_state, due <= now + LEARN_AHEAD

    def undo_last_answer(self, session_id):
        """Roll back the most recent answer of a session exactly.

        Restores the snapshotted prior FSRS state (due date, reps,
        lapses, card state) and marks the log entry undone so daily and
        session counters — all derived from the log — correct
        themselves. Returns the card_id, or None if nothing to undo.
        """
        entry = self.storage.last_review_log(session_id)
        if entry is None:
            return None
        previous = json.loads(entry["previous_state_json"])
        self.storage.restore_review_state(previous)
        self.storage.mark_log_undone(entry["id"])
        self.storage.commit()
        return entry["card_id"]

    # -- bury / suspend ------------------------------------------------------------

    def bury_card(self, card_id, session_id=None):
        """Hide a card until tomorrow ("not now")."""
        now = self.now()
        self.storage.set_buried_until(
            card_id, iso(next_local_midnight(now)), now_iso=iso(now))
        if session_id is not None:
            self.storage.bump_session_counter(session_id, "buried_count")

    def suspend_card(self, card_id, session_id=None):
        """Hide a card indefinitely ("bad card, parent should fix")."""
        now_iso = iso(self.now())
        self.storage.set_suspended(card_id, True, now_iso=now_iso)
        if session_id is not None:
            self.storage.bump_session_counter(session_id, "suspended_count")

    def unsuspend_card(self, card_id):
        self.storage.set_suspended(card_id, False, now_iso=iso(self.now()))

    # -- progress ---------------------------------------------------------------------

    def recent_daily_totals(self, days=7):
        """[(local_date, reviews, new)] for the last ``days`` days."""
        now = self.now()
        since = local_day_start(now) - timedelta(days=days - 1)
        per_day = {}
        first_seen_new = set()
        for row in self.storage.daily_history(iso(since)):
            when = datetime.fromisoformat(row["reviewed_at"]).astimezone()
            day = when.date()
            entry = per_day.setdefault(day, {"reviews": set(), "new": set()})
            if row["was_new"] and row["card_id"] not in first_seen_new:
                first_seen_new.add(row["card_id"])
                entry["new"].add(row["card_id"])
            else:
                entry["reviews"].add(row["card_id"])
        result = []
        for offset in range(days - 1, -1, -1):
            day = (now.astimezone() - timedelta(days=offset)).date()
            entry = per_day.get(day, {"reviews": set(), "new": set()})
            result.append((day, len(entry["reviews"]), len(entry["new"])))
        return result

    def sessions_per_day(self, profile_id, year, month):
        """{day-of-month: logged-session-count} for a local calendar month.

        A "logged session" is one where at least one card was actually
        reviewed, so merely opening the app and backing out earns no
        stamp. Sessions are grouped by *local* day (started_at is stored
        UTC); the DB window is padded a day on each side so a session
        whose UTC timestamp sits just outside the month but whose local
        day is inside it is still counted.
        """
        month_start = datetime(year, month, 1).astimezone()
        next_month = (datetime(year + 1, 1, 1) if month == 12
                      else datetime(year, month + 1, 1)).astimezone()
        window_start = (month_start - timedelta(days=1)).astimezone(
            timezone.utc)
        window_end = (next_month + timedelta(days=1)).astimezone(
            timezone.utc)

        counts = {}
        for row in self.storage.sessions_in_range(
                profile_id, iso(window_start), iso(window_end)):
            if not row["cards_reviewed"]:
                continue
            when = datetime.fromisoformat(row["started_at"]).astimezone()
            if when.year == year and when.month == month:
                counts[when.day] = counts.get(when.day, 0) + 1
        return counts
