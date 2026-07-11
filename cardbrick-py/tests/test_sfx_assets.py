"""Bundled UI-effect assets required by timed interface events."""

import os
import wave


ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def test_staple_clack_is_a_short_playable_wav():
    path = os.path.join(ROOT, "assets", "sfx", "staple-clack.wav")
    assert os.path.isfile(path)
    with wave.open(path, "rb") as sound:
        duration = sound.getnframes() / sound.getframerate()
        assert sound.getnchannels() in (1, 2)
        assert sound.getsampwidth() == 2
        assert 0.08 <= duration <= 0.30
