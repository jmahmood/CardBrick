# Plan: Flagging wrong / inappropriate sentences (TODO item #3)

## Context

Vocab cards pair a headword with an example sentence and English/Japanese
translations (`vocab_cards`: `word`, `word_en`, `word_jp`, `definitions`,
`example_es`, `example_en`, `example_jp`). In practice some example sentences
are **wrong or inappropriate for a new learner**:

- The English translation uses a sense of the word that isn't in the card's
  `definitions` list (the sentence "proves" a meaning the card never taught).
- The sentence is a colloquialism/idiom — fine in principle, but that sense is
  again missing from `definitions`, so the learner can't connect them.

Today the only recourse is **bury** (back tomorrow) or **suspend** (hide until
a parent unsuspends) — neither records *what* is wrong or feeds any correction
loop. This plan adds a first-class **flag** with a reason, a parent triage
screen, and an **export → local pipeline → apply** loop so flagged sentences
can be analyzed off-device and used to fix either the sentence or the word
card. The intended outcome: bad sentences stop reaching the child quickly, and
each flag becomes an actionable correction rather than a silently hidden card.

## Recommended defaults for the three open decisions

These shape the design below. Any can be changed before implementation.

1. **Pipeline host / backend — computer, pluggable backend (default: manual,
   optional local LLM).** The handheld (ARM) can't run an LLM. Flags already
   ride upstream inside the sync backup (`create_backup` copies the whole DB,
   `sync.py:190/223`), so the pipeline runs on your computer against that DB.
   The app owns only a stable **export/apply contract**; the classifier lives
   in a swappable script — a deterministic manual/review mode by default, with
   an optional local-LLM backend (Ollama/llama.cpp over HTTP).
2. **On flag — auto-suspend the card (default).** Flagging means "this is
   wrong/inappropriate," so the card is hidden from the child immediately via
   the existing suspend path, until the pipeline resolves it. Configurable.
3. **Flag entry — both.** A quick in-review flag action with a reason picker,
   plus a parent-mode triage screen to add/edit reasons and resolve.

## Data model

New table (additive migration — no backfill needed):

```sql
CREATE TABLE IF NOT EXISTS sentence_flags (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id       INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    reason        TEXT NOT NULL,       -- wrong_translation | meaning_not_listed
                                       -- | inappropriate | other
    note          TEXT,                -- optional free text
    example_snapshot TEXT,             -- JSON of example_es/en/jp at flag time
    status        TEXT NOT NULL DEFAULT 'open',   -- open | resolved | dismissed
    resolution    TEXT,                -- sentence_updated | card_updated
                                       -- | no_change | dropped
    created_at    TEXT NOT NULL,
    resolved_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_flags_status ON sentence_flags(status);
```

- Add the `CREATE TABLE`/index to the `SCHEMA` string and bump `SCHEMA_VERSION`
  in `cardbrick/storage.py`. `_migrate()` already runs `executescript(SCHEMA)`
  with `IF NOT EXISTS`, so no new migration branch is required (follows the
  same additive pattern as existing tables).
- The `example_snapshot` records what was on the card when flagged, so a later
  content re-import doesn't lose the evidence.

New `Storage` methods (next to `suspended_cards`/`set_suspended`,
`storage.py:560`): `add_sentence_flag(card_id, reason, note, snapshot)`,
`open_flags()` (join `vocab_cards` for display + export context),
`flags_for_card(card_id)`, `resolve_flag(flag_id, resolution)`,
`dismiss_flag(flag_id)`.

## In-review capture (child)

- Add one entry to `MENU_ENTRIES` (`app.py:1364`), e.g.
  `"Report sentence (wrong/inappropriate)"`. Selecting it opens a **reason
  sub-picker** (reuse the existing overlay-menu rendering) listing the four
  reasons in the user's own terms: *wrong translation*, *meaning not listed /
  colloquial*, *inappropriate*, *other*.
- On confirm: `storage.add_sentence_flag(...)`, then (default) suspend the card
  through the existing `discard_current("SUSPENDED", suspend=True)` closure
  (`app.py:1805`), printing a `FLAGGED` stamp instead of `SUSPENDED`. This
  reuses the suspend persistence path verified earlier, so the card leaves the
  child's rotation at once.
- Snapshot the current `example_*` fields from `vocab_detail` into the flag.

## Parent triage screen (`PARENT_FLAGGED`)

- Model it on `screen_parent_suspended` (`app.py:3199`): a scrollable list of
  open flags showing `card["front"]` + reason. Detail view shows word,
  `definitions`, `example_es/en/jp`, reason, and note. Actions: **Dismiss**
  (no_change), **Keep suspended**, **Unsuspend** (issue judged fine).
