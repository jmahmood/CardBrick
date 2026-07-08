"""Application-level review operations.

The rest of the app talks to ``ReviewService`` and never manipulates
FSRS internals directly. The service composes storage.py (persistence)
and scheduler.py (the py-fsrs wrapper) into the operations the spec
names:

    get_due_cards(profile, category_filter, limits)
    sprint_status(profile)
    answer_card(card_id, rating, elapsed_ms)
    undo_last_answer()
    bury_card(card_id)
    suspend_card(card_id)

The study day is modelled as a card goal (``daily_goal_cards``) chipped
away in short sprints — microstudying — rather than one sitting;
get_due_cards builds one sprint's queue and sprint_status reports how
many sprints are left.

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
    "daily_new_cards": 0,  # 0 = auto: the daily goal paces new intake
    "daily_goal_cards": 150,
    "session_card_limit": 20,
    "session_time_minutes": 10,
    "study_ahead_days": 1,
    "study_ahead_enabled": 1,
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

    def _sprint_pool(self, profile=None, category_filter=None,
                     deck_filter=None, limits=None, bonus=False):
        """Everything today's sprints could still draw on, due-first.

        Order: due review cards (most overdue first), then new cards,
        then soon-due cards pulled forward ("studying ahead", within
        ``study_ahead_days``). Suspended and buried cards never appear.

        New-card intake is paced by the daily goal itself: new cards
        fill whatever the goal has left, so a fresh deck supports a
        full goal-sized day, and as reviews build up they crowd new
        cards out automatically — the goal is the pacing. A non-zero
        ``daily_new_cards`` imposes a stricter fixed cap instead; a
        bonus sprint ignores both (the one place a keen child may pull
        extra new material, one sprint at a time).

        Returns (pool, limits, answered_today, cards_done) so callers
        can budget the pool and tell fresh cards — which still advance
        the distinct-card goal — from repeats already counted today.
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
        day_start = iso(local_day_start(now))
        new_done, review_done, _ = self.storage.daily_counts(day_start)
        answered = self.storage.cards_answered_since(day_start)
        if bonus:
            # A bonus sprint can't use more than one sprint's worth.
            remaining_new = limits["session_card_limit"]
        elif limits["daily_new_cards"]:
            remaining_new = max(limits["daily_new_cards"] - new_done, 0)
        else:  # auto: whatever the goal has left paces new intake
            remaining_new = max(limits["daily_goal_cards"]
                                - new_done - review_done, 0)

        pool = []
        for row in self.storage.queue_candidates(iso(now), new_cards=False,
                                                 decks=deck_filter):
            if card_matches_categories(row["tags"], category_filter):
                pool.append(row)
        for row in self.storage.queue_candidates(iso(now), new_cards=True,
                                                 decks=deck_filter):
            if remaining_new <= 0:
                break
            if card_matches_categories(row["tags"], category_filter):
                pool.append(row)
                remaining_new -= 1
        if limits["study_ahead_enabled"]:
            horizon = next_local_midnight(now) + timedelta(
                days=limits["study_ahead_days"])
            for row in self.storage.ahead_candidates(iso(now), iso(horizon),
                                                     decks=deck_filter):
                if card_matches_categories(row["tags"], category_filter):
                    pool.append(row)

        return pool, limits, answered, new_done + review_done

    def get_due_cards(self, profile=None, category_filter=None,
                      deck_filter=None, limits=None, bonus=False):
        """Build the queue for one sprint.

        The day is a card goal (``daily_goal_cards`` distinct cards)
        chipped away in short sprints; each call returns the next
        sprint's worth, at most ``session_card_limit`` cards and never
        more than what's left of the goal. Once nothing in the pool can
        still advance the goal (every remaining card was already
        answered today), the day is done and the queue is empty. All
        budgets subtract work already logged today, so restarting
        mid-day continues from where the day left off.

        ``bonus=True`` ignores the goal budget and the daily new-card
        cap: used for the optional bonus sprint offered once the day is
        done. It's an offer, not an obligation — nothing in the queue
        math ever *requires* more studying.

        ``category_filter``/``deck_filter`` of None fall back to the
        profile's ``active_categories``/``active_decks``; an explicit
        [] means "none active" (an empty queue), same as the profile
        field would.

        Returns a list of card rows (joined with review state).
        """
        pool, limits, answered, cards_done = self._sprint_pool(
            profile, category_filter, deck_filter, limits, bonus=bonus)
        if bonus:
            return pool[:limits["session_card_limit"]]
        goal_left = max(limits["daily_goal_cards"] - cards_done, 0)
        fresh = sum(1 for row in pool if row["id"] not in answered)
        if min(goal_left, fresh) == 0:
            return []  # day done: goal met or nothing can advance it
        return pool[:min(goal_left, limits["session_card_limit"])]

    def counts_for_queue(self, profile=None, category_filter=None,
                         deck_filter=None, limits=None, bonus=False):
        """(review, new) counts of what get_due_cards would return."""
        queue = self.get_due_cards(profile, category_filter, deck_filter,
                                   limits, bonus=bonus)
        new = sum(1 for row in queue if row["reps"] == 0)
        return len(queue) - new, new

    def sprint_status(self, profile=None, category_filter=None,
                      deck_filter=None, limits=None):
        """Where the day stands, in sprints.

        The daily goal is meant to be chipped away in short sprints
        spread across the day, but the schedule is the child's own:
        doing sprints back-to-back ("going ahead") decrements
        ``sprints_remaining`` exactly the same as spacing them out.
        Everything is derived from the review log, so the numbers
        survive restarts, crashes, and undo.

        The plan never promises more than the day can actually supply
        (``goal_today``), so the child always sees an honest,
        finishable day — e.g. when a small deck runs out of cards, or
        when a parent set a fixed ``daily_new_cards`` cap that limits a
        fresh deck's first days.

        Returns a dict:
            cards_done         distinct cards answered today (a card
                               repeating a learning step counts once)
            goal               daily_goal_cards as configured
            goal_today         what today can actually reach:
                               cards_done + min(goal left, fresh cards
                               still available)
            goal_met           cards_done >= the configured goal
            cards_remaining    what's left of goal_today
            sprints_planned    ceil(goal_today / sprint size)
            sprints_remaining  ceil(cards_remaining / sprint size)
            next_sprint_cards  size of the queue the next sprint gets
            bonus_cards        when the day is done: size of the
                               optional bonus sprint (0 otherwise)
        """
        pool, limits, answered, done = self._sprint_pool(
            profile, category_filter, deck_filter, limits)
        goal = limits["daily_goal_cards"]
        sprint_size = max(limits["session_card_limit"], 1)
        goal_left = max(goal - done, 0)
        fresh = sum(1 for row in pool if row["id"] not in answered)
        remaining = min(goal_left, fresh)
        goal_today = done + remaining
        next_sprint = 0 if remaining == 0 \
            else min(len(pool), goal_left, sprint_size)
        bonus = 0
        if next_sprint == 0:
            bonus = len(self.get_due_cards(profile, category_filter,
                                           deck_filter, limits, bonus=True))
        return {
            "cards_done": done,
            "goal": goal,
            "goal_today": goal_today,
            "goal_met": done >= goal,
            "cards_remaining": remaining,
            "sprints_planned": -(-goal_today // sprint_size),
            "sprints_remaining": -(-remaining // sprint_size),
            "next_sprint_cards": next_sprint,
            "bonus_cards": bonus,
        }

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
