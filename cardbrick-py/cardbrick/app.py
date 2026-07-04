"""CardBrick-style study appliance UI.

A state-driven pygame app aimed at children doing a short daily Spanish
session on a handheld. Screens:

    ChildStart -> Review (question/answer) -> SessionSummary -> ChildStart
    ChildStart -> ParentMode (import / categories / limits / suspended /
                              progress) -> ChildStart

Controller-first. Default roles (per the product spec):

    D-pad          reveal answer
    A              Good        B  Again
    X              Easy        Y  Hard
    L              replay audio
    R              bury until tomorrow
    SELECT         action menu / parent mode
    START          finish session / back

Keyboard fallback for desktop testing: arrows reveal, 1/2/3/4 =
Again/Hard/Good/Easy (or literal A/B/X/Y keys), L replay, R bury,
U undo, Tab menu, Esc finish.

Everything renders on a fixed logical canvas (default 640x480, the
RG35XX SP panel); pygame.SCALED stretches it to whatever display the
device has.
"""

import os

import pygame

from .importer import ApkgError, import_apkg
from .scheduler import iso
from .session import StudySession
from .textutil import wrap_text

FPS = 30

BG = (24, 26, 30)
FG = (235, 235, 228)
DIM = (140, 142, 148)
ACCENT = (120, 180, 250)
GOOD = (120, 210, 140)
WARN = (240, 180, 100)
BAD = (235, 120, 120)
DIVIDER = (70, 72, 78)
OVERLAY_BG = (36, 39, 45)

# Physical gamepad button numbers vary between handhelds; remap without
# touching code via CARDBRICK_JOYMAP="A=1,B=0,X=3,Y=2,L=4,R=5,SELECT=6,START=7"
DEFAULT_JOYMAP = {0: "A", 1: "B", 2: "X", 3: "Y", 4: "L", 5: "R",
                  6: "SELECT", 7: "START"}

# Button role -> FSRS rating (spec: A=Good, B=Again, X=Easy, Y=Hard).
RATING_FOR_BUTTON = {"B": 1, "Y": 2, "A": 3, "X": 4}

KEYBOARD_MAP = {
    pygame.K_UP: "UP", pygame.K_DOWN: "DOWN",
    pygame.K_LEFT: "LEFT", pygame.K_RIGHT: "RIGHT",
    pygame.K_RETURN: "A", pygame.K_SPACE: "A",
    pygame.K_a: "A", pygame.K_b: "B",
    pygame.K_x: "X", pygame.K_y: "Y",
    pygame.K_1: "B", pygame.K_2: "Y",     # 1=Again 2=Hard
    pygame.K_3: "A", pygame.K_4: "X",     # 3=Good  4=Easy
    pygame.K_l: "L", pygame.K_r: "R",
    pygame.K_u: "UNDO",
    pygame.K_TAB: "SELECT",
    pygame.K_ESCAPE: "START", pygame.K_q: "START",
}

DPAD = ("UP", "DOWN", "LEFT", "RIGHT")

FONT_CANDIDATES = [
    os.environ.get("CARDBRICK_FONT", ""),
    os.path.join(os.path.dirname(__file__), "..", "assets", "fonts",
                 "main.ttf"),
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
]


def _load_font(size):
    for path in FONT_CANDIDATES:
        if path and os.path.exists(path):
            try:
                return pygame.font.Font(path, size)
            except pygame.error:
                continue
    return pygame.font.Font(None, size + 6)  # pygame default runs small


def _parse_joymap():
    raw = os.environ.get("CARDBRICK_JOYMAP")
    if not raw:
        return dict(DEFAULT_JOYMAP)
    joymap = {}
    for pair in raw.split(","):
        name, _, num = pair.strip().partition("=")
        if num.isdigit():
            joymap[int(num)] = name.strip().upper()
    return joymap or dict(DEFAULT_JOYMAP)


class QuitApp(Exception):
    """Raised when the window is closed; unwinds any screen loop."""


