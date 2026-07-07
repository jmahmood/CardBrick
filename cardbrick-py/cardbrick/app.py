"""CardBrick-style study appliance UI.

A state-driven pygame app aimed at children microstudying Spanish on a
handheld: a daily card goal chipped away in short sprints across the
day (see ReviewService.sprint_status), never one long sitting. Screens:

    ChildStart -> DeckSelect (only when >1 deck is assigned)
              -> Review (one sprint) -> SessionSummary
                 -> Review again ("going ahead": the next sprint now)
                 -> or back to ChildStart
    ChildStart / SessionSummary -> Calendar (stamp calendar) -> back
    ChildStart -> ParentMode (import / decks / categories / limits /
                              suspended / progress / calendar /
                              controller setup) -> ChildStart

Parent Mode's Decks screen configures which decks are *assigned* to the
child at all (child_profiles.active_decks); DeckSelect is the child's
own per-sitting choice of which *one* assigned deck (or all of them
combined) to study right now. A single assigned deck skips the picker
entirely — no extra tap when there's no real choice to make.

Controller-first and deployment-hardened for Knulli-style devices:

- Input is translated to *semantic* actions (south_button, dpad_up,
  start, ...) through a JSON mapping calibrated on-device — raw SDL
  button indices are never trusted (see input_map.py). Face buttons are
  labelled by physical position ("Bottom = Good"), not A/B/X/Y.
- Everything renders on a fixed logical canvas (default 640x480, the
  RG35XX SP panel) which is scaled — integer scaling when it fits — to
  the real display.
- The app has its own exit paths (START from summary quits; SELECT +
  START held 2 s force-exits from anywhere; Esc on desktop) and never
  relies on RetroArch hotkeys.
- A bundled DejaVu Sans covers Spanish glyphs; font resolution and all
  startup facts are logged (see bootlog.py).

Keyboard fallback for desktop testing: arrows reveal, 1/2/3/4 =
Again/Hard/Good/Easy (or literal A/B/X/Y keys), L replay, R bury,
U undo, Tab menu, Esc finish/quit.
"""

import calendar as _calendar
import logging
import os
import time

import pygame

from .bootlog import (log_display_diagnostics, log_pygame_versions)
from .importer import ApkgError, import_apkg
from .input_map import (FACE_LABELS, InputMap, InputTranslator,
                        STUDY_ACTIONS)
from .scheduler import iso
from .session import StudySession
from .textutil import wrap_text

log = logging.getLogger(__name__)

FPS = 30

BG = (24, 26, 30)
FG = (235, 235, 228)
DIM = (140, 142, 148)
ACCENT = (120, 180, 250)
GOOD = (120, 210, 140)
WARN = (240, 180, 100)
DIVIDER = (70, 72, 78)
OVERLAY_BG = (36, 39, 45)

# Semantic face button -> FSRS rating. Position language, not A/B/X/Y:
# bottom=Good, right=Again, left=Easy, top=Hard.
RATING_FOR_SEMANTIC = {"east_button": 1, "north_button": 2,
                       "south_button": 3, "west_button": 4}

DPAD = ("dpad_up", "dpad_down", "dpad_left", "dpad_right")

_BUNDLED_FONT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "assets", "fonts")

FONT_CANDIDATES = [
    os.environ.get("CARDBRICK_FONT", ""),
    os.path.join(_BUNDLED_FONT_DIR, "DejaVuSans.ttf"),
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
]

