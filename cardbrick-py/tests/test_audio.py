"""Audio backend selection and graceful degradation.

pygame.mixer is optional (needs SDL_mixer); the player must fall back
to CLI players and finally to a silent no-op without ever crashing.
"""

import os

from cardbrick.audio import (AudioPlayer, _CommandBackend, _NullBackend)


class FakeProc:
    def __init__(self, argv):
        self.argv = argv
        self.terminated = False

    def poll(self):
        return None if not self.terminated else 0

    def terminate(self):
        self.terminated = True

    def wait(self, timeout=None):
        return 0


def make_backend(available, custom_cmd=None, launched=None):
    launched = launched if launched is not None else []

    def which(name):
        return f"/usr/bin/{name}" if name in available else None

    def popen(argv, **_kwargs):
        proc = FakeProc(argv)
        launched.append(proc)
        return proc

    backend = _CommandBackend(custom_cmd=custom_cmd, which_fn=which,
                              popen_fn=popen)
    return backend, launched


def test_command_backend_picks_player_by_extension():
    backend, _ = make_backend({"mpg123", "aplay"})
    assert backend.command_for("/m/hola.mp3")[0] == "mpg123"
    assert backend.command_for("/m/hola.wav")[0] == "aplay"
    assert backend.command_for("/m/hola.opus") is None  # nobody handles it


def test_command_backend_ffplay_handles_anything():
    backend, _ = make_backend({"ffplay"})
    assert backend.command_for("/m/hola.opus")[0] == "ffplay"
    assert "/m/hola.opus" in backend.command_for("/m/hola.opus")


def test_afplay_covers_macos_dev_boxes():
    backend, _ = make_backend({"afplay"})
    assert backend.command_for("/m/hola.mp3")[0] == "afplay"
    assert backend.command_for("/m/hola.wav")[0] == "afplay"


def test_custom_command_wins_and_gets_file_substituted():
    backend, launched = make_backend({"mycustomplayer"},
                                     custom_cmd="mycustomplayer -x {file}")
    assert backend.play("/m/hola.mp3") is True
    assert launched[0].argv == ["mycustomplayer", "-x", "/m/hola.mp3"]


def test_play_terminates_previous_clip():
    backend, launched = make_backend({"ffplay"})
    backend.play("/m/uno.mp3")
    backend.play("/m/dos.mp3")
    assert launched[0].terminated
    assert not launched[1].terminated
    backend.stop()
    assert launched[1].terminated


def test_no_cli_player_raises_for_null_fallback():
    import pytest
    with pytest.raises(RuntimeError):
        make_backend(set())


def test_player_with_null_backend_never_crashes(tmp_path):
    player = AudioPlayer(str(tmp_path), backend=_NullBackend())
    assert player.enabled is False
    assert player.play("anything.mp3") is False
    player.stop()  # no-op, no crash


def test_missing_media_file_is_soft(tmp_path):
    backend, launched = make_backend({"ffplay"})
    player = AudioPlayer(str(tmp_path), backend=backend)
    assert player.play("missing.mp3") is False
    assert launched == []


def test_existing_media_plays_through_backend(tmp_path):
    (tmp_path / "hola.mp3").write_bytes(b"x")
    backend, launched = make_backend({"ffplay"})
    player = AudioPlayer(str(tmp_path), backend=backend)
    assert player.play("hola.mp3") is True
    assert launched[0].argv[-1] == os.path.join(str(tmp_path), "hola.mp3")


def test_extra_media_dir_is_used_as_readonly_fallback(tmp_path):
    primary = tmp_path / "save-media"
    fallback = tmp_path / "seed-media"
    primary.mkdir()
    fallback.mkdir()
    (fallback / "hola.mp3").write_bytes(b"x")
    backend, launched = make_backend({"ffplay"})
    player = AudioPlayer(str(primary), backend=backend,
                         extra_media_dirs=[str(fallback)])

    assert player.available("hola.mp3") is True
    assert player.resolve("hola.mp3") == os.path.join(str(fallback),
                                                      "hola.mp3")
    assert player.play("hola.mp3") is True
    assert launched[0].argv[-1] == os.path.join(str(fallback), "hola.mp3")
