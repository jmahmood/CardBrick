# CardBrick Spanish — Knulli / RG35XX SP Deployment

> **Start with [`PACKAGING.md`](PACKAGING.md).** It documents the
> recommended pipeline — a self-contained pygame-ce SquashFS runtime
> (built the [viniciusfs/pygame-ce-runtime](https://github.com/viniciusfs/pygame-ce-runtime)
> way) plus `make package` / `make deploy-sd` / `make deploy-ssh`
> automation. This file describes the underlying app behaviour on the
> device and the older vendored-wheels fallback.

How to put the CardBrick-style Spanish study appliance on a Knulli
(Batocera-derived) handheld. Read `package_layout.md` for the exact
file tree and `smoke_test_checklist.md` for the pre-flight list.

The deployment model, in one line:

> Everything is prepared on a PC; the device only ever *runs* files —
> no on-device pip install, no compilation, no network.

## 1. What gets copied to the device

```
/userdata/roms/ports/CardBrickSpanish/
├── launch_cardbrick_spanish.sh      # adapted from this folder
├── cardbrick-py/                    # the whole app folder from the repo
│   ├── main.py
│   ├── cardbrick/                   # python package
│   └── assets/fonts/DejaVuSans.ttf  # bundled Spanish-safe font
├── vendor/                          # dependencies, prepared on a PC:
│   │                                #   pip install --target vendor \
│   │                                #     --platform manylinux2014_aarch64 \
│   │                                #     --only-binary=:all: \
│   │                                #     -r requirements-runtime.txt
└── runtime/                         # OPTIONAL portable ARM64 Python
                                     # (python-build-standalone etc.)
                                     # if the firmware's python3 is
                                     # missing or too old (< 3.9)
```

Knulli ships a system `python3` on current builds; try Option B
(vendored site-packages via `PYTHONPATH`) first, and fall back to a
bundled runtime (`PYTHONHOME` + its own binary) only if needed. Both
options are pre-wired in the launch script.

## 2. Where things live at runtime

| What                    | Where                                        |
|-------------------------|----------------------------------------------|
| App + runtime (read-only ok) | `/userdata/roms/ports/CardBrickSpanish/` |
| **All mutable data**    | `/userdata/saves/cardbrick/`                 |
| SQLite database         | `/userdata/saves/cardbrick/cardbrick.db`     |
| Settings (hand-editable JSON) | `/userdata/saves/cardbrick/settings.json` |
| Controller mapping      | `/userdata/saves/cardbrick/input_mapping.json` |
| Imported audio          | `/userdata/saves/cardbrick/media/`           |
| App log (rotating)      | `/userdata/saves/cardbrick/logs/cardbrick.log` |
| Launch-script log       | `/userdata/saves/cardbrick/logs/launch.log`  |
| Migration backups       | `/userdata/saves/cardbrick/cardbrick.db.backup-*` |

The data root is chosen in this priority order (logged at startup):
`--data-dir` flag → `CARD_BRICK_DATA_DIR` env → `CARDBRICK_DATA` env
(legacy) → `/userdata/saves/cardbrick` when `/userdata` exists →
`./data` on a desktop. Nothing mutable is ever written inside the app
or runtime folders, so they can live on a squashfs/read-only mount.

## 3. Smoke test (run this before anything else)

On the PC and again on the device (SSH in, or check `launch.log` — the
launch script runs it automatically on first boot):

```sh
python3 main.py --knulli --smoke-test
```

Prints one `[PASS]/[WARN]/[FAIL]` line per subsystem (data dir, log,
settings, DB + migrations, cards, profile, scheduler, pygame, display,
Spanish font glyphs, joystick, audio, input mapping) and exits 0 only
if no hard check failed. `WARN` for "no joystick" / "no audio" is
normal on a desktop.

## 4. Controller test & calibration

```sh
python3 main.py --input-diagnostic        # straight into the screen
```

or on-device: Parent Mode → *Controller test & setup*. The screen shows
every raw event (button/hat/axis index) next to the semantic action and
study action the current mapping produces. **Hold any single button ~3
seconds to start calibration** — that works even when the current
mapping is completely wrong. Calibration prompts for each physical
button by position ("press the BOTTOM face button…") and saves to
`input_mapping.json`. Raw SDL button numbers are never trusted as
final truth; the defaults are only a starting guess.

## 4b. Audio: SDL_mixer cannot be guaranteed

`pygame.mixer` is an optional compiled module that depends on
SDL_mixer; a vendored wheel or firmware SDL without it means no mixer.
The app does **not** assume it exists — playback auto-selects:

1. `pygame.mixer` if it initialises;
2. else the first CLI player on PATH: `mpg123`, `ogg123`, `aplay`,
   `paplay`, `ffplay` (launched non-blocking per clip);
3. else silent no-op (study loop unaffected).

The chosen backend appears in the smoke-test output and the log.
Overrides for stubborn devices:

```sh
export CARDBRICK_AUDIO=command            # skip mixer entirely
export CARDBRICK_AUDIO_CMD="mpg123 -q {file}"   # exact player command
```

If your deck's audio is MP3 and the device has neither SDL_mixer nor
an MP3-capable CLI player, re-encode the media to WAV on the PC
(`aplay` is almost always present on ALSA-based firmware).

## 5. Importing an .apkg

Export from Anki desktop **with "Support older Anki versions" checked**
(the new zstd `.anki21b` format is rejected with a clear message).
Then either:

- copy `Deck.apkg` into `/userdata/saves/cardbrick/` (or `import/`
  inside it) and use Parent Mode → *Import deck*, or
- on a PC: `python3 main.py --data-dir <path> import Deck.apkg` and
  copy the whole data folder to the device.

Re-importing never resets learning progress.

## 6. Configuring the child profile

On-device: Parent Mode (SELECT on the start screen) → Categories /
Daily limits / Direction. From a PC:

```sh
python3 main.py profile --name Maya --daily-new 10 --daily-review 40 \
    --session-cards 50 --session-minutes 15 --categories restaurant,food
```

## 7. Resetting settings without losing review history

Review history lives in `cardbrick.db`; preferences live next to it.
To reset preferences only, delete these files (the app recreates them
with defaults):

```sh
rm /userdata/saves/cardbrick/settings.json
rm /userdata/saves/cardbrick/input_mapping.json   # controller remap only
```

Never delete `cardbrick.db` unless you intend to lose all progress.
Migration backups (`cardbrick.db.backup-*`) can be renamed back over
`cardbrick.db` to roll back a bad upgrade.

## 8. Exiting the app on-device

RetroArch hotkeys do **not** apply to native apps. Built-in exits:

- START during study → session summary; START again → quit.
- SELECT + START held for 2 seconds → force exit from anywhere.
- Every completed answer is already committed to the DB, so even a
  battery pull loses nothing.

## 9. Known limitations

- The `runtime/` portable-Python option is a documented template, not a
  tested artifact — validate the exact python-build-standalone build on
  real hardware (see `smoke_test_checklist.md`).
- Exact Knulli Ports-menu integration (gamelist entry, `.sh` vs
  `.pygame` wrapper) varies by firmware version; the launch script is
  the stable part.
- One audio file per card; no TTS; no images; text-only cards.
- Single child profile in the UI for now.

## Things We Deliberately Avoid

- **No Anki runtime on the handheld** — `.apkg` is an import format
  only; after import the app runs from its own SQLite DB.
- **No AnkiConnect.**
- **No Rust backend.**
- **No on-device pip install expectation** — dependencies are vendored
  or bundled on a PC beforehand.
- **No webview / browser engine.**
- **No full Anki template rendering** — HTML is stripped to text at
  import time.
- **No direct mutation of `.anki2`/`.apkg` files** — imports read them;
  all writes go to the app's own database.