_font_path_logged = False


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
        log.warning("no bundled/system font found — using pygame builtin "
                    "(Spanish accents may render poorly)")
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
    def __init__(self, storage, service, audio, settings, paths=None,
                 fullscreen=None, initial_state=None):
        self.storage = storage
        self.service = service
        self.audio = audio
        self.settings = settings
        self.paths = paths
        self.initial_state = initial_state

        self.input_map = InputMap(paths.input_map_path if paths else None)
        self._image_cache = {}  # vocab card images, decoded once per file
        self._session_deck_filter = None  # child's per-sitting deck pick
        self._session_bonus = False  # next sprint ignores the daily goal
        self._sprint_label = ""      # "Sprint 3/8" shown in the review header
        self._calendar_return = "CHILD_START"  # where the stamp calendar exits to
        self.input = InputTranslator(self.input_map)

        pygame.init()
        log_pygame_versions()
        ensure_font_support()
        self.w = int(settings.get("logical_width", 640))
        self.h = int(settings.get("logical_height", 480))
        if fullscreen is None:
            fullscreen = bool(settings.get("fullscreen"))
        self.fullscreen = fullscreen
        self._init_display()
        pygame.display.set_caption("CardBrick — Spanish Practice")
        pygame.mouse.set_visible(False)
        for i in range(pygame.joystick.get_count()):
            pygame.joystick.Joystick(i).init()

        self.font_big = _load_font(38)
        self.font = _load_font(26)
        self.font_small = _load_font(18)
        self.clock = pygame.time.Clock()

        log_display_diagnostics(self.display.get_size(), (self.w, self.h),
                                self.fullscreen, resolve_font_path())

        # Crash recovery: close sessions that never got an end stamp.
        recovered = self.storage.close_dangling_sessions(
            iso(self.service.now()))
        if recovered:
            log.info("recovered %d interrupted session(s): %s",
                     len(recovered), recovered)
        self.profile = self._boot_profile()

    # -- display / scaling -------------------------------------------------------

    def _init_display(self):
        """Logical canvas + real display; scaling computed once."""
        try:
            if self.fullscreen:
                self.display = pygame.display.set_mode(
                    (0, 0), pygame.FULLSCREEN)
                if self.display.get_size() < (self.w, self.h):
                    log.warning("display %s smaller than logical %s",
                                self.display.get_size(), (self.w, self.h))
            else:
                self.display = pygame.display.set_mode((self.w, self.h))
        except pygame.error as exc:
            log.error("display init failed: %s (SDL_VIDEODRIVER=%s)",
                      exc, os.environ.get("SDL_VIDEODRIVER", "auto"))
            raise DisplayInitError(str(exc)) from exc

        dw, dh = self.display.get_size()
        if (dw, dh) == (0, 0):  # dummy driver corner case
            self.display = pygame.display.set_mode((self.w, self.h))
            dw, dh = self.w, self.h

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
            pygame.transform.scale(self.canvas,
                                   self._scaled.get_size(), self._scaled)
            self.display.blit(self._scaled, self._scale_offset)
        pygame.display.flip()

    # -- boot ------------------------------------------------------------------------

    def _boot_profile(self):
        profile_id = self.settings.get("current_child_profile_id")
        profile = self.storage.get_profile(profile_id) if profile_id else None
        if profile is None:
            profile = self.storage.ensure_default_profile()
            self.settings.set("current_child_profile_id", profile["id"])
            log.info("using default profile %r (id=%s)", profile["name"],
                     profile["id"])
        if profile["active_categories"] == []:
            log.warning("profile %r has an EMPTY category list — the "
                        "child will see no cards until a parent fixes it",
                        profile["name"])
        return profile

    def _reload_profile(self):
        self.profile = self.storage.get_profile(self.profile["id"])

    def _card_count(self):
        return self.storage.conn.execute(
            "SELECT COUNT(*) AS n FROM cards").fetchone()["n"]

    # -- main state machine ------------------------------------------------------

    def run(self):
        if self.initial_state:
            state = self.initial_state
        elif self._card_count() == 0:
            # Nothing imported yet: route straight to setup/parent mode.
            log.warning("no cards in database — starting in parent mode")
            state = "PARENT_MENU"
        else:
            state = "CHILD_START"
        handlers = {
            "CHILD_START": self.screen_child_start,
            "DECK_SELECT": self.screen_deck_select,
            "REVIEW": self.screen_review,
            "SUMMARY": self.screen_summary,
            "CALENDAR": self.screen_calendar,
            "PARENT_MENU": self.screen_parent_menu,
            "PARENT_IMPORT": self.screen_parent_import,
            "PARENT_CATEGORIES": self.screen_parent_categories,
            "PARENT_DECKS": self.screen_parent_decks,
            "PARENT_LIMITS": self.screen_parent_limits,
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

    def poll(self):
        """Next semantic action, or None.

        Consumes exactly one meaningful event per call so rapid inputs
        queued between frames are never dropped. Also services joystick
        hot-plug and the SELECT+START force-exit gesture.
        """
        while True:
            event = pygame.event.poll()
            if event.type == pygame.NOEVENT:
                if self.input.force_exit_held():
                    log.info("SELECT+START held — force exit")
                    raise QuitApp
                return None
            if event.type == pygame.QUIT:
                raise QuitApp
            if event.type == pygame.JOYDEVICEADDED:
                joy = pygame.joystick.Joystick(event.device_index)
                joy.init()
                log.info("joystick connected: %r", joy.get_name())
                continue
            if event.type == pygame.JOYDEVICEREMOVED:
                log.warning("joystick disconnected")
                continue
            action = self.input.translate(event)
            if action:
                return action

    # -- child start -----------------------------------------------------------------

    def _resolve_available_decks(self):
        """Decks the parent has assigned to this profile: every deck if
        active_decks is None, else that explicit (possibly empty) list."""
        active = self.profile["active_decks"]
        return self.storage.deck_names_list() if active is None \
            else list(active)

    def screen_child_start(self):
        self._session_deck_filter = None  # cleared each time we land here
        self._session_bonus = False
        status = self.service.sprint_status(profile=self.profile)
        available_decks = self._resolve_available_decks()
        categories = self.profile["active_categories"]
        cat_label = "All categories" if categories is None else \
            ", ".join(categories) if categories else "No categories set"
        decks = self.profile["active_decks"]
        deck_label = "All decks" if decks is None else \
            ", ".join(decks) if decks else "No decks set"
        startable = status["next_sprint_cards"] > 0
        bonus = not startable and status["bonus_cards"] > 0

        while True:
            action = self.poll()
            if action == "start":
                return "QUIT"
            if action == "select":
                return "PARENT_MENU"
            if action == "north_button":
                self._calendar_return = "CHILD_START"
                return "CALENDAR"
            if action in ("south_button", "unmapped") and \
                    (startable or bonus):
                self._session_bonus = bonus
                return "DECK_SELECT" if len(available_decks) > 1 \
                    else "REVIEW"

            self.screen.fill(BG)
            self._center(self.font_small.render("SPANISH PRACTICE", True,
                                                DIM), 36)
            self._center(self.font_big.render(self.profile["name"], True,
                                              FG), 80)
            self._center(self.font.render(cat_label, True, ACCENT), 150)
            self._center(self.font_small.render(deck_label, True, DIM), 182)
            if startable:
                # "Last sprint" only makes sense once earlier sprints
                # happened; an untouched day gets "today" phrasing.
                n = status["sprints_remaining"]
                if status["cards_done"] == 0:
                    headline = "Just one sprint today!" if n == 1 else \
                        f"{n} sprints today"
                else:
                    headline = "Last sprint of the day!" if n == 1 else \
                        f"{n} sprints to go today"
                self._center(self.font_big.render(headline, True, FG), 215)
                progress = (f"{status['cards_done']} / "
                            f"{status['goal_today']} cards done")
                if status["goal_today"] < status["goal"]:
                    # Supply-limited (e.g. a fresh deck paced by
                    # daily_new_cards): say why the day is short.
                    progress += " — more unlock tomorrow"
                self._center(self.font.render(progress, True, DIM), 265)
                minutes = self.profile["session_time_minutes"]
                sprint_line = (f"next sprint: "
                               f"{status['next_sprint_cards']} cards"
                               + (f" / about {minutes} min" if minutes
                                  else ""))
                self._center(self.font_small.render(sprint_line, True, DIM),
                             305)
                self._center(self.font.render(
                    "Press the bottom button to start!", True, GOOD), 360)
            elif bonus:
                headline = "Goal reached! Great job!" if status["goal_met"] \
                    else "That's everything for today!"
                self._center(self.font_big.render(headline, True, GOOD),
                             215)
                self._center(self.font.render(
                    f"{status['cards_done']} cards done today", True, DIM),
                    265)
                self._center(self.font_small.render(
                    f"Spare time? A bonus sprint of "
                    f"{status['bonus_cards']} cards is ready.", True, DIM),
                    305)
                self._center(self.font.render(
                    "Bottom button = bonus sprint (totally optional!)",
                    True, GOOD), 360)
            else:
                self._center(self.font_big.render("All done for today!",
                                                  True, GOOD), 230)
                self._center(self.font.render("Come back tomorrow.", True,
                                              DIM), 285)
            self._footer(
                "Bottom = Start    Top = Calendar" if (startable or bonus)
                else "Top = Calendar",
                "SELECT = Parent    START = Quit")
            self.present()
            self.clock.tick(FPS)

    def screen_deck_select(self):
        """Child-facing deck picker: shown only when the parent has
        assigned more than one deck (screen_child_start skips straight
        to REVIEW otherwise, keeping the fast path fast). Lets the
        child choose one specific assigned deck, or all of them
        combined, for just this sitting."""
        available = self._resolve_available_decks()
        entries = [("All assigned decks", None)] + \
            [(name, [name]) for name in available]
        # Due/new counts computed once up front, not per frame — this
        # screen redraws every tick like the other menus, and querying
        # the DB per entry per frame would not be free on a big deck.
        counts = [self.service.counts_for_queue(profile=self.profile,
                                                deck_filter=decks,
                                                bonus=self._session_bonus)
                 for _label, decks in entries]
        index = 0

        while True:
            action = self.poll()
            if action in ("start", "east_button", "select"):
                return "CHILD_START"
            if action == "dpad_up":
                index = (index - 1) % len(entries)
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
            elif action == "south_button":
                self._session_deck_filter = entries[index][1]
                return "REVIEW"

            self.screen.fill(BG)
            self._center(self.font_big.render("Choose a Deck", True, FG),
                         40)
            self._center(self.font_small.render(
                "Which deck do you want to study?", True, DIM), 90)
            y = 140
            for i, (label, _decks) in enumerate(entries):
                due, new = counts[i]
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                text = f"{prefix}{label}   ({due + new} due)"
                self.screen.blit(self.font.render(text, True, color),
                                (60, y))
                y += 44
            self._footer("Up/Down = Choose   Bottom = Select   "
                         "Right = Back")
            self.present()
            self.clock.tick(FPS)

    # -- review ----------------------------------------------------------------------

    MENU_ENTRIES = ("Undo last answer", "Bury card (back tomorrow)",
                    "Suspend card (parent will check)", "End session",
                    "Cancel")

    # Four-phase vocab cards (word -> example -> image -> definition):
    # the phase reached when "I know this" is pressed *is* the rating.
    # No separate Again/Hard/Good/Easy buttons for these cards.
    VOCAB_MAX_PHASE = 3
    VOCAB_PHASE_RATING = {0: 4, 1: 3, 2: 2, 3: 1}  # Easy, Good, Hard, Again

    def screen_review(self):
        if self._session_bonus:
            self._sprint_label = "Bonus sprint"
        else:
            status = self.service.sprint_status(profile=self.profile)
            number = min(status["sprints_planned"] -
                         status["sprints_remaining"] + 1,
                         status["sprints_planned"])
            self._sprint_label = f"Sprint {number}/{status['sprints_planned']}"
        session = StudySession(self.storage, self.service, self.profile,
                               deck_filter=self._session_deck_filter,
                               bonus=self._session_bonus)
        log.info("session %d started: %d cards queued (%s, deck filter: %s)",
                 session.session_id, session.planned_total,
                 self._sprint_label, self._session_deck_filter)
        try:
            return self._review_loop(session)
        finally:
            if not session.finished:
                session.finish()  # force exit / window close: still stamped
            log.info("session %d finished: %s", session.session_id,
                     {k: v for k, v in session.summary().items()
                      if not k.startswith("avg")})

    def _review_loop(self, session):
        reversed_mode = self.profile.get("study_direction") == "reversed"
        auto_play = bool(self.settings.get("auto_play_audio", True))

        flipped = False
        phase = 0            # vocab cards only: 0..VOCAB_MAX_PHASE
        vocab_detail = None  # vocab cards only: the vocab_cards row
        shown_at = None
        audio_status = None
        menu = None  # action-menu overlay index, or None when closed
        needs_draw = True
        heartbeat = 0

        def play_vocab():
            """Replay whatever audio belongs to the current phase: the
            word at phase 0, the example sentence from phase 1 on."""
            nonlocal audio_status
            filename = card["audio_filename"] if phase == 0 else (
                vocab_detail["example_audio"] if vocab_detail else None)
            if not filename:
                return
            audio_status = "playing" if self.audio.play(filename) \
                else "missing"

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
                card["audio_filename"] if forced else None)
            if not filename:
                return
            audio_status = "playing" if self.audio.play(filename) \
                else "missing"

        def begin_card(card):
            nonlocal flipped, phase, vocab_detail, shown_at, audio_status
            flipped = False
            phase = 0
            vocab_detail = self.storage.get_vocab_detail(card["id"]) \
                if card["card_type"] == "vocab" else None
            shown_at = self.service.now()
            audio_status = None
            if card["audio_filename"] and not self.audio.available(
                    card["audio_filename"]):
                audio_status = "missing"
            if auto_play:
                play_vocab() if vocab_detail is not None else \
                    play(card, "front")

        def advance():
            """Fetch the next card after the current one was consumed.

            The optional time limit is checked here, between cards, so
            a child is never cut off while thinking about an answer.
            """
            nonlocal card
            self.audio.stop()
            card = None if session.time_limit_reached() \
                else session.current_card()
            if card:
                begin_card(card)

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
            if menu is not None:
                if action == "dpad_up":
                    menu = (menu - 1) % len(self.MENU_ENTRIES)
                elif action == "dpad_down":
                    menu = (menu + 1) % len(self.MENU_ENTRIES)
                elif action in ("east_button", "select", "start"):
                    menu = None
                elif action == "south_button":
                    choice, menu = self.MENU_ENTRIES[menu], None
                    if choice.startswith("Undo"):
                        restored = session.undo()
                        if restored is not None:
                            card = restored
                            begin_card(card)
                    elif choice.startswith("Bury"):
                        session.bury_current()
                        advance()
                    elif choice.startswith("Suspend"):
                        session.suspend_current()
                        advance()
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
                restored = session.undo()
                if restored is not None:
                    card = restored
                    begin_card(card)
            elif vocab_detail is not None:
                if action in DPAD:
                    if phase < self.VOCAB_MAX_PHASE:
                        phase += 1
                        if auto_play:
                            play_vocab()
                elif action == "south_button":
                    elapsed = int((self.service.now() -
                                   shown_at).total_seconds() * 1000)
                    session.answer(self.VOCAB_PHASE_RATING[phase],
                                   elapsed_ms=elapsed)
                    advance()
                    continue
                elif action == "r1":
                    session.bury_current()
                    advance()
                    continue
            elif not flipped:
                if action in DPAD or action in ("south_button", "unmapped"):
                    flipped = True
                    if auto_play:
                        play(card, "back")
            elif action in RATING_FOR_SEMANTIC:
                elapsed = int((self.service.now() -
                               shown_at).total_seconds() * 1000)
                session.answer(RATING_FOR_SEMANTIC[action],
                               elapsed_ms=elapsed)
                advance()
                continue
            elif action == "r1":
                session.bury_current()
                advance()
                continue

            # The screen is static between inputs: redraw only when
            # something happened (plus a 1 s heartbeat as a safety net).
            heartbeat += 1
            if needs_draw or heartbeat >= FPS:
                self._draw_review(session, card, flipped, audio_status,
                                  menu, phase=phase, vocab=vocab_detail)
                needs_draw = False
                heartbeat = 0
            self.clock.tick(FPS)

    def _draw_review(self, session, card, flipped, audio_status, menu,
                     phase=0, vocab=None):
        self.screen.fill(BG)
        header = card["deck"]
        if card["tags"]:
            header += "  ·  " + " ".join(card["tags"].split()[:3])
        self.screen.blit(self.font_small.render(header, True, DIM), (16, 12))
        left = f"{self._sprint_label}  ·  {session.remaining()} left" \
            if self._sprint_label else f"{session.remaining()} left"
        surf = self.font_small.render(left, True, DIM)
        self.screen.blit(surf, (self.w - surf.get_width() - 16, 12))
        pygame.draw.line(self.screen, DIVIDER, (16, 40), (self.w - 16, 40))

        if vocab is not None:
            self._draw_vocab_phases(card, vocab, phase, audio_status,
                                    top=60)
            if phase < self.VOCAB_MAX_PHASE:
                self._footer("D-pad = Reveal more   Bottom = I know this",
                             "L1 = Replay audio   R1 = Bury   "
                             "SELECT = Menu")
            else:
                self._footer("Bottom = I know this",
                             "L1 = Replay audio   R1 = Bury   "
                             "SELECT = Menu   START = Finish")
            if menu is not None:
                self._draw_menu_overlay(menu)
            self.present()
            return

        reversed_mode = self.profile.get("study_direction") == "reversed"
        front, back = (card["back"], card["front"]) if reversed_mode \
            else (card["front"], card["back"])

        margin = 36
        max_width = self.w - 2 * margin
        y = self._block(front, self.font_big, FG, top=64,
                        max_width=max_width)

        if audio_status == "missing":
            self._center(self.font_small.render("(no audio)", True, WARN),
                         y + 6)
        elif card["audio_filename"]:
            self._center(self.font_small.render("♪  L1 = replay", True,
                                                DIM), y + 6)

        if flipped:
            div_y = max(y + 34, 210)
            pygame.draw.line(self.screen, DIVIDER, (margin, div_y),
                             (self.w - margin, div_y))
            self._block(back, self.font, ACCENT, top=div_y + 18,
                        max_width=max_width)
            self._footer("Bottom=Good  Right=Again  Left=Easy  Top=Hard",
                         "R1=Bury   SELECT=Menu   START=Finish")
        else:
            self._footer("D-pad = Show answer   L1 = Replay audio   "
                         "START = Finish")

        if menu is not None:
            self._draw_menu_overlay(menu)
        self.present()

    def _draw_vocab_phases(self, card, vocab, phase, audio_status, top):
        """Phase 0: word. 1: +example (headword highlighted). 2: +image.
        3: +gendered forms/definitions/translation. Each phase's content
        stays on screen as later phases are revealed underneath it."""
        margin = 36
        max_width = self.w - 2 * margin
        y = self._block(vocab["word"], self.font_big, FG, top=top,
                        max_width=max_width)

        if card["audio_filename"]:
            hint, color = (("(no audio)", WARN) if audio_status == "missing"
                          else ("♪  L1 = replay", DIM))
            self._center(self.font_small.render(hint, True, color), y + 4)
        y += 26

        if phase >= 1:
            pygame.draw.line(self.screen, DIVIDER, (margin, y),
                             (self.w - margin, y))
            y += 14
            y = self._draw_highlighted_block(
                vocab["example_es"] or "(no example sentence)",
                vocab["word"], self.font, top=y, max_width=max_width)
            y += 8

        if phase >= 2:
            image = self._vocab_image_surface(vocab["image_filename"])
            if image is not None:
                rect = image.get_rect()
                rect.centerx = self.w // 2
                rect.top = y
                self.screen.blit(image, rect)
                y = rect.bottom + 10
            else:
                self._center(self.font_small.render("(no image)", True,
                                                    DIM), y)
                y += 26

        if phase >= 3:
            pygame.draw.line(self.screen, DIVIDER, (margin, y),
                             (self.w - margin, y))
            y += 12
            text = vocab["definitions"] or "(no definition)"
            if vocab["gendered_forms"]:
                text = vocab["gendered_forms"] + "\n" + text
            if vocab["example_en"]:
                text += "\n" + vocab["example_en"]
            self._block(text, self.font_small, ACCENT, top=y,
                       max_width=max_width)

    def _vocab_image_surface(self, filename, max_height=140):
        """Loaded + scaled image surface, cached by filename so it is
        decoded once per card rather than every frame."""
        if not filename:
            return None
        key = (filename, max_height)
        if key in self._image_cache:
            return self._image_cache[key]
        surf = None
        path = os.path.join(self.audio.media_dir, os.path.basename(filename))
        if os.path.exists(path):
            try:
                raw = pygame.image.load(path)
                try:
                    raw = raw.convert_alpha()
                except pygame.error:
                    raw = raw.convert()
                if raw.get_height() > max_height:
                    scale = max_height / raw.get_height()
                    raw = pygame.transform.smoothscale(
                        raw, (max(1, int(raw.get_width() * scale)),
                             max_height))
                surf = raw
            except pygame.error as exc:
                log.warning("could not load image %s: %s", path, exc)
        else:
            log.warning("missing media file: %s", path)
        self._image_cache[key] = surf
        return surf

    def _draw_highlighted_block(self, text, word, font, top, max_width):
        """Like _block, but the headword is rendered in ACCENT wherever
        it occurs (case-insensitively) — a plain-text stand-in for the
        original card's HTML/CSS yellow-highlight span."""
        word_lower = (word or "").lower()
        y = top
        for line in wrap_text(font, text, max_width):
            if y > self.h - 80:
                break
            idx = line.lower().find(word_lower) if word_lower else -1
            if idx == -1:
                surf = font.render(line, True, FG)
                self.screen.blit(surf, ((self.w - surf.get_width()) // 2, y))
            else:
                self._blit_split_highlight(line, idx, len(word_lower),
                                           font, y)
            y += font.get_linesize()
        return y

    def _blit_split_highlight(self, line, start, length, font, y):
        pre, match, post = (line[:start], line[start:start + length],
                            line[start + length:])
        surf_pre = font.render(pre, True, FG)
        surf_match = font.render(match, True, ACCENT)
        surf_post = font.render(post, True, FG)
        total_w = (surf_pre.get_width() + surf_match.get_width() +
                  surf_post.get_width())
        x = (self.w - total_w) // 2
        for surf in (surf_pre, surf_match, surf_post):
            self.screen.blit(surf, (x, y))
            x += surf.get_width()

    def _draw_menu_overlay(self, index):
        entries = self.MENU_ENTRIES
        box_w, line_h = 440, 42
        box_h = line_h * len(entries) + 32
        x = (self.w - box_w) // 2
        y = (self.h - box_h) // 2
        pygame.draw.rect(self.screen, OVERLAY_BG, (x, y, box_w, box_h),
                         border_radius=8)
        pygame.draw.rect(self.screen, DIVIDER, (x, y, box_w, box_h), 2,
                         border_radius=8)
        for i, entry in enumerate(entries):
            color = ACCENT if i == index else FG
            prefix = "> " if i == index else "   "
            surf = self.font.render(prefix + entry, True, color)
            self.screen.blit(surf, (x + 24, y + 16 + i * line_h))

    # -- summary ---------------------------------------------------------------------

    def screen_summary(self):
        s = getattr(self, "_summary", None) or {}
        minutes = int(s.get("time_spent_seconds", 0) // 60)
        seconds = int(s.get("time_spent_seconds", 0) % 60)
        pass_rate = s.get("pass_rate")
        avg = s.get("avg_response_ms")

        # Where the day stands now that this sprint is in the log; the
        # deck filter is kept so "next sprint now" continues the same
        # deck the child picked for this sitting.
        status = self.service.sprint_status(
            profile=self.profile, deck_filter=self._session_deck_filter)
        more = status["next_sprint_cards"] > 0
        bonus = not more and status["bonus_cards"] > 0

        if more:
            n = status["sprints_remaining"]
            headline = "¡Buen trabajo!"
            subtitle = "SPRINT DONE — LAST ONE TO GO" if n == 1 else \
                f"SPRINT DONE — {n} TO GO TODAY"
        elif status["goal_met"]:
            headline = "Goal reached! Great job!"
            subtitle = f"{status['cards_done']} CARDS TODAY"
        else:
            headline = "That's everything for today!"
            subtitle = f"{status['cards_done']} CARDS TODAY"

        lines = [
            (f"Cards answered:  {s.get('cards_reviewed', 0)}", FG),
            (f"New cards:  {s.get('new_cards', 0)}    "
             f"Reviews:  {s.get('review_cards', 0)}", FG),
            (f"Again {s.get('again_count', 0)}   "
             f"Hard {s.get('hard_count', 0)}   "
             f"Good {s.get('good_count', 0)}   "
             f"Easy {s.get('easy_count', 0)}", DIM),
            (f"Pass rate:  {pass_rate:.0%}" if pass_rate is not None
             else "Pass rate:  —", DIM),
            (f"Time:  {minutes}m {seconds:02d}s" +
             (f"    Avg answer:  {avg / 1000:.1f}s" if avg else ""), DIM),
            (f"Today:  {status['cards_done']} / {status['goal_today']} "
             f"cards", DIM),
        ]
        if s.get("buried_count") or s.get("suspended_count"):
            lines.append((f"Buried {s.get('buried_count', 0)}   "
                          f"Suspended {s.get('suspended_count', 0)}", DIM))

        if more:
            go_label = "Bottom = Next sprint now    Right = Done for now"
        elif bonus:
            go_label = "Bottom = Bonus sprint    Right = Done for now"
        else:
            go_label = "Bottom = Done"

        while True:
            action = self.poll()
            if action == "start":
                return "QUIT"
            if action == "north_button":
                self._calendar_return = "CHILD_START"
                return "CALENDAR"
            if action in ("south_button", "unmapped"):
                # "Going ahead": roll straight into the next sprint —
                # spare time now means fewer sprints owed later.
                if more or bonus:
                    self._session_bonus = bonus
                    return "REVIEW"
                return "CHILD_START"
            if action in ("east_button", "select"):
                return "CHILD_START"

            self.screen.fill(BG)
            self._center(self.font_big.render(headline, True, GOOD), 56)
            self._center(self.font_small.render(subtitle, True, DIM), 108)
            y = 160
            for text, color in lines:
                self._center(self.font.render(text, True, color), y)
                y += 44
            self._footer(go_label, "Top = Calendar   START = Quit")
            self.present()
            self.clock.tick(FPS)

    # -- stamp calendar ---------------------------------------------------------------

    STAMP_COLORS = ((236, 108, 96), (120, 180, 250), (120, 210, 140),
                    (240, 180, 100), (200, 140, 230))

    def screen_calendar(self):
        """Brain Age-style stamp calendar: one stamp per day the child
        studied, with the number of sessions logged that day inside it
        (they can study repeatedly). Read-only; reachable from the child
        start screen (Top), the session summary (Top), and Parent Mode.
        Shows the current profile's sessions, one month at a time."""
        today = self.service.now().astimezone().date()
        year, month = today.year, today.month

        def load(y, m):
            return self.service.sessions_per_day(self.profile["id"], y, m)

        counts = load(year, month)

        def step_month(delta):
            nonlocal year, month, counts
            m0 = (year * 12 + (month - 1)) + delta
            year, month = m0 // 12, m0 % 12 + 1
            counts = load(year, month)

        while True:
            action = self.poll()
            if action in ("start", "select", "east_button"):
                return self._calendar_return
            if action in ("dpad_left", "l1"):
                step_month(-1)
            elif action in ("dpad_right", "r1"):
                step_month(1)
            elif action == "south_button":
                year, month = today.year, today.month
                counts = load(year, month)

            self._draw_calendar(year, month, counts, today)
            self.present()
            self.clock.tick(FPS)

    def _draw_calendar(self, year, month, counts, today):
        self.screen.fill(BG)
        # Title: ◀  MONTH YEAR  ▶
        title = f"◀   {_calendar.month_name[month].upper()} {year}   " \
                f"▶"
        self._center(self.font_big.render(title, True, FG), 14)
        total = sum(counts.values())
        active = len(counts)
        subtitle = (f"{total} session{'s' if total != 1 else ''} on "
                    f"{active} day{'s' if active != 1 else ''}"
                    if total else "No sessions this month yet")
        self._center(self.font_small.render(subtitle, True, DIM), 62)

        margin = 16
        grid_w = self.w - 2 * margin
        cell_w = grid_w / 7
        grid_top = 106
        cell_h = 49

        weekdays = ("SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT")
        for col, name in enumerate(weekdays):
            surf = self.font_small.render(name, True, DIM)
            cx = margin + col * cell_w + (cell_w - surf.get_width()) / 2
            self.screen.blit(surf, (int(cx), grid_top - 22))

        weeks = _calendar.Calendar(firstweekday=6).monthdayscalendar(
            year, month)
        for row, week in enumerate(weeks):
            for col, day in enumerate(week):
                if day == 0:
                    continue
                self._draw_day_cell(
                    day, counts.get(day, 0),
                    is_today=(year == today.year and month == today.month
                              and day == today.day),
                    x=int(margin + col * cell_w), y=grid_top + row * cell_h,
                    w=int(cell_w), h=cell_h)

        self._footer("L1/R1 = Month    Bottom = Today",
                     "SELECT = Back")

    def _draw_day_cell(self, day, count, is_today, x, y, w, h):
        if is_today:
            pygame.draw.rect(self.screen, ACCENT, (x + 2, y, w - 4, h - 2),
                             2, border_radius=6)
        # Day number, top-left corner.
        num = self.font_small.render(str(day), True,
                                     FG if count else DIM)
        self.screen.blit(num, (x + 6, y + 3))
        if count <= 0:
            return
        # Stamp: a filled circle with the session count inside it. Colour
        # varies with the count so busier days pop a little.
        color = self.STAMP_COLORS[min(count - 1, len(self.STAMP_COLORS) - 1)]
        cx, cy = x + w // 2, y + h // 2 + 6
        radius = min(h // 2 - 4, 18)
        pygame.draw.circle(self.screen, color, (cx, cy), radius)
        pygame.draw.circle(self.screen, BG, (cx, cy), radius, 2)
        label = self.font.render(str(count), True, BG)
        self.screen.blit(label, (cx - label.get_width() // 2,
                                 cy - label.get_height() // 2))

    # -- parent mode -----------------------------------------------------------------

    def screen_parent_menu(self):
        direction = self.profile.get("study_direction", "normal")
        entries = [
            ("Import deck (.apkg)", "PARENT_IMPORT"),
            ("Decks", "PARENT_DECKS"),
            ("Categories", "PARENT_CATEGORIES"),
            ("Daily goal & sprints", "PARENT_LIMITS"),
            ("Suspended cards", "PARENT_SUSPENDED"),
            ("Progress", "PARENT_PROGRESS"),
            ("Calendar (stamps)", "CALENDAR"),
            ("Controller test & setup", "INPUT_DIAG"),
            (f"Direction: "
             f"{'Reversed (back first)' if direction == 'reversed' else 'Normal (front first)'}",
             "TOGGLE_DIRECTION"),
            ("Back to study", "CHILD_START"),
        ]
        index = 0
        notice = None
        if self._card_count() == 0:
            notice = "No decks yet — copy an .apkg to the data folder " \
                     "and use Import."
        while True:
            action = self.poll()
            if action in ("start", "east_button", "select"):
                return "CHILD_START"
            if action == "dpad_up":
                index = (index - 1) % len(entries)
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
            elif action == "south_button":
                target = entries[index][1]
                if target == "TOGGLE_DIRECTION":
                    new = "normal" if direction == "reversed" else "reversed"
                    self.storage.update_profile(self.profile["id"],
                                                study_direction=new)
                    self._reload_profile()
                    return "PARENT_MENU"
                if target == "CALENDAR":
                    self._calendar_return = "PARENT_MENU"
                return target

            self.screen.fill(BG)
            self._center(self.font_big.render("Parent Mode", True, FG), 40)
            self._center(self.font_small.render(
                f"Profile: {self.profile['name']}", True, DIM), 90)
            y = 124
            for i, (label, _) in enumerate(entries):
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(prefix + label, True,
                                                  color), (90, y))
                y += 40
            if notice:
                for line in wrap_text(self.font_small, notice, self.w - 80):
                    self._center(self.font_small.render(line, True, WARN),
                                 y + 6)
                    y += 22
            self._footer("Up/Down = Choose   Bottom = Select   "
                         "Right = Back")
            self.present()
            self.clock.tick(FPS)

    def settings_dir(self):
        return os.path.dirname(os.path.abspath(self.settings.path))

    def _scan_apkg_files(self):
        """Places a parent might drop an .apkg: the data dir, an
        import/ subfolder of it, the app folder, and the cwd."""
        roots = [self.settings_dir(),
                 os.path.join(self.settings_dir(), "import"),
                 os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                 os.getcwd()]
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
            if action in ("start", "east_button", "select"):
                return "PARENT_MENU"
            if files and action == "dpad_up":
                index = (index - 1) % len(files)
            elif files and action == "dpad_down":
                index = (index + 1) % len(files)
            elif files and action == "south_button":
                self._draw_import(files, index, "Importing…")
                self.present()
                try:
                    stats = import_apkg(
                        files[index], self.storage,
                        self.service.scheduler,
                        os.path.join(self.settings_dir(), "media"))
                    message = stats.summary()
                    log.info("import %s: %s", files[index], message)
                except (ApkgError, OSError) as exc:
                    message = f"Import failed: {exc}"
                    log.error("import %s failed: %s", files[index], exc)

            self._draw_import(files, index, message)
            self.present()
            self.clock.tick(FPS)

    def _draw_import(self, files, index, message):
        self.screen.fill(BG)
        self._center(self.font_big.render("Import Deck", True, FG), 40)
        if not files:
            self._center(self.font.render("No .apkg files found.", True,
                                          DIM), 160)
            self._center(self.font_small.render(
                "Copy a deck into the data/ folder on the SD card.",
                True, DIM), 210)
        else:
            y = 110
            for i, path in enumerate(files[:7]):
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(
                    prefix + os.path.basename(path), True, color), (70, y))
                y += 38
        if message:
            y = self.h - 160
            for line in wrap_text(self.font_small, message, self.w - 80):
                self._center(self.font_small.render(line, True, WARN), y)
                y += 24
        self._footer("Bottom = Import   Right = Back")

    def screen_parent_categories(self):
        return self._screen_multi_select(
            title="Categories", subtitle="Choose what the child studies",
            all_label="[ All categories ]", items=self.storage.all_tags(),
            profile_field="active_categories",
            empty_message="No tags found — import a deck first.")

    def screen_parent_decks(self):
        return self._screen_multi_select(
            title="Decks", subtitle="Choose which decks the child studies",
            all_label="[ All decks ]", items=self.storage.deck_names_list(),
            profile_field="active_decks",
            empty_message="No decks found — import one first.")

    def _screen_multi_select(self, title, subtitle, all_label, items,
                             profile_field, empty_message):
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
                **{profile_field: (None if selected is None
                                  else sorted(selected))})
            self._reload_profile()

        while True:
            action = self.poll()
            if action in ("start", "east_button", "select"):
                save()
                return "PARENT_MENU"
            if action == "dpad_up":
                index = (index - 1) % len(entries)
            elif action == "dpad_down":
                index = (index + 1) % len(entries)
            elif action == "south_button":
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

            self.screen.fill(BG)
            self._center(self.font_big.render(title, True, FG), 36)
            self._center(self.font_small.render(subtitle, True, DIM), 86)
            y = 120
            for i in range(top, min(top + visible, len(entries))):
                label = entries[i]
                if i == 0:
                    mark = "(x)" if selected is None else "( )"
                else:
                    on = selected is None or label in selected
                    mark = "[x]" if on else "[ ]"
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(
                    f"{prefix}{mark} {label}", True, color), (80, y))
                y += 40
            if not items:
                self._center(self.font.render(empty_message, True, DIM),
                            200)
            self._footer("Bottom = Toggle   Right = Save & back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_limits(self):
        # The goal/sprint pair defines the child's day: goal cards split
        # into sprints of session_card_limit. daily_new_cards paces new
        # material; study-ahead settings are CLI-only for now.
        fields = [
            ("Daily goal (cards)", "daily_goal_cards", 10, 500),
            ("New cards per day", "daily_new_cards", 0, 100),
            ("Cards per sprint", "session_card_limit", 1, 200),
            ("Minutes per sprint (0 = off)", "session_time_minutes", 0, 90),
        ]
        values = {key: self.profile[key] for _, key, _, _ in fields}
        index = 0
        while True:
            action = self.poll()
            if action in ("start", "east_button", "select"):
                self.storage.update_profile(self.profile["id"], **values)
                self._reload_profile()
                return "PARENT_MENU"
            if action == "dpad_up":
                index = (index - 1) % len(fields)
            elif action == "dpad_down":
                index = (index + 1) % len(fields)
            elif action in ("dpad_left", "dpad_right", "l1", "r1"):
                label, key, lo, hi = fields[index]
                step = {"dpad_left": -1, "dpad_right": 1,
                        "l1": -10, "r1": 10}[action]
                values[key] = min(max(values[key] + step, lo), hi)

            self.screen.fill(BG)
            self._center(self.font_big.render("Daily Goal & Sprints", True,
                                              FG), 40)
            sprint_size = max(values["session_card_limit"], 1)
            sprints = -(-values["daily_goal_cards"] // sprint_size)
            self._center(self.font_small.render(
                f"= {sprints} sprint{'s' if sprints != 1 else ''} a day",
                True, DIM), 92)
            y = 140
            for i, (label, key, _, _) in enumerate(fields):
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(prefix + label, True,
                                                  color), (70, y))
                value = self.font.render(str(values[key]), True, color)
                self.screen.blit(value, (self.w - 120, y))
                y += 52
            self._footer("Left/Right = Adjust   L1/R1 = ±10   "
                         "Right btn = Save & back")
            self.present()
            self.clock.tick(FPS)

    def screen_parent_suspended(self):
        index, top, visible = 0, 0, 7
        cards = self.storage.suspended_cards()
        while True:
            action = self.poll()
            if action in ("start", "east_button", "select"):
                return "PARENT_MENU"
            if cards:
                if action == "dpad_up":
                    index = (index - 1) % len(cards)
                elif action == "dpad_down":
                    index = (index + 1) % len(cards)
                elif action == "south_button":
                    self.service.unsuspend_card(cards[index]["id"])
                    cards = self.storage.suspended_cards()
                    index = max(index - 1, 0)
            index = min(index, max(len(cards) - 1, 0))
            top = min(max(top, index - visible + 1), index) if cards else 0

            self.screen.fill(BG)
            self._center(self.font_big.render("Suspended Cards", True, FG),
                         36)
            if not cards:
                self._center(self.font.render("No suspended cards.", True,
                                              DIM), 200)
            y = 110
            for i in range(top, min(top + visible, len(cards))):
                card = cards[i]
                text = card["front"].replace("\n", " ")
                if len(text) > 38:
                    text = text[:37] + "…"
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(prefix + text, True,
                                                  color), (60, y))
                y += 40
            self._footer("Bottom = Unsuspend   Right = Back")
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

            self.screen.fill(BG)
            self._center(self.font_big.render("Progress", True, FG), 36)
            self._center(self.font_small.render(
                f"{self.profile['name']}  ·  {total_cards} cards  ·  "
                f"{suspended} suspended", True, DIM), 88)
            y = 130
            header = self.font_small.render(
                "Day            Reviews    New", True, DIM)
            self.screen.blit(header, (150, y))
            y += 30
            for day, reviews, new in totals:
                label = day.strftime("%a %b %d")
                row = f"{label:<14} {reviews:>7} {new:>6}"
                self.screen.blit(self.font.render(row, True, FG), (150, y))
                y += 38
            self._footer("Right = Back")
            self.present()
            self.clock.tick(FPS)

    # -- input diagnostic / calibration ------------------------------------------------

    def screen_input_diagnostic(self):
        """Raw-event viewer: shows what the device actually sends and
        how the current mapping interprets it. Hold any single gamepad
        button ~3 s (or press C on a keyboard) to start calibration —
        that works even when the current mapping is completely wrong."""
        events = []          # newest-first (raw text, semantic, study)
        held_since = {}      # raw button index -> monotonic time
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
                    pygame.joystick.Joystick(event.device_index).init()
                    raw = "joystick connected"
                if raw is not None:
                    semantic = self.input.translate(event)
                    study = STUDY_ACTIONS.get(semantic, "—")
                    events.insert(0, (raw, semantic or "—", study))
                    del events[10:]
                    if semantic == "start":
                        exit_screen = True
                event = pygame.event.poll()

            now = time.monotonic()
            if any(now - t >= HOLD_SECONDS for t in held_since.values()):
                calibrate = True
            if calibrate:
                return "CALIBRATE"
            if exit_screen:
                return "PARENT_MENU" if self.initial_state != "INPUT_DIAG" \
                    else "QUIT"

            self.screen.fill(BG)
            self._center(self.font_big.render("Controller Test", True, FG),
                         24)
            n_joy = pygame.joystick.get_count()
            names = ", ".join(
                pygame.joystick.Joystick(i).get_name()
                for i in range(n_joy)) or "none detected"
            self._center(self.font_small.render(
                f"Joysticks: {n_joy} ({names})", True, DIM), 74)
            y = 108
            header = self.font_small.render(
                f"{'raw event':<26}{'semantic':<16}study action", True, DIM)
            self.screen.blit(header, (48, y))
            y += 26
            for raw, semantic, study in events:
                line = f"{raw:<26}{semantic:<16}{study}"
                self.screen.blit(self.font_small.render(line, True, FG),
                                 (48, y))
                y += 24
            if not events:
                self._center(self.font.render(
                    "Press buttons to see events…", True, DIM), 220)
            self._footer("Hold any button 3s = remap buttons",
                         "START (mapped) or Esc = back")
            self.present()
            self.clock.tick(FPS)

    CALIBRATION_STEPS = (
        ("south_button", "the BOTTOM face button"),
        ("east_button", "the RIGHT face button"),
        ("west_button", "the LEFT face button"),
        ("north_button", "the TOP face button"),
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
                    pygame.K_ESCAPE, pygame.K_q):
                log.info("calibration cancelled")
                return "INPUT_DIAG"
            if event.type == pygame.JOYDEVICEADDED:
                pygame.joystick.Joystick(event.device_index).init()
            if event.type == pygame.JOYHATMOTION and \
                    event.value != (0, 0):
                if semantic.startswith("dpad_"):
                    dpad_is_hat = True
                    continue
                message = "That was the D-pad — press " + prompt
            if event.type == pygame.JOYBUTTONDOWN:
                if event.button in new_map.buttons:
                    already = new_map.buttons[event.button]
                    message = (f"Button {event.button} is already "
                               f"{already} — press a different one")
                else:
                    new_map.set_button(event.button, semantic)
                    log.info("calibrated %s = button %d", semantic,
                             event.button)
                    step += 1
                    message = None
                    continue

            self.screen.fill(BG)
            self._center(self.font_big.render("Controller Setup", True,
                                              FG), 40)
            self._center(self.font_small.render(
                f"Step {step + 1} of {len(steps)}", True, DIM), 100)
            self._center(self.font.render("Press " + prompt, True, ACCENT),
                         190)
            if message:
                self._center(self.font_small.render(message, True, WARN),
                             250)
            done = ", ".join(
                f"{FACE_LABELS.get(s, s)}={b}"
                for b, s in sorted(new_map.buttons.items()))
            if done:
                y = 300
                for line in wrap_text(self.font_small,
                                      "So far: " + done, self.w - 80):
                    self._center(self.font_small.render(line, True, DIM),
                                 y)
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

        self.screen.fill(BG)
        self._center(self.font_big.render("Controller Setup", True, FG),
                     40)
        self._center(self.font.render(message, True, GOOD), 200)
        self.present()
        pygame.time.wait(1200)
        return "INPUT_DIAG"

    # -- drawing helpers ----------------------------------------------------------------

    def _center(self, surf, y):
        self.screen.blit(surf, ((self.w - surf.get_width()) // 2, y))

    def _block(self, text, font, color, top, max_width):
        y = top
        for line in wrap_text(font, text, max_width):
            if y > self.h - 80:
                break  # clip very long cards; no scrolling
            surf = font.render(line, True, color)
            self.screen.blit(surf, ((self.w - surf.get_width()) // 2, y))
            y += font.get_linesize()
        return y

    def _footer(self, text, second_line=None):
        top = self.h - (64 if second_line else 44)
        pygame.draw.line(self.screen, DIVIDER, (16, top),
                         (self.w - 16, top))
        surf = self.font_small.render(text, True, DIM)
        self.screen.blit(surf, ((self.w - surf.get_width()) // 2,
                                top + 10))
        if second_line:
            surf2 = self.font_small.render(second_line, True, DIM)
            self.screen.blit(surf2, ((self.w - surf2.get_width()) // 2,
                                     top + 34))
