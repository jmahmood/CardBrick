# CardBrick-Py

A CardBrick-style **Spanish study appliance** for inexpensive ARM Linux
handhelds (RG35XX SP running Knulli, TrimUI Brick, and similar), written
in pure Python with pygame.

The target experience is a Game Boy-style educational cartridge, not a
desktop app: a child turns the handheld on, opens the app, reviews a
small capped number of cards with audio, sees a completion screen, and
is done. Parents configure everything else in a separate parent mode.

The objective is *not* to recreate Anki. Anki's `.apkg` files are used
as a **content format only** — after a one-time import, the application
runs entirely from its own SQLite database with no Anki desktop, no
AnkiConnect, no Rust backend, and no network access.

## Architecture

```
.apkg
  ↓
Importer          (cardbrick/importer.py — zipfile + sqlite3, reads the
  ↓                archive directly, no Anki code)
Local SQLite DB   (cardbrick/storage.py — cards, review_state, an
  ↓                append-only review_log, sessions, child_profiles)
FSRS scheduler    (cardbrick/scheduler.py — thin wrapper around py-fsrs;
  ↓                the algorithm itself is NOT reimplemented)
Review service    (cardbrick/service.py — daily limits, category
  ↓                filtering, undo, bury/suspend; injectable clock)
Session runner    (cardbrick/session.py — one sitting: queue, counters,
  ↓                summary)
Pygame app        (cardbrick/app.py — state-driven child/parent UI on a
                   640x480 logical canvas, controller-first)
```

Supporting modules: `cardbrick/audio.py` ([sound:...] playback, fails
soft without an audio device), `cardbrick/settings.py` (JSON app
settings), `cardbrick/textutil.py` (HTML stripping, wrapping), and
`cardbrick/ui.py` (the original minimal prototype reviewer, kept
working under `python main.py review`).

### Dependencies

Only two, both pure-Python-installable on ARM:

| Library  | Role                                     |
|----------|------------------------------------------|
| `pygame` | display, input, audio                     |
| `fsrs`   | py-fsrs, the FSRS spaced-repetition engine |

Everything else is the standard library. No Rust, no compilation beyond
what pygame's prebuilt wheels provide, no private Anki APIs.

## Usage

```bash
pip install -r requirements.txt

# One-time import (repeatable; progress is preserved). Also available
# inside the app's parent mode.
python main.py import MyDeck.apkg

# The study appliance (child/parent flow) — this is the default command
python main.py study                  # add --fullscreen on the handheld

# Configure the child profile from the command line (parent mode can do
# the same on-device)
python main.py profile --name Maya --daily-new 10 --daily-review 40 \
    --session-cards 50 --session-minutes 15 --categories restaurant,food
python main.py profile --categories all      # study every tag

# Legacy prototype reviewer (no limits/undo/profiles)
python main.py review [--deck NAME] [--fullscreen]
python main.py decks                  # list decks + due counts
```

No deck handy? Generate a test one:
`python scripts/make_sample_apkg.py sample.apkg`

Data lives in `./data/` next to `main.py` (override with `--data-dir`
or the `CARDBRICK_DATA` env var): `cardbrick.db` (SQLite),
`settings.json` (app settings, hand-editable), and `media/`. The whole
application plus its data is a single copyable folder.

## The daily loop

```
SPANISH PRACTICE
      Maya
restaurant, food
 12 cards today                Front: ¿Dónde está el baño?  ♪
 8 to review + 4 new     -->   D-pad = show answer          -->  ¡Buen trabajo!
Press A to start!              B=Again Y=Hard A=Good X=Easy      session stats
                               R=Bury  SELECT=Menu
```

### Controls (study)

| Input   | Question side       | Answer side |
|---------|---------------------|-------------|
| D-pad   | Reveal answer       | —           |
| A       | Reveal answer       | Good        |
| B       | —                   | Again       |
| X       | —                   | Easy        |
| Y       | —                   | Hard        |
| L       | Replay audio        | Replay audio|
| R       | —                   | Bury until tomorrow |
| SELECT  | Action menu (undo / bury / suspend / end) | same |
| START   | Finish session      | same        |

Keyboard fallback for desktop testing: arrows/Space reveal, `1/2/3/4` =
Again/Hard/Good/Easy (or literal `A/B/X/Y` keys), `L` replay, `R` bury,
`U` undo, `Tab` menu, `Esc` finish.

Gamepad button numbering differs between handhelds; remap without
touching code via
`CARDBRICK_JOYMAP="A=1,B=0,X=3,Y=2,L=4,R=5,SELECT=6,START=7"`.
A font override is available the same way:
`CARDBRICK_FONT=/path/to/font.ttf`.

### Parent mode

`SELECT` on the start screen. From there: import `.apkg` files found in
the data folder (or `data/import/`, or the app folder), choose active
categories (Anki tags) for the child, set daily limits, review/restore
suspended cards, see a 7-day progress table, and flip the study
direction (front-first / back-first). There is no PIN yet — the flows
are separated, not locked.

### Daily limits

Per child profile: `daily_new_cards` (default 10), `daily_review_cards`
(40), `session_card_limit` (50), `session_time_minutes` (15, 0 = off).
Limits count work already done today across restarts (derived from the
review log), reviews are queued before new cards, and a backlog never
floods the child — extra due cards simply wait for tomorrow.

### Durability

Every answer is committed to SQLite before the next card appears, with
the prior FSRS state snapshotted into an append-only `review_log` in the
same transaction — undo is an exact restore, and a crash or power-off
never loses a completed review. Sessions left open by a crash are closed
on the next boot. The schema migrates in place from the original
prototype database.

## The importer

`.apkg` is a zip archive containing an SQLite collection plus numbered
media files. The importer:

1. Extracts `collection.anki21` (or `collection.anki2`) and reads the
   `notes`, `cards`, and `col` tables directly.
2. Reduces each card to a **front/back pair** from the note's first two
   fields, honouring the card ordinal — so *Basic (and reversed card)*
   notes yield both directions. Cloze note types are skipped, with
   skip reasons reported.
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

## Tests

```bash
pip install pytest
python -m pytest tests/
```

Covers daily limits (including backlog protection and midnight
rollover), exact undo, bury/suspend, tag filtering, the importer, and
crash/restart persistence — all with an injected deterministic clock,
temp directories, and no pygame.

## Deploying to a handheld

1. Copy the `cardbrick-py/` folder to the device (SD card / SSH).
2. Vendor the two dependencies next to the app so nothing needs
   installing on-device:
   `pip install --target vendor --platform manylinux2014_aarch64 --only-binary=:all: pygame fsrs`
   and launch with `PYTHONPATH=vendor`. (Knulli and most Batocera-derived
   firmwares already ship Python; many also ship pygame.)
3. Add a Ports-style launcher script that runs
   `python3 main.py study --fullscreen`.
4. Import decks on your PC and copy the `data/` folder over, or drop the
   `.apkg` in `data/` and import from parent mode on-device.

## Scope (deliberately excluded)

No sync, no cloze, no image occlusion, no images at all, no HTML
rendering beyond tag stripping, no card templates, no `.apkg` export, no
TTS (the audio layer is structured so it can be added later), no
gamification. This is a focused daily study appliance:
**apkg → local DB → py-fsrs → pygame**.
