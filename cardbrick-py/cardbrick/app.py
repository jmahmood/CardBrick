"""CardBrick-style study appliance UI.

A state-driven pygame app aimed at children microstudying Spanish on a
handheld: a daily card goal chipped away in short sprints across the
day (see ReviewService.sprint_status), never one long sitting.

The visual theme is "Paper & Ink" built around a single metaphor: the
screen is a printer with an unlimited paper spool (see paperroll.py).
Cards are *printed* — every line of content is a line feed, every new
card a page feed past a dashed perforation, undo a reverse feed —
between tractor-feed sprocket strips, with button hints on the dark
chassis bar below the paper. All feeds are interruptible (any input
snaps them to done) and the reduced_motion setting disables them
entirely. Screens:

    ChildStart -> DeckSelect (only when >1 deck is assigned)
              -> CategorySelect (only when >1 category is assigned)
              -> Review (one sprint) -> SessionSummary
                 -> Review again ("going ahead": the next sprint now)
                 -> or back to ChildStart
    ChildStart / SessionSummary -> Calendar (stamp calendar) -> back
    ChildStart -> ParentMode (import / decks / categories / limits /
                              suspended / progress / calendar /
                              controller setup) -> ChildStart

Parent Mode's Decks/Categories screens configure which decks and tags
are *assigned* to the child at all (child_profiles.active_decks /
active_categories) — the scope a parent has decided is in bounds.
DeckSelect and CategorySelect are the child's own per-sitting choice of
which *one* assigned deck, or *one* assigned tag, to focus on right
now (or all of them combined, via "All assigned ..."). Choosing a tag
never moves cards or changes what they're tagged with — same as
Anki's filtered decks — so finishing the sprint (or picking "All
assigned categories" next time) simply returns to studying everything
in scope. A single assigned deck/category skips its picker entirely —
no extra tap when there's no real choice to make.

Controller-first and deployment-hardened for Knulli-style devices:

- Input is translated to *semantic* actions (south_button, dpad_up,
  start, ...) through a JSON mapping calibrated on-device — raw SDL
  button indices are never trusted (see input_map.py). Face buttons are
  labelled with the fixed layout used by this app (A = right, B = bottom,
  X = top, Y = left).
- Everything renders on a native handheld logical canvas (640x480 or
  720x480 when detected) which is scaled — integer scaling when it
  fits — to the real display.
- The app has its own exit path (SELECT quits/finishes) and never
  relies on RetroArch hotkeys.
- A bundled Unicode font covers Spanish/Japanese glyphs when present;
  font resolution and all startup facts are logged (see bootlog.py).

Keyboard fallback for desktop testing: Down reveals, 1/2/3/4 =
Again/Hard/Good/Easy (or literal A/B/X/Y keys), L replay, R bury,
U undo, Tab menu, Esc finish/quit.
"""

import calendar as _calendar
import logging
import os
import time

import pygame

from .bootlog import log_display_diagnostics, log_pygame_versions
from .importer import ApkgError, import_apkg
from .input_map import (
    AXIS_THRESHOLD,
    FACE_LABELS,
    KEYBOARD_SEMANTICS,
    STUDY_ACTIONS,
    InputMap,
    InputTranslator,
)
from .paperroll import PAGE_GAP, PERF_H, PaperRoll
from .scheduler import iso
from .session import StudySession
from .textutil import wrap_text

log = logging.getLogger(__name__)

FPS = 30
DEFAULT_LOGICAL_SIZE = (640, 480)
NATIVE_FULLSCREEN_SIZES = {
    (640, 480),  # RG35XX Plus and similar 4:3 panels
    (720, 480),  # RG34XX SP widescreen panel
}

# "Paper & Ink" palette: the whole UI is warm paper printed with ink,
# one vermillion stamp-red accent, and a dark printer-chassis bar.
BG = (242, 237, 227)  # warm paper
FG = (43, 45, 58)  # ink
DIM = (138, 133, 123)  # pencil grey
ACCENT = (217, 83, 74)  # vermillion stamp red
GOOD = (76, 154, 107)  # leaf green
WARN = (196, 138, 44)  # amber
DIVIDER = (214, 207, 193)  # faint printed rule
OVERLAY_BG = (250, 247, 240)  # paper slip laid on top of the roll

# Printer chrome: tractor-feed sprocket strips ride down both edges of
# the paper; button hints live on the chassis bar (the printer body).
STRIP = (233, 226, 211)
HOLE = (205, 196, 176)
STRIP_W = 34
HOLE_SPACING = 30
CHASSIS = (56, 58, 68)
CHASSIS_TEXT = (223, 219, 207)

# Semantic face button -> FSRS rating. Fixed face layout:
# A/right=Good, B/bottom=Again, Y/left=Easy, X/top=Hard.
RATING_FOR_SEMANTIC = {
    "east_button": 3,
    "north_button": 2,
    "south_button": 1,
    "west_button": 4,
}

DPAD = ("dpad_up", "dpad_down", "dpad_left", "dpad_right")
LINE_FEED_SFX = "line-tick.wav"
PAGE_FEED_SFX = "page-whirr.wav"

_BUNDLED_FONT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "fonts"
)
_ROOT_FONT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
    "assets",
    "font",
)

FONT_CANDIDATES = [
    os.environ.get("CARDBRICK_FONT", ""),
    os.path.join(_BUNDLED_FONT_DIR, "NotoSansJP-Regular.otf"),
    os.path.join(_BUNDLED_FONT_DIR, "NotoSansCJK-Regular.ttc"),
    os.path.join(_ROOT_FONT_DIR, "NotoSansJP-Regular.otf"),
    os.path.join(_ROOT_FONT_DIR, "NotoSansCJK-Regular.ttc"),
    os.path.join(_ROOT_FONT_DIR, "PixelMplus10-Regular.ttf"),
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansJP-Regular.otf",
    "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
    "/System/Library/Fonts/\u30d2\u30e9\u30ae\u30ce\u89d2\u30b4\u30b7\u30c3\u30af W3.ttc",
    "C:/Windows/Fonts/msgothic.ttc",
    "C:/Windows/Fonts/meiryo.ttc",
    os.path.join(_BUNDLED_FONT_DIR, "DejaVuSans.ttf"),
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
]

_font_path_logged = False


def _resolve_logical_size(configured_size, display_size=None, fullscreen=False):
    """Choose the canvas size for the current display.

    The saved/default logical size remains the portable 640x480 layout.
    In fullscreen on known handheld panels, use the physical panel size
    as the logical canvas so 720x480 devices do not get pillarboxed.
    """
    if (
        fullscreen
        and configured_size == DEFAULT_LOGICAL_SIZE
        and display_size in NATIVE_FULLSCREEN_SIZES
    ):
        return display_size
    return configured_size


def resolve_font_path():
    """First existing font candidate, or None for the builtin fallback."""
    for path in FONT_CANDIDATES:
        if path and os.path.exists(path):
            return path
    return None


def _load_font(size):
    global _font_path_logged
    path = resolve_font_path()
    if path:
        try:
            return pygame.font.Font(path, size)
        except pygame.error as exc:
            log.error("could not load font %s: %s", path, exc)
    if not _font_path_logged:
        _font_path_logged = True
        log.warning(
            "no bundled/system font found — using pygame builtin "
            "(Spanish/Japanese glyphs may render poorly)"
        )
    return pygame.font.Font(None, size + 6)  # pygame default runs small


class QuitApp(Exception):
    """Raised to unwind any screen loop and exit cleanly."""


class DisplayInitError(Exception):
    """Display could not be initialised; main() shows/logs the error."""


class FontSupportError(Exception):
    """This pygame build has no usable font module (SDL_ttf missing)."""


def ensure_font_support():
    """Fail fast, with an actionable message, on font-less pygame builds.

    pygame.font is optional: a from-source build without SDL_ttf (the
    usual outcome of `pip install pygame` on a Python version that has
    no prebuilt wheel, e.g. classic pygame on 3.13+) leaves pygame.font
    as a stub that raises on first use. A text UI cannot limp along
    without fonts, so turn that into a clear diagnosis up front.
    """
    try:
        pygame.font.init()
        pygame.font.Font(None, 12)  # exercises the real module
    except Exception as exc:
        raise FontSupportError(
            "This pygame build has no font support (SDL_ttf was missing "
            "when it was compiled). Install pygame-ce "
            "(`pip uninstall pygame; pip install pygame-ce`) or use a "
            f"Python version with prebuilt wheels. Underlying error: {exc}"
        ) from exc


