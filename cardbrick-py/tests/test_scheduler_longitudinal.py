"""Multi-day scheduling behaviour: does FSRS actually reschedule cards
by rating, do cards come back exactly when due (not early, not lost),
and does the review backlog crowd out new cards the way the daily
goal is supposed to guarantee (and un-crowd them once the backlog
clears)?

These complement test_limits.py / test_study_ahead.py, which check
single-snapshot budgeting; this file drives the clock across several
simulated days to check the *trajectory*, not just one queue build.
"""

import json
from datetime import datetime, timedelta

from conftest import seed_card
from fsrs import Card

from cardbrick.scheduler import iso
from cardbrick.service import DEFAULT_LIMITS


def _set_due(storage, card_id, due):
    """Force a real (already-graduated) card's due date, keeping its
    fsrs internals consistent -- unlike seed_card's reps= override,
    this produces a card FSRS itself believes is overdue, so reviewing
    it exercises the real Review/Relearning path instead of a
    first-ever New-card review."""
    state = dict(storage.get_review_state(card_id))
    card = Card.from_dict(json.loads(state["fsrs_json"]))
    card.due = due
    state["due"] = iso(card.due)
    state["fsrs_json"] = json.dumps(card.to_dict())
    storage.save_review_state(state, last_reviewed_at=state.get("last_reviewed_at"))


def _limits(**overrides):
    base = dict(DEFAULT_LIMITS, study_ahead_enabled=0)
    base.update(overrides)
    return base


def test_good_ratings_grow_the_interval_and_graduate(storage, service, clock):
    seed_card(storage, service, 1)  # new card

    gaps = []
    for _ in range(6):
        state, _ = service.answer_card(1, 3)  # Good
        due = datetime.fromisoformat(state["due"])
        gaps.append((due - clock.now()).total_seconds())
        clock.current = due  # review again exactly when it comes due

    # First Good is a same-day learning step; from then on each
    # successive Good should schedule a longer gap than the last.
    assert gaps[0] < 3600
    review_gaps = gaps[1:]
    assert all(b > a for a, b in zip(review_gaps, review_gaps[1:])), review_gaps

    final = storage.get_review_state(1)
    assert final["state"] == 2  # Review (graduated out of learning)
    assert final["lapses"] == 0


def test_hard_grows_the_interval_slower_than_good(storage, service, clock):
    # Graduate two identical cards out of learning first (two Goods
    # each), then diverge: one always Good, one always Hard.
    def graduate(card_id):
        state, _ = service.answer_card(card_id, 3)
        clock.current = datetime.fromisoformat(state["due"])
        state, _ = service.answer_card(card_id, 3)
        clock.current = datetime.fromisoformat(state["due"])
        return state

    seed_card(storage, service, 1)
    seed_card(storage, service, 2)
    start = clock.now()

    clock.current = start
    graduate(1)
    good_due = datetime.fromisoformat(storage.get_review_state(1)["due"])

    clock.current = start
    graduate(2)
    hard_state = storage.get_review_state(2)

    # Replay both cards forward a day at a time with their respective
    # rating and compare the resulting stability after the same number
    # of reps.
    clock.current = good_due
    good_stability = None
    for _ in range(6):
        state, _ = service.answer_card(1, 3)
        good_stability = state["stability"]
        clock.current += timedelta(days=1)

    clock.current = datetime.fromisoformat(hard_state["due"])
    hard_stability = None
    for _ in range(6):
        state, _ = service.answer_card(2, 2)
        hard_stability = state["stability"]
        clock.current += timedelta(days=1)

    assert good_stability > hard_stability


def test_again_collapses_the_interval_and_counts_a_lapse(storage, service, clock):
    seed_card(storage, service, 1)

    # Graduate and build up a real interval with a few Goods.
    pre_lapse_gap = None
    for _ in range(3):
        state, _ = service.answer_card(1, 3)
        due = datetime.fromisoformat(state["due"])
        pre_lapse_gap = (due - clock.now()).total_seconds()
        clock.current = due

    pre_lapses = state["lapses"]

    # Now the child gets it wrong.
    state, _ = service.answer_card(1, 1)  # Again
    post_gap = (datetime.fromisoformat(state["due"]) - clock.now()).total_seconds()

    assert state["lapses"] == pre_lapses + 1
    assert post_gap < pre_lapse_gap
    assert post_gap < 3600  # back to a same-day relearning step

    # And rebuilding afterwards starts from a smaller base than before
    # the lapse (a real reset, not a no-op).
    clock.current = datetime.fromisoformat(state["due"])
    rebuilt, _ = service.answer_card(1, 3)
    assert rebuilt["stability"] < 3  # nowhere near the pre-lapse level


def test_card_reappears_exactly_when_due_not_before(storage, service, clock):
    due = clock.now() + timedelta(days=5)
    seed_card(storage, service, 1, reps=1, due=due)
    limits = _limits()

    for day in range(5):
        queue = service.get_due_cards(limits=limits)
        assert queue == [], f"card showed up {5 - day} day(s) early"
        clock.advance(days=1)

    queue = service.get_due_cards(limits=limits)
    assert [row["id"] for row in queue] == [1]


def test_backlog_blocks_new_cards_then_recovers_next_day(storage, service, clock):
    # A backlog of 60 real, graduated review cards overdue by 3 days,
    # plus a big new-card pool, under a goal of 50: the whole sprint
    # should be reviews, zero new cards, because reviews are appended
    # to the pool unconditionally and new intake only fills what's
    # left of the goal (service.py's "auto" pacing).
    start = clock.now()
    for i in range(1, 61):
        seed_card(storage, service, i)
        clock.current = start
        state, _ = service.answer_card(i, 3)  # 1st Good: still a learning step
        clock.current = datetime.fromisoformat(state["due"])
        service.answer_card(i, 3)  # 2nd Good: graduates to Review
        _set_due(storage, i, start - timedelta(days=3))
    # Move to "today" so yesterday's graduation reviews (logged at
    # `start`) don't themselves count against today's daily budgets.
    clock.current = start + timedelta(days=1)
    for i in range(61, 161):
        seed_card(storage, service, i)

    limits = _limits(daily_goal_cards=50, daily_new_cards=0,
                     session_card_limit=100)

    day1 = service.get_due_cards(limits=limits)
    assert len(day1) == 50
    assert all(row["reps"] > 0 for row in day1)  # all reviews, no new cards
    for row in day1:
        service.answer_card(row["id"], 3)

    # 10 of the 60 backlog cards weren't reached today; they're still
    # due tomorrow (a fresh Good reschedules weeks out, so the 50
    # already answered won't reappear). Once the backlog shrinks, new
    # cards should start filling the remainder of the goal again.
    clock.advance(days=1)
    day2 = service.get_due_cards(limits=limits)
    reviews = sum(1 for row in day2 if row["reps"] > 0)
    new = sum(1 for row in day2 if row["reps"] == 0)
    assert reviews == 10
    assert new == 40
    assert len(day2) == 50
