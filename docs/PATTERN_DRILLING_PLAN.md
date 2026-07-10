# Pattern Drilling — Implementation Plan

**Status:** Proposal · **Date:** 2026-07-09 · **Owner:** Jawaad Mahmood
**Source:** "CardBrick Spanish Construction System for Mexico City"
(deep-research report, 500-pattern inventory) · **Sibling doc:**
[LINGUISTIC_INFRASTRUCTURE.md](LINGUISTIC_INFRASTRUCTURE.md)

## 1. What we accept and reject from the report

The report delivers two separable things: a **content inventory**
(500 construction families with tiers, CEFR gates, priority scores,
transformation sets, and constraint annotations) and a **delivery
design** (drill progressions, scoring, ASR, session shape). The
inventory is the valuable part. The delivery design is right in
outline and wrong in several hardware assumptions.

**Keep:**

- The 500-pattern inventory, its tier/gate/score structure, and the
  JSON schema (trimmed — see §5).
- The six-transformation taxonomy: lexical substitution, person
  change, negation, tense/aspect change, question formation,
  politeness shift.
- The drill-progression shape: model → substitutions/transformations
  → prompted production → one open prompt per session.
- "Separate production **mode** in the UX, not a separate data
  model." This matches our architecture exactly.
- Learner-side assessment (not ASR) as the primary mastery signal —
  but expressed through CardBrick's scaffold-ladder grammar, never
  as an explicit rating choice (see Reject list and §3).
- Short hard-bounded sessions (3–5 min) — this is literally
  CardBrick's existing sprint system.
- The Mexico City safety micro-pack pinned first, and the
  Week A–D rollout ordering.
- Launch with ~180 patterns, model all 500.

**Reject:**

- **Anything requiring a microphone.** RG35XX SP / TrimUI Brick class
  devices have no mic. No recording, no waveform playback of the
  learner, no ASR — not even as a "support layer." The report itself
  concedes ASR is unreliable for grading; we go further because the
  hardware makes the question moot. Replacement signals: the
  scaffold-ladder position at which the learner could answer (§3),
  the B-mistake press after reveal, and **cue-to-reveal latency**
  (objective fluency proxy, logged from day one, used for scheduling
  later).
- **Explicit Again/Hard/Good/Easy buttons.** CardBrick already
  rejected Anki's four-way self-grading for vocab cards; drills must
  not reintroduce it. The vocab interaction grammar is preserved
  wholesale: D-pad = more help, A = "I know it" (→ reveal →
  confirm, B = "I was wrong"), and *the amount of help consumed is
  the rating*. No rating choice, ever — for children or adults.
- **Any on-device generation.** The handheld stays a dumb offline
  appliance (per LINGUISTIC_INFRASTRUCTURE's non-negotiable
  constraints). All expansion, validation, and audio synthesis happen
  at compile time on the Mac.
- **Three scheduling channels at launch.** Lexeme mastery,
  construction mastery, and lexeme-inside-construction mastery as
  separate FSRS channels is premature (and sibling-doc H2 exists
  precisely to test whether channels diverge). MVP: one FSRS state
  per construction card. Log per-step evidence so channels can be
  split later without losing history.

## 2. Separate application vs. integrated mode

### Separate app

*Pluses:* total freedom to iterate; zero risk to the working
kid-facing appliance; UI can be adult-oriented without compromise;
could ship on other platforms later.

*Minuses:* duplicates nearly everything that took real effort to get
right — FSRS integration, durable SQLite write path, undo, sprint
accounting, controller mapping/calibration, audio backend fallback,
fonts, Knulli deploy pipeline, smoke tests; two databases means no
unified mastery picture (directly contradicts the central-
infrastructure direction); double maintenance on a hobby time budget;
and the trip deadline makes a from-scratch build the slowest path.

### Integrated mode (recommended)

*Pluses:* the four-phase vocab card already proves the architectural
slot we need — **a card type with multi-step presentation that
produces a single FSRS rating** (`card_type` column + satellite
table + a presentation branch in `app.py`). A drill chain is the same
shape with different steps. Reuses scheduler, storage, session
runner, sprints, deck/topic pickers, parent mode, stamp calendar,
deploy. One review log feeds the future (unit, skill) mastery model.
Family members already map to child profiles.

*Minuses:* `app.py` is 2,736 lines and grows again (mitigation: new
`drill_card.py` module, `app.py` only dispatches); a regression risk
to the child's working app right before a trip (mitigation: new
card type is inert unless a pattern pack is imported — zero behavior
change for existing decks; plus the existing pytest suite).

