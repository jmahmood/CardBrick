"""Metric-driven footer layout for readable handheld typography."""

import pygame
import pytest

from cardbrick.app import _footer_height, _wrap_footer_text


@pytest.fixture(scope="module")
def font():
    pygame.font.init()
    value = pygame.font.Font(None, 31)  # builtin font equivalent of requested 25
    yield value
    pygame.font.quit()


def test_short_hint_stays_on_one_line(font):
    lines = _wrap_footer_text(font, "A = Start", None, 688)
    assert lines == ["A = Start"]
    assert _footer_height(font, len(lines)) >= 44


def test_long_hint_wraps_without_exceeding_width(font):
    lines = _wrap_footer_text(
        font,
        "Down = Reveal more   Hold Up = Undo   A = I know this   START = Menu",
        None,
        400,
    )
    assert len(lines) == 2
    assert all(font.size(line)[0] <= 400 for line in lines)


def test_explicit_two_line_hint_has_non_overlapping_chassis_height(font):
    lines = _wrap_footer_text(font, "A = Continue", "D-pad = Scroll", 688)
    assert lines == ["A = Continue", "D-pad = Scroll"]
    line_height = font.get_linesize()
    assert _footer_height(font, 2) >= 16 + 2 * line_height + 4
