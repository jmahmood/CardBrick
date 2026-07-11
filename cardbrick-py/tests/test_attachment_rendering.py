"""Deterministic rendering checks for vocabulary paper attachments."""

import os
import sys

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import pygame  # noqa: E402

from cardbrick.app import CardBrickApp, _autoplay_vocab_phase  # noqa: E402


def make_app_with_image(filename, size):
    pygame.init()
    app = CardBrickApp.__new__(CardBrickApp)
    app.w = 640
    image = pygame.Surface(size, pygame.SRCALPHA)
    image.fill((20, 80, 140, 255))
    app._image_cache = {(filename, None): image}
    return app


def test_attachment_is_bordered_bounded_and_deterministic():
    app = make_app_with_image("wide-photo.png", (900, 120))

    first, first_angle = app._vocab_attachment_surface("wide-photo.png")
    second, second_angle = app._vocab_attachment_surface("wide-photo.png")

    assert first is second
    assert first_angle == second_angle
    assert 1.0 <= abs(first_angle) <= 2.0
    assert first.get_width() <= app.content_x1 - app.content_x0
    # Rotation adds a handful of pixels around the 80%-wide mount.
    assert first.get_width() >= int(app.w * 0.80)
    assert first.get_height() > 0


def test_small_portrait_is_upscaled_instead_of_left_as_a_thumbnail():
    app = make_app_with_image("portrait.png", (100, 160))

    mounted, _ = app._vocab_attachment_surface("portrait.png")

    assert mounted.get_width() >= int(app.w * 0.80)
    assert mounted.get_height() > mounted.get_width()


def test_different_filenames_can_choose_opposite_crooked_directions():
    left = make_app_with_image("a.png", (100, 80))
    right = make_app_with_image("cat.png", (100, 80))

    _, left_angle = left._vocab_attachment_surface("a.png")
    _, right_angle = right._vocab_attachment_surface("cat.png")

    assert left_angle * right_angle < 0


def test_sentence_audio_does_not_repeat_for_picture_or_definition_phase():
    assert _autoplay_vocab_phase(0, 1)
    assert not _autoplay_vocab_phase(1, 2)
    assert not _autoplay_vocab_phase(1, 3)  # picture absent: phase 2 skipped
    assert not _autoplay_vocab_phase(2, 3)


def test_staple_impact_uses_dedicated_effect_at_audible_level():
    calls = []

    class Audio:
        def play_effect(self, filename, volume):
            calls.append((filename, volume))

    app = CardBrickApp.__new__(CardBrickApp)
    app.settings = {
        "paper_feed_sfx_enabled": True,
        "paper_feed_sfx_volume": 0.15,
    }
    app.audio = Audio()

    app._paper_feed_sound("staple")

    assert calls == [("staple-clack.wav", 0.375)]