**Hybrid for the launcher:** if "feels like a separate app" matters
on-device, add a second Ports menu entry (`CardBrick Drills`) that
launches the same install with a `--profile`/deck preselection flag.
Two icons, one codebase, one database. Decide at deploy time; costs
one launch script.

**Decision: integrated mode.** The report reached the same conclusion
("separate production mode in the UX, but not a separate data
model") and our codebase makes the integrated path the cheap one.

## 3. How a user will use it (UX)

### Hardware reality

D-pad, A/B/X/Y, L1/R1, START/SELECT, speaker. No mic, no keyboard,
no touch, no network at study time. 640×480-class screen. Everything
below uses only these.

### The drill sprint

A pattern pack is a deck. It shows up in the existing child-facing
deck picker ("Mexico City — Drills") and topic picker (tags = tier,
gate, and topic: `safety`, `transit`, `restaurant`, …). A sprint is
5–8 pattern cards; each card is a **drill chain** of 4–8 steps.
Target: 3–5 minutes, same sprint bounds as today.

### The interaction grammar (identical to vocab cards)

The four-phase vocab card established CardBrick's answer model, and
drills reuse it without modification: **D-pad down = show me more
help; A = "I know it" → full answer revealed → A confirms / B =
"I was wrong"; the phase you answered at *is* the rating**
(`{0: Easy, 1: Good, 2: Hard, 3: Again}`, same map as
`VOCAB_PHASE_RATING`). No Again/Hard/Good/Easy buttons anywhere.

What changes is only *what the phases contain*. For a vocab card the
ladder is word → example → image → definition. For a drill step the
ladder is a **production scaffold**:

| Phase | Shown (cumulative) | Answering here means |
|---|---|---|
| 0 | The task cue: "Ask whether you have to pay here" / "Make it negative: *Tenemos que pagar aquí*" | Easy — produced cold |
| 1 | + a **sibling sentence**: the same construction with different content, with audio — "*¿Tengo que salir temprano?*" | Good — the pattern came back after one reminder |
| 2 | + the **skeleton**: the answer with its slots blanked — "¿ ___ que pagar aquí?" | Hard — needed the frame spelled out |
| 3 | + the full model answer + audio; echo it aloud | Again |

Phase 1 is the "path to remembering" — a similar sentence rather
than the answer, so a stuck learner gets re-exposed to the *pattern*
and still produces the *target* themselves, credited as
Good-not-Easy. It also doubles as covert extra exposure: every hint
is another instance of the construction, with audio.

Pressing A at phases 0–2 reveals the remaining phases (so the model
answer + audio always plays for verification — the learner says
their sentence aloud, then hears the model) and enters the same
confirmation state as vocab: A = I had it, B = I was wrong → Again.
Cue-to-A latency is recorded per step.

Sibling sentences and skeletons are compiler outputs — the expansion
machinery already generates multiple fillers per pattern, so hints
are free content, validated like everything else.

### Drill chains

A pattern card in chain form (Phase 2) is 4–8 steps — model echo,
then substitutions/transformations, then a prompted production —
each presented with the scaffold ladder above. The common case is
fast: a known step is A → (glance/listen) → A, two presses; the
ladder only unrolls when stuck. The ECHO step (model sentence +
audio, say it aloud, press to continue) has no grading — warm-up.

Chain → single FSRS rating, derived mechanically, no buttons:
**the worst rung reached across the chain's graded steps** (any
B-mistake → Again). Same semantics as vocab — "how much help did
this construction need today" — and per-step phases, mistakes, and
latencies go to a new `drill_step_log`, while the aggregate rating
goes through the **existing** review path (review_log, undo,
durability) untouched. If worst-rung proves too harsh over long
chains, soften it from the step log data later; don't add UI.

One **OPEN** step per sprint (not per card), at the end: a scenario
cue ("You missed a gate change. Ask if you have to go to another
terminal."), same ladder, graded leniently (its sibling hint is a
worked example of a valid answer).

### Multiple-choice cards (3 options, A reserved)

MCQ is in, with one rule protecting the interaction grammar:
**exactly 3 options, mapped to X / Y / B — A is never an answer
button.** A stays reserved for "I know it" everywhere else (and for
future use on MCQ screens), so there is no A-spamming and no
conflict with the confirm grammar. Behavior:

- Cue plus 3 candidate sentences, one correct, two violating the
  pattern's constraint annotations (agreement, preposition,
  ser/estar, mood). Option order is shuffled deterministically per
  card.
- Correct → Good; correct and fast (≤ ~5 s, tunable) → Easy. No
  buttons involved in the rating — latency does the promoting.
