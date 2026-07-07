"""Studying ahead: sprint accounting, ahead fill, and bonus sprints.

The day is a card goal chipped away in 5-10 minute sprints; a child
with spare time can "go ahead" — do the next sprints now and owe fewer
later — and, once the goal is met, optionally keep going on soon-due
cards pulled forward (safe under FSRS: early reviews just earn a
smaller stability gain).
"""

import sqlite3
from datetime import datetime, timedelta

from conftest import seed_card

from cardbrick.scheduler import iso
from cardbrick.session import StudySession
from cardbrick.storage import Storage


def _limits(**overrides):
    base = {"daily_new_cards": 10, "daily_goal_cards": 150,
            "session_card_limit": 20, "session_time_minutes": 10,
            "study_ahead_days": 1, "study_ahead_enabled": 1}
    base.update(overrides)
    return base


def _ids(queue):
    return [row["id"] for row in queue]


# -- sprint accounting ---------------------------------------------------------


def test_sprint_status_derives_counts_from_the_log(storage, service, clock):
    for i in range(1, 7):
        seed_card(storage, service, i)
    limits = _limits(daily_goal_cards=40, daily_new_cards=6,
                     session_card_limit=10, study_ahead_enabled=0)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 0
    assert status["sprints_planned"] == 4
    assert status["sprints_remaining"] == 4
    assert status["next_sprint_cards"] == 6

    for card_id in (1, 2, 3):
        service.answer_card(card_id, 4)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 3
    assert status["cards_remaining"] == 37
    assert status["sprints_remaining"] == 4  # ceil(37 / 10)


def test_going_ahead_decrements_sprints_remaining(storage, service, clock):
    # Three back-to-back sprints burn down three sprints' worth of the
    # goal — spare time now means fewer sprints owed later.
    long_ago = clock.now() - timedelta(days=10)
    for i in range(1, 31):
        seed_card(storage, service, i, reps=2, due=long_ago)
    limits = _limits(daily_goal_cards=30, daily_new_cards=0,
                     session_card_limit=10, study_ahead_enabled=0)
    assert service.sprint_status(limits=limits)["sprints_remaining"] == 3

    for expected_left in (2, 1, 0):
        for row in service.get_due_cards(limits=limits):
            service.answer_card(row["id"], 4)
        status = service.sprint_status(limits=limits)
        assert status["sprints_remaining"] == expected_left
    assert status["next_sprint_cards"] == 0  # goal met: nothing owed


def test_partial_sprint_earns_partial_credit(storage, service, clock):
    # Quitting a sprint early only banks the cards actually answered;
    # the remaining count stays honest.
    long_ago = clock.now() - timedelta(days=10)
    for i in range(1, 21):
        seed_card(storage, service, i, reps=2, due=long_ago)
    limits = _limits(daily_goal_cards=20, daily_new_cards=0,
                     session_card_limit=10, study_ahead_enabled=0)
    for row in service.get_due_cards(limits=limits)[:4]:
        service.answer_card(row["id"], 4)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 4
    assert status["sprints_remaining"] == 2  # ceil(16 / 10)


def test_undo_restores_the_sprint_count(storage, service, clock):
    seed_card(storage, service, 1)
    limits = _limits(daily_goal_cards=1, daily_new_cards=1,
                     study_ahead_enabled=0)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.answer_card(1, 4, session_id=session_id)
    assert service.sprint_status(limits=limits)["cards_remaining"] == 0

    service.undo_last_answer(session_id)
    status = service.sprint_status(limits=limits)
    assert status["cards_remaining"] == 1
    assert status["next_sprint_cards"] == 1


# -- ahead fill ---------------------------------------------------------------


def test_ahead_fill_tops_up_when_due_pool_runs_dry(storage, service, clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now - timedelta(hours=1))
    seed_card(storage, service, 2, reps=1, due=now + timedelta(hours=20))
    seed_card(storage, service, 3, reps=1, due=now + timedelta(hours=4))
    queue = service.get_due_cards(limits=_limits(daily_new_cards=0))
    # Due card first, then soon-due cards pulled forward, soonest first.
    assert _ids(queue) == [1, 3, 2]