class CardBrickApp:
    def __init__(
        self,
        storage,
        service,
        audio,
        settings,
        paths=None,
        fullscreen=None,
        initial_state=None,
    ):
        self.storage = storage
        self.service = service
        self.audio = audio
        self.settings = settings
        self.paths = paths
        self.initial_state = initial_state

        self.input_map = InputMap(paths.input_map_path if paths else None)
        self._image_cache = {}  # vocab card images, decoded once per file
        self._session_deck_filter = None  # child's per-sitting deck pick
        self._session_category_filter = None  # ...and per-sitting tag pick
        self._session_bonus = False  # next sprint ignores the daily goal
        self._deck_select_handoff_roll = None
        self._category_select_handoff_roll = None
        self._child_start_handoff_roll = None
        self._review_printer_handoff_roll = None
        self._start_printer_handoff_roll = None
        self._sprint_label = ""  # "Sprint 3/8" shown in the review header
        self._ticket_printed = False  # child-start prints once per run
        self._calendar_return = "CHILD_START"  # where the stamp calendar exits to
        self.input = InputTranslator(self.input_map)
        self._held_inputs = {}  # input source -> semantic currently held

        pygame.init()
        log_pygame_versions()
        ensure_font_support()
        self.w = int(settings.get("logical_width", DEFAULT_LOGICAL_SIZE[0]))
        self.h = int(settings.get("logical_height", DEFAULT_LOGICAL_SIZE[1]))
        if fullscreen is None:
            fullscreen = bool(settings.get("fullscreen"))
        self.fullscreen = fullscreen
        self._init_display()
        pygame.display.set_caption("CardBrick — Spanish Practice")
        pygame.mouse.set_visible(False)
        # In pygame 2 the SDL device closes again when a Joystick object
        # is garbage collected, and SDL only delivers button/hat events
        # for open devices — these references are what keeps input alive.
        self._joysticks = {}
        for i in range(pygame.joystick.get_count()):
            self._open_joystick(i)

        self.font_big = _load_font(38)
        self.font = _load_font(26)
        self.font_small = _load_font(18)
        self.clock = pygame.time.Clock()

        log_display_diagnostics(
            self.display.get_size(),
            (self.w, self.h),
            self.fullscreen,
            resolve_font_path(),
        )

        # Crash recovery: close sessions that never got an end stamp.
        recovered = self.storage.close_dangling_sessions(iso(self.service.now()))
        if recovered:
            log.info(
                "recovered %d interrupted session(s): %s", len(recovered), recovered
            )
        self.profile = self._boot_profile()

    # -- display / scaling -------------------------------------------------------

    def _init_display(self):
        """Logical canvas + real display; scaling computed once."""
        configured_size = (self.w, self.h)
        try:
            if self.fullscreen:
                self.display = pygame.display.set_mode((0, 0), pygame.FULLSCREEN)
            else:
                self.display = pygame.display.set_mode((self.w, self.h))
        except pygame.error as exc:
            log.error(
                "display init failed: %s (SDL_VIDEODRIVER=%s)",
                exc,
                os.environ.get("SDL_VIDEODRIVER", "auto"),
            )
            raise DisplayInitError(str(exc)) from exc

        dw, dh = self.display.get_size()
        if (dw, dh) == (0, 0):  # dummy driver corner case
            self.display = pygame.display.set_mode((self.w, self.h))
            dw, dh = self.w, self.h

        logical_size = _resolve_logical_size(
            configured_size, display_size=(dw, dh), fullscreen=self.fullscreen
        )
        if logical_size != configured_size:
            log.info(
                "logical size: %sx%s -> %sx%s for native display %sx%s",
                configured_size[0],
                configured_size[1],
                logical_size[0],
                logical_size[1],
                dw,
                dh,
            )
            self.w, self.h = logical_size
        if dw < self.w or dh < self.h:
            log.warning(
                "display %s smaller than logical %s",
                self.display.get_size(),
                (self.w, self.h),
            )

        self.canvas = pygame.Surface((self.w, self.h))
        self.screen = self.canvas  # all drawing targets the canvas
        if (dw, dh) == (self.w, self.h):
            self._scaled = None  # fast path: blit 1:1
            log.info("scaling: none (display matches logical size)")
        else:
            if self.settings.get("integer_scaling", True):
                factor = max(min(dw // self.w, dh // self.h), 1)
                tw, th = self.w * factor, self.h * factor
                log.info("scaling: integer x%d -> %dx%d", factor, tw, th)
            else:
                ratio = min(dw / self.w, dh / self.h)
                tw, th = int(self.w * ratio), int(self.h * ratio)
                log.info("scaling: aspect-fit -> %dx%d", tw, th)
            self._scaled = pygame.Surface((tw, th))
            self._scale_offset = ((dw - tw) // 2, (dh - th) // 2)
            self.display.fill((0, 0, 0))  # letterbox borders

    def present(self):
        """Blit the logical canvas to the display and flip."""
        if self._scaled is None:
            self.display.blit(self.canvas, (0, 0))
        else:
            pygame.transform.scale(self.canvas, self._scaled.get_size(), self._scaled)
            self.display.blit(self._scaled, self._scale_offset)
        pygame.display.flip()

    # -- boot ------------------------------------------------------------------------

    def _boot_profile(self):
        profile_id = self.settings.get("current_child_profile_id")
        profile = self.storage.get_profile(profile_id) if profile_id else None
        if profile is None:
            profile = self.storage.ensure_default_profile()
            self.settings.set("current_child_profile_id", profile["id"])
            log.info("using default profile %r (id=%s)", profile["name"], profile["id"])
        if profile["active_categories"] == []:
            log.warning(
                "profile %r has an EMPTY category list — the "
                "child will see no cards until a parent fixes it",
                profile["name"],
            )
        return profile

    def _reload_profile(self):
        self.profile = self.storage.get_profile(self.profile["id"])

    def _card_count(self):
        return self.storage.conn.execute("SELECT COUNT(*) AS n FROM cards").fetchone()[
            "n"
        ]

    # -- main state machine ------------------------------------------------------

    def run(self):
        if self.initial_state:
            state = self.initial_state
        elif self.input_map.source == "defaults" and pygame.joystick.get_count() > 0:
            # A joystick is present but never calibrated: the built-in
            # DEFAULT_BUTTONS guess is only a starting point and is
            # frequently wrong for a given device's raw SDL button
            # order. Landing in Parent Menu with a wrong guess strands
            # the child/parent on a screen that doesn't respond to any
            # button — calibration has to come first so face-button
            # (etc.) actually means something. Keyboard-only setups
            # (desktop testing) skip this: KEYBOARD_SEMANTICS is fixed
            # and never needs calibration.
            log.warning(
                "joystick present with no saved mapping — starting in controller setup"
            )
            state = "INPUT_DIAG"
        elif self._card_count() == 0:
            # Nothing imported yet: route straight to setup/parent mode.
            log.warning("no cards in database — starting in parent mode")
            state = "PARENT_MENU"
        else:
            state = "CHILD_START"
        handlers = {
            "CHILD_START": self.screen_child_start,
            "DECK_SELECT": self.screen_deck_select,
            "CATEGORY_SELECT": self.screen_category_select,
            "REVIEW_PRINTER_HANDOFF": self.screen_review_printer_handoff,
            "REVIEW": self.screen_review,
            "SUMMARY": self.screen_summary,
            "START_PRINTER_HANDOFF": self.screen_start_printer_handoff,
            "CALENDAR": self.screen_calendar,
            "PARENT_MENU": self.screen_parent_menu,
            "PARENT_IMPORT": self.screen_parent_import,
            "PARENT_CATEGORIES": self.screen_parent_categories,
            "PARENT_DECKS": self.screen_parent_decks,
            "PARENT_LIMITS": self.screen_parent_limits,
            "PARENT_AUDIO": self.screen_parent_audio,
            "PARENT_SUSPENDED": self.screen_parent_suspended,
            "PARENT_PROGRESS": self.screen_parent_progress,
            "INPUT_DIAG": self.screen_input_diagnostic,
            "CALIBRATE": self.screen_calibrate,
        }
        try:
            while state != "QUIT":
                state = handlers[state]()
        except QuitApp:
            log.info("exit requested")
        finally:
            pygame.quit()
            log.info("shut down cleanly")

    # -- input ---------------------------------------------------------------------

    def _open_joystick(self, device_index):
        """Open a joystick and retain the reference (see __init__)."""
        try:
            joy = pygame.joystick.Joystick(device_index)
        except pygame.error as exc:
            log.error("could not open joystick %d: %s", device_index, exc)
            return None
        self._joysticks[joy.get_instance_id()] = joy
        return joy

    def poll(self):
        """Next semantic action, or None.

        Consumes exactly one meaningful event per call so rapid inputs
        queued between frames are never dropped. Also services joystick
        hot-plug.
        """
        while True:
            event = pygame.event.poll()
            if event.type == pygame.NOEVENT:
                return None
            if event.type == pygame.QUIT:
                raise QuitApp
            if event.type == pygame.JOYDEVICEADDED:
                joy = self._open_joystick(event.device_index)
                if joy:
                    log.info("joystick connected: %r", joy.get_name())
                continue
            if event.type == pygame.JOYDEVICEREMOVED:
                self._joysticks.pop(event.instance_id, None)
                log.warning("joystick disconnected")
                continue
            self._track_held_input(event)
            action = self.input.translate(event)
            if action:
                if action == "select":
                    return "start"
                if action == "start":
                    return "select"
                return action

    def _track_held_input(self, event):
        """Remember held semantic inputs so review can detect a long
        Up press even when SDL emits only a press event and a later
        release/centre event."""
        if event.type == pygame.KEYDOWN:
            semantic = self._semantic_for_key(event.key)
            if semantic:
                self._held_inputs[("key", event.key)] = semantic
        elif event.type == pygame.KEYUP:
            self._held_inputs.pop(("key", event.key), None)
        elif event.type == pygame.JOYBUTTONDOWN:
            semantic = self.input_map.semantic_for_button(event.button)
            if semantic:
                source = ("button", getattr(event, "instance_id", 0), event.button)
                self._held_inputs[source] = semantic
        elif event.type == pygame.JOYBUTTONUP:
            source = ("button", getattr(event, "instance_id", 0), event.button)
            self._held_inputs.pop(source, None)
        elif event.type == pygame.JOYHATMOTION:
            source = ("hat", getattr(event, "instance_id", 0), event.hat)
            self._held_inputs.pop(source, None)
            semantic = self._semantic_for_hat(event.value)
            if semantic:
                self._held_inputs[source] = semantic
        elif event.type == pygame.JOYAXISMOTION and event.axis <= 1:
            source = ("axis", getattr(event, "instance_id", 0), event.axis)
            self._held_inputs.pop(source, None)
            semantic = self._semantic_for_axis(event.axis, event.value)
            if semantic:
                self._held_inputs[source] = semantic

    def _semantic_held(self, semantic):
        return semantic in self._held_inputs.values()

    @staticmethod
    def _semantic_for_key(key):
        name = pygame.key.name(key)
        return KEYBOARD_SEMANTICS.get(name)

    @staticmethod
    def _semantic_for_hat(value):
        x, y = value
        if y == 1:
            return "dpad_up"
        if y == -1:
            return "dpad_down"
        if x == -1:
            return "dpad_left"
        if x == 1:
            return "dpad_right"
        return None

    @staticmethod
    def _semantic_for_axis(axis, value):
        if axis == 0:
            if value > AXIS_THRESHOLD:
                return "dpad_right"
            if value < -AXIS_THRESHOLD:
                return "dpad_left"
        elif axis == 1:
            if value > AXIS_THRESHOLD:
                return "dpad_down"
            if value < -AXIS_THRESHOLD:
                return "dpad_up"
        return None

    # -- child start -----------------------------------------------------------------

    def _resolve_available_decks(self):
        """Decks the parent has assigned to this profile: every deck if
        active_decks is None, else that explicit (possibly empty) list."""
        active = self.profile["active_decks"]
        return self.storage.deck_names_list() if active is None else list(active)

    def _resolve_available_categories(self):
        """Tags the parent has assigned to this profile: every observed
        tag if active_categories is None, else that explicit (possibly
        empty) list. Same convention as _resolve_available_decks."""
        active = self.profile["active_categories"]
        return self.storage.all_tags() if active is None else list(active)

    def _next_after_deck_choice(self):
        """Where to go once the deck for this sitting is settled: the
        topic/tag picker if there's a real choice to make, else
        straight to Review — mirrors the deck-count threshold."""
        return (
            "CATEGORY_SELECT"
            if len(self._resolve_available_categories()) > 1
            else "REVIEW"
        )

    def screen_child_start(self):
        self._session_deck_filter = None  # cleared each time we land here
        self._session_category_filter = None
        self._session_bonus = False
        status = self.service.sprint_status(profile=self.profile)
        available_decks = self._resolve_available_decks()
        startable = status["next_sprint_cards"] > 0
        bonus = not startable and status["bonus_cards"] > 0

        roll = self._child_start_handoff_roll
        self._child_start_handoff_roll = None
        if roll is None:
            roll = self._build_child_start_roll(status, startable, bonus)
        if self._ticket_printed and not hasattr(roll, "_child_start_view_target"):
            roll.finish()
        self._ticket_printed = True
        if not hasattr(roll, "_child_start_view_target"):
            roll._child_start_view_target = roll._target
        if not hasattr(roll, "_child_start_total"):
            roll._child_start_total = roll._child_start_view_target + roll.print_y

        while True:
            action = self.poll()
            if action and roll.busy:
                roll.finish()
            if action == "start":
                return "QUIT"
            if action == "select":
                return "PARENT_MENU"
            if action == "north_button":
                self._calendar_return = "CHILD_START"
                return "CALENDAR"
            if action in ("east_button", "unmapped") and (startable or bonus):
                self._session_bonus = bonus
                roll._child_start_view_target = roll._view_target
                roll._child_start_total = roll._view_target + roll.print_y
                if len(available_decks) > 1:
                    self._deck_select_handoff_roll = roll
                    return "DECK_SELECT"
                next_state = self._next_after_deck_choice()
                if next_state == "CATEGORY_SELECT":
                    self._category_select_handoff_roll = roll
                return next_state

            roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            self._footer(
                "A = Start    X = Calendar"
                if (startable or bonus)
                else "X = Calendar",
                "START = Parent    SELECT = Quit",
            )
            self.present()
            self.clock.tick(FPS)

    def _build_child_start_roll(self, status, startable, bonus):
        # The day's job ticket, printed onto the roll once on first
        # entry this run; later visits snap instantly so navigation
        # never animates in the child's way.
        roll = self._new_roll(print_y=self.h - 100)
        roll.feed_page(perf=False)
        roll.feed_gap(8)
        roll.feed(
            self._stub_surface(
                "cardbrick",
                self.service.now().astimezone().strftime("%a %b %d").upper(),
            ),
            reveal=False,
        )
        roll.feed(self._rule_surface(), reveal=False)
        roll.feed_gap(14)
        if startable:
            # "Last sprint" only makes sense once earlier sprints
            # happened; an untouched day gets "today" phrasing.
            n = status["sprints_remaining"]
            if status["cards_done"] == 0:
                headline = "Just one sprint today!" if n == 1 else f"{n} sprints today"
            else:
                headline = (
                    "Last sprint of the day!" if n == 1 else f"{n} sprints to go today"
                )
            roll.feed(self.font_big.render(headline, True, FG))
            roll.feed_gap(8)
            progress = f"{status['cards_done']} / {status['goal_today']} cards done"
            if status["goal_today"] < status["goal"]:
                # Supply-limited (e.g. a fresh deck paced by
                # daily_new_cards): say why the day is short.
                progress += " — more unlock tomorrow"
            roll.feed(self.font.render(progress, True, DIM))
            roll.feed_gap(4)
            minutes = self.profile["session_time_minutes"]
            sprint_line = f"next sprint: {status['next_sprint_cards']} cards" + (
                f" / about {minutes} min" if minutes else ""
            )
            roll.feed(self.font_small.render(sprint_line, True, DIM))
            roll.feed_gap(18)
            roll.feed(self.font.render("Press the A button to start!", True, GOOD))
        elif bonus:
            headline = (
                "Goal reached! Great job!"
                if status["goal_met"]
                else "That's everything for today!"
            )
            roll.feed(self.font_big.render(headline, True, GOOD))
            roll.feed_gap(8)
            roll.feed(
                self.font.render(f"{status['cards_done']} cards done today", True, DIM)
            )
            roll.feed_gap(4)
            roll.feed(
                self.font_small.render(
                    f"Spare time? A bonus sprint of "
                    f"{status['bonus_cards']} cards is ready.",
                    True,
                    DIM,
                )
            )
            roll.feed_gap(18)
            roll.feed(
                self.font.render("A button = bonus sprint (totally optional!)", True, GOOD)
            )
        else:
            roll.feed_gap(10)
            roll.feed(self._stamp_surface("All done for today!", GOOD), reveal=False)
            roll.feed_gap(12)
            roll.feed(self.font.render("Come back tomorrow.", True, DIM))
        return roll

    def screen_deck_select(self):
        """Child-facing deck picker: shown only when the parent has
        assigned more than one deck (screen_child_start skips straight
        to REVIEW otherwise, keeping the fast path fast). Lets the
        child choose one specific assigned deck, or all of them
        combined, for just this sitting."""
        available = self._resolve_available_decks()
        entries = [("All assigned decks", None)] + [
            (name, [name]) for name in available
        ]
        # Due/new counts computed once up front, not per frame — this
        # screen redraws every tick like the other menus, and querying
        # the DB per entry per frame would not be free on a big deck.
        counts = [
            self.service.counts_for_queue(
                profile=self.profile, deck_filter=decks, bonus=self._session_bonus
            )
            for _label, decks in entries
        ]
        index = 0
        roll = self._deck_select_handoff_roll
        self._deck_select_handoff_roll = None
        setup_roll = roll
        if roll is None:
            roll = self._new_roll()
            setup_roll = None
        rows = self._print_choice_page(
            roll,
            "Choose a Deck",
            "Which deck do you want to study?",
            entries,
            counts,
        )
        if setup_roll is None:
            roll.finish()
            self._scroll_choice_to_index(roll, rows, index)
        choice_aligned = setup_roll is None

        while True:
            action = self.poll()
            if action and roll.printing_busy:
                roll.finish()
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            if action in ("start", "south_button", "select"):
                return self._return_to_child_start_with_roll(roll)
            if action == "dpad_up":
                index = (index - 1) % len(entries)
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            elif action == "east_button":
                self._session_deck_filter = entries[index][1]
                next_state = self._next_after_deck_choice()
                if next_state == "CATEGORY_SELECT" and setup_roll is not None:
                    roll.scroll_to_print_position()
                    self._category_select_handoff_roll = roll
                return next_state

            if not roll.printing_busy and not choice_aligned:
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            if roll.busy:
                roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            if not roll.printing_busy:
                self._draw_choice_marker(roll, rows, index)
            self._footer("Up/Down = Choose   A = Select   B = Back")
            self.present()
            self.clock.tick(FPS)

    def screen_category_select(self):
        """Child-facing topic/tag picker: shown only when the parent
        has assigned more than one category (screen_child_start and
        DeckSelect both skip straight to REVIEW otherwise). Lets the
        child focus on one specific assigned tag for just this sitting
        — the same non-destructive filtering as Anki's filtered decks:
        no card's tags or deck ever change, so finishing (or picking
        "All assigned categories" next time) simply returns to
        studying everything in scope."""
        available = self._resolve_available_categories()
        entries = [("All assigned categories", None)] + [
            (name, [name]) for name in available
        ]
        counts = [
            self.service.counts_for_queue(
                profile=self.profile,
                deck_filter=self._session_deck_filter,
                category_filter=cats,
                bonus=self._session_bonus,
            )
            for _label, cats in entries
        ]
        index = 0
        roll = self._category_select_handoff_roll
        self._category_select_handoff_roll = None
        setup_roll = roll
        if roll is None:
            roll = self._new_roll()
            setup_roll = None
        rows = self._print_choice_page(
            roll,
            "Choose a Topic",
            "Want to focus on something specific?",
            entries,
            counts,
        )
        if setup_roll is None:
            roll.finish()
            self._scroll_choice_to_index(roll, rows, index)
        choice_aligned = setup_roll is None

        while True:
            action = self.poll()
            if action and roll.printing_busy:
                roll.finish()
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            if action in ("start", "south_button", "select"):
                return self._return_to_child_start_with_roll(roll)
            if action == "dpad_up":
                index = (index - 1) % len(entries)
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            elif action == "east_button":
                self._session_category_filter = entries[index][1]
                if setup_roll is not None:
                    roll.scroll_to_print_position()
                    self._print_topic_selected(roll, entries[index][0])
                    self._review_printer_handoff_roll = roll
                    return "REVIEW_PRINTER_HANDOFF"
                return "REVIEW"

            if not roll.printing_busy and not choice_aligned:
                self._scroll_choice_to_index(roll, rows, index)
                choice_aligned = True
            if roll.busy:
                roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            if not roll.printing_busy:
                self._draw_choice_marker(roll, rows, index)
            self._footer("Up/Down = Choose   A = Select   B = Back")
            self.present()
            self.clock.tick(FPS)

    def _print_choice_page(self, roll, title, subtitle, entries, counts):
        """Print a full picker page and return row coordinates for the
        moving selection mark. The choices live on the roll, so Up/Down
        scrolls through printed paper instead of redrawing a window."""
        cursor = roll._total + sum(item.h for item in roll._queue)

        def feed_gap(h):
            nonlocal cursor
            roll.feed_gap(h)
            cursor += h

        def feed(surf, x=None, reveal=True):
            nonlocal cursor
            roll.feed(surf, x=x, reveal=reveal)
            cursor += surf.get_height()

        has_previous_page = roll.page_count > 0
        roll.feed_page(perf=has_previous_page)
        if has_previous_page:
            cursor += PAGE_GAP + PERF_H + PAGE_GAP
        feed_gap(10)
        feed(self.font_big.render(title, True, FG))
        feed_gap(6)
        feed(self.font_small.render(subtitle, True, DIM))
        feed_gap(8)
        feed(self._rule_surface(), reveal=False)
        feed_gap(10)
        rows = []
        for i, (label, _filter) in enumerate(entries):
            due, new = counts[i]
            text = self._truncate_to_width(
                self.font, f"{label}   ({due + new} due)", self.w - 160
            )
            surf = self.font.render(text, True, FG)
            rows.append(
                {
                    "coord": cursor,
                    "x": 80,
                    "width": surf.get_width(),
                }
            )
            feed(surf, x=80)
            feed_gap(10)
        return rows

    def _scroll_choice_to_index(self, roll, rows, index):
        if not rows:
            return
        focus_y = min(self.h - 150, 205)
        wanted_offset = rows[index]["coord"] - focus_y
        roll.scroll(wanted_offset - roll._view_target)

    def _draw_choice_marker(self, roll, rows, index):
        if not rows:
            return
        row = rows[index]
        y = int(row["coord"] - roll.offset)
        if y < -self.font.get_linesize() or y > self.h - 62:
            return
        x = row["x"]
        marker = self.font.render("▶", True, ACCENT)
        self.screen.blit(marker, (x - marker.get_width() - 12, y))
        base = y + self.font.get_ascent() + 3
        pygame.draw.line(self.screen, FG, (x, base), (x + row["width"], base), 2)

    def _return_to_child_start_with_roll(self, roll):
        target = getattr(roll, "_child_start_view_target", None)
        if target is None:
            return "CHILD_START"

        if roll.printing_busy:
            roll.finish()
        roll.scroll(target - roll._view_target)
        while roll.busy:
            if self.poll():
                roll._view_target = target
                roll.offset = target
                break
            roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            self._footer("Returning to the start ticket...")
            self.present()
            self.clock.tick(FPS)

        roll._view_target = target
        roll.offset = target
        self._trim_roll_to_child_start(roll)
        self._child_start_handoff_roll = roll
        return "CHILD_START"

    def _trim_roll_to_child_start(self, roll):
        total = getattr(roll, "_child_start_total", None)
        if total is None:
            return

        y = roll._base
        keep = 0
        for item in roll._items:
            bottom = y + item.h
            if bottom > total + 0.5:
                break
            keep += 1
            y = bottom
        del roll._items[keep:]
        roll._queue.clear()
        roll._total = total
        roll._target = total - roll.print_y
        roll._view_target = roll._target
        roll.offset = roll._target
        roll._active_reveal = None
        roll._pages = [
            (coord, idx)
            for coord, idx in roll._pages
            if coord <= total + 0.5 and idx <= len(roll._items)
        ]

    def _print_topic_selected(self, roll, label):
        roll.feed_gap(10)
        roll.feed(self._rule_surface(), reveal=False)
        roll.feed_gap(10)
        text = self._truncate_to_width(
            self.font_small, f"TOPIC SELECTED: {label}", self.w - 170
        )
        roll.feed(self.font_small.render(text, True, GOOD), x=80)

    def _draw_category_select(self, entries, counts, index, top, visible):
        self._paper()
        self._page_header("Choose a Topic", "Want to focus on something specific?")
        y = 140
        for i in range(top, min(top + visible, len(entries))):
            label = entries[i][0]
            due, new = counts[i]
            text = self._truncate_to_width(
                self.font, f"{label}   ({due + new} due)", self.w - 160
            )
            self._menu_row(text, 80, y, i == index)
            y += 44

    # -- review ----------------------------------------------------------------------

    MENU_ENTRIES = (
        "Undo last answer",
        "Bury card (back tomorrow)",
        "Suspend card (parent will check)",
        "End session",
        "Cancel",
    )

    # Four-phase vocab cards (word -> example -> image -> definition):
    # the phase reached when "I know this" is pressed *is* the rating.
    # No separate Again/Hard/Good/Easy buttons for these cards.
    VOCAB_MAX_PHASE = 3
    VOCAB_PHASE_RATING = {0: 4, 1: 3, 2: 2, 3: 1}  # Easy, Good, Hard, Again
    def screen_review_printer_handoff(self):
        setup_roll = self._review_printer_handoff_roll
        self._review_printer_handoff_roll = None
        if setup_roll is None or self.settings.get("reduced_motion"):
            return "REVIEW"

        review_roll = self._new_roll()

        while setup_roll.busy:
            action = self.poll()
            if action:
                setup_roll.finish()
            else:
                setup_roll.update()
            self._paper(setup_roll)
            self._draw_roll(setup_roll)
            self._footer("Topic selected", "Looking over to the card printer...")
            self.present()
            self.clock.tick(FPS)

        frames = max(1, int(FPS * 0.32))
        for frame in range(frames + 1):
            if self.poll():
                return "REVIEW"
            t = frame / frames
            eased = t * t * (3 - 2 * t)
            old_x = -int(self.w * eased)
            new_x = self.w - int(self.w * eased)
            self.screen.fill(BG)
            self._draw_printer_view_at(setup_roll, old_x)
            self._draw_printer_view_at(review_roll, new_x)
            self._footer("Topic selected", "Card printer ready")
            self.present()
            self.clock.tick(FPS)
        return "REVIEW"

    def screen_review(self):
        if self._session_bonus:
            self._sprint_label = "Bonus sprint"
        else:
            status = self.service.sprint_status(profile=self.profile)
            number = min(
                status["sprints_planned"] - status["sprints_remaining"] + 1,
                status["sprints_planned"],
            )
            self._sprint_label = f"Sprint {number}/{status['sprints_planned']}"
        session = StudySession(
            self.storage,
            self.service,
            self.profile,
            deck_filter=self._session_deck_filter,
            category_filter=self._session_category_filter,
            bonus=self._session_bonus,
        )
        log.info(
            "session %d started: %d cards queued (%s, deck filter: "
            "%s, category filter: %s)",
            session.session_id,
            session.planned_total,
            self._sprint_label,
            self._session_deck_filter,
            self._session_category_filter,
        )
        try:
            return self._review_loop(session)
        finally:
            if not session.finished:
                session.finish()  # force exit / window close: still stamped
            log.info(
                "session %d finished: %s",
                session.session_id,
                {k: v for k, v in session.summary().items() if not k.startswith("avg")},
            )

    def _review_loop(self, session):
        reversed_mode = self.profile.get("study_direction") == "reversed"
        auto_play = bool(self.settings.get("auto_play_audio", True))

        flipped = False
        phase = 0  # vocab cards only: 0..VOCAB_MAX_PHASE
        vocab_detail = None  # vocab cards only: the vocab_cards row
        known_confirmation = None  # vocab only: pending "I know this" confirmation
        shown_at = None
        audio_status = None
        menu = None  # action-menu overlay index, or None when closed
        needs_draw = True
        heartbeat = 0
        tear_next_page = False
        up_hold_started = None
        up_hold_undo_fired = False
        UP_UNDO_HOLD_SECONDS = 0.65

        # The card is *printed* onto the paper roll: content is fed as
        # line feeds, each new card starts with a page feed past a
        # perforation, and undo rewinds the spool. Items are rendered
        # once when printed, not every frame.
        roll = self._new_roll()
        printed_phase = 0  # vocab: highest phase already on the paper
        max_width = self.content_x1 - self.content_x0 - 12
        scroll_step = 58

        def can_scroll_card():
            if menu is not None or roll.printing_busy:
                return False
            if vocab_detail is not None:
                return known_confirmation is not None or phase >= self.VOCAB_MAX_PHASE
            return flipped

        def scroll_card(action):
            if action not in ("dpad_up", "dpad_down") or not can_scroll_card():
                return False
            roll.scroll(-scroll_step if action == "dpad_up" else scroll_step)
            return True

        def vocab_image_surface():
            if vocab_detail is None:
                return None
            return self._vocab_image_surface(vocab_detail["image_filename"])

        def next_vocab_phase(current_phase):
            next_phase = min(current_phase + 1, self.VOCAB_MAX_PHASE)
            if next_phase == 2 and vocab_image_surface() is None:
                next_phase = 3
            return next_phase

        def prepare_for_next_print():
            roll.scroll_to_print_position()

        def print_block(text, font, color):
            for line in wrap_text(font, text, max_width):
                roll.feed(font.render(line, True, color))

        def print_stub():
            """Receipt stub at the top of each card's page."""
            left = card["deck"]
            if card["tags"]:
                left += "  ·  " + " ".join(card["tags"].split()[:3])
            right = (
                f"{self._sprint_label}  ·  {session.remaining()} left"
                if self._sprint_label
                else f"{session.remaining()} left"
            )
            roll.feed_gap(8)
            roll.feed(self._stub_surface(left, right), reveal=False)
            roll.feed(self._rule_surface(), reveal=False)
            roll.feed_gap(10)

        def print_audio_hint():
            if audio_status == "missing":
                roll.feed(self.font_small.render("(no audio)", True, WARN))
            elif card["audio_filename"]:
                roll.feed(self.font_small.render("♪  L1 = replay", True, DIM))

        def print_front():
            if vocab_detail is not None:
                print_block(vocab_detail["word"], self.font_big, FG)
            else:
                print_block(
                    card["back"] if reversed_mode else card["front"],
                    self.font_big,
                    FG,
                )
            print_audio_hint()

        def print_back():
            roll.feed_gap(14)
            roll.feed(self.font_small.render("—  RESPUESTA  —", True, ACCENT))
            roll.feed_gap(8)
            print_block(card["front"] if reversed_mode else card["back"], self.font, FG)

        def print_vocab_phase(p):
            """Phase 1: +example (headword highlighted). 2: +image.
            3: +gendered forms/definitions/translation. Earlier phases
            stay on the paper — printing is additive by nature."""
            if p == 1:
                roll.feed_gap(10)
                roll.feed(self._rule_surface(), reveal=False)
                roll.feed_gap(8)
                text = vocab_detail["example_es"] or "(no example sentence)"
                for line in wrap_text(self.font, text, max_width):
                    roll.feed(
                        self._render_highlight_line(line, vocab_detail["word"], self.font)
                    )
            elif p == 2:
                image = vocab_image_surface()
                if image is not None:
                    roll.feed_gap(8)
                    roll.feed(image, reveal=False)
            elif p == 3:
                roll.feed_gap(10)
                roll.feed(self._rule_surface(), reveal=False)
                roll.feed_gap(8)
                lines = []
                if vocab_detail["gendered_forms"]:
                    lines.append(vocab_detail["gendered_forms"])
                lines.append(vocab_detail["definitions"] or "(no definition)")
                if vocab_detail["word_en"]:
                    lines.append("EN: " + vocab_detail["word_en"])
                if vocab_detail["word_jp"]:
                    lines.append("JP: " + vocab_detail["word_jp"])
                if vocab_detail["example_en"]:
                    lines.append("Example EN: " + vocab_detail["example_en"])
                if vocab_detail["example_jp"]:
                    lines.append("Example JP: " + vocab_detail["example_jp"])
                if vocab_detail["report_link"]:
                    lines.append("Report: " + vocab_detail["report_link"])
                print_block("\n".join(lines), self.font_small, FG)

        def print_up_to(target_phase):
            nonlocal printed_phase
            while printed_phase < target_phase:
                printed_phase += 1
                print_vocab_phase(printed_phase)

        RATING_STAMPS = {
            1: ("AGAIN", ACCENT),
            2: ("HARD", WARN),
            3: ("GOOD", GOOD),
            4: ("EASY", GOOD),
        }

        def print_stamp(rating):
            """Ink-stamp the rating onto the card before the page feed
            carries it away."""
            text, color = RATING_STAMPS[rating]
            roll.feed_gap(12)
            roll.feed(self._stamp_surface(text, color), reveal=False)

        def print_small_stamp(text):
            roll.feed_gap(10)
            roll.feed(self._stamp_surface(text, DIM, self.font_small), reveal=False)

        def play_vocab():
            """Replay whatever audio belongs to the current phase: the
            word at phase 0, the example sentence from phase 1 on."""
            nonlocal audio_status
            filename = (
                card["audio_filename"]
                if phase == 0
                else (vocab_detail["example_audio"] if vocab_detail else None)
            )
            if not filename:
                return
            audio_status = "playing" if self.audio.play(filename) else "missing"

        def audio_for(card, side):
            """Audio filename if the card's audio belongs on this side
            of the *displayed* card (side: 'front'|'back')."""
            if not card["audio_filename"]:
                return None
            actual = card["audio_side"]
            if reversed_mode and actual in ("front", "back"):
                actual = "back" if actual == "front" else "front"
            return card["audio_filename"] if actual == side else None

        def play(card, side, forced=False):
            nonlocal audio_status
            filename = audio_for(card, side) or (
                card["audio_filename"] if forced else None
            )
            if not filename:
                return
            audio_status = "playing" if self.audio.play(filename) else "missing"

        def begin_card(card, new_page=True):
            nonlocal flipped, phase, vocab_detail, known_confirmation
            nonlocal shown_at, audio_status, printed_phase, tear_next_page
            flipped = False
            phase = 0
            printed_phase = 0
            known_confirmation = None
            vocab_detail = (
                self.storage.get_vocab_detail(card["id"])
                if card["card_type"] == "vocab"
                else None
            )
            shown_at = self.service.now()
            audio_status = None
            if card["audio_filename"] and not self.audio.available(
                card["audio_filename"]
            ):
                audio_status = "missing"
            if auto_play:
                play_vocab() if vocab_detail is not None else play(card, "front")
            # Print the card. new_page=False after an undo rewind: the
            # spool has already rolled back to this page's blank start.
            # The very first page on the roll starts without a tear.
            tear = tear_next_page
            tear_next_page = False
            roll.feed_page(perf=new_page and roll.page_count > 0, tear=tear)
            print_stub()
            print_front()

        def confirm_known(mistake=False):
            rating = (
                self.VOCAB_PHASE_RATING[known_confirmation["phase"]]
                if not mistake
                else 1
            )
            prepare_for_next_print()
            print_stamp(rating)
            session.answer(rating, elapsed_ms=known_confirmation["elapsed_ms"])
            advance()

        def advance():
            """Fetch the next card after the current one was consumed.

            The optional time limit is checked here, between cards, so
            a child is never cut off while thinking about an answer.
            """
            nonlocal card
            self.audio.stop()
            card = None if session.time_limit_reached() else session.current_card()
            if card:
                begin_card(card)

        def discard_current(stamp_text, suspend=False):
            nonlocal tear_next_page
            if suspend:
                session.suspend_current()
            else:
                session.bury_current()
            prepare_for_next_print()
            print_small_stamp(stamp_text)
            tear_next_page = True
            advance()

        def undo_last_answer():
            nonlocal card
            restored = session.undo()
            if restored is None:
                return False
            # Reverse feed: the spool visibly rolls back past the
            # perforation, then the restored card prints again where it
            # used to be.
            rewound = roll.rewind_page()
            card = restored
            begin_card(card, new_page=not rewound)
            return True

        def before_flip_controls_active():
            return (
                menu is None
                and card is not None
                and not roll.printing_busy
                and (
                    (vocab_detail is None and not flipped)
                    or (
                        vocab_detail is not None
                        and known_confirmation is None
                        and phase < self.VOCAB_MAX_PHASE
                    )
                )
            )

        def end_session():
            self.audio.stop()
            session.finish()
            self._summary = session.summary()
            return "SUMMARY"

        card = session.current_card()
        if card:
            begin_card(card)

        while True:
            if card is None:
                return end_session()

            action = self.poll()
            if action:
                needs_draw = True
                if roll.printing_busy:
                    # Golden rule: never make the child wait. Any input
                    # snaps the feed to its end, then applies normally.
                    roll.finish()
            up_is_held = self._semantic_held("dpad_up")
            if not up_is_held:
                up_hold_started = None
                up_hold_undo_fired = False
            elif before_flip_controls_active():
                if up_hold_started is None:
                    up_hold_started = time.monotonic()
                    up_hold_undo_fired = False
                elif (
                    not up_hold_undo_fired
                    and time.monotonic() - up_hold_started >= UP_UNDO_HOLD_SECONDS
                ):
                    if undo_last_answer():
                        needs_draw = True
                    up_hold_undo_fired = True
            else:
                # If this hold already fired, keep it spent until the
                # user releases Up. Otherwise start timing only once the
                # pre-flip controls are active.
                if not up_hold_undo_fired:
                    up_hold_started = None
            if menu is not None:
                if action == "dpad_up":
                    menu = (menu - 1) % len(self.MENU_ENTRIES)
                elif action == "dpad_down":
                    menu = (menu + 1) % len(self.MENU_ENTRIES)
                elif action in ("south_button", "select", "start"):
                    menu = None
                elif action == "east_button":
                    choice, menu = self.MENU_ENTRIES[menu], None
                    if choice.startswith("Undo"):
                        undo_last_answer()
                    elif choice.startswith("Bury"):
                        discard_current("BURIED")
                    elif choice.startswith("Suspend"):
                        discard_current("SUSPENDED", suspend=True)
                    elif choice == "End session":
                        return end_session()
            elif action == "start":
                return end_session()
            elif action == "select":
                menu = 0
            elif action == "l1":
                if vocab_detail is not None:
                    play_vocab()
                else:
                    play(card, "back" if flipped else "front", forced=True)
            elif action == "undo":
                undo_last_answer()
            elif vocab_detail is not None:
                if scroll_card(action):
                    continue
                if known_confirmation is not None:
                    if action == "east_button":
                        confirm_known()
                        continue
                    if action == "south_button":
                        confirm_known(mistake=True)
                        continue
                    if action == "r1":
                        discard_current("BURIED")
                        continue
                elif action == "dpad_down":
                    if phase < self.VOCAB_MAX_PHASE:
                        phase = next_vocab_phase(phase)
                        print_up_to(phase)
                        if auto_play:
                            play_vocab()
                elif action == "east_button":
                    elapsed = int(
                        (self.service.now() - shown_at).total_seconds() * 1000
                    )
                    if phase >= self.VOCAB_MAX_PHASE:
                        prepare_for_next_print()
                        print_stamp(self.VOCAB_PHASE_RATING[phase])
                        session.answer(self.VOCAB_PHASE_RATING[phase], elapsed_ms=elapsed)
                        advance()
                        continue
                    known_confirmation = {
                        "phase": phase,
                        "elapsed_ms": elapsed,
                    }
                    phase = self.VOCAB_MAX_PHASE
                    print_up_to(phase)
                    if auto_play:
                        play_vocab()
                elif action == "r1":
                    discard_current("BURIED")
                    continue
            elif not flipped:
                if action == "dpad_down" or action in ("east_button", "unmapped"):
                    flipped = True
                    print_back()
                    if auto_play:
                        play(card, "back")
            elif scroll_card(action):
                continue
            elif action in RATING_FOR_SEMANTIC:
                elapsed = int((self.service.now() - shown_at).total_seconds() * 1000)
                prepare_for_next_print()
                print_stamp(RATING_FOR_SEMANTIC[action])
                session.answer(RATING_FOR_SEMANTIC[action], elapsed_ms=elapsed)
                advance()
                continue
            elif action == "r1":
                discard_current("BURIED")
                continue

            # The screen is static between inputs: redraw only while
            # the roll is feeding or when something happened (plus a
            # 1 s heartbeat as a safety net).
            if roll.busy:
                roll.update()
                needs_draw = True
            heartbeat += 1
            if needs_draw or heartbeat >= FPS:
                self._draw_review(
                    flipped,
                    menu,
                    phase=phase,
                    vocab=vocab_detail,
                    known_confirmation=known_confirmation,
                    roll=roll,
                )
                needs_draw = False
                heartbeat = 0
            self.clock.tick(FPS)

    def _draw_review(self, flipped, menu, phase, vocab, known_confirmation, roll):
        """One frame of the review screen: the paper roll carries all
        card content (pre-rendered when printed); only the chrome —
        sprocket strips, chassis hints, overlay menu — is drawn here."""
        self._paper(roll)
        self._draw_roll(roll)
        if vocab is not None:
            if known_confirmation is not None:
                self._footer(
                    "A = Yes, I knew it   B = I made a mistake",
                    "D-pad up/down = Scroll   START = Menu",
                )
            elif phase < self.VOCAB_MAX_PHASE:
                self._footer(
                    "Down = Reveal more   Hold Up = Undo   A = I know this",
                    "L1 = Replay audio   R1 = Bury   START = Menu",
                )
            else:
                self._footer(
                    "A = Go to next card",
                    "D-pad=Scroll   L1=Replay   R1=Bury   START=Menu",
                )
        elif flipped:
            self._footer(
                "A=Good  B=Again  Y=Easy  X=Hard",
                "D-pad=Scroll   R1=Bury   START=Menu   SELECT=Finish",
            )
        else:
            self._footer(
                "Down/A = Show answer   Hold Up = Undo",
                "START = Menu   SELECT = Finish",
            )

        if menu is not None:
            self._draw_menu_overlay(menu)
        self.present()

    def _vocab_image_surface(self, filename, max_height=140):
        """Loaded + scaled image surface, cached by filename so it is
        decoded once per card rather than every frame."""
        if not filename:
            return None
        key = (filename, max_height)
        if key in self._image_cache:
            return self._image_cache[key]
        surf = None
        path = self.audio.resolve(filename)
        if path is not None:
            try:
                raw = pygame.image.load(path)
                try:
                    raw = raw.convert_alpha()
                except pygame.error:
                    raw = raw.convert()
                if raw.get_height() > max_height:
                    scale = max_height / raw.get_height()
                    raw = pygame.transform.smoothscale(
                        raw, (max(1, int(raw.get_width() * scale)), max_height)
                    )
                surf = raw
            except pygame.error as exc:
                log.warning("could not load image %s: %s", path, exc)
        else:
            log.warning("missing media file: %s",
                        os.path.join(self.audio.media_dir,
                                     os.path.basename(filename)))
        self._image_cache[key] = surf
        return surf

    def _render_highlight_line(self, line, word, font):
        """One wrapped line as a surface, with the headword rendered in
        ACCENT wherever it occurs (case-insensitively) — a plain-text
        stand-in for the original card's HTML yellow-highlight span."""
        word_lower = (word or "").lower()
        idx = line.lower().find(word_lower) if word_lower else -1
        if idx == -1:
            return font.render(line, True, FG)
        parts = [
            font.render(line[:idx], True, FG),
            font.render(line[idx : idx + len(word_lower)], True, ACCENT),
            font.render(line[idx + len(word_lower) :], True, FG),
        ]
        surf = pygame.Surface(
            (sum(p.get_width() for p in parts), font.get_linesize()), pygame.SRCALPHA
        )
        x = 0
        for p in parts:
            surf.blit(p, (x, 0))
            x += p.get_width()
        return surf

    def _draw_menu_overlay(self, index):
        """The in-review action menu: a paper slip laid on top of the
        roll, with an ink border like a printed form."""
        entries = self.MENU_ENTRIES
        box_w, line_h = 500, 42
        box_h = line_h * len(entries) + 32
        x = (self.w - box_w) // 2
        y = (self.h - box_h) // 2
        pygame.draw.rect(self.screen, OVERLAY_BG, (x, y, box_w, box_h), border_radius=8)
        pygame.draw.rect(self.screen, FG, (x, y, box_w, box_h), 2, border_radius=8)
        for i, entry in enumerate(entries):
            self._menu_row(entry, x + 44, y + 16 + i * line_h, i == index)

    # -- summary ---------------------------------------------------------------------

    def screen_summary(self):
        s = getattr(self, "_summary", None) or {}
        minutes = int(s.get("time_spent_seconds", 0) // 60)
        seconds = int(s.get("time_spent_seconds", 0) % 60)
        pass_rate = s.get("pass_rate")
        avg = s.get("avg_response_ms")

        # Where the day stands now that this sprint is in the log; the
        # deck/category filters are kept so "next sprint now" continues
        # the same deck/topic the child picked for this sitting.
        status = self.service.sprint_status(
            profile=self.profile,
            deck_filter=self._session_deck_filter,
            category_filter=self._session_category_filter,
        )
        more = status["next_sprint_cards"] > 0
        bonus = not more and status["bonus_cards"] > 0

        if more:
            n = status["sprints_remaining"]
            headline = "¡Buen trabajo!"
            subtitle = (
                "SPRINT DONE — LAST ONE TO GO"
                if n == 1
                else f"SPRINT DONE — {n} TO GO TODAY"
            )
        elif status["goal_met"]:
            headline = "Goal reached! Great job!"
            subtitle = f"{status['cards_done']} CARDS TODAY"
        else:
            headline = "That's everything for today!"
            subtitle = f"{status['cards_done']} CARDS TODAY"

        rows = [
            ("Cards answered", str(s.get("cards_reviewed", 0)), FG),
            (
                f"New {s.get('new_cards', 0)}  ·  Reviews {s.get('review_cards', 0)}",
                "",
                FG,
            ),
            (
                f"Again {s.get('again_count', 0)}   "
                f"Hard {s.get('hard_count', 0)}   "
                f"Good {s.get('good_count', 0)}   "
                f"Easy {s.get('easy_count', 0)}",
                "",
                DIM,
            ),
            (
                "Pass rate",
                f"{pass_rate:.0%}" if pass_rate is not None else "—",
                DIM,
            ),
            (
                "Time",
                f"{minutes}m {seconds:02d}s"
                + (f"  ·  avg {avg / 1000:.1f}s" if avg else ""),
                DIM,
            ),
            ("Today", f"{status['cards_done']} / {status['goal_today']} cards", DIM),
        ]
        if s.get("buried_count") or s.get("suspended_count"):
            rows.append(
                (
                    f"Buried {s.get('buried_count', 0)}   "
                    f"Suspended {s.get('suspended_count', 0)}",
                    "",
                    DIM,
                )
            )

        if more:
            go_label = "A = Next sprint now    B = Done for now"
        elif bonus:
            go_label = "A = Bonus sprint    B = Done for now"
        else:
            go_label = "A = Done"

        # The summary prints like a till receipt: stub, the headline
        # stamped in ink, then stat rows with dotted leaders, and a
        # final perforation ready to tear off.
        roll = self._new_roll(print_y=self.h - 80)
        roll.feed_page(perf=False)
        roll.feed_gap(8)
        roll.feed(self._stub_surface("SESSION RECEIPT", subtitle), reveal=False)
        roll.feed(self._rule_surface(), reveal=False)
        roll.feed_gap(10)
        roll.feed(
            self._stamp_surface(headline, GOOD if status["goal_met"] or more else FG),
            reveal=False,
        )
        roll.feed_gap(12)
        for label, value, color in rows:
            roll.feed(self._receipt_row(label, value, color), reveal=False)
            roll.feed_gap(4)
        roll.feed_gap(6)
        roll.feed_perf()

        while True:
            action = self.poll()
            if action and roll.busy:
                roll.finish()
            if action == "start":
                return "QUIT"
            if action == "north_button":
                self._calendar_return = "CHILD_START"
                return "CALENDAR"
            if action in ("east_button", "unmapped"):
                # "Going ahead": roll straight into the next sprint —
                # spare time now means fewer sprints owed later.
                if more or bonus:
                    self._session_bonus = bonus
                    return "REVIEW"
                self._start_printer_handoff_roll = roll
                return "START_PRINTER_HANDOFF"
            if action in ("south_button", "select"):
                self._start_printer_handoff_roll = roll
                return "START_PRINTER_HANDOFF"

            roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            self._footer(go_label, "X = Calendar   START = Quit")
            self.present()
            self.clock.tick(FPS)

    def screen_start_printer_handoff(self):
        review_roll = self._start_printer_handoff_roll
        self._start_printer_handoff_roll = None
        if review_roll is None or self.settings.get("reduced_motion"):
            return "CHILD_START"

        status = self.service.sprint_status(profile=self.profile)
        startable = status["next_sprint_cards"] > 0
        bonus = not startable and status["bonus_cards"] > 0
        start_roll = self._build_child_start_roll(status, startable, bonus)
        start_roll.finish()
        self._ticket_printed = True

        if review_roll.busy:
            review_roll.finish()

        frames = max(1, int(FPS * 0.32))
        for frame in range(frames + 1):
            if self.poll():
                return "CHILD_START"
            t = frame / frames
            eased = t * t * (3 - 2 * t)
            start_x = -self.w + int(self.w * eased)
            review_x = int(self.w * eased)
            self.screen.fill(BG)
            self._draw_printer_view_at(start_roll, start_x)
            self._draw_printer_view_at(review_roll, review_x)
            self._footer("Returning to the setup printer")
            self.present()
            self.clock.tick(FPS)
        return "CHILD_START"

    # -- stamp calendar ---------------------------------------------------------------

    # Ink-pad colours for the day stamps, vermillion first; busier days
    # move through the pad so they pop a little.
    STAMP_COLORS = (
        (217, 83, 74),  # vermillion
        (76, 154, 107),  # leaf
        (196, 138, 44),  # amber
        (90, 110, 180),  # ink blue
        (150, 90, 160),  # plum
    )

    def screen_calendar(self):
        """Brain Age-style stamp calendar: one stamp per day the child
        studied, with the number of sessions logged that day inside it
        (they can study repeatedly). Read-only; reachable from the child
        start screen (X), the session summary (X), and Parent Mode.
        Each month is a printed sheet on the roll; changing month
        page-feeds to the next sheet."""
        today = self.service.now().astimezone().date()
        year, month = today.year, today.month

        def load(y, m):
            return self.service.sessions_per_day(self.profile["id"], y, m)

        counts = load(year, month)
        roll = self._new_roll(print_y=self.h - 74)
        roll.feed_page(perf=False)
        roll.feed(self._calendar_sheet(year, month, counts, today), reveal=False)
        roll.finish()  # entering the calendar shows this month instantly

        def show_month(y, m):
            nonlocal year, month, counts
            year, month = y, m
            counts = load(year, month)
            roll.feed_page()
            roll.feed(self._calendar_sheet(year, month, counts, today), reveal=False)

        while True:
            action = self.poll()
            if action and roll.busy:
                roll.finish()
            if action in ("start", "select", "south_button"):
                return self._calendar_return
            if action in ("dpad_left", "l1"):
                m0 = (year * 12 + (month - 1)) - 1
                show_month(m0 // 12, m0 % 12 + 1)
            elif action in ("dpad_right", "r1"):
                m0 = (year * 12 + (month - 1)) + 1
                show_month(m0 // 12, m0 % 12 + 1)
            elif action == "east_button":
                if (year, month) != (today.year, today.month):
                    show_month(today.year, today.month)

            roll.update()
            self._paper(roll)
            self._draw_roll(roll)
            self._footer("L1/R1 = Month    A = Today", "SELECT = Back")
            self.present()
            self.clock.tick(FPS)

    def _calendar_sheet(self, year, month, counts, today):
        """One month rendered as a surface — a printed sheet the roll
        can feed past like any other page."""
        width = self.content_x1 - self.content_x0 - 8
        weeks = _calendar.Calendar(firstweekday=6).monthdayscalendar(year, month)
        grid_top = 96
        cell_h = 46
        sheet = pygame.Surface(
            (width, grid_top + len(weeks) * cell_h + 6), pygame.SRCALPHA
        )

        title = f"◀   {_calendar.month_name[month].upper()} {year}   ▶"
        t = self.font_big.render(title, True, FG)
        sheet.blit(t, ((width - t.get_width()) // 2, 0))
        total = sum(counts.values())
        active = len(counts)
        subtitle = (
            f"{total} session{'s' if total != 1 else ''} on "
            f"{active} day{'s' if active != 1 else ''}"
            if total
            else "No sessions this month yet"
        )
        s = self.font_small.render(subtitle, True, DIM)
        sheet.blit(s, ((width - s.get_width()) // 2, 52))

        cell_w = width / 7
        weekdays = ("SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT")
        for col, name in enumerate(weekdays):
            surf = self.font_small.render(name, True, DIM)
            cx = col * cell_w + (cell_w - surf.get_width()) / 2
            sheet.blit(surf, (int(cx), grid_top - 22))

        for row, week in enumerate(weeks):
            for col, day in enumerate(week):
                if day == 0:
                    continue
                self._draw_day_cell(
                    sheet,
                    day,
                    counts.get(day, 0),
                    is_today=(
                        year == today.year and month == today.month and day == today.day
                    ),
                    x=int(col * cell_w),
                    y=grid_top + row * cell_h,
                    w=int(cell_w),
                    h=cell_h,
                )
        return sheet

    def _draw_day_cell(self, target, day, count, is_today, x, y, w, h):
        if is_today:
            pygame.draw.rect(
                target, ACCENT, (x + 2, y, w - 4, h - 2), 2, border_radius=6
            )
        # Day number, top-left corner.
        num = self.font_small.render(str(day), True, FG if count else DIM)
        target.blit(num, (x + 6, y + 3))
        if count <= 0:
            return
        # Stamp: a filled ink circle with the session count punched out
        # of it in paper colour, ringed like a real rubber stamp.
        color = self.STAMP_COLORS[min(count - 1, len(self.STAMP_COLORS) - 1)]
        cx, cy = x + w // 2, y + h // 2 + 6
        radius = min(h // 2 - 4, 18)
        pygame.draw.circle(target, color, (cx, cy), radius)
        pygame.draw.circle(target, BG, (cx, cy), radius, 2)
        label = self.font.render(str(count), True, BG)
        target.blit(
            label, (cx - label.get_width() // 2, cy - label.get_height() // 2)
        )

    # -- parent mode -----------------------------------------------------------------

    def screen_parent_menu(self):
        direction = self.profile.get("study_direction", "normal")
        entries = [
            ("Import deck (.apkg)", "PARENT_IMPORT"),
            ("Decks", "PARENT_DECKS"),
            ("Categories", "PARENT_CATEGORIES"),
            ("Daily goal & sprints", "PARENT_LIMITS"),
            ("Paper feed sound", "PARENT_AUDIO"),
            ("Suspended cards", "PARENT_SUSPENDED"),
            ("Progress", "PARENT_PROGRESS"),
            ("Calendar (stamps)", "CALENDAR"),
            ("Controller test & setup", "INPUT_DIAG"),
            (
                f"Direction: "
                f"{'Reversed (back first)' if direction == 'reversed' else 'Normal (front first)'}",
                "TOGGLE_DIRECTION",
            ),
            ("Back to study", "CHILD_START"),
        ]
        index = 0
        top = 0
        notice = None
        if self._card_count() == 0:
            notice = "No decks yet — copy an .apkg to the data folder and use Import."
        # Leave room for the notice lines under the list when it shows.
        visible = 6 if notice else 7
        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                return "CHILD_START"
            if action == "dpad_up":
                index = (index - 1) % len(entries)
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
            elif action == "east_button":
                target = entries[index][1]
                if target == "TOGGLE_DIRECTION":
                    new = "normal" if direction == "reversed" else "reversed"
                    self.storage.update_profile(self.profile["id"], study_direction=new)
                    self._reload_profile()
                    return "PARENT_MENU"
                if target == "CALENDAR":
                    self._calendar_return = "PARENT_MENU"
                return target
            top = min(max(top, index - visible + 1), index)

            self._paper()
            self._page_header("Parent Mode", f"Profile: {self.profile['name']}")
            y = 124
            for i in range(top, min(top + visible, len(entries))):
                self._menu_row(entries[i][0], 100, y, i == index)
                y += 40
            if notice:
                for line in wrap_text(self.font_small, notice, self.w - 120):
                    self._center(self.font_small.render(line, True, WARN), y + 6)
                    y += 22
            self._footer("Up/Down = Choose   A = Select   B = Back")
            self.present()
            self.clock.tick(FPS)

    def settings_dir(self):
        return os.path.dirname(os.path.abspath(self.settings.path))

    def _scan_apkg_files(self):
        """Places a parent might drop an .apkg: the data dir, an
        import/ subfolder of it, the app folder, and the cwd."""
        roots = [
            self.settings_dir(),
            os.path.join(self.settings_dir(), "import"),
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            os.getcwd(),
        ]
        seen, found = set(), []
        for root in roots:
            if not root or not os.path.isdir(root):
                continue
            for name in sorted(os.listdir(root)):
                if name.lower().endswith(".apkg"):
                    path = os.path.join(root, name)
                    if path not in seen:
                        seen.add(path)
                        found.append(path)
        return found

    def screen_parent_import(self):
        files = self._scan_apkg_files()
        index = 0
        message = None
        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                return "PARENT_MENU"
            if files and action == "dpad_up":
                index = (index - 1) % len(files)
            elif files and action == "dpad_down":
                index = (index + 1) % len(files)
            elif files and action == "east_button":
                self._draw_import(files, index, "Importing…")
                self.present()
                try:
                    stats = import_apkg(
                        files[index],
                        self.storage,
                        self.service.scheduler,
                        os.path.join(self.settings_dir(), "media"),
                    )
                    message = stats.summary()
                    log.info("import %s: %s", files[index], message)
                except (ApkgError, OSError) as exc:
                    message = f"Import failed: {exc}"
                    log.error("import %s failed: %s", files[index], exc)

            self._draw_import(files, index, message)
            self.present()
            self.clock.tick(FPS)

    def _draw_import(self, files, index, message):
        self._paper()
        self._page_header("Import Deck")
        if not files:
            self._center(self.font.render("No .apkg files found.", True, DIM), 160)
            self._center(
                self.font_small.render(
                    "Copy a deck into the data/ folder on the SD card.", True, DIM
                ),
                210,
            )
        else:
            y = 112
            for i, path in enumerate(files[:7]):
                self._menu_row(os.path.basename(path), 84, y, i == index)
                y += 38
        if message:
            y = self.h - 160
            for line in wrap_text(self.font_small, message, self.w - 120):
                self._center(self.font_small.render(line, True, WARN), y)
                y += 24
        self._footer("A = Import   B = Back")

    def screen_parent_categories(self):
        return self._screen_multi_select(
            title="Categories",
            subtitle="Choose what the child studies",
            all_label="[ All categories ]",
            items=self.storage.all_tags(),
            profile_field="active_categories",
            empty_message="No tags found — import a deck first.",
        )

    def screen_parent_decks(self):
        return self._screen_multi_select(
            title="Decks",
            subtitle="Choose which decks the child studies",
            all_label="[ All decks ]",
            items=self.storage.deck_names_list(),
            profile_field="active_decks",
            empty_message="No decks found — import one first.",
        )

    def _screen_multi_select(
        self, title, subtitle, all_label, items, profile_field, empty_message
    ):
        """Shared toggle-list UI behind Categories and Decks: a
        "[ All ... ]" entry plus one per item, with the active subset
        persisted to the profile (None means "all active")."""
        active = self.profile[profile_field]
        selected = None if active is None else set(active)
        index = 0
        entries = [all_label] + items
        top = 0
        visible = 7

        def save():
            self.storage.update_profile(
                self.profile["id"],
                **{profile_field: (None if selected is None else sorted(selected))},
            )
            self._reload_profile()

        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                save()
                return "PARENT_MENU"
            if action == "dpad_up":
                index = (index - 1) % len(entries)
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
            elif action == "east_button":
                if index == 0:
                    selected = None
                else:
                    item = entries[index]
                    if selected is None:
                        selected = {item}
                    elif item in selected:
                        selected.discard(item)
                    else:
                        selected.add(item)
            top = min(max(top, index - visible + 1), index)

            self._paper()
            self._page_header(title, subtitle)
            y = 124
            for i in range(top, min(top + visible, len(entries))):
                label = entries[i]
                if i == 0:
                    mark = "(x)" if selected is None else "( )"
                else:
                    on = selected is None or label in selected
                    mark = "[x]" if on else "[ ]"
                text = self._truncate_to_width(
                    self.font, f"{mark} {label}", self.w - 160
                )
                self._menu_row(text, 90, y, i == index)
                y += 40
            if not items:
                self._center(self.font.render(empty_message, True, DIM), 200)
            self._footer("A = Toggle   B = Save & back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_limits(self):
        # The goal/sprint pair defines the child's day: goal cards split
        # into sprints of session_card_limit. daily_new_cards paces new
        # material; study-ahead settings are CLI-only for now.
        fields = [
            ("Daily goal (cards)", "daily_goal_cards", 10, 500),
            ("New cards per day (0 = auto)", "daily_new_cards", 0, 500),
            ("Cards per sprint", "session_card_limit", 1, 200),
            ("Minutes per sprint (0 = off)", "session_time_minutes", 0, 90),
        ]
        values = {key: self.profile[key] for _, key, _, _ in fields}
        index = 0
        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                self.storage.update_profile(self.profile["id"], **values)
                self._reload_profile()
                return "PARENT_MENU"
            if action == "dpad_up":
                index = (index - 1) % len(fields)
            elif action == "dpad_down":
                index = (index + 1) % len(fields)
            elif action in ("dpad_left", "dpad_right", "l1", "r1"):
                label, key, lo, hi = fields[index]
                step = {"dpad_left": -1, "dpad_right": 1, "l1": -10, "r1": 10}[action]
                values[key] = min(max(values[key] + step, lo), hi)

            self._paper()
            sprint_size = max(values["session_card_limit"], 1)
            sprints = -(-values["daily_goal_cards"] // sprint_size)
            self._page_header(
                "Daily Goal & Sprints",
                f"= {sprints} sprint{'s' if sprints != 1 else ''} a day",
            )
            y = 140
            for i, (label, key, _, _) in enumerate(fields):
                self._menu_row(label, 84, y, i == index)
                value = self.font.render(str(values[key]), True, FG)
                self.screen.blit(value, (self.w - 130, y))
                y += 52
            self._footer("D-pad = Adjust   L1/R1 = +/-10   B = Save & back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_audio(self):
        enabled = bool(self.settings.get("paper_feed_sfx_enabled", True))
        volume = int(round(
            float(self.settings.get("paper_feed_sfx_volume", 0.15)) * 100
        ))
        volume = min(max(volume, 0), 100)
        index = 0
        fields = ("Paper feed SFX", "Volume")

        def save():
            self.settings.set("paper_feed_sfx_enabled", enabled)
            self.settings.set("paper_feed_sfx_volume", volume / 100)

        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                save()
                return "PARENT_MENU"
            if action == "dpad_up":
                index = (index - 1) % len(fields)
            elif action == "dpad_down":
                index = (index + 1) % len(fields)
            elif action == "east_button" and index == 0:
                enabled = not enabled
            elif action in ("dpad_left", "dpad_right", "l1", "r1") and index == 1:
                step = {
                    "dpad_left": -5,
                    "dpad_right": 5,
                    "l1": -10,
                    "r1": 10,
                }[action]
                volume = min(max(volume + step, 0), 100)
                self.audio.play_effect(LINE_FEED_SFX, volume=volume / 100)

            self._paper()
            self._page_header("Paper Feed Sound", "Tick and page-feed whirr")
            rows = [
                ("Paper feed SFX", "On" if enabled else "Off"),
                ("Volume", f"{volume}%"),
            ]
            y = 150
            for i, (label, value) in enumerate(rows):
                self._menu_row(label, 100, y, i == index)
                value_s = self.font.render(value, True, FG)
                self.screen.blit(value_s, (self.w - 190, y))
                y += 56
            self._footer("A = Toggle   D-pad/L1/R1 = Volume", "B = Save & back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_suspended(self):
        index, top, visible = 0, 0, 7
        cards = self.storage.suspended_cards()
        while True:
            action = self.poll()
            if action in ("start", "south_button", "select"):
                return "PARENT_MENU"
            if cards:
                if action == "dpad_up":
                    index = (index - 1) % len(cards)
                elif action == "dpad_down":
                    index = (index + 1) % len(cards)
                elif action == "east_button":
                    self.service.unsuspend_card(cards[index]["id"])
                    cards = self.storage.suspended_cards()
                    index = max(index - 1, 0)
            index = min(index, max(len(cards) - 1, 0))
            top = min(max(top, index - visible + 1), index) if cards else 0

            self._paper()
            self._page_header("Suspended Cards")
            if not cards:
                self._center(self.font.render("No suspended cards.", True, DIM), 200)
            y = 112
            for i in range(top, min(top + visible, len(cards))):
                card = cards[i]
                text = card["front"].replace("\n", " ")
                if len(text) > 38:
                    text = text[:37] + "…"
                self._menu_row(text, 80, y, i == index)
                y += 40
            self._footer("A = Unsuspend   B = Back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_progress(self):
        totals = self.service.recent_daily_totals(7)
        suspended = len(self.storage.suspended_cards())
        total_cards = self._card_count()
        while True:
            action = self.poll()
            if action in ("start", "south_button", "east_button", "select"):
                return "PARENT_MENU"

            self._paper()
            self._page_header(
                "Progress",
                f"{self.profile['name']}  ·  {total_cards} cards  ·  "
                f"{suspended} suspended",
            )
            y = 130
            header = self.font_small.render("Day            Reviews    New", True, DIM)
            self.screen.blit(header, (150, y))
            y += 30
            for day, reviews, new in totals:
                label = day.strftime("%a %b %d")
                row = f"{label:<14} {reviews:>7} {new:>6}"
                self.screen.blit(self.font.render(row, True, FG), (150, y))
                y += 38
            self._footer("B = Back")
            self.present()
            self.clock.tick(FPS)

    # -- input diagnostic / calibration ------------------------------------------------

    def screen_input_diagnostic(self):
        """Raw-event viewer: shows what the device actually sends and
        how the current mapping interprets it. Hold any single gamepad
        button ~3 s (or press C on a keyboard) to start calibration —
        that works even when the current mapping is completely wrong."""
        events = []  # newest-first (raw text, semantic, study)
        held_since = {}  # raw button index -> monotonic time
        HOLD_SECONDS = 3.0

        while True:
            calibrate = False
            exit_screen = False
            event = pygame.event.poll()
            while event.type != pygame.NOEVENT:
                raw, semantic = None, None
                if event.type == pygame.QUIT:
                    raise QuitApp
                if event.type == pygame.KEYDOWN:
                    if event.key == pygame.K_c:
                        calibrate = True
                    if event.key in (pygame.K_ESCAPE, pygame.K_q):
                        exit_screen = True
                    raw = f"key {pygame.key.name(event.key)}"
                elif event.type == pygame.JOYBUTTONDOWN:
                    raw = f"button {event.button}"
                    held_since[event.button] = time.monotonic()
                elif event.type == pygame.JOYBUTTONUP:
                    held_since.pop(event.button, None)
                elif event.type == pygame.JOYHATMOTION:
                    raw = f"hat {event.hat} value={event.value}"
                elif event.type == pygame.JOYAXISMOTION:
                    if abs(event.value) > 0.5:
                        raw = f"axis {event.axis} value={event.value:+.2f}"
                elif event.type == pygame.JOYDEVICEADDED:
                    self._open_joystick(event.device_index)
                    raw = "joystick connected"
                if raw is not None:
                    semantic = self.input.translate(event)
                    study = STUDY_ACTIONS.get(semantic, "—")
                    events.insert(0, (raw, semantic or "—", study))
                    del events[10:]
                    if semantic == "select":
                        exit_screen = True
                event = pygame.event.poll()

            now = time.monotonic()
            if any(now - t >= HOLD_SECONDS for t in held_since.values()):
                calibrate = True
            if calibrate:
                return "CALIBRATE"
            if exit_screen:
                return "PARENT_MENU" if self.initial_state != "INPUT_DIAG" else "QUIT"

            self._paper()
            self._center(self.font_big.render("Controller Test", True, FG), 24)
            n_joy = pygame.joystick.get_count()
            names = (
                ", ".join(pygame.joystick.Joystick(i).get_name() for i in range(n_joy))
                or "none detected"
            )
            self._center(
                self.font_small.render(f"Joysticks: {n_joy} ({names})", True, DIM), 74
            )
            y = 108
            header = self.font_small.render(
                f"{'raw event':<26}{'semantic':<16}study action", True, DIM
            )
            self.screen.blit(header, (48, y))
            y += 26
            for raw, semantic, study in events:
                line = f"{raw:<26}{semantic:<16}{study}"
                self.screen.blit(self.font_small.render(line, True, FG), (48, y))
                y += 24
            if not events:
                self._center(
                    self.font.render("Press buttons to see events…", True, DIM), 220
                )
            self._footer(
                "Hold any button 3s = remap buttons", "SELECT or Esc = back"
            )
            self.present()
            self.clock.tick(FPS)

    CALIBRATION_STEPS = (
        ("south_button", "the B face button"),
        ("east_button", "the A face button"),
        ("west_button", "the Y face button"),
        ("north_button", "the X face button"),
        ("l1", "the LEFT shoulder button (L1)"),
        ("r1", "the RIGHT shoulder button (R1)"),
        ("select", "SELECT"),
        ("start", "START"),
    )

    def screen_calibrate(self):
        """Assign physical buttons to semantic slots, one prompt at a
        time, then persist the mapping to JSON. D-pads that report as
        hats/axes need no calibration; ones that report as buttons get
        picked up here too (extra optional steps)."""
        steps = list(self.CALIBRATION_STEPS) + [
            ("dpad_up", "D-pad UP (auto-skips if it moved earlier)"),
            ("dpad_down", "D-pad DOWN"),
            ("dpad_left", "D-pad LEFT"),
            ("dpad_right", "D-pad RIGHT"),
        ]
        new_map = InputMap(None)
        new_map.buttons = {}
        new_map.path = self.input_map.path
        step = 0
        message = None
        dpad_is_hat = False

        while step < len(steps):
            semantic, prompt = steps[step]
            if dpad_is_hat and semantic.startswith("dpad_"):
                break  # hat d-pad confirmed: no button mapping needed

            event = pygame.event.poll()
            if event.type == pygame.QUIT:
                raise QuitApp
            if event.type == pygame.KEYDOWN and event.key in (
                pygame.K_ESCAPE,
                pygame.K_q,
            ):
                log.info("calibration cancelled")
                return "INPUT_DIAG"
            if event.type == pygame.JOYDEVICEADDED:
                self._open_joystick(event.device_index)
            if event.type == pygame.JOYHATMOTION and event.value != (0, 0):
                if semantic.startswith("dpad_"):
                    dpad_is_hat = True
                    continue
                message = "That was the D-pad — press " + prompt
            if event.type == pygame.JOYBUTTONDOWN:
                if event.button in new_map.buttons:
                    already = new_map.buttons[event.button]
                    message = (
                        f"Button {event.button} is already "
                        f"{already} — press a different one"
                    )
                else:
                    new_map.set_button(event.button, semantic)
                    log.info("calibrated %s = button %d", semantic, event.button)
                    step += 1
                    message = None
                    continue

            self._paper()
            self._center(self.font_big.render("Controller Setup", True, FG), 40)
            self._center(
                self.font_small.render(f"Step {step + 1} of {len(steps)}", True, DIM),
                100,
            )
            self._center(self.font.render("Press " + prompt, True, ACCENT), 190)
            if message:
                self._center(self.font_small.render(message, True, WARN), 250)
            done = ", ".join(
                f"{FACE_LABELS.get(s, s)}={b}"
                for b, s in sorted(new_map.buttons.items())
            )
            if done:
                y = 300
                for line in wrap_text(self.font_small, "So far: " + done, self.w - 80):
                    self._center(self.font_small.render(line, True, DIM), y)
                    y += 22
            self._footer("Esc = cancel (keyboard)")
            self.present()
            self.clock.tick(FPS)

        self.input_map.buttons = dict(new_map.buttons)
        try:
            self.input_map.save()
            message = "Saved!"
        except OSError as exc:
            log.error("could not save input mapping: %s", exc)
            message = f"Could not save mapping: {exc}"
        self.input = InputTranslator(self.input_map)

        self._paper()
        self._center(self.font_big.render("Controller Setup", True, FG), 40)
        self._center(self.font.render(message, True, GOOD), 200)
        self.present()
        pygame.time.wait(1200)
        return "INPUT_DIAG"

    # -- drawing helpers ----------------------------------------------------------------

    # Content span between the sprocket strips; everything printed on
    # the paper stays inside it.
    @property
    def content_x0(self):
        return STRIP_W + 6

    @property
    def content_x1(self):
        return self.w - STRIP_W - 6

    def _paper(self, roll=None):
        """Paper background + tractor-feed sprocket strips. The punched
        holes are on the paper, so they ride with the roll's offset —
        the sheet visibly moves during a feed."""
        self.screen.fill(BG)
        scroll = roll.offset if roll is not None else 0.0
        phase = int(-scroll) % HOLE_SPACING
        for x_edge, seam_x in ((0, STRIP_W), (self.w - STRIP_W, self.w - STRIP_W - 1)):
            pygame.draw.rect(self.screen, STRIP, (x_edge, 0, STRIP_W, self.h))
            pygame.draw.line(self.screen, DIVIDER, (seam_x, 0), (seam_x, self.h))
            cx = x_edge + STRIP_W // 2
            for y in range(phase - HOLE_SPACING, self.h + HOLE_SPACING, HOLE_SPACING):
                pygame.draw.circle(self.screen, HOLE, (cx, y), 5)

    def _new_roll(self, print_y=None):
        roll = PaperRoll(
            print_y=self.h * 2 // 3 if print_y is None else print_y,
            reduced_motion=bool(self.settings.get("reduced_motion")),
            on_feed=self._paper_feed_sound,
        )
        roll.perf_color = DIVIDER
        roll.head_color = FG
        return roll

    def _paper_feed_sound(self, event):
        if not self.settings.get("paper_feed_sfx_enabled", True):
            return
        volume = float(self.settings.get("paper_feed_sfx_volume", 0.15))
        if event == "line":
            self.audio.play_effect(LINE_FEED_SFX, volume=volume)
        elif event == "page":
            self.audio.play_effect(PAGE_FEED_SFX, volume=volume)

    def _draw_roll(self, roll):
        roll.draw(self.screen, self.content_x0, self.content_x1, self.h - 44)

    def _draw_printer_view_at(self, roll, x):
        view = pygame.Surface((self.w, self.h))
        previous = self.screen
        self.screen = view
        try:
            self._paper(roll)
            self._draw_roll(roll)
        finally:
            self.screen = previous
        self.screen.blit(view, (x, 0))

    def _menu_row(self, text, x, y, selected, font=None):
        """One list entry. Selection is a vermillion margin marker plus
        an ink underline — a proofreader's mark on the page edge, not a
        colour-swapped cursor line."""
        font = font or self.font
        surf = font.render(text, True, FG)
        self.screen.blit(surf, (x, y))
        if selected:
            marker = font.render("▶", True, ACCENT)
            self.screen.blit(marker, (x - marker.get_width() - 12, y))
            base = y + font.get_ascent() + 3
            pygame.draw.line(self.screen, FG, (x, base), (x + surf.get_width(), base), 2)

    def _page_header(self, title, subtitle=None, top=36):
        """Worksheet-style screen title: centred ink caps over a rule."""
        self._center(self.font_big.render(title, True, FG), top)
        rule_y = top + self.font_big.get_linesize() + 6
        pygame.draw.line(
            self.screen, DIVIDER, (self.content_x0 + 30, rule_y), (self.content_x1 - 30, rule_y)
        )
        y = rule_y + 10
        if subtitle:
            self._center(self.font_small.render(subtitle, True, DIM), y)
            y += 26
        return y

    def _stub_surface(self, left, right):
        """Receipt stub printed at the top of every page: small pencil
        caps, left and right ends of one row."""
        width = self.content_x1 - self.content_x0 - 32
        surf = pygame.Surface((width, 24), pygame.SRCALPHA)
        right_s = self.font_small.render(right, True, DIM)
        left_max = width - right_s.get_width() - 24
        left_s = self.font_small.render(
            self._truncate_to_width(self.font_small, left, left_max), True, DIM
        )
        surf.blit(left_s, (0, 0))
        surf.blit(right_s, (width - right_s.get_width(), 0))
        return surf

    def _rule_surface(self, inset=16):
        width = self.content_x1 - self.content_x0 - 2 * inset
        surf = pygame.Surface((width, 9), pygame.SRCALPHA)
        pygame.draw.line(surf, DIVIDER, (0, 4), (width, 4))
        return surf

    def _stamp_surface(self, text, color, font=None):
        """Rubber-stamp impression: bordered caps, slightly rotated, a
        touch translucent like real ink."""
        font = font or self.font
        label = font.render(text.upper(), True, color)
        pad_x, pad_y = 18, 8
        box = pygame.Surface(
            (label.get_width() + 2 * pad_x, label.get_height() + 2 * pad_y),
            pygame.SRCALPHA,
        )
        pygame.draw.rect(box, color, box.get_rect(), 3, border_radius=8)
        box.blit(label, (pad_x, pad_y))
        stamp = pygame.transform.rotate(box, 5)
        stamp.set_alpha(215)
        return stamp

    def _receipt_row(self, label, value, color=FG):
        """Till-receipt line: label left, value right, dotted leader
        between them (no leader for full-width labels)."""
        width = self.content_x1 - self.content_x0 - 100
        line_h = self.font.get_linesize()
        surf = pygame.Surface((width, line_h), pygame.SRCALPHA)
        label_s = self.font.render(label, True, color)
        surf.blit(label_s, (0, 0))
        if value:
            value_s = self.font.render(value, True, color)
            surf.blit(value_s, (width - value_s.get_width(), 0))
            dot_y = self.font.get_ascent() - 3
            for x in range(label_s.get_width() + 12, width - value_s.get_width() - 12, 8):
                pygame.draw.circle(surf, DIVIDER, (x, dot_y), 1)
        return surf

    def _center(self, surf, y):
        self.screen.blit(surf, ((self.w - surf.get_width()) // 2, y))

    @staticmethod
    def _truncate_to_width(font, text, max_width):
        """``text``, shortened with a trailing "…" if it exceeds
        ``max_width`` pixels in ``font`` — never lets a long deck name
        or tag list run into whatever is drawn next to it."""
        if max_width <= 0:
            return ""
        if font.size(text)[0] <= max_width:
            return text
        ellipsis = "…"
        ellipsis_width = font.size(ellipsis)[0]
        if ellipsis_width > max_width:
            return ""  # not even room for the ellipsis alone
        budget = max_width - ellipsis_width
        truncated = text
        while truncated and font.size(truncated)[0] > budget:
            truncated = truncated[:-1]
        return truncated + ellipsis

    def _footer(self, text, second_line=None):
        """Button hints on the chassis bar — the printer body under the
        paper. Fixed: it never scrolls with the roll."""
        top = self.h - (64 if second_line else 44)
        pygame.draw.rect(self.screen, CHASSIS, (0, top, self.w, self.h - top))
        surf = self.font_small.render(text, True, CHASSIS_TEXT)
        self.screen.blit(surf, ((self.w - surf.get_width()) // 2, top + 10))
        if second_line:
            surf2 = self.font_small.render(second_line, True, CHASSIS_TEXT)
            self.screen.blit(surf2, ((self.w - surf2.get_width()) // 2, top + 34))