- Register the screen in the state dispatch table (`app.py:685`, alongside
  `"PARENT_SUSPENDED"`) and add a menu entry in `screen_parent_menu`
  (`app.py:2598`), e.g. `("Flagged sentences", "PARENT_FLAGGED")` with the
  `self._jp(...)` bilingual label like its neighbors.

## Export → pipeline → apply

**Export — `main.py` subcommand `flags export [--out flags.json]`.** Emits
`open_flags()` as JSON, each entry carrying full context the classifier needs:
`card_id`, `word`, `word_en/jp`, `definitions`, `example_es/en/jp`,
`image_filename`, `reason`, `note`, `example_snapshot`. Reuse the existing
subcommand wiring (`sub.add_parser(...)` in `main.py:62`) and open `Storage`
exactly as the other commands do.

**Pipeline — `scripts/flag_pipeline.py` (runs on your computer).** Reads the
exported JSON and, per flag, classifies into an action:
- `rewrite_sentence` — replace `example_es`/translations with a clean sentence
  that actually demonstrates a listed meaning;
- `extend_card` — add the missing (colloquial/symbolic) sense to `definitions`
  (and/or `word_en/jp`) so the existing sentence is justified;
- `drop` — inappropriate; remove/replace the sentence.

Backend is an interface with two implementations: `manual` (prints the card
context and prompts you to choose/type the fix — deterministic, default) and
`llm` (posts the context to a local Ollama/llama.cpp endpoint and parses a
structured suggestion). Output is a **resolution pack**: updated vocab rows
(CSV in the existing `vocab_csv` shape, or JSON) plus a `resolutions.json`
mapping `flag_id → {resolution, fields}`. Keeping the classifier here (not in
the app) is what makes it "local, on my device" and swappable.

**Apply — `main.py` subcommand `flags apply <resolution-pack>`.** For each
resolution: update `vocab_cards` via the existing `upsert_vocab_card`
(`storage.py:342`) — which by design **updates content only and preserves FSRS
progress** — then `resolve_flag(flag_id, resolution)`; unsuspend cards whose
sentence was fixed. Because updated rows are just vocab content, the same fixes
can alternatively be shipped as a normal content pack and applied through the
existing sync → `import_content` → upsert path; `flags apply` is the direct
offline route.

No new sync protocol is needed: flags are DB rows, so they already travel in
the backup upload; corrections travel back as vocab content the existing
importer knows how to apply.

## Files to modify / add

- `cardbrick/storage.py` — `sentence_flags` table in `SCHEMA`, `SCHEMA_VERSION`
  bump, flag CRUD methods.
- `cardbrick/app.py` — reason picker off `MENU_ENTRIES`; `FLAGGED` capture via
  `discard_current`; `screen_parent_flagged`; state-dispatch + parent-menu
  entries; footer/label copy.
- `main.py` — `flags export` / `flags apply` subcommands + handlers.
- `scripts/flag_pipeline.py` *(new)* — pluggable classifier (manual + optional
  local-LLM backend), emits the resolution pack.
- `tests/test_sentence_flags.py` *(new)* — storage CRUD, auto-suspend on flag,
  export shape, apply-preserves-progress; extend an existing menu/footer test.
- `README.md` / `docs/` — document the flag action, parent screen, and the
  export→pipeline→apply loop.

## Verification

1. **Unit tests** (`python -m pytest`): flag insert/open/resolve/dismiss;
   flagging auto-suspends; `flags export` JSON contains full card context;
   `flags apply` updates `vocab_cards` while `review_state` (reps/due) is
   unchanged — assert the FSRS-progress-preserved guarantee explicitly.
2. **Headless end-to-end** (a script like the suspend repro used earlier):
   seed a vocab card → add a flag (verify auto-suspend + it appears in
   `open_flags()` and `suspended_cards()`) → `flags export` → hand-write a
   resolution pack → `flags apply` → assert the sentence/definitions changed,
   the flag is `resolved`, the card is unsuspended, and reps/due are intact.
3. **Run the app** (`python main.py`): flag a sentence from the in-review menu
   (reason picker + `FLAGGED` stamp, card leaves rotation), then open parent
   mode → *Flagged sentences* and confirm the entry, detail view, and
   dismiss/unsuspend actions.

## Suggested build order

1. Storage table + methods + tests.
2. In-review reason picker + auto-suspend capture.
3. Parent `PARENT_FLAGGED` triage screen.
4. `flags export` / `flags apply` CLI + apply-preserves-progress test.
5. `scripts/flag_pipeline.py` scaffold (manual backend first, LLM backend behind
   the same interface).
6. Docs.

This also lays the groundwork for TODO item #5 (audit feed): the same
`sentence_flags` rows and export contract are exactly what an automated audit
would populate and consume.