def test_ahead_fill_respects_the_horizon(storage, service, clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now + timedelta(hours=20))
    seed_card(storage, service, 2, reps=1, due=now + timedelta(days=3))
    one_day = service.get_due_cards(
        limits=_limits(daily_new_cards=0, study_ahead_days=1))
    assert _ids(one_day) == [1]
    three_days = service.get_due_cards(
        limits=_limits(daily_new_cards=0, study_ahead_days=3))
    assert _ids(three_days) == [1, 2]


def test_ahead_fill_only_after_due_and_new(storage, service, clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now + timedelta(hours=5))
    seed_card(storage, service, 2)  # new
    seed_card(storage, service, 3, reps=1, due=now - timedelta(hours=1))
    queue = service.get_due_cards(limits=_limits())
    assert _ids(queue) == [3, 2, 1]  # due, new, then ahead


def test_ahead_fill_never_resurrects_buried_or_suspended(storage, service,
                                                         clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now + timedelta(hours=5))
    seed_card(storage, service, 2, reps=1, due=now + timedelta(hours=6))
    seed_card(storage, service, 3, reps=1, due=now + timedelta(hours=7))
    service.bury_card(1)      # "not today" must hold against ahead fill
    service.suspend_card(2)
    queue = service.get_due_cards(limits=_limits(daily_new_cards=0))
    assert _ids(queue) == [3]


def test_ahead_fill_respects_deck_and_category_filters(storage, service,
                                                       clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now + timedelta(hours=5),
              deck="Spanish", tags="food")
    seed_card(storage, service, 2, reps=1, due=now + timedelta(hours=5),
              deck="French", tags="food")
    seed_card(storage, service, 3, reps=1, due=now + timedelta(hours=5),
              deck="Spanish", tags="numbers")
    queue = service.get_due_cards(deck_filter=["Spanish"],
                                  category_filter=["food"],
                                  limits=_limits(daily_new_cards=0))
    assert _ids(queue) == [1]


def test_study_ahead_disabled_means_the_day_ends_early(storage, service,
                                                       clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() + timedelta(hours=5))
    limits = _limits(daily_new_cards=0, study_ahead_enabled=0)
    assert service.get_due_cards(limits=limits) == []
    status = service.sprint_status(limits=limits)
    assert status["next_sprint_cards"] == 0
    assert status["bonus_cards"] == 0


# -- FSRS early review ---------------------------------------------------------


def test_early_review_produces_a_sane_state(storage, service, clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() + timedelta(hours=20))
    state, _came_back = service.answer_card(1, 3)
    assert state["reps"] == 2
    assert datetime.fromisoformat(state["due"]) > clock.now()


def test_again_on_an_ahead_card_comes_back_in_session(storage, service,
                                                      clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() + timedelta(hours=20))
    _state, came_back = service.answer_card(1, 1)
    assert came_back  # LEARN_AHEAD requeue works the same when ahead


# -- bonus sprints -------------------------------------------------------------


def test_bonus_sprint_offered_only_after_goal_met(storage, service, clock):
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now - timedelta(hours=1))
    seed_card(storage, service, 2, reps=1, due=now + timedelta(hours=20))
    limits = _limits(daily_goal_cards=1, daily_new_cards=0)

    status = service.sprint_status(limits=limits)
    assert status["next_sprint_cards"] == 1  # goal still open: no bonus
    assert status["bonus_cards"] == 0

    service.answer_card(1, 4)
    status = service.sprint_status(limits=limits)
    assert status["cards_remaining"] == 0
    assert status["next_sprint_cards"] == 0
    assert status["bonus_cards"] >= 1  # card 2 pulled forward


def test_bonus_sprints_drain_the_ahead_pool(storage, service, clock):
    # Each early review pushes the card's due date out, so repeated
    # bonus sprints terminate on their own.
    now = clock.now()
    for i in range(1, 6):
        seed_card(storage, service, i, reps=1,
                  due=now + timedelta(hours=12 + i))
    limits = _limits(daily_goal_cards=0, daily_new_cards=0)
    for _round in range(20):  # safety bound; should finish much sooner
        queue = service.get_due_cards(limits=limits, bonus=True)
        if not queue:
            break
        for row in queue:
            service.answer_card(row["id"], 4)  # Easy: due leaves horizon
    assert service.get_due_cards(limits=limits, bonus=True) == []


