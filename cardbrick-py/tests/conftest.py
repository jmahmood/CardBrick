import os
import sys
from datetime import datetime, timedelta, timezone

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from cardbrick.scheduler import ReviewScheduler, iso  # noqa: E402
from cardbrick.service import ReviewService  # noqa: E402
from cardbrick.storage import Storage  # noqa: E402


class FakeClock:
    """Deterministic injectable time source.

    Starts mid-morning local time so small advances stay within the
    same day unless a test crosses midnight on purpose.
    """

    def __init__(self, start=None):
        self.current = start or datetime(2026, 7, 4, 10, 0,
                                         tzinfo=timezone.utc)

    def now(self):
        return self.current

    def advance(self, **kwargs):
        self.current += timedelta(**kwargs)
        return self.current


@pytest.fixture
def clock():
    return FakeClock()


@pytest.fixture
def storage(tmp_path):
    store = Storage(str(tmp_path / "test.db"))
    yield store
    store.close()


@pytest.fixture
def service(storage, clock):
    return ReviewService(storage, ReviewScheduler(), now_fn=clock.now)


@pytest.fixture
def profile(storage):
    return storage.ensure_default_profile("Testkid")


def seed_card(storage, service, card_id, tags="", reps=0, due=None,
              deck="Spanish", audio=None):
    """Insert a card with review state. reps>0 + past due = review card;
    reps=0 = new card."""
    now = service.now()
    state = service.scheduler.initial_state(card_id, now=due or now)
    state["reps"] = reps
    if due is not None:
        state["due"] = iso(due)
    storage.upsert_card(card_id, card_id, deck, f"front {card_id}",
                        f"back {card_id}", tags,
                        audio_filename=audio,
                        audio_side="front" if audio else None,
                        now_iso=iso(now))
    storage.init_review_state(state)
    storage.commit()
    return card_id
