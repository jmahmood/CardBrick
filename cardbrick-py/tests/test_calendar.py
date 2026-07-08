"""Per-day session counts behind the Brain Age-style stamp calendar."""

from datetime import datetime, timezone

from conftest import seed_card
from cardbrick.session import StudySession


def _add_session(storage, profile_id, local_dt, cards_reviewed):
    """Insert a session started at a given *local* time. Stored UTC, so
    write and read both go through local time and the runner's timezone
    cancels out."""
    started = local_dt.astimezone(timezone.utc).isoformat()
    sid = storage.create_session(profile_id, started, None)
    storage.conn.execute(
        "UPDATE sessions SET cards_reviewed=? WHERE id=?",
        (cards_reviewed, sid))
    storage.conn.commit()
    return sid


def test_multiple_sessions_same_day_are_counted(storage, service):
    _add_session(storage, 1, datetime(2026, 7, 10, 9, 0), 5)
    _add_session(storage, 1, datetime(2026, 7, 10, 14, 0), 3)
    _add_session(storage, 1, datetime(2026, 7, 10, 20, 0), 8)
    assert service.sessions_per_day(1, 2026, 7) == {10: 3}


def test_empty_session_earns_no_stamp(storage, service):
    _add_session(storage, 1, datetime(2026, 7, 10, 9, 0), 0)   # backed out
    _add_session(storage, 1, datetime(2026, 7, 10, 10, 0), 4)  # real
    assert service.sessions_per_day(1, 2026, 7) == {10: 1}


def test_unfinished_sprint_with_review_earns_stamp(storage, service, profile,
                                                   clock):
    seed_card(storage, service, 1)
    session = StudySession(storage, service, profile)
    session.answer(4)

    when = clock.now().astimezone()
    assert service.sessions_per_day(
        profile["id"], when.year, when.month) == {when.day: 1}


def test_undone_review_no_longer_earns_stamp(storage, service, profile,
                                             clock):
    seed_card(storage, service, 1)
    session = StudySession(storage, service, profile)
    session.answer(4)
    session.undo()

    when = clock.now().astimezone()
    assert service.sessions_per_day(
        profile["id"], when.year, when.month) == {}


def test_sessions_spread_across_days(storage, service):
    _add_session(storage, 1, datetime(2026, 7, 1, 9, 0), 5)
    _add_session(storage, 1, datetime(2026, 7, 15, 9, 0), 5)
    _add_session(storage, 1, datetime(2026, 7, 15, 18, 0), 5)
    _add_session(storage, 1, datetime(2026, 7, 28, 9, 0), 5)
    assert service.sessions_per_day(1, 2026, 7) == {1: 1, 15: 2, 28: 1}


def test_other_months_excluded(storage, service):
    _add_session(storage, 1, datetime(2026, 6, 30, 12, 0), 5)
    _add_session(storage, 1, datetime(2026, 7, 10, 12, 0), 5)
    _add_session(storage, 1, datetime(2026, 8, 1, 12, 0), 5)
    assert service.sessions_per_day(1, 2026, 7) == {10: 1}


def test_other_profiles_excluded(storage, service):
    _add_session(storage, 1, datetime(2026, 7, 10, 12, 0), 5)
    _add_session(storage, 2, datetime(2026, 7, 10, 12, 0), 5)
    assert service.sessions_per_day(1, 2026, 7) == {10: 1}
    assert service.sessions_per_day(2, 2026, 7) == {10: 1}


def test_empty_month_is_empty_dict(storage, service):
    _add_session(storage, 1, datetime(2026, 7, 10, 12, 0), 5)
    assert service.sessions_per_day(1, 2026, 12) == {}


def test_december_rollover_window(storage, service):
    """The month window steps into January; make sure Dec 31 counts and
    Jan 1 does not leak in."""
    _add_session(storage, 1, datetime(2026, 12, 31, 12, 0), 5)
    _add_session(storage, 1, datetime(2027, 1, 1, 12, 0), 5)
    assert service.sessions_per_day(1, 2026, 12) == {31: 1}


def test_sessions_in_range_filters_by_profile_and_window(storage):
    start = datetime(2026, 7, 1, 0, 0, tzinfo=timezone.utc).isoformat()
    end = datetime(2026, 8, 1, 0, 0, tzinfo=timezone.utc).isoformat()
    _add_session(storage, 1, datetime(2026, 7, 10, 12, 0), 5)   # in
    _add_session(storage, 1, datetime(2026, 8, 1, 12, 0), 5)    # after window
    _add_session(storage, 2, datetime(2026, 7, 15, 12, 0), 5)   # other profile

    rows = storage.sessions_in_range(1, start, end)
    assert len(rows) == 1
    assert rows[0]["cards_reviewed"] == 5