def test_bonus_session_flows_through_study_session(storage, service, clock,
                                                   profile):
    storage.update_profile(profile["id"], daily_goal_cards=1,
                           daily_new_cards=0)
    profile = storage.get_profile(profile["id"])
    now = clock.now()
    seed_card(storage, service, 1, reps=1, due=now - timedelta(hours=1))
    seed_card(storage, service, 2, reps=1, due=now + timedelta(hours=20))
    service.answer_card(1, 4)  # goal of 1 met

    session = StudySession(storage, service, profile, bonus=True)
    assert session.planned_total == 1
    card = session.current_card()
    assert card["id"] == 2
    session.answer(4)
    summary = session.finish()
    assert summary["cards_reviewed"] == 1
    # Bonus sprints still earn a stamp on the calendar.
    when = now.astimezone()
    stamps = service.sessions_per_day(profile["id"], when.year, when.month)
    assert stamps[when.day] >= 1


# -- rollover ------------------------------------------------------------------


def test_day_rolls_over_clean_after_going_ahead(storage, service, clock):
    seed_card(storage, service, 1)
    limits = _limits(daily_goal_cards=1, daily_new_cards=1,
                     study_ahead_enabled=0)
    service.answer_card(1, 4)
    assert service.sprint_status(limits=limits)["cards_remaining"] == 0

    clock.advance(days=1)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 0  # yesterday's work stays yesterday's
    assert status["cards_remaining"] == 1


# -- migration -----------------------------------------------------------------


def test_migration_seeds_goal_from_old_caps(tmp_path):
    # A schema-v4 database (pre-goal): opening it must add the new
    # columns and seed daily_goal_cards from the old review+new caps so
    # nobody's day suddenly triples.
    db_path = str(tmp_path / "old.db")
    conn = sqlite3.connect(db_path)
    conn.executescript("""
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
        INSERT INTO meta VALUES ('schema_version', '4');
        CREATE TABLE cards (
            id INTEGER PRIMARY KEY, note_id INTEGER NOT NULL,
            deck TEXT NOT NULL, front TEXT NOT NULL, back TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '', audio_filename TEXT,
            audio_side TEXT, suspended INTEGER NOT NULL DEFAULT 0,
            buried_until TEXT, created_at TEXT, updated_at TEXT,
            card_type TEXT NOT NULL DEFAULT 'basic');
        CREATE TABLE child_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
            active_categories TEXT, active_decks TEXT,
            daily_new_cards INTEGER NOT NULL DEFAULT 10,
            daily_review_cards INTEGER NOT NULL DEFAULT 40,
            session_card_limit INTEGER NOT NULL DEFAULT 50,
            session_time_minutes INTEGER NOT NULL DEFAULT 15,
            study_direction TEXT NOT NULL DEFAULT 'normal');
        INSERT INTO child_profiles (name, daily_new_cards,
                                    daily_review_cards)
        VALUES ('Maya', 5, 30);
    """)
    conn.commit()
    conn.close()

    storage = Storage(db_path)
    try:
        assert storage.schema_version() == 5
        maya = storage.list_profiles()[0]
        assert maya["daily_goal_cards"] == 35  # 5 new + 30 review
        assert maya["study_ahead_days"] == 1
        assert maya["study_ahead_enabled"] == 1
    finally:
        storage.close()


def test_cards_done_counts_distinct_cards(storage, service, clock):
    seed_card(storage, service, 1)
    limits = _limits(daily_goal_cards=10, study_ahead_enabled=0)
    assert service.sprint_status(limits=limits)["cards_done"] == 0
    service.answer_card(1, 1)  # Again: the card will repeat...
    service.answer_card(1, 3)  # ...but stays one card toward the goal
    assert service.sprint_status(limits=limits)["cards_done"] == 1
