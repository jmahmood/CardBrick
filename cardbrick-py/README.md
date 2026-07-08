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
                   native 640x480 or 720x480 handheld canvas,
                   controller-first)
```

Supporting modules: `cardbrick/audio.py` (pluggable playback backends —
see below), `cardbrick/input_map.py` (semantic controller mapping +
calibration persistence), `cardbrick/paths.py` (writable data-root
resolution), `cardbrick/bootlog.py` (file logging + startup
diagnostics), `cardbrick/smoke.py` (`--smoke-test`),
`cardbrick/errors.py` (visible fatal-error screens),
`cardbrick/settings.py` (JSON app settings), `cardbrick/textutil.py`
(HTML stripping, wrapping), and `cardbrick/ui.py` (the original
minimal prototype reviewer, kept working under `python main.py
review`).

### Dependencies

Minimal and pure-pip-installable on ARM64 — no Rust, no compilation,
no private Anki APIs:

| Library             | Role                                        |
|---------------------|---------------------------------------------|
| `pygame-ce`         | display, input, audio (classic `pygame`     |
|                     | >= 2.1 also works — install one, not both)  |
| `fsrs`              | py-fsrs, the FSRS spaced-repetition engine  |
| `typing-extensions` | pulled in by fsrs                           |

Everything else is the standard library (`sqlite3`, `zipfile`, `json`,
`datetime`, `logging`, `shutil`, ...).

**Pick a Python with prebuilt wheels.** On a Python version that has no
prebuilt wheel for your pygame flavour (e.g. classic `pygame` on
3.13/3.14), pip silently compiles it from source, and the build will be
missing whatever SDL satellite libraries weren't installed — typically
`pygame.font` (SDL_ttf) and `pygame.mixer` (SDL_mixer). The symptom is
a "partially initialized module 'pygame.font'" ImportError at first
render. `--smoke-test` detects this and names the fix; the fix is
`pip uninstall pygame && pip install pygame-ce`, or Python 3.11/3.12.
Audio degrades gracefully (CLI players, incl. macOS `afplay`), but
fonts are load-bearing for a text UI, so the app refuses to start with
a clear message instead.

### Audio backends

`pygame.mixer` is an *optional* compiled pygame module that requires
SDL_mixer, which cannot be guaranteed on handheld firmware. Playback
therefore auto-selects the first working backend and logs the choice:

1. `mixer` — pygame.mixer, if the module exists and initialises;
2. `command` — an external CLI player found on PATH (`mpg123`,
   `ogg123`, `aplay`, `paplay`, `ffplay`), launched non-blocking;
3. `none` — logged no-op: the app runs silent, never crashes.

Override with `CARDBRICK_AUDIO=auto|mixer|command|none` or point at an
exact player with `CARDBRICK_AUDIO_CMD="mpg123 -q {file}"`.

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
python main.py profile --name Maya --daily-goal 150 \
    --sprint-cards 20 --sprint-minutes 10 --categories restaurant,food
python main.py profile --daily-new 10    # optional fixed new-card drip
                                         # (default 0 = goal-paced)
python main.py profile --study-ahead-days 2   # pull-forward horizon
python main.py profile --study-ahead off      # day ends with the due pool
python main.py profile --categories all      # study every tag

# Legacy prototype reviewer (no limits/undo/profiles)
python main.py review [--deck NAME] [--fullscreen]
python main.py decks                  # list decks + due counts

# Deployment tooling (see deploy/knulli/ at the repo root)
python main.py --smoke-test           # non-interactive sanity checks
python main.py --input-diagnostic     # controller test + calibration
python main.py --desktop|--knulli     # force platform mode
```

No deck handy? Generate a test one:
`python scripts/make_sample_apkg.py sample.apkg`

The writable data root is resolved in this priority order and logged
at startup: `--data-dir` → `CARD_BRICK_DATA_DIR` env → `CARDBRICK_DATA`
env (legacy) → `/userdata/saves/cardbrick` on Knulli-style devices →
`./data` next to `main.py`. It contains `cardbrick.db` (SQLite),
`settings.json` and `input_mapping.json` (hand-editable JSON),
`media/`, and `logs/cardbrick.log` (rotating). Nothing mutable is ever
written into the app folder, so the app itself can live on a read-only
mount.

## The daily loop

```
SPANISH PRACTICE
      Maya
restaurant, food
 12 cards today                Front: ¿Dónde está el baño?  ♪
 8 to review + 4 new     -->   D-pad = show answer          -->  ¡Buen trabajo!
Press A to start!              B=Again Y=Hard A=Good X=Easy      session stats
                               R=Bury  START=Menu
```

### Controls (study)

Buttons use this fixed face layout:

