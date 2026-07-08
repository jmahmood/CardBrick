"""Pygame reviewer UI.

A single 640x480 logical screen (the RG35XX SP panel resolution) with
three states: deck picker, card review, and an idle screen shown when
nothing is currently due. Designed for gamepad input with a keyboard
fallback for desktop testing.
"""

import os
from datetime import datetime

import pygame

from .scheduler import now_utc, iso
from .textutil import wrap_text

SCREEN_W, SCREEN_H = 640, 480
FPS = 30

BG = (24, 26, 30)
FG = (230, 230, 225)
DIM = (140, 142, 148)
ACCENT = (120, 180, 250)
DIVIDER = (70, 72, 78)

# Physical gamepad button numbers vary between handhelds. These defaults
# match the common SDL joystick ordering on Anbernic devices under
# Knulli (A=0? B=1? — adjust here if your device differs, or set the
# CARDBRICK_JOYMAP env var, e.g. "A=1,B=0,X=2,Y=3,START=7,SELECT=6").
DEFAULT_JOYMAP = {0: "A", 1: "B", 2: "X", 3: "Y", 6: "SELECT", 7: "START"}

# Face buttons are labelled by their rating role from the spec:
#   A = Again, B = Hard, X = Good, Y = Easy, Start = Exit.
RATING_FOR_BUTTON = {"A": 1, "B": 2, "X": 3, "Y": 4}

KEYBOARD_MAP = {
    pygame.K_a: "A", pygame.K_1: "A",
    pygame.K_b: "B", pygame.K_2: "B",
    pygame.K_x: "X", pygame.K_3: "X",
    pygame.K_y: "Y", pygame.K_4: "Y",
    pygame.K_RETURN: "START_ALT",   # flip/select on desktop
    pygame.K_SPACE: "START_ALT",
    pygame.K_ESCAPE: "START",
    pygame.K_q: "START",
    pygame.K_UP: "UP", pygame.K_DOWN: "DOWN",
}

_HERE = os.path.dirname(os.path.abspath(__file__))
_BUNDLED_FONT_DIR = os.path.join(_HERE, "..", "assets", "fonts")
_ROOT_FONT_DIR = os.path.join(_HERE, "..", "..", "assets", "font")

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


def _load_font(size):
    for path in FONT_CANDIDATES:
        if path and os.path.exists(path):
            try:
                return pygame.font.Font(path, size)
            except pygame.error:
                continue
    return pygame.font.Font(None, size + 6)  # pygame default is smaller


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


