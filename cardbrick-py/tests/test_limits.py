"""Daily-goal, new-card, and sprint-size behaviour of the review queue.

Study-ahead fill is disabled in these limits so each budget can be
tested in isolation; test_study_ahead.py covers the fill behaviour.
"""

from datetime import timedelta

from conftest import seed_card


def _limits(**overrides):
    base = {"daily_new_cards": 10, "daily_goal_cards": 150,
            "session_card_limit": 50, "session_time_minutes": 15,
            "study_ahead_enabled": 0}
    base.update(overrides)
    return base


def test_new_card_cap_is_respected(storage, service):
    for i in range(1, 31):
        seed_card(storage, service, i)
    queue = service.get_due_cards(limits=_limits(daily_new_cards=10))
    assert len(queue) == 10
    assert all(row["reps"] == 0 for row in queue)


def test_daily_goal_caps_the_queue(storage, service, clock):
    past = clock.now() - timedelta(days=3)
    for i in range(1, 61):
        seed_card(storage, service, i, reps=1, due=past)
    queue = service.get_due_cards(
        limits=_limits(daily_goal_cards=40, daily_new_cards=0,
                       session_card_limit=100))
    assert len(queue) == 40
    assert all(row["reps"] > 0 for row in queue)


def test_sprint_cap_trumps_daily_budgets(storage, service, clock):
    past = clock.now() - timedelta(days=1)
    for i in range(1, 21):
        seed_card(storage, service, i, reps=1, due=past)
    for i in range(21, 41):
        seed_card(storage, service, i)
    queue = service.get_due_cards(
        limits=_limits(daily_new_cards=20, session_card_limit=15))
    assert len(queue) == 15


def test_backlog_does_not_flood_a_sprint(storage, service, clock):
    # A week of missed reviews: 500 due cards arrive one sprint at a
    # time, never all at once.
    long_ago = clock.now() - timedelta(days=30)
    for i in range(1, 501):
        seed_card(storage, service, i, reps=2, due=long_ago)
    queue = service.get_due_cards(limits=_limits())
    assert len(queue) == 50  # session_card_limit, not 500


def test_reviews_come_before_new_cards(storage, service, clock):
    seed_card(storage, service, 1)  # new
    seed_card(storage, service, 2, reps=1,
              due=clock.now() - timedelta(hours=2))
    queue = service.get_due_cards(limits=_limits())
    assert [row["id"] for row in queue] == [2, 1]


def test_most_overdue_reviews_first(storage, service, clock):
    seed_card(storage, service, 1, reps=1,
              due=clock.now() - timedelta(days=1))
    seed_card(storage, service, 2, reps=1,
              due=clock.now() - timedelta(days=5))
    queue = service.get_due_cards(limits=_limits())
    assert [row["id"] for row in queue] == [2, 1]


def test_daily_cap_counts_work_already_done_today(storage, service, clock):
    for i in range(1, 16):
        seed_card(storage, service, i)
    # Answer 6 new cards in a first sprint.
    for row in service.get_due_cards(limits=_limits(daily_new_cards=6)):
        service.answer_card(row["id"], 3)
    # Second sprint the same day: only 4 new remain under a cap of 10.
    queue = service.get_due_cards(limits=_limits(daily_new_cards=10))
    new_ids = {row["id"] for row in queue if row["reps"] == 0}
    assert len(new_ids) == 4


def test_goal_counts_distinct_cards_not_answers(storage, service, clock):
    # A learning-step repeat is the same card, so it consumes the goal
    # once: the contract is "N distinct cards", and a hard card that
    # comes back three times doesn't eat three cards' worth of goal.
    seed_card(storage, service, 1)
    seed_card(storage, service, 2, reps=1,
              due=clock.now() - timedelta(days=1))
    limits = _limits(daily_goal_cards=2, daily_new_cards=10)
    service.answer_card(1, 1)  # Again: the card comes back...
    service.answer_card(1, 3)  # ...but the repeat counts it only once.
    queue = service.get_due_cards(limits=limits)
    assert [row["id"] for row in queue] == [2]  # room for one more card


def test_daily_budgets_reset_at_local_midnight(storage, service, clock):
    for i in range(1, 6):
        seed_card(storage, service, i)
    limits = _limits(daily_new_cards=5)
    for row in service.get_due_cards(limits=limits):
        service.answer_card(row["id"], 4)  # Easy: graduates, due days out
    assert service.get_due_cards(limits=limits) == []

    clock.advance(days=1)
    for i in range(6, 11):
        seed_card(storage, service, i)
    queue = service.get_due_cards(limits=limits)
    assert len(queue) == 5  # fresh allowance after rollover


def test_profile_limits_are_used(storage, service, clock, profile):
    storage.update_profile(profile["id"], daily_new_cards=3)
    profile = storage.get_profile(profile["id"])
    for i in range(1, 11):
        seed_card(storage, service, i)
    queue = service.get_due_cards(profile=profile)
    assert len(queue) == 3
