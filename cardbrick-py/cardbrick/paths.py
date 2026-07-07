"""Data-directory resolution and app paths.

The app/runtime bundle must stay read-only on the handheld (it may be
mounted from SquashFS); every mutable file lives under a single data
root resolved here. Priority:

    1. --data-dir CLI argument
    2. CARD_BRICK_DATA_DIR environment variable
    3. CARDBRICK_DATA environment variable (legacy name, still honoured)
    4. Knulli/Batocera userdata path (/userdata/saves/cardbrick) when
       running on such a device
    5. ./data next to main.py (desktop development)

The chosen directory is logged at startup so a parent debugging on the
device can find the database, settings, and logs.
"""

import logging
import os

log = logging.getLogger(__name__)

APP_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Batocera-derived firmwares (Knulli, EmuELEC cousins) mount the user
# partition here; it is writable and survives firmware upgrades.
KNULLI_USERDATA = "/userdata"
KNULLI_DATA_DIR = os.path.join(KNULLI_USERDATA, "saves", "cardbrick")


def detect_mode(cli_mode=None):
    """'desktop' or 'knulli'. CLI flag > CARDBRICK_MODE env > detection."""
    if cli_mode in ("desktop", "knulli"):
        return cli_mode
    env = os.environ.get("CARDBRICK_MODE", "").strip().lower()
    if env in ("desktop", "knulli"):
        return env
    return "knulli" if os.path.isdir(KNULLI_USERDATA) else "desktop"


def resolve_data_dir(cli_data_dir=None, mode="desktop"):
    if cli_data_dir:
        return os.path.abspath(cli_data_dir), "--data-dir"
    for var in ("CARD_BRICK_DATA_DIR", "CARDBRICK_DATA"):
        value = os.environ.get(var, "").strip()
        if value:
            return os.path.abspath(value), var
    if mode == "knulli":
        return KNULLI_DATA_DIR, "knulli userdata default"
    return os.path.join(APP_DIR, "data"), "desktop default"


class AppPaths:
    """All filesystem locations, derived from one writable data root."""

    def __init__(self, data_dir, mode="desktop", source="default"):
        self.mode = mode
        self.source = source
        self.data_dir = data_dir
        self.db_path = os.path.join(data_dir, "cardbrick.db")
        self.media_dir = os.path.join(data_dir, "media")
        self.settings_path = os.path.join(data_dir, "settings.json")
        self.input_map_path = os.path.join(data_dir, "input_mapping.json")
        self.log_dir = os.path.join(data_dir, "logs")
        self.log_path = os.path.join(self.log_dir, "cardbrick.log")
        self.import_dir = os.path.join(data_dir, "import")

    @classmethod
    def resolve(cls, cli_data_dir=None, cli_mode=None):
        mode = detect_mode(cli_mode)
        data_dir, source = resolve_data_dir(cli_data_dir, mode)
        return cls(data_dir, mode=mode, source=source)

    def ensure_directories(self):
        """Create every writable directory; raises OSError on failure."""
        for path in (self.data_dir, self.media_dir, self.log_dir,
                     self.import_dir):
            os.makedirs(path, exist_ok=True)
        return self

    def describe(self):
        return {
            "mode": self.mode,
            "data_dir_source": self.source,
            "data_dir": self.data_dir,
            "db_path": self.db_path,
            "media_dir": self.media_dir,
            "settings_path": self.settings_path,
            "input_map_path": self.input_map_path,
            "log_path": self.log_path,
        }
