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
from cardbrick.storage import SCHEMA_VERSION, Storage


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
    long_ago = clock.now() - timedelta(days=10)
    for i in range(1, 46):
        seed_card(storage, service, i, reps=2, due=long_ago)
    limits = _limits(daily_goal_cards=40, daily_new_cards=0,
                     session_card_limit=10, study_ahead_enabled=0)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 0
    assert status["goal_today"] == 40
    assert status["sprints_planned"] == 4
    assert status["sprints_remaining"] == 4
    assert status["next_sprint_cards"] == 10

    for card_id in (1, 2, 3):
        service.answer_card(card_id, 4)
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 3
    assert status["cards_remaining"] == 37
    assert status["sprints_remaining"] == 4  # ceil(37 / 10)


def test_plan_never_promises_more_than_the_day_can_supply(storage, service,
                                                          clock):
    # With a parent-set fixed new-card cap, a freshly imported deck
    # holds only daily_new_cards cards today — the plan says "1 sprint,
    # 10 cards", not an unreachable "0 / 50".
    for i in range(1, 101):
        seed_card(storage, service, i)
    limits = _limits(daily_goal_cards=50, daily_new_cards=10,
                     session_card_limit=20)
    status = service.sprint_status(limits=limits)
    assert status["goal_today"] == 10
    assert status["sprints_planned"] == 1
    assert status["sprints_remaining"] == 1
    assert status["next_sprint_cards"] == 10


def test_goal_paces_new_intake_by_default(storage, service, clock):
    # daily_new_cards=0 (the default) means the goal itself is the
    # pacing: a fresh deck supports a full goal-sized day of sprints
    # instead of a single drip-fed one.
    for i in range(1, 101):
        seed_card(storage, service, i)
    limits = _limits(daily_goal_cards=50, daily_new_cards=0,
                     session_card_limit=20)
    status = service.sprint_status(limits=limits)
    assert status["goal_today"] == 50
    assert status["sprints_planned"] == 3
    assert status["next_sprint_cards"] == 20
    queue = service.get_due_cards(limits=limits)
    assert len(queue) == 20
    assert all(row["reps"] == 0 for row in queue)


def test_reviews_crowd_out_new_cards_in_auto_mode(storage, service, clock):
    # As the review load grows toward the goal, new intake shrinks by
    # itself: 40 due reviews under a goal of 50 leave room for only 10
    # new cards today.
    past = clock.now() - timedelta(days=3)
    for i in range(1, 41):
        seed_card(storage, service, i, reps=1, due=past)
    for i in range(41, 141):
        seed_card(storage, service, i)
    limits = _limits(daily_goal_cards=50, daily_new_cards=0,
                     session_card_limit=100, study_ahead_enabled=0)
    queue = service.get_due_cards(limits=limits)
    assert len(queue) == 50
    assert sum(1 for row in queue if row["reps"] == 0) == 10
    assert all(row["reps"] > 0 for row in queue[:40])  # reviews first


def test_bonus_sprint_can_pull_extra_new_cards(storage, service, clock):
    # Day one of a fresh deck: after the paced 10 new cards the day is
    # done, but a keen child can keep going — bonus sprints ignore the
    # new-card cap (one optional sprint at a time).
    for i in range(1, 101):
        seed_card(storage, service, i)
    limits = _limits(daily_goal_cards=50, daily_new_cards=10,
                     session_card_limit=20)
    for row in service.get_due_cards(limits=limits):
        service.answer_card(row["id"], 4)
    status = service.sprint_status(limits=limits)
    assert status["next_sprint_cards"] == 0
    assert not status["goal_met"]
    assert status["bonus_cards"] == 20  # a full sprint of extra new cards
    queue = service.get_due_cards(limits=limits, bonus=True)
    assert len(queue) == 20
    assert all(row["reps"] == 0 for row in queue)


def test_repeats_do_not_stall_the_day(storage, service, clock):
    # Cards already answered today can't advance the distinct-card
    # goal, so once only repeats remain the day completes instead of
    # showing "sprints to go" forever.
    seed_card(storage, service, 1)
    seed_card(storage, service, 2)
    limits = _limits(daily_goal_cards=5, daily_new_cards=5)
    service.answer_card(1, 3)  # Good: learning step, due again soon —
    service.answer_card(2, 3)  # squarely inside the study-ahead horizon
    status = service.sprint_status(limits=limits)
    assert status["cards_remaining"] == 0
    assert status["next_sprint_cards"] == 0  # done, not stuck


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
    service.answer_card(1, 3)  # Good: due again within the hour
    assert service.sprint_status(limits=limits)["cards_remaining"] == 0

    clock.advance(days=1)  # the card is due again, and it's a new day
    status = service.sprint_status(limits=limits)
    assert status["cards_done"] == 0  # yesterday's work stays yesterday's
    assert status["cards_remaining"] == 1


# -- migration -----------------------------------------------------------------


def test_migration_seeds_goal_from_old_caps(tmp_path):
    # A schema-v4 database (pre-goal): opening it must add the new
    # columns, seed daily_goal_cards from the old review+new caps so
    # nobody's day suddenly triples, and re-baseline sitting-sized
    # session limits (the old 50-card/15-min defaults) to sprint scale
    # — while leaving deliberately customised limits alone.
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
        INSERT INTO child_profiles (name, session_card_limit,
                                    session_time_minutes)
        VALUES ('Leo', 30, 8);
    """)
    conn.commit()
    conn.close()

    storage = Storage(db_path)
    try:
        assert storage.schema_version() == SCHEMA_VERSION
        maya, leo = storage.list_profiles()
        assert maya["daily_goal_cards"] == 35  # 5 new + 30 review
        assert maya["study_ahead_days"] == 1
        assert maya["study_ahead_enabled"] == 1
        assert maya["session_card_limit"] == 20   # old default, rebased
        assert maya["session_time_minutes"] == 10
        assert maya["daily_new_cards"] == 5       # custom cap kept
        assert leo["session_card_limit"] == 30    # custom values kept
        assert leo["session_time_minutes"] == 8
        assert leo["daily_new_cards"] == 0        # old default -> auto
    finally:
        storage.close()


def test_v5_databases_get_sprint_sized_limits(tmp_path):
    # A database that already ran the v5 migration kept the old
    # sitting-sized 50/15 limits, which made the whole day one sprint;
    # v6 re-baselines those too.
    db_path = str(tmp_path / "v5.db")
    storage = Storage(db_path)
    profile = storage.ensure_default_profile("Student")
    storage.update_profile(profile["id"], session_card_limit=50,
                           session_time_minutes=15)
    storage.conn.execute(
        "UPDATE meta SET value = '5' WHERE key = 'schema_version'")
    storage.commit()
    storage.close()

    storage = Storage(db_path)
    try:
        assert storage.schema_version() == SCHEMA_VERSION
        student = storage.list_profiles()[0]
        assert student["session_card_limit"] == 20
        assert student["session_time_minutes"] == 10
    finally:
        storage.close()


def test_cards_done_counts_distinct_cards(storage, service, clock):
    seed_card(storage, service, 1)
    limits = _limits(daily_goal_cards=10, study_ahead_enabled=0)
    assert service.sprint_status(limits=limits)["cards_done"] == 0
    service.answer_card(1, 1)  # Again: the card will repeat...
    service.answer_card(1, 3)  # ...but stays one card toward the goal
    assert service.sprint_status(limits=limits)["cards_done"] == 1