class CardBrickApp:
    def __init__(self, storage, service, audio, settings, fullscreen=None):
        self.storage = storage
        self.service = service
        self.audio = audio
        self.settings = settings
        self.joymap = _parse_joymap()

        pygame.init()
        width = int(settings.get("logical_width", 640))
        height = int(settings.get("logical_height", 480))
        self.w, self.h = width, height
        if fullscreen is None:
            fullscreen = bool(settings.get("fullscreen"))
        flags = pygame.SCALED | (pygame.FULLSCREEN if fullscreen else 0)
        self.screen = pygame.display.set_mode((width, height), flags)
        pygame.display.set_caption("CardBrick — Spanish Practice")
        pygame.mouse.set_visible(False)
        for i in range(pygame.joystick.get_count()):
            pygame.joystick.Joystick(i).init()

        self.font_big = _load_font(38)
        self.font = _load_font(26)
        self.font_small = _load_font(18)
        self.clock = pygame.time.Clock()

        # Crash recovery: close sessions that never got an end stamp.
        self.storage.close_dangling_sessions(iso(self.service.now()))
        self.profile = self._boot_profile()

    def _boot_profile(self):
        profile_id = self.settings.get("current_child_profile_id")
        profile = self.storage.get_profile(profile_id) if profile_id else None
        if profile is None:
            profile = self.storage.ensure_default_profile()
            self.settings.set("current_child_profile_id", profile["id"])
        return profile

    def _reload_profile(self):
        self.profile = self.storage.get_profile(self.profile["id"])

    # -- main state machine ------------------------------------------------------

    def run(self):
        state = "CHILD_START"
        handlers = {
            "CHILD_START": self.screen_child_start,
            "REVIEW": self.screen_review,
            "SUMMARY": self.screen_summary,
            "PARENT_MENU": self.screen_parent_menu,
            "PARENT_IMPORT": self.screen_parent_import,
            "PARENT_CATEGORIES": self.screen_parent_categories,
            "PARENT_LIMITS": self.screen_parent_limits,
            "PARENT_SUSPENDED": self.screen_parent_suspended,
            "PARENT_PROGRESS": self.screen_parent_progress,
        }
        try:
            while state != "QUIT":
                state = handlers[state]()
        except QuitApp:
            pass
        finally:
            pygame.quit()

    # -- input ---------------------------------------------------------------------

    def poll(self):
        """Next logical button press, or None.

        Consumes exactly one meaningful event per call so rapid inputs
        queued between frames are never dropped.
        """
        while True:
            event = pygame.event.poll()
            if event.type == pygame.NOEVENT:
                return None
            if event.type == pygame.QUIT:
                raise QuitApp
            if event.type == pygame.KEYDOWN:
                action = KEYBOARD_MAP.get(event.key)
                if action:
                    return action
            elif event.type == pygame.JOYBUTTONDOWN:
                return self.joymap.get(event.button, "OTHER")
            elif event.type == pygame.JOYHATMOTION:
                x, y = event.value
                if y == 1:
                    return "UP"
                if y == -1:
                    return "DOWN"
                if x == -1:
                    return "LEFT"
                if x == 1:
                    return "RIGHT"

    # -- child start -----------------------------------------------------------------

    def screen_child_start(self):
        review_n, new_n = self.service.counts_for_queue(profile=self.profile)
        total = review_n + new_n
        categories = self.profile["active_categories"]
        cat_label = "All categories" if categories is None else \
            ", ".join(categories) if categories else "No categories!"

        while True:
            action = self.poll()
            if action == "START":
                return "QUIT"
            if action == "SELECT":
                return "PARENT_MENU"
            if action in ("A", "OTHER") and total > 0:
                return "REVIEW"

            self.screen.fill(BG)
            self._center(self.font_small.render("SPANISH PRACTICE", True,
                                                DIM), 36)
            self._center(self.font_big.render(self.profile["name"], True,
                                              FG), 80)
            self._center(self.font.render(cat_label, True, ACCENT), 150)
            if total > 0:
                due_text = f"{total} cards today"
                detail = f"{review_n} to review  +  {new_n} new"
                self._center(self.font_big.render(due_text, True, FG), 215)
                self._center(self.font.render(detail, True, DIM), 265)
                limit_line = (f"up to {self.profile['session_card_limit']} "
                              f"cards / "
                              f"{self.profile['session_time_minutes']} min")
                self._center(self.font_small.render(limit_line, True, DIM),
                             305)
                self._center(self.font.render("Press A to start!", True,
                                              GOOD), 360)
            else:
                self._center(self.font_big.render("All done for today!",
                                                  True, GOOD), 230)
                self._center(self.font.render("Come back tomorrow.", True,
                                              DIM), 285)
            self._footer("A = Start   SELECT = Parent mode   START = Quit"
                         if total else
                         "SELECT = Parent mode   START = Quit")
            pygame.display.flip()
            self.clock.tick(FPS)

    # -- review ----------------------------------------------------------------------

    MENU_ENTRIES = ("Undo last answer", "Bury card (back tomorrow)",
                    "Suspend card (parent will check)", "End session",
                    "Cancel")

    def screen_review(self):
        session = StudySession(self.storage, self.service, self.profile)
        reversed_mode = self.profile.get("study_direction") == "reversed"
        auto_play = bool(self.settings.get("auto_play_audio", True))

        flipped = False
        shown_at = None
        audio_status = None
        menu = None  # action-menu overlay index, or None when closed

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
            nonlocal flipped, shown_at, audio_status
            flipped = False
            shown_at = self.service.now()
            audio_status = None
            if card["audio_filename"] and not self.audio.available(
                    card["audio_filename"]):
                audio_status = "missing"
            if auto_play:
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
            if menu is not None:
                if action == "UP":
                    menu = (menu - 1) % len(self.MENU_ENTRIES)
                elif action == "DOWN":
                    menu = (menu + 1) % len(self.MENU_ENTRIES)
                elif action in ("B", "SELECT", "START"):
                    menu = None
                elif action == "A":
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
            elif action == "START":
                return end_session()
            elif action == "SELECT":
                menu = 0
            elif action == "L":
                play(card, "back" if flipped else "front", forced=True)
            elif action == "UNDO":
                restored = session.undo()
                if restored is not None:
                    card = restored
                    begin_card(card)
            elif not flipped:
                if action in DPAD or action in ("A", "OTHER"):
                    flipped = True
                    if auto_play:
                        play(card, "back")
            elif action in RATING_FOR_BUTTON:
                elapsed = int((self.service.now() -
                               shown_at).total_seconds() * 1000)
                session.answer(RATING_FOR_BUTTON[action], elapsed_ms=elapsed)
                advance()
                continue
            elif action == "R":
                session.bury_current()
                advance()
                continue

            self._draw_review(session, card, flipped, audio_status, menu)
            self.clock.tick(FPS)

    def _draw_review(self, session, card, flipped, audio_status, menu):
        self.screen.fill(BG)
        reversed_mode = self.profile.get("study_direction") == "reversed"
        front, back = (card["back"], card["front"]) if reversed_mode \
            else (card["front"], card["back"])

        header = card["deck"]
        if card["tags"]:
            header += "  ·  " + " ".join(card["tags"].split()[:3])
        self.screen.blit(self.font_small.render(header, True, DIM), (16, 12))
        left = f"{session.remaining()} left"
        surf = self.font_small.render(left, True, DIM)
        self.screen.blit(surf, (self.w - surf.get_width() - 16, 12))
        pygame.draw.line(self.screen, DIVIDER, (16, 40), (self.w - 16, 40))

        margin = 36
        max_width = self.w - 2 * margin
        y = self._block(front, self.font_big, FG, top=64,
                        max_width=max_width)

        if audio_status == "missing":
            self._center(self.font_small.render("(no audio)", True, WARN),
                         y + 6)
        elif card["audio_filename"]:
            self._center(self.font_small.render("♪  L = replay", True, DIM),
                         y + 6)

        if flipped:
            div_y = max(y + 34, 210)
            pygame.draw.line(self.screen, DIVIDER, (margin, div_y),
                             (self.w - margin, div_y))
            self._block(back, self.font, ACCENT, top=div_y + 18,
                        max_width=max_width)
            self._footer("B=Again  Y=Hard  A=Good  X=Easy   "
                         "R=Bury  SELECT=Menu  START=Finish")
        else:
            self._footer("D-pad = Show answer   L = Replay audio   "
                         "START = Finish")

        if menu is not None:
            self._draw_menu_overlay(menu)
        pygame.display.flip()

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
        ]
        if s.get("buried_count") or s.get("suspended_count"):
            lines.append((f"Buried {s.get('buried_count', 0)}   "
                          f"Suspended {s.get('suspended_count', 0)}", DIM))

        while True:
            action = self.poll()
            if action == "START":
                return "QUIT"
            if action in ("A", "B", "OTHER", "SELECT"):
                return "CHILD_START"

            self.screen.fill(BG)
            self._center(self.font_big.render("¡Buen trabajo!", True, GOOD),
                         56)
            self._center(self.font_small.render("SESSION COMPLETE", True,
                                                DIM), 108)
            y = 160
            for text, color in lines:
                self._center(self.font.render(text, True, color), y)
                y += 44
            self._footer("A = Done   START = Quit")
            pygame.display.flip()
            self.clock.tick(FPS)

    # -- parent mode -----------------------------------------------------------------

    def screen_parent_menu(self):
        direction = self.profile.get("study_direction", "normal")
        entries = [
            ("Import deck (.apkg)", "PARENT_IMPORT"),
            ("Categories", "PARENT_CATEGORIES"),
            ("Daily limits", "PARENT_LIMITS"),
            ("Suspended cards", "PARENT_SUSPENDED"),
            ("Progress", "PARENT_PROGRESS"),
            (f"Direction: "
             f"{'Reversed (back first)' if direction == 'reversed' else 'Normal (front first)'}",
             "TOGGLE_DIRECTION"),
            ("Back to study", "CHILD_START"),
        ]
        index = 0
        while True:
            action = self.poll()
            if action in ("START", "B", "SELECT"):
                return "CHILD_START"
            if action == "UP":
                index = (index - 1) % len(entries)
            elif action == "DOWN":
                index = (index + 1) % len(entries)
            elif action == "A":
                target = entries[index][1]
                if target == "TOGGLE_DIRECTION":
                    new = "normal" if direction == "reversed" else "reversed"
                    self.storage.update_profile(self.profile["id"],
                                                study_direction=new)
                    self._reload_profile()
                    return "PARENT_MENU"
                return target

            self.screen.fill(BG)
            self._center(self.font_big.render("Parent Mode", True, FG), 44)
            self._center(self.font_small.render(
                f"Profile: {self.profile['name']}", True, DIM), 96)
            y = 140
            for i, (label, _) in enumerate(entries):
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(prefix + label, True,
                                                  color), (90, y))
                y += 42
            self._footer("Up/Down = Choose   A = Select   B = Back")
            pygame.display.flip()
            self.clock.tick(FPS)

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

    def settings_dir(self):
        return os.path.dirname(os.path.abspath(self.settings.path))

    def screen_parent_import(self):
        files = self._scan_apkg_files()
        index = 0
        message = None
        while True:
            action = self.poll()
            if action in ("START", "B", "SELECT"):
                return "PARENT_MENU"
            if files and action == "UP":
                index = (index - 1) % len(files)
            elif files and action == "DOWN":
                index = (index + 1) % len(files)
            elif files and action == "A":
                self._draw_import(files, index, "Importing…")
                pygame.display.flip()
                try:
                    stats = import_apkg(
                        files[index], self.storage,
                        self.service.scheduler,
                        os.path.join(self.settings_dir(), "media"))
                    message = stats.summary()
                except (ApkgError, OSError) as exc:
                    message = f"Import failed: {exc}"

            self._draw_import(files, index, message)
            pygame.display.flip()
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
        self._footer("A = Import   B = Back")

    def screen_parent_categories(self):
        tags = self.storage.all_tags()
        active = self.profile["active_categories"]
        selected = None if active is None else set(active)
        index = 0
        entries = ["[ All categories ]"] + tags
        top = 0
        visible = 7

        def save():
            self.storage.update_profile(
                self.profile["id"],
                active_categories=(None if selected is None
                                   else sorted(selected)))
            self._reload_profile()

        while True:
            action = self.poll()
            if action in ("START", "B", "SELECT"):
                save()
                return "PARENT_MENU"
            if action == "UP":
                index = (index - 1) % len(entries)
            elif action == "DOWN":
                index = (index + 1) % len(entries)
            elif action == "A":
                if index == 0:
                    selected = None
                else:
                    tag = entries[index]
                    if selected is None:
                        selected = {tag}
                    elif tag in selected:
                        selected.discard(tag)
                    else:
                        selected.add(tag)
            top = min(max(top, index - visible + 1), index)

            self.screen.fill(BG)
            self._center(self.font_big.render("Categories", True, FG), 36)
            self._center(self.font_small.render(
                "Choose what the child studies", True, DIM), 86)
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
            if not tags:
                self._center(self.font.render(
                    "No tags found — import a deck first.", True, DIM), 200)
            self._footer("A = Toggle   B = Save & back")
            pygame.display.flip()
            self.clock.tick(FPS)

    def screen_parent_limits(self):
        fields = [
            ("New cards per day", "daily_new_cards", 0, 100),
            ("Review cards per day", "daily_review_cards", 0, 300),
            ("Cards per session", "session_card_limit", 1, 200),
            ("Minutes per session (0 = off)", "session_time_minutes", 0, 90),
        ]
        values = {key: self.profile[key] for _, key, _, _ in fields}
        index = 0
        while True:
            action = self.poll()
            if action in ("START", "B", "SELECT"):
                self.storage.update_profile(self.profile["id"], **values)
                self._reload_profile()
                return "PARENT_MENU"
            if action == "UP":
                index = (index - 1) % len(fields)
            elif action == "DOWN":
                index = (index + 1) % len(fields)
            elif action in ("LEFT", "RIGHT"):
                label, key, lo, hi = fields[index]
                step = -1 if action == "LEFT" else 1
                values[key] = min(max(values[key] + step, lo), hi)

            self.screen.fill(BG)
            self._center(self.font_big.render("Daily Limits", True, FG), 40)
            y = 140
            for i, (label, key, _, _) in enumerate(fields):
                color = ACCENT if i == index else FG
                prefix = "> " if i == index else "   "
                self.screen.blit(self.font.render(prefix + label, True,
                                                  color), (70, y))
                value = self.font.render(str(values[key]), True, color)
                self.screen.blit(value, (self.w - 120, y))
                y += 52
            self._footer("Left/Right = Adjust   B = Save & back")
            pygame.display.flip()
            self.clock.tick(FPS)

    def screen_parent_suspended(self):
        index, top, visible = 0, 0, 7
        cards = self.storage.suspended_cards()
        while True:
            action = self.poll()
            if action in ("START", "B", "SELECT"):
                return "PARENT_MENU"
            if cards:
                if action == "UP":
                    index = (index - 1) % len(cards)
                elif action == "DOWN":
                    index = (index + 1) % len(cards)
                elif action == "A":
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
            self._footer("A = Unsuspend   B = Back")
            pygame.display.flip()
            self.clock.tick(FPS)

    def screen_parent_progress(self):
        totals = self.service.recent_daily_totals(7)
        suspended = len(self.storage.suspended_cards())
        total_cards = self.storage.conn.execute(
            "SELECT COUNT(*) AS n FROM cards").fetchone()["n"]
        while True:
            action = self.poll()
            if action in ("START", "A", "B", "SELECT"):
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
            self._footer("B = Back")
            pygame.display.flip()
            self.clock.tick(FPS)

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

    def _footer(self, text):
        pygame.draw.line(self.screen, DIVIDER, (16, self.h - 44),
                         (self.w - 16, self.h - 44))
        surf = self.font_small.render(text, True, DIM)
        self.screen.blit(surf, ((self.w - surf.get_width()) // 2,
                                self.h - 33))