| Input          | Question side       | Answer side |
|----------------|---------------------|-------------|
| D-pad          | Reveal answer       | —           |
| B              | Reveal answer       | Again       |
| A              | —                   | Good        |
| Y              | —                   | Easy        |
| X              | —                   | Hard        |
| L1             | Replay audio        | Replay audio|
| R1             | —                   | Bury until tomorrow |
| START          | Action menu (undo / bury / suspend / end) | same |
| SELECT         | Finish session / quit | same |

Keyboard fallback for desktop testing: arrows/Space reveal, `1/2/3/4` =
Again/Hard/Good/Easy (or literal `A/B/X/Y` keys), `L` replay, `R` bury,
`U` undo, `Tab` menu, `Esc` finish/quit.

Raw button indices are mapped to semantic actions through
`input_mapping.json` in the data folder, written by the in-app
calibration screen (Parent Mode → *Controller test & setup*, or
`python main.py --input-diagnostic`). The defaults match common
Anbernic/Knulli ordering but are only a starting guess — calibrate on
real hardware. A font override is available via
`CARDBRICK_FONT=/path/to/font.ttf`; by default the app prefers bundled
Noto Sans CJK/JP fonts when present, then falls back to DejaVu Sans
(full Spanish coverage: á é í ó ú ñ ü ¿ ¡). The packaged Knulli build
copies Noto from the repository's root `assets/font/` folder so
Japanese vocab translations render as real glyphs instead of boxes.
The legacy `review` prototype still honours the
old `CARDBRICK_JOYMAP` env var.

### Four-phase vocab cards

Cards imported from the "Español MX (word + audio + example)" Anki note
type (or an equivalent CSV — see below) use a different review flow
from plain front/back cards, and the two types can be mixed freely in
the same deck/queue:

```
Phase 0: word (+ audio, autoplays)
Phase 1: + example sentence (headword highlighted, + its own audio)
Phase 2: + image
Phase 3: + gendered forms / definition / English and Japanese translations
```

D-pad reveals the next phase; there are no separate rating buttons —
pressing **A ("I know this")** rates the card by *how
much you needed to see*:

| Pressed at phase | Meaning | Rating |
|---|---|---|
| 0 (word only) | knew it instantly | Easy |
| 1 (needed the sentence) | Good |
| 2 (needed the image) | Hard |
| 3 (needed the full definition) | Again |

L1 replays the current phase's audio (word audio at phase 0, example
audio from phase 1 on); R1 bury and START menu work the same as
regular cards. The header, definitions, and gendered forms are shown
as plain text (HTML/CSS from the original card is not rendered — see
Scope, below); the headword highlight inside the example sentence is
reconstructed by a case-insensitive substring match rather than the
original `<span>`.

Import either from `.apkg` (the note type is detected by field names —
`Word` first, an `Example ES` field present — not by a hardcoded model
id) or from a CSV with the same columns:

```bash
python main.py import VocabDeck.apkg
python main.py import vocab.csv --media-dir ./my_media --deck "Mi Vocabulario"
```

CSV audio/image cells accept a bare filename (`gato.mp3`) or an
Anki-style tag (`[sound:gato.mp3]`, `<img src="gato.jpg">`) copied
straight out of a spreadsheet. `--media-dir` points at a folder holding
the referenced files (already-named); they're copied into the app's
media folder. **A CSV card's identity is a hash of its `Word` field**
(there is no Anki note id to key on) — keep that field unique and
unedited across re-imports, or a rename will create a new card instead
of updating the old one's progress.

### Parent mode

`START` on the start screen. From there: import `.apkg` files found in
the data folder (or `data/import/`, or the app folder), choose active
**decks** (which imported decks are *assigned* to the child at all)
and active **categories** (Anki tags — a second, independent filter;
both must match for a card to appear), set daily limits, review/restore
suspended cards, see a 7-day progress table, and flip the study
direction (front-first / back-first). There is no PIN yet — the flows
are separated, not locked.

Decks and categories both default to "all" (`None` in the profile) and
follow the same convention: selecting nothing explicitly means *no*
cards match, rather than falling back to "everything." Set from the
CLI too:

```bash
python main.py profile --decks "Español de México — Vocabulario"
python main.py profile --decks all           # clear the deck filter
python main.py profile --categories restaurant,greetings
```

### Child-facing deck picker

Parent Mode's Decks screen decides which decks are *assigned* to the
child at all; the child still picks which *one* of the assigned decks
(or all of them combined) to study **this sitting**. If more than one
deck is assigned, pressing A on the start screen opens
a picker first — D-pad to choose, A to confirm, B
to go back — showing each option's due-card count. If only one
deck is assigned (or only one deck exists at all), the picker is
skipped automatically and the session starts immediately: no extra tap
when there's no real choice to make.

### Child-facing topic (tag) picker

