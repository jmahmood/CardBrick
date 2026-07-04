"""Undo must be an exact rollback, never a guess."""

from datetime import timedelta

from conftest import seed_card
from cardbrick.scheduler import iso
from cardbrick.session import StudySession


def _state_dict(storage, card_id):
    return dict(storage.get_review_state(card_id))


def test_answering_mutates_fsrs_state(storage, service):
    seed_card(storage, service, 1)
    before = _state_dict(storage, 1)
    service.answer_card(1, 3)
    after = _state_dict(storage, 1)
    assert after["reps"] == before["reps"] + 1
    assert after["due"] != before["due"]
    assert after["fsrs_json"] != before["fsrs_json"]


def test_undo_restores_exact_prior_state(storage, service, clock):
    seed_card(storage, service, 1, reps=3,
              due=clock.now() - timedelta(days=2))
    session_id = storage.create_session(None, iso(clock.now()), None)
    before = _state_dict(storage, 1)

    service.answer_card(1, 1, elapsed_ms=1500, session_id=session_id)
    assert _state_dict(storage, 1) != before

    assert service.undo_last_answer(session_id) == 1
    assert _state_dict(storage, 1) == before


def test_undo_marks_log_entry_undone(storage, service, clock):
    seed_card(storage, service, 1)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.answer_card(1, 3, session_id=session_id)
    service.undo_last_answer(session_id)

    rows = storage.conn.execute("SELECT * FROM review_log").fetchall()
    assert len(rows) == 1  # append-only: the entry stays, flagged
    assert rows[0]["undone"] == 1
    assert storage.last_review_log(session_id) is None


def test_undo_with_empty_log_is_a_noop(storage, service, clock):
    session_id = storage.create_session(None, iso(clock.now()), None)
    assert service.undo_last_answer(session_id) is None


def test_undo_corrects_session_counters(storage, service, clock, profile):
    for i in range(1, 4):
        seed_card(storage, service, i)
    session = StudySession(storage, service, profile)

    session.answer(3, elapsed_ms=1000)   # card 1: Good
    session.answer(1, elapsed_ms=1000)   # card 2: Again
    counts = storage.session_counts(session.session_id)
    assert counts["cards_reviewed"] == 2
    assert counts["again_count"] == 1
    assert counts["new_cards"] == 2

    restored = session.undo()            # roll back the "Again"
    assert restored["id"] == 2
    counts = storage.session_counts(session.session_id)
    assert counts["cards_reviewed"] == 1
    assert counts["again_count"] == 0
    assert counts["new_cards"] == 1
    # The undone card is presented again immediately.
    assert session.current_card()["id"] == 2


def test_undo_corrects_daily_counts(storage, service, clock):
    from cardbrick.service import local_day_start
    seed_card(storage, service, 1)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.answer_card(1, 3, session_id=session_id)
    day_start = iso(local_day_start(clock.now()))
    assert storage.daily_counts(day_start)[0] == 1
    service.undo_last_answer(session_id)
    assert storage.daily_counts(day_start)[0] == 0


def test_undo_only_rolls_back_most_recent(storage, service, clock):
    seed_card(storage, service, 1)
    seed_card(storage, service, 2)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.answer_card(1, 3, session_id=session_id)
    after_first = _state_dict(storage, 1)
    service.answer_card(2, 3, session_id=session_id)

    assert service.undo_last_answer(session_id) == 2
    assert _state_dict(storage, 1) == after_first  # card 1 untouched
    assert service.undo_last_answer(session_id) == 1  # then card 1
