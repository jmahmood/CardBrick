# Pattern Content Generation Guide

**Purpose:** turn rows from the pattern inventory (files `01`–`08` in
this folder) into JSON drill items for CardBrick's `pattern_pack`
importer. Read this file once; each tier file (`01-core.md`, etc.)
is just a table of patterns plus a short reminder to come back here.

Background: [PATTERN_DRILLING_PLAN.md](../PATTERN_DRILLING_PLAN.md)
(product design). Ground truth for the output format:
[`cardbrick-py/cardbrick/pattern_pack.py`](../../cardbrick-py/cardbrick/pattern_pack.py)
— if this guide and that file ever disagree, the code wins.

## What you're producing

A **JSON array of item objects** — not a full pack file. Don't wrap
it in `{"format": ..., "deck": ..., "items": [...]}`; that wrapper is
assembled at merge time from one or more generated batches. Just
output the array.

Two kinds of item, both produced from the same pattern row:

- **`production`** — a scaffolded say-it-aloud card. The learner sees
  an English cue, and can ask for progressively more help: a sibling
  sentence, then a fill-in-the-blank skeleton, then the answer.
- **`mcq`** — a 3-option multiple-choice card. Produce one for
  patterns whose "Generation constraints" column names a discrete,
  teachable contrast (see "When to add an MCQ" below).

Default: **one `production` item per pattern**, plus an `mcq` item
where a good contrast exists. Don't feel obligated to produce both
for every row.

## Required fields (must match the importer's validation exactly)

Common to both kinds:

| Field | Value |
|---|---|
| `pattern_id` | Copy verbatim from the table, e.g. `"P012"`. |
| `kind` | `"production"` or `"mcq"`. |
| `variant` | Integer, default `1`. Only increment if you're deliberately generating a second card for the same pattern+kind (see "Card identity" below). |
| `tier` | Copy from the tier file's front matter, e.g. `"core"`. |
| `gate` | Copy from the table's Gate column, e.g. `"A1"`. |
| `priority_score` | Table's Score column **divided by 100**, e.g. table score `96` → `0.96`. (Matches the convention already shipped in `assets/patterns/sample_pack.json`.) |
| `template` | Copy the Canonical template column verbatim. |
| `tags` | One or more freeform `topic:xxx` tags reflecting the sentence's real-world context — `topic:transit`, `topic:health`, `topic:money`, `topic:emergency`, `topic:smalltalk`, `topic:taxi`, `topic:restaurant`, `topic:shopping`, `topic:requests`, etc. Space-separated if more than one. Grammar labels such as `topic:past-tense`, `topic:subjunctive`, or `topic:fronting` are not topics and must not appear here. **Do not** add `tier:` or `gate:` tags yourself — the importer adds those automatically from the fields above. |

`production` adds:

| Field | Value |
|---|---|
| `prompt_en` | An English **task cue**, not a translation — "Ask whether...", "Tell the pharmacist...", "Say that...". Describe a realistic situation (Mexico City travel context where the pattern allows it — see tier files for which ones are safety/travel-weighted). **A competent speaker given `prompt_en` must be able to produce `answer_es` or a trivial variant: put every content word needed by the target in the cue rather than leaving the event, object, place, or other details open. When the pattern drills verb morphology, explicitly pin the intended person and number (and any otherwise ambiguous gender) in the cue.** |
| `sibling_es` | A **different, complete** Spanish sentence using the *same construction*, with different lexical content than the answer. This is the "similar sentence" hint — it must itself be natural and correct. |
| `skeleton_es` | The `answer_es` sentence with its key inflected word/phrase (the part that varies by person/tense/mood) replaced by `___`. Everything else stays in Spanish — this is a scaffold, not an English gloss. |
| `answer_es` | The single canonical, idiomatic Spanish sentence that correctly answers `prompt_en` and satisfies the pattern's template and constraints. |
| `answer_audio`, `sibling_audio` | Leave as `""` — no audio in this build. |

`mcq` adds:

| Field | Value |
|---|---|
| `cue_en` | English situation cue ending in "Which is correct?" |
| `options` | Exactly 3 objects `{"text": "...", "correct": true\|false}`, **exactly one** `correct: true`. The two wrong options must be plausible near-misses that break one specific rule each (see below) — not absurd, not unrelated vocabulary. |
| `constraint_note` | One sentence explaining the rule that makes the correct option right and the others wrong — teach it, don't just flag it. Required; the importer rejects items without one. |

## Card identity — don't rename things

The importer identifies a card by hashing `pattern_id:kind:variant`.
Re-importing an edited pack updates that card's content in place
without resetting the learner's progress — **but only if the triple
stays the same.** Never invent a new `pattern_id` for content that
belongs to an existing row; use the table's ID. If you want a second
production card drilling the same pattern with different content, set
`"variant": 2` on it rather than duplicating `variant: 1`.

