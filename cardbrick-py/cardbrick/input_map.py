"""Controller input mapping with semantic actions.

Cheap handhelds expose gamepad buttons in inconsistent SDL orders, and
the raw SDL indices vary by device. The app
therefore never reasons about raw button numbers or vendor labels:

    raw pygame event
      -> semantic action ("south_button", "dpad_up", "start", ...)
      -> study action decided per screen by the UI

The raw-index -> semantic mapping is persisted as JSON in the data
directory and can be rewritten by the in-app calibration screen, so no
device's numbering is ever hardcoded as final truth. The defaults below
match the common SDL ordering on Anbernic/Knulli devices, but they are
only a starting point.

Semantic face buttons are named by the fixed face-button layout used by
this app: south = B, east = A, west = Y, north = X.
"""

import json
import logging
import os

log = logging.getLogger(__name__)

SEMANTIC_ACTIONS = (
    "south_button", "east_button", "west_button", "north_button",
    "l1", "r1", "select", "start",
    "dpad_up", "dpad_down", "dpad_left", "dpad_right",
)

FACE_LABELS = {
    "south_button": "B",
    "east_button": "A",
    "west_button": "Y",
    "north_button": "X",
}

# Semantic action -> study action (documentation of intent; the UI
# interprets semantics per screen but follows this table for review).
STUDY_ACTIONS = {
    "south_button": "rate_again",
    "east_button": "rate_good",
    "west_button": "rate_easy",
    "north_button": "rate_hard",
    "l1": "replay_audio",
    "r1": "bury_card",
    "select": "quit",
    "start": "open_action_menu",
    "dpad_up": "hold_to_undo_before_reveal",
    "dpad_down": "reveal_answer",
    "dpad_left": "scroll_after_reveal",
    "dpad_right": "scroll_after_reveal",
}

# Starting-point guess for unmapped devices: Knulli handhelds label the
# right/east face button as A and the bottom/south face button as B.
# The calibration screen overwrites this.
DEFAULT_BUTTONS = {
    0: "east_button", 1: "south_button", 2: "west_button",
    3: "north_button", 4: "l1", 5: "r1", 6: "select", 7: "start",
}

# Keyboard fallback for desktop testing (semantic tokens, plus the
# extra "undo" convenience that has no controller button).
KEYBOARD_SEMANTICS = {
    "up": "dpad_up", "down": "dpad_down",
    "left": "dpad_left", "right": "dpad_right",
    "return": "east_button", "space": "east_button",
    "a": "east_button", "b": "south_button",
    "x": "west_button", "y": "north_button",
    "1": "south_button",  # Again
    "2": "north_button",  # Hard
    "3": "east_button",   # Good
    "4": "west_button",   # Easy
    "l": "l1", "r": "r1",
    "tab": "start",
    "escape": "select", "q": "select",
    "u": "undo",
    "backspace": "south_button",
}

AXIS_THRESHOLD = 0.6


class InputMap:
    """Persisted raw-index -> semantic mapping."""

    def __init__(self, path=None):
        self.path = path
        self.buttons = dict(DEFAULT_BUTTONS)
        self.source = "defaults"
        if path:
            self.load()

    def load(self):
        try:
            with open(self.path, encoding="utf-8") as f:
                data = json.load(f)
            buttons = data.get("buttons", {})
            parsed = {}
            for key, semantic in buttons.items():
                if semantic in SEMANTIC_ACTIONS and str(key).isdigit():
                    parsed[int(key)] = semantic
            if parsed:
                self.buttons = parsed
                self.source = self.path
                log.info("input mapping loaded from %s (%d buttons)",
                         self.path, len(parsed))
        except FileNotFoundError:
            log.info("no input mapping file at %s — using defaults",
                     self.path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            log.error("invalid input mapping %s: %s — using defaults",
                      self.path, exc)
        return self

    def save(self):
        data = {
            "_comment": "Raw SDL button index -> semantic action. "
                        "Rewritten by the in-app controller setup.",
            "buttons": {str(k): v for k, v in self.buttons.items()},
        }
        os.makedirs(os.path.dirname(os.path.abspath(self.path)),
                    exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, sort_keys=True)
        os.replace(tmp, self.path)
        log.info("input mapping saved to %s", self.path)

    def set_button(self, index, semantic):
        # One physical button per semantic slot: drop any older binding.
        self.buttons = {k: v for k, v in self.buttons.items()
                        if v != semantic}
        self.buttons[index] = semantic

    def semantic_for_button(self, index):
        return self.buttons.get(index)


class InputTranslator:
    """Turns pygame events into semantic actions, one per call.

    Also debounces analog-stick axes into d-pad semantics.
    """

    def __init__(self, input_map):
        self.map = input_map
        self._axis_state = {}    # axis index -> -1/0/1

    def translate(self, event):
        """Semantic action for one pygame event, or None."""
        import pygame
        if event.type == pygame.KEYDOWN:
            name = pygame.key.name(event.key)
            return KEYBOARD_SEMANTICS.get(name)
        if event.type == pygame.KEYUP:
            return None
        if event.type == pygame.JOYBUTTONDOWN:
            semantic = self.map.semantic_for_button(event.button)
            if semantic:
                return semantic
            return "unmapped"
        if event.type == pygame.JOYBUTTONUP:
            return None
        if event.type == pygame.JOYHATMOTION:
            x, y = event.value
            if y == 1:
                return "dpad_up"
            if y == -1:
                return "dpad_down"
            if x == -1:
                return "dpad_left"
            if x == 1:
                return "dpad_right"
            return None
        if event.type == pygame.JOYAXISMOTION:
            return self._translate_axis(event)
        return None

    def _translate_axis(self, event):
        """Left stick as a d-pad: emit once per threshold crossing."""
        if event.axis > 1:
            return None
        direction = (1 if event.value > AXIS_THRESHOLD else
                     -1 if event.value < -AXIS_THRESHOLD else 0)
        previous = self._axis_state.get(event.axis, 0)
        self._axis_state[event.axis] = direction
        if direction == 0 or direction == previous:
            return None
        if event.axis == 0:
            return "dpad_right" if direction > 0 else "dpad_left"
        return "dpad_down" if direction > 0 else "dpad_up"
