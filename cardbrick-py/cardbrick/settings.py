"""App settings, persisted as plain JSON next to the database.

Durable and user-editable on purpose: a parent can fix a bad setting
with a text editor on the SD card. Unknown keys are preserved.
"""

import json
import logging
import os

log = logging.getLogger(__name__)

DEFAULTS = {
    "language": "en",
    "current_child_profile_id": None,
    "auto_play_audio": True,
    "paper_feed_sfx_enabled": True,
    "paper_feed_sfx_volume": 0.15,
    "reduced_motion": False,  # True = paper-roll feeds snap instantly
    "fullscreen": False,
    "logical_width": 640,
    "logical_height": 480,
    "integer_scaling": True,
}


class AppSettings:
    def __init__(self, path):
        self.path = path
        self.data = dict(DEFAULTS)
        self.load()

    def load(self):
        try:
            with open(self.path, encoding="utf-8") as f:
                loaded = json.load(f)
            if isinstance(loaded, dict):
                self.data.update(loaded)
            else:
                log.error("settings %s is not a JSON object — "
                          "using defaults", self.path)
        except FileNotFoundError:
            log.info("no settings file at %s — using defaults", self.path)
        except json.JSONDecodeError as exc:
            log.error("invalid settings JSON %s (%s) — using defaults; "
                      "the file will be rewritten on next change",
                      self.path, exc)
        except OSError as exc:
            log.error("cannot read settings %s: %s — using defaults",
                      self.path, exc)
        return self

    def save(self):
        os.makedirs(os.path.dirname(os.path.abspath(self.path)),
                    exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(self.data, f, indent=2, sort_keys=True)
        os.replace(tmp, self.path)  # atomic: never a half-written file

    def get(self, key, default=None):
        return self.data.get(key, default)

    def set(self, key, value):
        self.data[key] = value
        self.save()
