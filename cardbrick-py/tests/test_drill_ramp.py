"""Drill-deck economics: the ramp that keeps pattern drilling gentle.

Pattern drill cards live in a separate world from the vocab day:
they appear only when a sitting explicitly names drill deck(s), run
in tiny time-boxed sprints, take new patterns as a fixed drip, and
order recognition (MCQ) before production. Nothing they do counts
against the vocab goal, and vice versa.
"""

from cardbrick.session import StudySession
from cardbrick.vocab_csv import _stable_id
from cardbrick.pattern_pack import import_pattern_pack

from conftest import seed_card
from test_pattern_pack import mcq_item, production_item, write_pack

DRILL_DECK = "Mexico City — Drills"


def import_drill_items(tmp_path, storage, service, items):
    pack = write_pack(tmp_path / "ramp_pack.json", items)
    return import_pattern_pack(pack, storage, service.scheduler)


def many_production_items(n, start=100):
    return [production_item(pattern_id=f"P{start + i:03d}",
                            priority_score=0.5)
            for i in range(n)]


def drill_card_id(item):
    return _stable_id(
        f"{item['pattern_id']}:{item['kind']}:{item['variant']}")


# -- world split ------------------------------------------------------------


def test_drill_filter_detection(tmp_path, storage, service):
    import_drill_items(tmp_path, storage, service, [production_item()])
    seed_card(storage, service, 1, deck="Spanish")
    assert service.is_drill_filter([DRILL_DECK])
    assert not service.is_drill_filter(["Spanish"])
    assert not service.is_drill_filter(["Spanish", DRILL_DECK])
    assert not service.is_drill_filter(None)
    assert not service.is_drill_filter([])


def test_normal_sittings_never_serve_pattern_cards(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(5))
    seed_card(storage, service, 1, deck="Spanish")
    queue = service.get_due_cards(profile=profile)
    assert [row["id"] for row in queue] == [1]


def test_drill_sittings_serve_only_pattern_cards(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(5))
    seed_card(storage, service, 1, deck="Spanish")
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    assert queue and all(row["card_type"] == "pattern" for row in queue)


# -- drill economics ---------------------------------------------------------


def test_new_patterns_arrive_as_a_fixed_drip(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    assert len(queue) == 3  # drill_daily_new default, not goal-paced


def test_drip_resets_at_midnight(tmp_path, storage, service, profile,
                                 clock):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    for row in service.get_due_cards(profile=profile,
                                     deck_filter=[DRILL_DECK]):
        service.answer_card(row["id"], 3)
    assert service.get_due_cards(profile=profile,
                                 deck_filter=[DRILL_DECK]) == []
    clock.advance(days=1)
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    # Yesterday's three come back as reviews, plus a fresh 3-card drip.
    assert len(queue) == 6
    assert sum(1 for row in queue if row["reps"] == 0) == 3


def test_drill_sprints_are_tiny(tmp_path, storage, service, profile,
                                clock):
    items = many_production_items(10)
    import_drill_items(tmp_path, storage, service, items)
    for item in items:
        service.answer_card(drill_card_id(item), 3)
    clock.advance(days=40)  # everything due for review
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    assert len(queue) == 6  # drill_sprint_cards, not session_card_limit


def test_drill_profile_overrides_apply(tmp_path, storage, service,
                                       profile):
    storage.update_profile(profile["id"], drill_daily_new=1,
                           drill_sprint_cards=2, drill_sprint_minutes=7)
    profile = storage.get_profile(profile["id"])
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    assert len(queue) == 1
    limits = service.session_limits(profile, deck_filter=[DRILL_DECK])
    assert limits["session_card_limit"] == 2
    assert limits["session_time_minutes"] == 7


# -- goal isolation -----------------------------------------------------------


def test_drilling_never_advances_the_vocab_goal(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(5))
    seed_card(storage, service, 1, deck="Spanish")
    for row in service.get_due_cards(profile=profile,
                                     deck_filter=[DRILL_DECK]):
        service.answer_card(row["id"], 3)
    status = service.sprint_status(profile=profile)
    assert status["cards_done"] == 0  # the vocab day is untouched


def test_vocab_reviews_never_consume_the_drill_drip(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    for card_id in range(1, 6):
        seed_card(storage, service, card_id, deck="Spanish")
        service.answer_card(card_id, 3)
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    assert len(queue) == 3  # full drip still available


def test_drill_sprint_status_reports_the_drill_day(
        tmp_path, storage, service, profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    status = service.sprint_status(profile=profile,
                                   deck_filter=[DRILL_DECK])
    assert status["goal_today"] == 3  # the drip is the whole drill day
    assert status["next_sprint_cards"] == 3
    assert status["sprints_remaining"] == 1


# -- recognition before production --------------------------------------------


def test_new_drill_intake_orders_mcq_before_production(
        tmp_path, storage, service, profile):
    items = [
        production_item(pattern_id="P900", priority_score=0.99),
        mcq_item(pattern_id="P901", priority_score=0.5),
        mcq_item(pattern_id="P902", priority_score=0.9),
        production_item(pattern_id="P903", priority_score=0.4),
    ]
    import_drill_items(tmp_path, storage, service, items)
    queue = service.get_due_cards(profile=profile,
                                  deck_filter=[DRILL_DECK])
    kinds_and_ids = [
        (storage.get_pattern_detail(row["id"])["kind"],
         storage.get_pattern_detail(row["id"])["pattern_id"])
        for row in queue
    ]
    # MCQs first (higher priority first), production only after.
    assert kinds_and_ids == [
        ("mcq", "P902"), ("mcq", "P901"), ("production", "P900")]


# -- picker counts & sessions --------------------------------------------------


def test_picker_counts_split_the_worlds(tmp_path, storage, service,
                                        profile):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    seed_card(storage, service, 1, deck="Spanish")
    counts = service.counts_for_queue_many(
        [(None, None), ([DRILL_DECK], None), (["Spanish"], None)],
        profile=profile)
    assert counts[0] == (0, 1)  # all-decks: the one (new) vocab card,
    assert counts[1] == (0, 3)  # drills excluded; drill deck: the drip
    assert counts[2] == (0, 1)


def test_drill_session_uses_the_drill_time_box(
        tmp_path, storage, service, profile, clock):
    import_drill_items(tmp_path, storage, service,
                       many_production_items(10))
    seed_card(storage, service, 1, deck="Spanish")
    drill_session = StudySession(storage, service, profile,
                                 deck_filter=[DRILL_DECK])
    vocab_session = StudySession(storage, service, profile,
                                 deck_filter=["Spanish"])
    clock.advance(minutes=5, seconds=1)
    assert drill_session.time_limit_reached()
    assert not vocab_session.time_limit_reached()  # vocab box is 10 min


def test_new_profiles_get_drill_defaults(storage):
    profile = storage.ensure_default_profile("Maya")
    assert profile["drill_sprint_cards"] == 6
    assert profile["drill_sprint_minutes"] == 5
    assert profile["drill_daily_new"] == 3
