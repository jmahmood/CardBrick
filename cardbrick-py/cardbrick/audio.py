"""Audio playback with pluggable backends.

pygame.mixer is an *optional* compiled pygame module that depends on
SDL_mixer — a handheld firmware's SDL (or a vendored wheel) may simply
not have it. Audio therefore goes through the first backend that
actually works:

    1. "mixer"   — pygame.mixer, if the module exists AND init succeeds
    2. "command" — an external CLI player found on PATH (mpg123, ffplay,
                   aplay, ...), launched non-blocking per playback
    3. "none"    — logged no-op; the app runs silent but never crashes

Overrides via environment:

    CARDBRICK_AUDIO=auto|mixer|command|none   force a backend
    CARDBRICK_AUDIO_CMD="mpg123 -q {file}"    exact player command
                                              ({file} = media path)

Every failure path is logged so the on-device log explains silent
sessions, and playback never blocks the review loop.
"""

import logging
import os
import shlex
import shutil
import subprocess

log = logging.getLogger(__name__)

# name, argv prefix, extensions handled (None = anything)
_CLI_PLAYERS = [
    ("mpg123", ["mpg123", "-q"], {".mp3"}),
    ("ogg123", ["ogg123", "-q"], {".ogg"}),
    ("aplay", ["aplay", "-q"], {".wav"}),
    ("paplay", ["paplay"], {".wav", ".ogg", ".flac"}),
    ("afplay", ["afplay"], None),   # macOS built-in (dev boxes)
    ("ffplay", ["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet"],
     None),
]


class _MixerBackend:
    name = "mixer"

    def __init__(self):
        import pygame
        if not getattr(pygame, "mixer", None):
            raise RuntimeError("pygame built without mixer module")
        pygame.mixer.init()  # raises pygame.error if SDL_mixer/device bad
        self._music = pygame.mixer.music
        self._error = pygame.error
        log.info("audio: pygame.mixer initialised: %s",
                 pygame.mixer.get_init())

    def play(self, path):
        try:
            self._music.stop()
            self._music.load(path)
            self._music.play()
            return True
        except self._error as exc:
            log.warning("mixer could not play %s: %s", path, exc)
            return False

    def stop(self):
        try:
            self._music.stop()
        except self._error:
            pass


class _CommandBackend:
    """Plays media by spawning a CLI player, non-blocking.

    The child process is detached from the review loop; starting a new
    clip (or stop()) terminates the previous one, and finished players
    are reaped opportunistically so nothing zombifies.
    """

    name = "command"

    def __init__(self, custom_cmd=None, which_fn=None, popen_fn=None):
        self._which = which_fn or shutil.which
        self._popen = popen_fn or subprocess.Popen
        self._proc = None
        self._players = []  # (argv, extensions or None)

        if custom_cmd:
            argv = shlex.split(custom_cmd)
            binary = argv[0] if argv else ""
            if "{file}" not in argv:
                argv.append("{file}")
            if binary and self._which(binary):
                self._players.append((argv, None))
                log.info("audio: using custom player command: %s",
                         custom_cmd)
            else:
                log.error("audio: CARDBRICK_AUDIO_CMD player %r not on "
                          "PATH — ignoring", binary)
        for name, argv, extensions in _CLI_PLAYERS:
            if self._which(name):
                self._players.append((argv + ["{file}"], extensions))
        if not self._players:
            raise RuntimeError("no CLI audio player found on PATH")
        log.info("audio: CLI players available: %s",
                 ", ".join(argv[0] for argv, _ in self._players))

    def command_for(self, path):
        extension = os.path.splitext(path)[1].lower()
        for argv, extensions in self._players:
            if extensions is None or extension in extensions:
                return [path if part == "{file}" else part
                        for part in argv]
        return None

    def play(self, path):
        argv = self.command_for(path)
        if argv is None:
            log.warning("audio: no CLI player handles %s", path)
            return False
        self.stop()
        try:
            self._proc = self._popen(
                argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                stdin=subprocess.DEVNULL)
            return True
        except OSError as exc:
            log.warning("audio: could not run %s: %s", argv[0], exc)
            return False

    def stop(self):
        if self._proc is not None:
            if self._proc.poll() is None:
                try:
                    self._proc.terminate()
                except OSError:
                    pass
            try:
                self._proc.wait(timeout=0.5)  # reap; clips are short
            except (subprocess.TimeoutExpired, OSError):
                pass
            self._proc = None


class _NullBackend:
    name = "none"

    def play(self, path):
        return False

    def stop(self):
        pass


def _pick_backend(choice, custom_cmd):
    if choice == "none":
        log.info("audio: disabled by CARDBRICK_AUDIO=none")
        return _NullBackend()
    if choice in ("auto", "mixer"):
        try:
            return _MixerBackend()
        except Exception as exc:  # pygame.error or RuntimeError
            level = log.warning if choice == "auto" else log.error
            level("audio: pygame.mixer unavailable (%s)%s", exc,
                  " — trying CLI players" if choice == "auto" else "")
            if choice == "mixer":
                return _NullBackend()
    try:
        return _CommandBackend(custom_cmd=custom_cmd)
    except RuntimeError as exc:
        log.warning("audio: %s — continuing silent", exc)
        return _NullBackend()


class AudioPlayer:
    def __init__(self, media_dir, backend=None):
        self.media_dir = media_dir
        self._missing_logged = set()
        if backend is None:
            backend = _pick_backend(
                os.environ.get("CARDBRICK_AUDIO", "auto").strip().lower(),
                os.environ.get("CARDBRICK_AUDIO_CMD"))
        self.backend = backend
        self.enabled = backend.name != "none"
        log.info("audio backend: %s", backend.name)

    def available(self, filename):
        """True if the media file exists locally (missing-audio check)."""
        if not filename:
            return False
        return os.path.exists(
            os.path.join(self.media_dir, os.path.basename(filename)))

    def play(self, filename):
        """Play a media file by its imported name. Returns True if played."""
        if not self.enabled or not filename:
            return False
        path = os.path.join(self.media_dir, os.path.basename(filename))
        if not os.path.exists(path):
            if filename not in self._missing_logged:
                self._missing_logged.add(filename)
                log.warning("missing media file: %s", path)
            return False
        return self.backend.play(path)

    def stop(self):
        self.backend.stop()
