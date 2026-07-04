"""FSRS scheduling wrapper.

All spaced-repetition math is delegated to the py-fsrs library. This
module only converts between py-fsrs Card objects and the flat dicts
that storage.py persists, and maintains the reps/lapses counters.
"""

import json
from datetime import datetime, timezone

from fsrs import Card, Rating, Scheduler, State

RATINGS = {
    1: Rating.Again,
    2: Rating.Hard,
    3: Rating.Good,
    4: Rating.Easy,
}


def now_utc():
    return datetime.now(timezone.utc)


def iso(dt):
    return dt.astimezone(timezone.utc).isoformat()


class ReviewScheduler:
    def __init__(self, desired_retention=0.9):
        self.scheduler = Scheduler(desired_retention=desired_retention)

    def initial_state(self, card_id, now=None):
        """State dict for a freshly imported card (due immediately)."""
        now = now or now_utc()
        card = Card(card_id=card_id, due=now)
        return self._to_state(card, reps=0, lapses=0,
                              elapsed_days=0, scheduled_days=0)

    def review(self, state_row, rating, now=None):
        """Apply a 1-4 rating to a stored review_state row.

        Returns the updated state dict, ready for
        Storage.save_review_state().
        """
        now = now or now_utc()
        card = Card.from_dict(json.loads(state_row["fsrs_json"]))
        was_reviewing = card.state == State.Review
        last_review = card.last_review

        card, _log = self.scheduler.review_card(
            card, RATINGS[int(rating)], review_datetime=now)

        reps = state_row["reps"] + 1
        lapses = state_row["lapses"]
        if int(rating) == 1 and was_reviewing:
            lapses += 1
        elapsed_days = (now - last_review).days if last_review else 0
        scheduled_days = max((card.due - now).days, 0)
        return self._to_state(card, reps=reps, lapses=lapses,
                              elapsed_days=elapsed_days,
                              scheduled_days=scheduled_days)

    @staticmethod
    def _to_state(card, reps, lapses, elapsed_days, scheduled_days):
        return {
            "card_id": card.card_id,
            "due": iso(card.due),
            "stability": card.stability,
            "difficulty": card.difficulty,
            "elapsed_days": elapsed_days,
            "scheduled_days": scheduled_days,
            "reps": reps,
            "lapses": lapses,
            "state": int(card.state),
            "fsrs_json": json.dumps(card.to_dict()),
        }