## Reading the notation columns

Each tier table carries the report's original `Pre` (grammatical
prerequisites) and `Xf` (allowed transformations) columns and a
`Generation constraints` column. You don't need to satisfy `Xf` in
this build (that's for future drill chains) — but `Pre` and
`Generation constraints` tell you what the sentence must get right:

| Constraint tag | Means | MCQ opportunity |
|---|---|---|
| `SER/EST` | ser vs. estar contrast | Yes — classic distractor: swap ser/estar |
| `PREP(x)` | requires a specific preposition | Yes — swap for a similar preposition (por/para, a/en, de/desde) |
| `PA` | personal *a* before an animate direct object | Yes — omit or misplace the *a* |
| `AGR` | agreement (gender/number) | Yes — mismatch the agreement |
| `GUS` | gustar-type verb agrees with the thing liked, not the person | Yes — the classic "me gusta los tacos" error |
| `REFL` | reflexive/pronominal verb | Yes — drop or misplace the reflexive pronoun |
| `CL` | clitic pronoun placement | Yes — misplace the clitic |
| `SE` | impersonal/passive se | Sometimes — omit se or use wrong person |
| `UST` | tú/usted register choice | Yes — swap tú for usted imperative or vice versa |
| `SUBJ` | subjunctive mood triggered | Sometimes — swap in indicative |
| `MX` | use Mexico-appropriate vocabulary | No — just a lexical reminder, not a distractor source |
| `PR`/`PT`/`IM`/`PF`/`COND`/`MOD` | tense/aspect/modality requirement | Sometimes — wrong tense as a distractor, if it's a common learner error |

`Pre` values (`SER,PR`, `PT,MOD`, etc.) tell you what grammar the
sentence assumes is already in place — mostly useful context, not
something you need to output.

## Register and vocabulary

- Everything is **Mexican Spanish**. Prefer *boleto* over *billete*,
  *plática* is fine but keep vocabulary broadly understandable to a
  traveler; when a constraint tag says `MX`, that row specifically
  flagged a Mexico-specific lexical choice — get it right.
  - Default to **usted** for service interactions (taxi drivers,
    police, waitstaff, hotel staff, doctors) and **tú** for
    peer/family framing, following the table's own Example column as
    a guide when given. Where a row's `Generation constraints` names
    `UST` explicitly, get the register right — that's the point of
    the pattern.
- Use real accents and punctuation: `¿…?`, `¡…!`, á é í ó ú ñ ü.
- It's fine to skip a pattern (note it and move on) if you can't
  produce a natural, unambiguous sentence for it — this is rare but
  happens for a few of the more abstract narrative/discourse
  patterns.

## Worked examples

```json
{
  "pattern_id": "P012", "kind": "production", "variant": 1,
  "tier": "core", "gate": "A1", "priority_score": 0.96,
  "template": "¿[S] tener que [Inf]?",
  "tags": "topic:transactions",
  "prompt_en": "Ask whether you have to pay here.",
  "sibling_es": "¿Tengo que salir temprano?",
  "skeleton_es": "¿ ___ que pagar aquí?",
  "answer_es": "¿Tengo que pagar aquí?",
  "answer_audio": "", "sibling_audio": ""
}
```

```json
{
  "pattern_id": "P018", "kind": "mcq", "variant": 1,
  "tier": "core", "gate": "A1", "priority_score": 0.95,
  "template": "a [S] le gusta(n) [N]",
  "tags": "topic:smalltalk",
  "cue_en": "Say that your daughter likes tacos. Which is correct?",
  "options": [
    {"text": "A mi hija le gustan los tacos.", "correct": true},
    {"text": "A mi hija le gusta los tacos.", "correct": false},
    {"text": "Mi hija gusta los tacos.", "correct": false}
  ],
  "constraint_note": "gustar agrees with the thing liked (los tacos → gustan), and the person takes 'a … le'."
}
```

More worked examples covering other constraint types (SER/EST,
PREP(x), PA, UST) are in
[`assets/patterns/sample_pack.json`](../../cardbrick-py/assets/patterns/sample_pack.json)
— that file is real, already-imported content, not just a template.

## Batch size

Generate in batches of roughly **15–20 patterns per request**, even
within a single tier file — asking for a whole 80–100 row tier in one
shot tends to produce shallower, more repetitive sentences than
working through it in chunks. Reference the pattern IDs you're
covering (e.g. "P001–P020") when you ask.

## Merging output

Collected item arrays get concatenated into the `items` list of a
pack file:

```json
{
  "format": "cardbrick-pattern-pack",
  "version": 1,
  "deck": "Mexico City — Drills",
  "items": [ /* generated items go here */ ]
}
```

then imported with:

```bash
python main.py import path/to/pack.json --deck "Mexico City — Drills"
```
