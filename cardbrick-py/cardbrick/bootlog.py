"""File logging and startup diagnostics.

Handhelds rarely show a terminal, so everything interesting is written
to a rotating log file under the data directory. ``log_environment`` is
called before pygame exists; ``log_display_diagnostics`` after the
display and joysticks are initialised.
"""

import logging
import logging.handlers
import os
import platform
import sys

log = logging.getLogger("cardbrick")


def setup_logging(log_path, verbose=False):
    """Log to a size-capped rotating file plus stderr.

    Returns the path actually used, or None if the file could not be
    opened (the app keeps running with stderr-only logging).
    """
    root = logging.getLogger()
    root.setLevel(logging.DEBUG if verbose else logging.INFO)
    fmt = logging.Formatter(
        "%(asctime)s %(levelname).1s %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S")

    stream = logging.StreamHandler(sys.stderr)
    stream.setFormatter(fmt)
    root.addHandler(stream)

    try:
        os.makedirs(os.path.dirname(os.path.abspath(log_path)),
                    exist_ok=True)
        # Small caps: SD cards are slow and small; two 512 KiB files of
        # history are plenty for debugging a session.
        file_handler = logging.handlers.RotatingFileHandler(
            log_path, maxBytes=512 * 1024, backupCount=2,
            encoding="utf-8")
        file_handler.setFormatter(fmt)
        root.addHandler(file_handler)
        return log_path
    except OSError as exc:
        root.warning("Could not open log file %s: %s "
                     "(continuing with stderr only)", log_path, exc)
        return None


def _dist_version(name):
    try:
        from importlib import metadata
        return metadata.version(name)
    except Exception:
        return None


def log_environment(paths, app_version):
    """Pre-pygame diagnostics: interpreter, platform, resolved paths."""
    log.info("CardBrick Spanish v%s starting", app_version)
    log.info("Python %s on %s", sys.version.split()[0], platform.platform())
    log.info("cwd=%s", os.getcwd())
    log.info("executable=%s argv0=%s", sys.executable,
             os.path.abspath(sys.argv[0]))
    for key, value in paths.describe().items():
        log.info("%s=%s", key, value)
    fsrs_version = _dist_version("fsrs")
    log.info("fsrs version: %s", fsrs_version or "unknown")
    for var in ("CARD_BRICK_DATA_DIR", "CARDBRICK_DATA", "CARDBRICK_MODE",
                "CARDBRICK_FONT", "SDL_VIDEODRIVER", "SDL_AUDIODRIVER"):
        if os.environ.get(var):
            log.info("env %s=%s", var, os.environ[var])


def log_pygame_versions():
    import pygame
    flavour = "pygame-ce" if getattr(pygame, "IS_CE", False) else "pygame"
    log.info("%s %s (SDL %s)", flavour, pygame.version.ver,
             ".".join(str(x) for x in pygame.version.SDL))


def log_display_diagnostics(screen_size, logical_size, fullscreen,
                            font_path):
    """Post-init diagnostics: SDL drivers, display, joysticks, font."""
    import pygame
    try:
        log.info("SDL video driver: %s", pygame.display.get_driver())
    except pygame.error as exc:
        log.error("SDL video driver unavailable: %s", exc)
    log.info("display=%sx%s logical=%sx%s fullscreen=%s",
             screen_size[0], screen_size[1],
             logical_size[0], logical_size[1], fullscreen)

    mixer = pygame.mixer.get_init()
    if mixer:
        log.info("audio: mixer initialised (freq=%s, channels=%s)",
                 mixer[0], mixer[2])
    else:
        log.warning("audio: mixer NOT initialised — playback disabled")

    count = pygame.joystick.get_count()
    log.info("joysticks detected: %d", count)
    if count == 0:
        log.warning("no joystick detected — keyboard input only")
    for i in range(count):
        try:
            joy = pygame.joystick.Joystick(i)
            guid = getattr(joy, "get_guid", lambda: "n/a")()
            log.info("joystick %d: name=%r guid=%s buttons=%d hats=%d "
                     "axes=%d", i, joy.get_name(), guid,
                     joy.get_numbuttons(), joy.get_numhats(),
                     joy.get_numaxes())
        except pygame.error as exc:
            log.error("joystick %d: could not query (%s)", i, exc)

    log.info("font: %s", font_path or "pygame builtin fallback "
                                      "(NOT safe for Spanish/Japanese)")
