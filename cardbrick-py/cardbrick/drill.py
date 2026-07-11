"""Rating logic for pattern drill cards (pygame-free).

Pattern cards keep CardBrick's no-rating-buttons rule: the learner
never picks Again/Hard/Good/Easy. For production-scaffold cards the
phase at which "I know this" is pressed *is* the rating (the same
ladder the vocab cards use); for multiple-choice cards the rating is
derived from correctness. Keeping the mappings here, out of app.py,
makes them unit-testable without a display.
"""

import json
import random

# Phase reached when "I know this" is pressed -> FSRS rating.
# 0 = answered from the bare cue (Easy) ... 3 = needed the full
# model answer (Again). Shared by vocab and pattern-production cards.
PHASE_RATING = {0: 4, 1: 3, 2: 2, 3: 1}
MAX_PHASE = 3

def mcq_rating(correct, elapsed_ms=None):
    """FSRS rating for a multiple-choice answer.

    Wrong (or bailed out with "show me") is Again; correct is Easy —
    recognition is a ramp stage, and a correct pick graduates
    immediately instead of earning a same-session learning step (the
    pattern's production cards are the real consolidation). Latency
    is logged with the review but never grades: reading three
    sentences takes as long as it takes.
    """
    return 4 if correct else 1


def mcq_option_order(card_id, n=3):
    """Display order of MCQ options, shuffled deterministically.

    Seeded by the card id so a card always shows its options in the
    same order — across sessions, devices, and re-imports — while
    different cards get different orders.
    """
    order = list(range(n))
    random.Random(card_id).shuffle(order)
    return order


def parse_mcq_options(options_json):
    """Decode a pattern_cards.options_json value.

    Returns (texts, correct_index) in authored order.
    """
    options = json.loads(options_json)
    texts = [option["text"] for option in options]
    correct_index = next(
        i for i, option in enumerate(options) if option.get("correct")
    )
    return texts, correct_index
