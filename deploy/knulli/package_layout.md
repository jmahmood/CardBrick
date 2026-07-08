# Package Layout

What to assemble on a PC before copying to the SD card, and what ends
up where on the device.

## Build the bundle on a PC

```sh
# 1. Start from the repo
cp -r cardbrick-py /tmp/CardBrickSpanish/cardbrick-py
cp deploy/knulli/launch_cardbrick_spanish.sh /tmp/CardBrickSpanish/

# 2. Vendor the dependencies for ARM64 (no on-device pip!)
cd /tmp/CardBrickSpanish
pip install --target vendor \
    --platform manylinux2014_aarch64 --only-binary=:all: \
    -r cardbrick-py/../deploy/knulli/requirements-runtime.txt
# (equivalently: pygame-ce fsrs typing-extensions)

# 3. OPTIONAL: bundled interpreter, only if the firmware python3 is
#    unusable. Unpack an aarch64 python-build-standalone release into
#    ./runtime and uncomment the PYTHONHOME block in the launch script.

# 4. Trim what the device never needs
rm -rf cardbrick-py/tests cardbrick-py/data cardbrick-py/__pycache__
find . -name '__pycache__' -type d -exec rm -rf {} +
```

## On the device

```
/userdata/roms/ports/CardBrickSpanish/     # read-only is fine
├── launch_cardbrick_spanish.sh
├── cardbrick-py/
│   ├── main.py
│   ├── cardbrick/            # code
│   ├── assets/fonts/         # Noto Sans JP/CJK + DejaVu fallback
│   │                         # (Spanish + Japanese glyph coverage)
│   └── scripts/              # optional (sample deck generator)
├── vendor/                   # pygame-ce, fsrs, typing-extensions
└── runtime/                  # optional portable Python

/userdata/saves/cardbrick/                 # created on first run
├── cardbrick.db              # SQLite: cards, FSRS state, review log
├── cardbrick.db.backup-*     # automatic pre-migration backups
├── settings.json             # app settings (hand-editable)
├── input_mapping.json        # calibrated controller mapping
├── media/                    # audio extracted from .apkg files
├── import/                   # drop .apkg files here for parent mode
└── logs/
    ├── cardbrick.log         # app log (rotating, 3 x 512 KiB max)
    └── launch.log            # launch-script + smoke-test output
```

## Rules

1. The app/runtime tree is treated as **read-only**. If it is mounted
   from squashfs, nothing breaks — every write goes to the data root.
2. The data root is one folder; backing up a child's entire history is
   `cp -r /userdata/saves/cardbrick`.
3. `vendor/` must be built for the *device's* Python minor version
   (check `python3 --version` over SSH) and `aarch64`. A wheel built
   for the wrong CPython ABI is the most likely packaging failure —
   the smoke test catches it at the `pygame init` step.
