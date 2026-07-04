"""Non-interactive smoke test: run before copying to the handheld.

    python main.py --smoke-test [--data-dir PATH] [--knulli|--desktop]

Checks every subsystem the study loop depends on and prints a PASS /
FAIL / WARN line per check plus a summary. Exit code 0 = all hard
checks passed. Warnings (no cards yet, no joystick, no audio device)
don't fail the run — they are normal on a desktop dev box.
"""

import logging
import os

log = logging.getLogger(__name__)

PASS, WARN, FAIL = "PASS", "WARN", "FAIL"


class SmokeResult:
    def __init__(self):
        self.checks = []  # (status, name, detail)

    def add(self, status, name, detail=""):
        self.checks.append((status, name, detail))
        line = f"[{status}] {name}" + (f": {detail}" if detail else "")
        print(line)
        (log.error if status == FAIL else log.info)("smoke %s", line)

    @property
    def ok(self):
        return not any(status == FAIL for status, _, _ in self.checks)

    def summary(self):
        counts = {s: 0 for s in (PASS, WARN, FAIL)}
        for status, _, _ in self.checks:
            counts[status] += 1
        verdict = "SMOKE TEST PASSED" if self.ok else "SMOKE TEST FAILED"
        return (f"{verdict} — {counts[PASS]} passed, "
                f"{counts[WARN]} warnings, {counts[FAIL]} failures")


def run_smoke_test(paths):
    """Run all checks against the resolved AppPaths. Returns SmokeResult."""
    result = SmokeResult()

    # -- writable data root + log ------------------------------------------------
    try:
        paths.ensure_directories()
        probe = os.path.join(paths.data_dir, ".write-probe")
        with open(probe, "w", encoding="utf-8") as f:
            f.write("ok")
        os.remove(probe)
        result.add(PASS, "data dir writable", paths.data_dir)
    except OSError as exc:
        result.add(FAIL, "data dir writable", str(exc))
        return result  # nothing else can work

    try:
        with open(paths.log_path, "a", encoding="utf-8"):
            pass
        result.add(PASS, "log file writable", paths.log_path)
    except OSError as exc:
        result.add(FAIL, "log file writable", str(exc))

    # -- settings ------------------------------------------------------------------
    from .settings import AppSettings
    try:
        settings = AppSettings(paths.settings_path)
        result.add(PASS, "settings load",
                    f"fullscreen={settings.get('fullscreen')}")
    except Exception as exc:  # noqa: BLE001
        result.add(FAIL, "settings load", str(exc))
        settings = None

    # -- database + migration --------------------------------------------------------
    storage = None
    try:
        from .storage import SCHEMA_VERSION, Storage
        storage = Storage(paths.db_path)
        version = storage.schema_version()
        if version == SCHEMA_VERSION:
            result.add(PASS, "database open + migrations",
                        f"{paths.db_path} (schema v{version})")
        else:
            result.add(FAIL, "database open + migrations",
                        f"schema v{version}, expected v{SCHEMA_VERSION}")
    except Exception as exc:  # noqa: BLE001
        result.add(FAIL, "database open + migrations", str(exc))

    # -- content / profile -------------------------------------------------------------
    if storage:
        n_cards = storage.conn.execute(
            "SELECT COUNT(*) AS n FROM cards").fetchone()["n"]
        if n_cards:
            result.add(PASS, "cards present", f"{n_cards} cards")
        else:
            result.add(WARN, "cards present",
                        "no cards imported yet (import an .apkg)")

        profiles = storage.list_profiles()
        if profiles:
            profile = profiles[0]
            cats = profile["active_categories"]
            result.add(PASS, "child profile",
                        f"{profile['name']} "
                        f"(categories: {'all' if cats is None else cats})")
            if cats == []:
                result.add(WARN, "active categories",
                            "empty category list — child will see 0 cards")
        else:
            result.add(WARN, "child profile",
                        "none configured yet (created on first run)")

        try:
            from .scheduler import ReviewScheduler
            from .service import ReviewService
            service = ReviewService(storage, ReviewScheduler())
            queue = service.get_due_cards(
                profile=profiles[0] if profiles else None)
            result.add(PASS, "scheduler queue query",
                        f"{len(queue)} cards due under current limits")
        except Exception as exc:  # noqa: BLE001
            result.add(FAIL, "scheduler queue query", str(exc))

    if os.path.isdir(paths.media_dir):
        n_media = len(os.listdir(paths.media_dir))
        result.add(PASS, "media directory", f"{n_media} files")
    else:
        result.add(FAIL, "media directory", "missing " + paths.media_dir)

    # -- pygame / display / font / joystick / audio -----------------------------------------
    try:
        import pygame
        pygame.init()
        result.add(PASS, "pygame init",
                    f"{'pygame-ce' if getattr(pygame, 'IS_CE', False) else 'pygame'} "
                    f"{pygame.version.ver}")

        try:
            pygame.display.set_mode((64, 48))
            result.add(PASS, "display init",
                        f"driver={pygame.display.get_driver()}")
        except pygame.error as exc:
            result.add(FAIL, "display init",
                        f"{exc} (try SDL_VIDEODRIVER=dummy for headless)")

        from .app import _load_font, resolve_font_path
        font_path = resolve_font_path()
        font = _load_font(24)
        spanish = "áéíóúñü¿¡"
        width = font.size(spanish)[0]
        if font_path and width > 0:
            result.add(PASS, "font (Spanish glyphs)", font_path)
        elif width > 0:
            result.add(WARN, "font (Spanish glyphs)",
                        "builtin fallback font — bundle assets/fonts")
        else:
            result.add(FAIL, "font (Spanish glyphs)",
                        "cannot render Spanish characters")

        try:
            count = pygame.joystick.get_count()
            for i in range(count):
                pygame.joystick.Joystick(i).init()
            result.add(PASS if count else WARN, "joystick detection",
                        f"{count} device(s)"
                        + ("" if count else " — keyboard only"))
        except pygame.error as exc:
            result.add(FAIL, "joystick detection", str(exc))

        try:
            from .audio import AudioPlayer
            player = AudioPlayer(paths.media_dir)
            if player.backend.name == "mixer":
                result.add(PASS, "audio backend", "pygame.mixer")
            elif player.backend.name == "command":
                result.add(PASS, "audio backend",
                            "CLI player fallback (no SDL_mixer needed)")
            else:
                result.add(WARN, "audio backend",
                            "none available — app runs silent")
            player.stop()
        except Exception as exc:  # noqa: BLE001
            result.add(FAIL, "audio backend", str(exc))
        pygame.quit()
    except Exception as exc:  # noqa: BLE001
        result.add(FAIL, "pygame init", str(exc))

    # -- input mapping -----------------------------------------------------------------------
    try:
        from .input_map import InputMap
        imap = InputMap(paths.input_map_path)
        result.add(PASS, "input mapping",
                    f"{len(imap.buttons)} buttons ({imap.source})")
    except Exception as exc:  # noqa: BLE001
        result.add(FAIL, "input mapping", str(exc))

    if storage:
        storage.close()
    print(result.summary())
    log.info("smoke %s", result.summary())
    return result
