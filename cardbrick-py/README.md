# CardBrick-Py

A proof-of-concept flashcard reviewer for inexpensive ARM Linux handhelds
(RG35XX SP running Knulli, TrimUI Brick, and similar), written in pure
Python with pygame.

This is a deliberate pivot from the original Rust implementation in the
parent directory. Native compilation made deployment across the zoo of
cheap handhelds difficult; this prototype trades raw performance for
portability: **if the device can run Python and pygame, it can run
CardBrick-Py.**

The objective is *not* to recreate Anki. Anki's `.apkg` files are used as
a **content format only** — after a one-time import, the application runs
entirely from its own SQLite database with no Anki desktop, no
AnkiConnect, no Rust backend, and no network access.

## Architecture

```
.apkg
  ↓
Importer          (cardbrick/importer.py — zipfile + sqlite3, reads the
  ↓                archive directly, no Anki code)
Local SQLite DB   (cardbrick/storage.py — cards + review_state tables)
  ↓
FSRS scheduler    (cardbrick/scheduler.py — thin wrapper around py-fsrs;
  ↓                the algorithm itself is NOT reimplemented)
Pygame reviewer   (cardbrick/ui.py — 640x480, controller-friendly)
```

Supporting modules: `cardbrick/audio.py` (pygame.mixer playback of
`[sound:...]` references, fails soft without an audio device) and
`cardbrick/textutil.py` (HTML stripping, audio-tag extraction, pixel
word-wrapping with per-character fallback for CJK text).

### Dependencies

Only two, both pure-Python-installable on ARM:

| Library  | Role                                     |
|----------|------------------------------------------|
| `pygame` | display, input, audio                     |
| `fsrs`   | py-fsrs, the FSRS spaced-repetition engine |

Everything else is the standard library (`sqlite3`, `zipfile`, `json`,
`html`). No Rust, no compilation beyond what pygame's prebuilt wheels
provide, no private Anki APIs.

### The importer

`.apkg` is a zip archive containing an SQLite collection plus numbered
media files. The importer:

1. Extracts `collection.anki21` (or `collection.anki2`) and reads the
   `notes`, `cards`, and `col` tables directly.
2. Reduces each card to a **front/back pair** from the note's first two
   fields, honouring the card ordinal — so *Basic (and reversed card)*
   notes yield both directions. Cloze note types are skipped.
3. Strips HTML down to plain text and pulls out `[sound:file]`
   references into an `audio_filename` + `audio_side` column.
4. Copies media into `data/media/` under their real names, using the
   archive's JSON media map (with path-traversal protection).
5. Seeds a fresh FSRS state (due immediately) for each card —
   **re-importing never resets review progress**.

The new zstd-compressed `collection.anki21b` format is deliberately not
supported (it would need a native zstd + protobuf dependency). Anki
desktop exports the legacy format when *"Support older Anki versions"*
is checked.

### The database

`data/cardbrick.db`, two tables:

- **cards** — `id, note_id, deck, front, back, tags, audio_filename,
  audio_side`
- **review_state** — `card_id, due, stability, difficulty, elapsed_days,
  scheduled_days, reps, lapses, state, fsrs_json`

`fsrs_json` holds the py-fsrs card serialization verbatim
(`Card.to_dict()`), so scheduling state round-trips losslessly through
the library; the flat columns exist for querying and inspection. All
timestamps are UTC ISO-8601 strings, which compare correctly as text in
SQL.

### The scheduler

`ReviewScheduler` delegates every scheduling decision to
`fsrs.Scheduler.review_card()`. The wrapper only converts between stored
rows and `fsrs.Card` objects and maintains the `reps`/`lapses` counters.
Learning steps are py-fsrs defaults (1 min, 10 min), so freshly-rated
cards come back within the same session; when nothing is due the UI
shows an "all caught up" screen that automatically resumes when the next
learning-step card matures.

## Usage

```bash
pip install -r requirements.txt

# One-time import (repeatable; progress is preserved)
python main.py import MyDeck.apkg

# Review
python main.py review                 # deck picker if multiple decks
python main.py review --deck Spanish --fullscreen
python main.py decks                  # list decks + due counts
```

No deck handy? Generate a test one:
`python scripts/make_sample_apkg.py sample.apkg`

Data lives in `./data/` next to `main.py` (override with `--data-dir`
or the `CARDBRICK_DATA` env var) — the whole application plus its data
is a single copyable folder, which is exactly what handheld "ports"
launchers want.

## Controls

```
------------------------------------
Deck: Spanish
¿Dónde está el baño?
------------------------------------
Any button = Show answer
A = Again    B = Hard
X = Good     Y = Easy
Start = Exit
```

Keyboard fallback for desktop testing: `A/B/X/Y` or `1/2/3/4` to rate,
`Space`/`Enter` to flip, `Esc` to exit, arrows to navigate the deck menu.

Gamepad button numbering differs between handhelds; remap without
touching code via
`CARDBRICK_JOYMAP="A=1,B=0,X=3,Y=2,START=7,SELECT=6"`.
A font override is available the same way: `CARDBRICK_FONT=/path/to/font.ttf`
(drop a Noto CJK font there for Japanese decks; the repo's
`assets/font/NotoSansCJK-Regular.ttc` works).

## Deploying to a handheld

1. Copy the `cardbrick-py/` folder to the device (SD card / SSH).
2. Vendor the two dependencies next to the app so nothing needs
   installing on-device:
   `pip install --target vendor --platform manylinux2014_aarch64 --only-binary=:all: pygame fsrs`
   and launch with `PYTHONPATH=vendor`. (Knulli and most Batocera-derived
   firmwares already ship Python; many also ship pygame.)
3. Add a Ports-style launcher script that runs
   `python3 main.py review --fullscreen`.
4. Import decks on your PC and copy the `data/` folder over, or run the
   import on-device — either works, it's the same folder.

## Scope (deliberately excluded)

No sync, no editing, no statistics, no HTML rendering beyond tag
stripping, no card templates, no cloze, no image occlusion, no images at
all. This is a prototype proving the pipeline:
**apkg → local DB → py-fsrs → pygame**.