class ReviewApp:
    def __init__(self, storage, scheduler, audio, deck=None,
                 fullscreen=False):
        self.storage = storage
        self.scheduler = scheduler
        self.audio = audio
        self.deck = deck
        self.joymap = _parse_joymap()

        pygame.init()
        flags = pygame.FULLSCREEN if fullscreen else 0
        self.screen = pygame.display.set_mode((SCREEN_W, SCREEN_H), flags)
        pygame.display.set_caption("CardBrick")
        pygame.mouse.set_visible(False)
        # Keep references: in pygame 2 a garbage-collected Joystick
        # object closes the SDL device and its events stop arriving.
        self._joysticks = [pygame.joystick.Joystick(i)
                           for i in range(pygame.joystick.get_count())]

        self.font_big = _load_font(34)
        self.font = _load_font(24)
        self.font_small = _load_font(17)

        self.clock = pygame.time.Clock()
        self.card = None
        self.flipped = False
        self.running = True
        self.menu_index = 0

    # -- main loop -----------------------------------------------------------

    def run(self):
        if self.deck is None:
            self._deck_menu()
        while self.running:
            self._load_next_card()
            if self.card is None:
                self._idle_screen()
                continue
            self._review_card()
        pygame.quit()

    def _load_next_card(self):
        self.card = self.storage.next_due_card(iso(now_utc()), self.deck)
        self.flipped = False

    # -- input ---------------------------------------------------------------

    def _poll_button(self):
        """Next logical button press, or None. Unmapped joypad buttons
        report as OTHER so 'any other button flips the card' works."""
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                return "START"
            if event.type == pygame.KEYDOWN:
                return KEYBOARD_MAP.get(event.key, "OTHER")
            if event.type == pygame.JOYBUTTONDOWN:
                return self.joymap.get(event.button, "OTHER")
            if event.type == pygame.JOYHATMOTION:
                if event.value[1] == 1:
                    return "UP"
                if event.value[1] == -1:
                    return "DOWN"
        return None

    # -- screens ---------------------------------------------------------------

    def _deck_menu(self):
        decks = self.storage.decks(iso(now_utc()))
        if not decks:
            self._message_screen("No decks imported yet.",
                                 "Run: python main.py import <deck.apkg>")
            self.running = False
            return
        if len(decks) == 1:
            self.deck = decks[0]["name"]
            return

        entries = [("All decks", None)] + \
                  [(d["name"], d["name"]) for d in decks]
        while self.running:
            button = self._poll_button()
            if button == "START":
                self.running = False
            elif button == "UP":
                self.menu_index = (self.menu_index - 1) % len(entries)
            elif button == "DOWN":
                self.menu_index = (self.menu_index + 1) % len(entries)
            elif button in ("A", "START_ALT", "OTHER", "X"):
                self.deck = entries[self.menu_index][1]
                return

            self.screen.fill(BG)
            self._blit_center(self.font_big.render("CardBrick", True, FG),
                              48)
            y = 130
            for i, (label, name) in enumerate(entries):
                due = "" if name is None else \
                    f"  ({next(d['due'] for d in decks if d['name'] == name)} due)"
                color = ACCENT if i == self.menu_index else FG
                prefix = "> " if i == self.menu_index else "  "
                text = self.font.render(prefix + label + due, True, color)
                self.screen.blit(text, (60, y))
                y += 40
            self._footer("Up/Down = Choose   A = Select   Start = Exit")
            pygame.display.flip()
            self.clock.tick(FPS)

    def _review_card(self):
        card = self.card
        if card["audio_side"] == "front":
            self.audio.play(card["audio_filename"])
        while self.running:
            button = self._poll_button()
            if button == "START":
                self.running = False
                return
            if button is not None:
                if not self.flipped:
                    self.flipped = True
                    if card["audio_side"] == "back":
                        self.audio.play(card["audio_filename"])
                elif button in RATING_FOR_BUTTON:
                    new_state = self.scheduler.review(
                        card, RATING_FOR_BUTTON[button])
                    self.storage.save_review_state(new_state)
                    self.audio.stop()
                    return  # main loop fetches the next due card

            self._draw_card(card)
            self.clock.tick(FPS)

    def _idle_screen(self):
        """Nothing due right now: wait, re-checking once per second."""
        next_due = self.storage.next_due_time(self.deck)
        if next_due is None:
            self._message_screen("This deck is empty.",
                                 "Start = Exit")
            self.running = False
            return

        while self.running:
            button = self._poll_button()
            if button == "START":
                self.running = False
                return
            if self.storage.due_count(iso(now_utc()), self.deck):
                return  # a learning-step card came due; resume reviewing

            wait = datetime.fromisoformat(next_due) - now_utc()
            minutes = max(int(wait.total_seconds()) // 60, 0)
            when = (f"in {minutes // 1440} day(s)" if minutes >= 1440 else
                    f"in {minutes // 60}h {minutes % 60}m" if minutes >= 60
                    else f"in {minutes + 1} minute(s)")
            self.screen.fill(BG)
            self._blit_center(
                self.font_big.render("All caught up!", True, FG), 170)
            self._blit_center(
                self.font.render(f"Next card due {when}", True, DIM), 230)
            self._footer("Start = Exit")
            pygame.display.flip()
            self.clock.tick(4)

    def _message_screen(self, title, subtitle):
        self.screen.fill(BG)
        self._blit_center(self.font_big.render(title, True, FG), 190)
        self._blit_center(self.font.render(subtitle, True, DIM), 250)
        pygame.display.flip()
        pygame.time.wait(2500)

    # -- drawing ---------------------------------------------------------------

    def _draw_card(self, card):
        self.screen.fill(BG)
        due = self.storage.due_count(iso(now_utc()), self.deck)
        header = f"Deck: {card['deck']}"
        self.screen.blit(self.font_small.render(header, True, DIM), (16, 12))
        due_text = self.font_small.render(f"{due} due", True, DIM)
        self.screen.blit(due_text, (SCREEN_W - due_text.get_width() - 16, 12))
        pygame.draw.line(self.screen, DIVIDER, (16, 38), (SCREEN_W - 16, 38))

        margin = 32
        max_width = SCREEN_W - 2 * margin
        y = self._draw_block(card["front"], self.font_big, FG,
                             top=60, max_width=max_width)

        if self.flipped:
            y = max(y + 16, 200)
            pygame.draw.line(self.screen, DIVIDER,
                             (margin, y), (SCREEN_W - margin, y))
            self._draw_block(card["back"], self.font, ACCENT,
                             top=y + 16, max_width=max_width)
            self._footer("A = Again   B = Hard   X = Good   Y = Easy   "
                         "Start = Exit")
        else:
            self._footer("Any button = Show answer   Start = Exit")
        pygame.display.flip()

    def _draw_block(self, text, font, color, top, max_width):
        y = top
        for line in wrap_text(font, text, max_width):
            if y > SCREEN_H - 70:
                break  # clip overly long cards; prototype has no scrolling
            surf = font.render(line, True, color)
            self.screen.blit(surf, ((SCREEN_W - surf.get_width()) // 2, y))
            y += font.get_linesize()
        return y

    def _blit_center(self, surf, y):
        self.screen.blit(surf, ((SCREEN_W - surf.get_width()) // 2, y))

    def _footer(self, text):
        surf = self.font_small.render(text, True, DIM)
        pygame.draw.line(self.screen, DIVIDER,
                         (16, SCREEN_H - 42), (SCREEN_W - 16, SCREEN_H - 42))
        self.screen.blit(
            surf, ((SCREEN_W - surf.get_width()) // 2, SCREEN_H - 32))
