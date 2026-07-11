"""Rating logic for pattern drill cards (cardbrick/drill.py)."""

import json

from cardbrick import drill


def test_phase_rating_map_matches_ladder():
    # 0 = bare cue (Easy) ... 3 = needed the answer (Again)
    assert drill.PHASE_RATING == {0: 4, 1: 3, 2: 2, 3: 1}
    assert drill.MAX_PHASE == 3


def test_mcq_wrong_is_again_regardless_of_speed():
    assert drill.mcq_rating(False, 0) == 1
    assert drill.mcq_rating(False, 100000) == 1
    assert drill.mcq_rating(False, None) == 1


def test_mcq_correct_is_easy_regardless_of_speed():
    # Recognition graduates immediately: a correct pick never earns a
    # same-session learning step, however long the reading took.
    assert drill.mcq_rating(True, 0) == 4
    assert drill.mcq_rating(True, 100000) == 4
    assert drill.mcq_rating(True, None) == 4


def test_mcq_option_order_is_deterministic_per_card():
    assert drill.mcq_option_order(12345) == drill.mcq_option_order(12345)


def test_mcq_option_order_is_a_permutation():
    for card_id in range(50):
        assert sorted(drill.mcq_option_order(card_id)) == [0, 1, 2]


def test_mcq_option_order_actually_shuffles():
    orders = {tuple(drill.mcq_option_order(card_id))
              for card_id in range(50)}
    assert len(orders) > 1
    assert any(order != (0, 1, 2) for order in orders)


def test_parse_mcq_options():
    options_json = json.dumps([
        {"text": "A mi hija le gustan los tacos.", "correct": True},
        {"text": "A mi hija le gusta los tacos.", "correct": False},
        {"text": "Mi hija gusta los tacos.", "correct": False},
    ])
    texts, correct_index = drill.parse_mcq_options(options_json)
    assert texts == [
        "A mi hija le gustan los tacos.",
        "A mi hija le gusta los tacos.",
        "Mi hija gusta los tacos.",
    ]
    assert correct_index == 0
