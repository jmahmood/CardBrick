"""CardBrickApp._truncate_to_width: the guard against long deck names,
tag lists, or category labels overlapping/overflowing whatever is
drawn next to them (the review header, the deck/category pickers, and
Parent Mode's category/deck assignment screen all rely on it)."""

import os

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import pygame  # noqa: E402
import pytest  # noqa: E402

from cardbrick.app import CardBrickApp  # noqa: E402


@pytest.fixture
def font():
    # Function-scoped: other test modules (e.g. test_input_map.py) call
    # pygame.quit() in their own teardown, which invalidates any font
    # created outside a test's own init/quit bracket.
    pygame.font.init()
    yield pygame.font.Font(None, 24)
    pygame.font.quit()


def test_short_text_is_unchanged(font):
    text = "Spanish"
    assert CardBrickApp._truncate_to_width(font, text, 400) == text


def test_text_wider_than_budget_is_shortened_with_ellipsis(font):
    long_text = "isla::Gramatica isla::Directores nivel::intermedio"
    budget = font.size(long_text)[0] // 2
    result = CardBrickApp._truncate_to_width(font, long_text, budget)
    assert result.endswith("…")
    assert result != long_text
    assert font.size(result)[0] <= budget


def test_result_never_exceeds_the_budget_even_for_tiny_budgets(font):
    for budget in (0, 1, 5, 10, 50):
        result = CardBrickApp._truncate_to_width(
            font, "a very long deck name indeed", budget)
        assert font.size(result)[0] <= budget


def test_zero_or_negative_budget_yields_empty_string(font):
    assert CardBrickApp._truncate_to_width(font, "anything", 0) == ""
    assert CardBrickApp._truncate_to_width(font, "anything", -5) == ""