Parent Mode's Categories screen decides which tags are *assigned* to
the child at all; the same "pick one for just this sitting" pattern as
the deck picker applies to tags too. After the deck choice (if any),
if more than one category is assigned, a "Choose a Topic" screen
offers "All assigned categories" plus one entry per assigned tag, each
with its own due-card count. Picking a tag is pure session-level
filtering — the same non-destructive idea as Anki's filtered decks:
no card's tags or deck membership ever change, so the tag stays part
of the whole collection and "All assigned categories" always includes
it again next time. Skipped automatically when zero or one category is
assigned, same as the deck picker.

### Stamp calendar

A Brain Age-style calendar that stamps each day the child studied, with
**the number of sessions logged that day inside the stamp** — sessions
are meant to be repeated through the day, so the count is the point.
Only sessions with at least one card actually reviewed earn a stamp
(opening the app and backing out doesn't). Today is outlined; a
subtitle sums the month ("36 sessions on 20 days"). Read-only, and
reachable three ways: **X** on the start screen, **X**
on the session-complete screen (the natural "you earned a stamp"
moment), and Parent Mode → *Calendar (stamps)*. Navigate months with
**L1/R1** (or D-pad left/right), **A** jumps back to the current
month, **B**, **START**, or **SELECT** exits. Stamps are per-profile.

### Admin commands (CLI only, destructive)

```bash
python main.py admin purge-decks              # purge every deck
python main.py admin purge-decks --deck Spanish --deck French
python main.py admin purge-decks --yes        # skip the confirmation prompt
python main.py admin reset                    # wipe ALL study context
```

`purge-decks` permanently deletes cards, their FSRS review state,
review log entries, and vocab-card content (cascading deletes, scoped
to the named decks or every deck if `--deck` is omitted). Child
profiles and settings are untouched.

`reset` goes further — a true clean slate: it deletes the whole
database (decks, review history, sessions, child profiles) and all
imported media, so the next start behaves like a first install with a
fresh default profile. Display settings and controller calibration
(`settings.json` / `input_mapping.json`) are kept — they're device
facts, not study context.

Both commands always back up the database first, to
`cardbrick.db.backup-<label>-<timestamp>` next to the live database,
and always ask `Type 'yes' to confirm:` unless `--yes` is passed —
there's no undo button in the child-facing UI, so these are the only
commands in the app that discard data outright. They are admin/CLI
only; deliberately not exposed in Parent Mode's on-device UI.

### Daily goal & sprints (microstudying)

The day is a card goal chipped away in short sprints, not one sitting.
Per child profile: `daily_goal_cards` (default 150) is the day's goal
in distinct cards (a card repeating a learning step counts once); each
sprint is bounded by `session_card_limit`
(default 20) and `session_time_minutes` (10, 0 = off), so the child
always knows how many sprints they still owe ("3 sprints to go today").
Doing sprints back-to-back — "going ahead" — burns the count down just
the same as spacing them out; the schedule is the child's own and
nothing nags.

New-card intake is paced by the goal itself (`daily_new_cards` default
0 = auto): new cards fill whatever the goal has left after due
reviews, so day one of a fresh deck is a full goal-sized day of
sprints, and as reviews build up they crowd new cards out
automatically — when reviews alone exceed the goal, no new cards enter
until the backlog drains. Set `daily_new_cards` to a number for a
stricter fixed drip instead.

The plan never promises more than the day can supply — a small deck
that runs out of cards, or a fixed new-card cap, shrinks `goal_today`
so the child always sees an honest, finishable day.
When today's due cards run out before the goal, sprints top up with
soon-due cards pulled forward (`study_ahead_days`, default 1 — safe
under FSRS, an early review just earns a smaller stability gain), and
once the day is done an optional bonus sprint is offered; bonus
sprints may also pull extra new cards beyond the daily pacing, one
sprint at a time, so a keen child never hits a dead end. Set
`study_ahead_enabled` to 0 to end the day when the due pool is empty
instead. All counters are derived from the review log, so they survive
restarts and undo; a backlog never floods the child — extra due cards
arrive one sprint at a time.

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

Full instructions, launch-script template, package layout, and a
manual pre-flight checklist live in **`deploy/knulli/`** at the repo
root. The short version:

1. On a PC, vendor the dependencies (no on-device pip, ever):
   `pip install --target vendor --platform manylinux2014_aarch64
   --only-binary=:all: -r requirements.txt`
2. Copy `cardbrick-py/` + `vendor/` + the adapted
   `deploy/knulli/launch_cardbrick_spanish.sh` to
   `/userdata/roms/ports/CardBrickSpanish/`.
3. Run `python3 main.py --knulli --smoke-test` (the launch script does
   this automatically on first boot) and read
   `/userdata/saves/cardbrick/logs/`.
4. Calibrate the controller in Parent Mode before the first session.

## Scope (deliberately excluded)

No sync, no cloze, no image occlusion, no images at all, no HTML
rendering beyond tag stripping, no card templates, no `.apkg` export, no
TTS (the audio layer is structured so it can be added later), no
gamification. This is a focused daily study appliance:
**apkg → local DB → py-fsrs → pygame**.
