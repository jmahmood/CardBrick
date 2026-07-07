"""Study session runner.

Owns the card queue for one sitting: pops cards, applies answers through
ReviewService, requeues intra-session learning cards, tracks the session
row in the database, and produces the end-of-session summary. All
counters are derived from the append-only review log, so a crash mid-
session loses nothing and undo corrects the numbers automatically.
"""

from collections import deque
from datetime import datetime

from .scheduler import iso


class StudySession:
    def __init__(self, storage, service, profile, deck_filter=None):
        """``deck_filter`` overrides the profile's assigned decks for
        just this sitting — e.g. a child picking one specific deck out
        of several the parent has assigned (see app.py's DECK_SELECT
        screen). None falls back to the profile's active_decks, same
        convention as ReviewService.get_due_cards."""
        self.storage = storage
        self.service = service
        self.profile = profile
        self.started_at = service.now()
        self.session_id = storage.create_session(
            profile_id=profile["id"],
            started_at=iso(self.started_at),
            category_filter=profile.get("active_categories"))

        rows = service.get_due_cards(profile=profile,
                                     deck_filter=deck_filter)
        self.queue = deque(row["id"] for row in rows)
        self.planned_total = len(rows)
        self.finished = False
        self._current = None

    # -- flow -------------------------------------------------------------------

    def current_card(self):
        """The card to show now, or None when the session is complete.

        Skips cards that were buried/suspended while queued (e.g. via
        undo + bury) by re-checking the row at pop time.
        """
        while self._current is None and self.queue:
            row = self.storage.get_card(self.queue[0])
            if row is None or row["suspended"]:
                self.queue.popleft()
                continue
            self._current = row
            break
        return self._current

    def answer(self, rating, elapsed_ms=None):
        """Rate the current card; requeues it if a learning step brings
        it back within this session."""
        card = self._pop_current()
        _state, comes_back = self.service.answer_card(
            card["id"], rating, elapsed_ms=elapsed_ms,
            session_id=self.session_id)
        if comes_back:
            self.queue.append(card["id"])
        return comes_back

    def undo(self):
        """Undo the previous answer and put that card back up front.

        Returns the restored card row, or None if there was nothing to
        undo in this session.
        """
        card_id = self.service.undo_last_answer(self.session_id)
        if card_id is None:
            return None
        # Remove any requeued copy, then show the card again immediately.
        try:
            self.queue.remove(card_id)
        except ValueError:
            pass
        self.queue.appendleft(card_id)
        self._current = None
        return self.current_card()

    def bury_current(self):
        card = self._pop_current()
        self.service.bury_card(card["id"], session_id=self.session_id)

    def suspend_current(self):
        card = self._pop_current()
        self.service.suspend_card(card["id"], session_id=self.session_id)

    def _pop_current(self):
        card = self.current_card()
        if card is None:
            raise RuntimeError("No current card")
        self.queue.popleft()
        self._current = None
        return card

    # -- status -------------------------------------------------------------------

    def remaining(self):
        return len(self.queue)

    def elapsed_seconds(self):
        return (self.service.now() - self.started_at).total_seconds()

    def time_limit_reached(self):
        minutes = self.profile.get("session_time_minutes")
        if not minutes:
            return False
        return self.elapsed_seconds() >= minutes * 60

    def is_done(self):
        return self.finished or self.current_card() is None

    # -- finish ---------------------------------------------------------------------

    def finish(self):
        if not self.finished:
            self.finished = True
            self.storage.finish_session(self.session_id,
                                        iso(self.service.now()))
        return self.summary()

    def summary(self):
        """Concise end-of-session stats for the summary screen."""
        counts = self.storage.session_counts(self.session_id)
        row = self.storage.session_row(self.session_id)
        reviewed = counts["cards_reviewed"]
        passed = reviewed - counts["again_count"]
        ended = (datetime.fromisoformat(row["ended_at"])
                 if row["ended_at"] else self.service.now())
        started = datetime.fromisoformat(row["started_at"])
        return {
            "cards_reviewed": reviewed,
            "unique_cards": counts["unique_cards"],
            "new_cards": counts["new_cards"],
            "review_cards": counts["review_cards"],
            "again_count": counts["again_count"],
            "hard_count": counts["hard_count"],
            "good_count": counts["good_count"],
            "easy_count": counts["easy_count"],
            "pass_rate": (passed / reviewed) if reviewed else None,
            "time_spent_seconds": max(
                (ended - started).total_seconds(), 0),
            "avg_response_ms": (counts["elapsed_ms"] / reviewed)
                               if reviewed and counts["elapsed_ms"] else None,
            "buried_count": row["buried_count"],
            "suspended_count": row["suspended_count"],
        }
