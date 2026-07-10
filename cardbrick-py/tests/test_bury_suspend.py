"""Bury (until tomorrow) and suspend (indefinitely) queue exclusion."""

from datetime import timedelta

from conftest import seed_card
from cardbrick.session import StudySession


def _queue_ids(service, **limits):
    base = {"daily_new_cards": 100, "daily_review_cards": 100,
            "session_card_limit": 100}
    base.update(limits)
    return [row["id"] for row in service.get_due_cards(limits=base)]


def test_buried_card_disappears_until_tomorrow(storage, service, clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() - timedelta(days=1))
    assert _queue_ids(service) == [1]

    service.bury_card(1)
    assert _queue_ids(service) == []

    clock.advance(hours=2)  # later the same day: still buried
    assert _queue_ids(service) == []

    clock.advance(days=1)   # past local midnight: it comes back
    assert _queue_ids(service) == [1]


def test_bury_counts_in_session_row(storage, service, clock):
    from cardbrick.scheduler import iso
    seed_card(storage, service, 1)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.bury_card(1, session_id=session_id)
    assert storage.session_row(session_id)["buried_count"] == 1


def test_bury_can_be_undone_in_session(storage, service, clock, profile):
    seed_card(storage, service, 1)
    seed_card(storage, service, 2)
    session = StudySession(storage, service, profile)

    session.bury_current()
    assert session.current_card()["id"] == 2
    restored = session.undo()

    assert restored["id"] == 1
    assert restored["buried_until"] is None
    assert storage.session_row(session.session_id)["buried_count"] == 0


def test_suspend_can_be_undone_in_session(storage, service, clock, profile):
    seed_card(storage, service, 1)
    session = StudySession(storage, service, profile)

    session.suspend_current()
    restored = session.undo()

    assert restored["id"] == 1
    assert restored["suspended"] == 0
    assert storage.session_row(session.session_id)["suspended_count"] == 0


def test_suspended_card_never_appears(storage, service, clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() - timedelta(days=1))
    service.suspend_card(1)
    assert _queue_ids(service) == []
    clock.advance(days=30)
    assert _queue_ids(service) == []


def test_unsuspended_card_reappears_when_due(storage, service, clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() - timedelta(days=1))
    service.suspend_card(1)
    assert _queue_ids(service) == []
    service.unsuspend_card(1)
    assert _queue_ids(service) == [1]


def test_suspended_cards_visible_to_parent(storage, service):
    seed_card(storage, service, 1)
    seed_card(storage, service, 2)
    service.suspend_card(2)
    suspended = storage.suspended_cards()
    assert [c["id"] for c in suspended] == [2]
