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
    CARDBRICK_EXTRA_MEDIA_DIRS=/path:/path     read-only fallback media dirs

Every failure path is logged so the on-device log explains silent
sessions, and playback never blocks the review loop.
"""

import logging
import os
import shlex
import shutil
import subprocess
import tempfile
import time
import wave
import struct

log = logging.getLogger(__name__)

# Mixer device settings (PERFORMANCE.md). 48000 Hz matches the
# handheld's DAC (/proc/asound hw_params: rate 48000, period 1024) —
# opening at 44100 made SDL resample every sample of every callback,
# which is where most of the measured 8.6%-CPU-while-silent went; ALSA
# clamps the period size anyway, so a bigger buffer alone didn't help.
# SDL_mixer converts each file once at load, so 44.1 kHz media still
# plays fine.
MIXER_SETTINGS = {"frequency": 48000, "size": -16, "channels": 2,
                  "buffer": 2048}
# Close the audio device once nothing has been audible for this long;
# it reopens transparently on the next play. An open device costs CPU
# (and battery) even in silence.
AUDIO_IDLE_CLOSE_S = 10.0

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
        self._pygame = pygame
        # raises pygame.error if SDL_mixer/device bad
        pygame.mixer.init(**MIXER_SETTINGS)
        self._music = pygame.mixer.music
        self._sounds = {}
        self._sound_cls = pygame.mixer.Sound
        self._error = pygame.error
        self._last_active = time.monotonic()
        log.info("audio: pygame.mixer initialised: %s",
                 pygame.mixer.get_init())

    def _ensure_ready(self):
        """Reopen the device if release_if_idle() closed it."""
        if self._pygame.mixer.get_init():
            return True
        try:
            self._pygame.mixer.init(**MIXER_SETTINGS)
        except self._error as exc:
            log.warning("mixer could not reopen audio device: %s", exc)
            return False
        # Sound objects don't outlive a device session.
        self._sounds.clear()
        self._sound_cls = self._pygame.mixer.Sound
        self._music = self._pygame.mixer.music
        log.debug("audio: mixer reopened")
        return True

    def release_if_idle(self, idle_s=AUDIO_IDLE_CLOSE_S):
        """Close the audio device after ``idle_s`` of silence.

        An open device runs its mixing callback (and costs CPU) even
        with nothing to play; screens call this from their idle path.
        """
        mixer = self._pygame.mixer
        if not mixer.get_init():
            return
        if mixer.get_busy() or self._music.get_busy():
            self._last_active = time.monotonic()
            return
        if time.monotonic() - self._last_active < idle_s:
            return
        mixer.quit()
        self._sounds.clear()
        log.debug("audio: mixer closed after %.0fs of silence", idle_s)

    def play(self, path):
        if not self._ensure_ready():
            return False
        self._last_active = time.monotonic()
        try:
            self._music.stop()
            self._music.load(path)
            self._music.play()
            return True
        except self._error as exc:
            log.warning("mixer could not play %s: %s", path, exc)
            return False

    def play_effect(self, path, volume=1.0):
        if not self._ensure_ready():
            return False
        self._last_active = time.monotonic()
        try:
            sound = self._sounds.get(path)
            if sound is None:
                sound = self._sound_cls(path)
                self._sounds[path] = sound
            sound.set_volume(volume)
            sound.play()
            return True
        except self._error as exc:
            log.warning("mixer could not play effect %s: %s", path, exc)
            return False

    def stop(self):
        if not self._pygame.mixer.get_init():
            return
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
        self._effect_procs = []
        self._effect_cache = {}
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

    def play_effect(self, path, volume=1.0):
        path = self._effect_path(path, volume)
        argv = self.command_for(path)
        if argv is None:
            log.warning("audio: no CLI player handles effect %s", path)
            return False
        self._reap_effects()
        try:
            self._effect_procs.append(self._popen(
                argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                stdin=subprocess.DEVNULL))
            return True
        except OSError as exc:
            log.warning("audio: could not run effect player %s: %s",
                        argv[0], exc)
            return False

    def _effect_path(self, path, volume):
        if volume >= 0.995 or os.path.splitext(path)[1].lower() != ".wav":
            return path
        key = (path, round(max(volume, 0.0), 3))
        cached = self._effect_cache.get(key)
        if cached and os.path.exists(cached):
            return cached
        try:
            scaled = _scaled_wav(path, key[1])
        except (OSError, wave.Error, struct.error) as exc:
            log.warning("audio: could not scale effect %s: %s", path, exc)
            return path
        self._effect_cache[key] = scaled
        return scaled

    def _reap_effects(self):
        alive = []
        for proc in self._effect_procs:
            if proc.poll() is None:
                alive.append(proc)
                continue
            try:
                proc.wait(timeout=0)
            except (subprocess.TimeoutExpired, OSError):
                pass
        self._effect_procs = alive

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

    def play_effect(self, path, volume=1.0):
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
    def __init__(self, media_dir, backend=None, extra_media_dirs=None,
                 sfx_dirs=None):
        self.media_dir = media_dir
        if extra_media_dirs is None:
            extra_media_dirs = os.environ.get("CARDBRICK_EXTRA_MEDIA_DIRS", "")
            extra_media_dirs = [
                path for path in extra_media_dirs.split(os.pathsep) if path
            ]
        self.media_dirs = [media_dir] + list(extra_media_dirs)
        if sfx_dirs is None:
            sfx_dirs = self._default_sfx_dirs()
        self.sfx_dirs = list(sfx_dirs)
        self._missing_logged = set()
        self._missing_effects_logged = set()
        if backend is None:
            backend = _pick_backend(
                os.environ.get("CARDBRICK_AUDIO", "auto").strip().lower(),
                os.environ.get("CARDBRICK_AUDIO_CMD"))
        self.backend = backend
        self.enabled = backend.name != "none"
        log.info("audio backend: %s", backend.name)
        if len(self.media_dirs) > 1:
            log.info("media search dirs: %s", ", ".join(self.media_dirs))
        if self.sfx_dirs:
            log.info("sfx search dirs: %s", ", ".join(self.sfx_dirs))

    @staticmethod
    def _default_sfx_dirs():
        cardbrick_dir = os.path.dirname(os.path.abspath(__file__))
        app_dir = os.path.dirname(cardbrick_dir)
        repo_dir = os.path.dirname(app_dir)
        candidates = [
            os.path.join(app_dir, "assets", "sfx"),
            os.path.join(repo_dir, "assets", "sfx"),
        ]
        return [path for path in candidates if os.path.isdir(path)]

    def resolve(self, filename):
        if not filename:
            return None
        basename = os.path.basename(filename)
        for media_dir in self.media_dirs:
            path = os.path.join(media_dir, basename)
            if os.path.exists(path):
                return path
        return None

    def available(self, filename):
        """True if the media file exists locally (missing-audio check)."""
        return self.resolve(filename) is not None

    def play(self, filename):
        """Play a media file by its imported name. Returns True if played."""
        if not self.enabled or not filename:
            return False
        path = self.resolve(filename)
        if path is None:
            if filename not in self._missing_logged:
                self._missing_logged.add(filename)
                log.warning("missing media file: %s",
                            os.path.join(self.media_dir,
                                         os.path.basename(filename)))
            return False
        return self.backend.play(path)

    def resolve_effect(self, filename_or_path):
        if not filename_or_path:
            return None
        if os.path.isabs(filename_or_path) and os.path.exists(filename_or_path):
            return filename_or_path
        if os.path.dirname(filename_or_path) and os.path.exists(filename_or_path):
            return os.path.abspath(filename_or_path)
        basename = os.path.basename(filename_or_path)
        for sfx_dir in self.sfx_dirs:
            path = os.path.join(sfx_dir, basename)
            if os.path.exists(path):
                return path
        return None

    def play_effect(self, filename_or_path, volume=1.0):
        """Play a bundled UI sound effect without interrupting media."""
        if not self.enabled or not filename_or_path:
            return False
        path = self.resolve_effect(filename_or_path)
        if path is None:
            if filename_or_path not in self._missing_effects_logged:
                self._missing_effects_logged.add(filename_or_path)
                log.warning("missing sound effect: %s", filename_or_path)
            return False
        volume = min(max(float(volume), 0.0), 1.0)
        if volume <= 0:
            return False
        return self.backend.play_effect(path, volume=volume)

    def stop(self):
        self.backend.stop()

    def release_if_idle(self, idle_s=AUDIO_IDLE_CLOSE_S):
        """Let the backend close its audio device after silence."""
        release = getattr(self.backend, "release_if_idle", None)
        if release is not None:
            release(idle_s)


def _scaled_wav(path, volume):
    """Return a temp WAV copy with sample amplitude scaled by volume."""
    with wave.open(path, "rb") as src:
        params = src.getparams()
        frames = src.readframes(src.getnframes())
    if params.sampwidth != 2:
        raise wave.Error("only 16-bit PCM effects can be scaled")
    samples = struct.unpack("<" + "h" * (len(frames) // 2), frames)
    scaled = bytearray()
    for sample in samples:
        value = int(sample * volume)
        value = max(-32768, min(32767, value))
        scaled.extend(struct.pack("<h", value))
    fd, out = tempfile.mkstemp(prefix="cardbrick-sfx-", suffix=".wav")
    os.close(fd)
    with wave.open(out, "wb") as dst:
        dst.setparams(params)
        dst.writeframes(bytes(scaled))
    return out