- Wrong → Again; the correct sentence is printed below with a short
  note on *why* the distractors are wrong.
- D-pad down while unanswered = "show me" bail-out → Again — the
  same "D-pad = help" philosophy as the ladder.

MCQ gives the objective signal self-assessment can't, without
reintroducing rating buttons.

### Adults vs. child

Same appliance, same controls. Family members are child profiles;
each gets their own FSRS state, sprint counters, and stamps. The
deck/category assignment in parent mode already scopes who sees
drills. The daily loop stays: pick deck → sprint → completion screen
→ stamp.

## 4. How we serve it (content pipeline)

All heavy lifting happens on the Mac, in `cardbrick-py/scripts/`:

```
patterns.json          family.json         lexicon/*.json
(500 entries,          (names, genders,    (slot fillers per
 report schema,         relations, trip     topic: foods, lines,
 hand-corrected)        details)            neighborhoods)
        \                   |                   /
         \                  |                  /
          `pattern pack compiler` (scripts/build_pattern_pack.py)
            1. select patterns (tier/gate/score filter, e.g. MVP-180)
            2. expand slots → concrete steps per chain
               (LLM-assisted generation on the Mac, then a rule
                validator enforcing each pattern's constraint
                annotations: AGR, PREP(x), SER/EST, GUS, SE, PA,
                clitic placement, mood licensing)
            3. render a human review sheet (markdown/HTML) —
               spot-check before packaging
            4. TTS every model sentence (offline es-MX voice —
               Piper es_MX, or macOS `say -v Paulina`; decide in
               Phase 1 by ear)
            5. emit pack: drills.json + media/*.mp3 (zip)
                    |
                    v
        python main.py import mexico-drills.cbpack
        (importer extension; same durability rules as .apkg —
         re-import preserves review state, card identity keyed
         on pattern id P###)
```

Design rules, inherited from the report and the importer's existing
conventions:

- **Validate templates, don't trust free generation.** The compiler's
  validator rejects any expansion that violates the pattern's
  declared constraints. LLM proposes; rules dispose; a human skims
  the review sheet.
- **Card identity = pattern id** (`P012`), like the CSV `Word`-hash
  rule: re-importing a rebuilt pack (new fillers, fixed typo) updates
  content without resetting FSRS progress.
- **Gating via tags.** `gate:A0`…`gate:B1`, `tier:core`,
  `topic:safety` — parent mode's existing category machinery does
  ordering and filtering without new UI. The safety micro-pack is
  just `topic:safety` assigned first.

### Data model (delta)

- `cards.card_type = 'pattern'` (column exists; new value).
- New satellite table `pattern_cards`: `card_id`, `pattern_id`,
  `tier`, `gate`, `priority_score`, `template`, `steps_json`,
  `notes`. Same pattern as `vocab_cards`. Each step in `steps_json`
  carries its full scaffold ladder: type, cue, sibling sentence,
  skeleton, model answer, and audio filenames (sibling + answer).
- New `drill_step_log`: `card_id`, `session_id`, `step_index`,
  `step_type`, `phase_reached`, `mistake` (B on confirm),
  `latency_ms`, `ts`. Append-only. This is
  the raw evidence that later feeds channel-splitting (H2) and
  latency-aware scheduling; the FSRS write path does not change.
- One FSRS `review_state` row per pattern card, as today.

## 5. Family personalization (design now, cards later)

The memorability lever: fill `[S]`, `[person]`, `[N]`, `[Loc]` slots
with **your actual family and your actual trip** (self-reference
effect, plus the blunt practical fact that "Mi hija es alérgica
a …" and "¿Cómo llego al hotel en …?" are the exact sentences you
will need in Mexico City).

`family.json` (schema defined in Phase 0, content later):

```json
{
  "members": [
    {"name": "…", "relation_es": "mi hija", "gender": "f",
     "notes": ["alergias", "gustos"]}
  ],
  "home": {"city": "…", "country_es": "Canadá"},
  "trip": {"city": "Ciudad de México",
           "neighborhood": "…", "hotel_name": "…",
           "planned_places": ["el Zócalo", "Coyoacán", "…"]}
}
```

Compiler uses it three ways:

1. **Slot filling** — substitution steps rotate through family
   members and planned places instead of stock nouns; gender/number
   drive the agreement the validator checks (`Mi hermana es médica`
   vs `Mi hermano es médico` becomes a real contrast, about real
   people).
2. **Person-change cues** — "Now say it about ⟨name⟩" instead of
   abstract "third person singular."
3. **Trip grounding** — route/taxi/safety patterns reference the
   actual hotel neighborhood and destinations.

Privacy: `family.json` lives on the Mac, compiled output lives on
the family's own devices. Nothing leaves the house.

Explicitly deferred: writing the actual family content and generating
those cards. Phase 0 freezes the schema so pattern data files can
reference slots; population is Phase 3.

## 6. Phased plan

### Phase 0 — Foundations (no device code)
- Commit this plan; freeze the trimmed pattern JSON schema (report's
  schema minus `mexico_notes` prose, plus `steps` recipe per
  transformation) and the `family.json` schema.
- Transcribe the report's tables into `data/patterns/patterns.json`
  (all 500 rows — mechanical; LLM-assisted extraction, hand-skim).
- Select the MVP-180 slice: core P001–P060, interaction P101–P140,
  transactional P241–P280, travel/safety P301–P380 highlights, plus
  the emergency/health tier P421–P434 (report's safety micro-pack).
- **Exit:** `patterns.json` validates against the schema; MVP slice
  tagged.

### Phase 1 — MVP on device (single-step cards)
The fastest path to drilling before the trip: skip chains, ship
**one production step per card** with the full scaffold ladder —
cue → sibling → skeleton → answer, A-to-answer, confirm/mistake —
plus standalone 3-choice MCQ cards (§3) in the same queue.
The interaction is *exactly* the vocab flow with different phase
content, so the on-device work is a focused generalization, not a
new screen:
- Extract the vocab card's phase machinery (`phase`,
  `known_confirmation`, `VOCAB_PHASE_RATING`, `print_up_to`) into a
  shared "phased card" path where vocab and pattern are two
  phase-content providers; `pattern_cards` migration in
  `storage.py` (same `_upgrade_table` mechanism as vocab).
- `build_pattern_pack.py` v1: expansion (target + sibling +
  skeleton per pattern) + validator + review sheet + TTS + emit a
  pack the importer understands.
- Pick the TTS voice by ear (Piper es_MX vs macOS Paulina).
- Import on device, assign deck/tags in parent mode, study.
- **Exit:** a family member completes a 20-card drill sprint on the
  RG35XX SP from the MVP slice, with audio, never seeing a rating
  button.

### Phase 2 — Drill chains
- `drill_step_log` migration; chain runner in `drill_card.py`:
  step sequencing (ECHO → transforms → prompted production), the
  scaffold ladder per step, worst-rung rating aggregation, latency
  capture; `app.py` dispatches on `card_type == 'pattern'` exactly
  as it does for `vocab`.
- Compiler v2 emits full chains; `.cbpack` import.
- One OPEN step per sprint, injected by the session runner.
- Tests: chain aggregation, step logging, undo across a chain,
  crash mid-chain (mirrors existing durability tests, injected
  clock, no pygame).
- **Exit:** sprint of 5–8 chains ≈ 3–5 min; per-step log rows on
  device; existing decks and tests unaffected.

### Phase 3 — Family personalization + rollout packs
- Populate `family.json`; compiler slot-filling + person-change cues.
- Build the four rollout packs (report Week A–D) as separate decks/
  tag sets; assign per family member in parent mode; safety pack
  first.
- Optional second launcher entry ("CardBrick Drills").
- **Exit:** each traveling family member has an assigned,
  personalized drill queue; safety pack completed before departure.

### Phase 4 — Evidence & convergence (post-trip, optional)
- Use `drill_step_log` latencies to modulate ratings (fast Good →
  Easy), per the report's staged-evidence weighting.
- Evaluate splitting construction vs lexeme channels — this is
  sibling-doc H2; the step log is the dataset.
- Fold into the central (unit, skill) mastery model when that work
  lands.

## 7. Risks

- **Content quality** is the real risk, not code. Mitigation: the
  validator + human review sheet; start with the report's own
  examples verbatim (already vetted) and personalize incrementally.
- **Self-assessment honesty** (especially kids). The ladder already
  helps — a child who doesn't know the answer has a rewarding move
  (D-pad for a hint) that isn't an admission of failure, and the
  reveal-then-confirm step makes "I had it" a concrete comparison
  against the model sentence rather than an abstract grade. Latency
  logging gives a silent check; drills are also assignable per
  profile via parent mode.
- **TTS accent/quality** for es-MX. Mitigation: Phase 1 ear test;
  worst case, record the safety micro-pack by a human and TTS the
  long tail.
- **app.py growth / regression before the trip.** Mitigation:
  Phase 1 requires zero app changes; Phase 2 isolates the chain
  logic in `drill_card.py`; card type inert without a pack.
