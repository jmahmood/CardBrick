# Sentence Pattern Files

Content-generation source material for CardBrick's pattern drill
cards — the 500-pattern Mexico City Spanish inventory, split by tier
so an AI assistant (this one or another) can work from one file at a
time instead of the whole set.

**Start here:** [`00-generation-guide.md`](00-generation-guide.md) —
notation key, output JSON schema, worked examples, and authoring
rules. Every tier file below assumes you've read it.

| File | Tier | IDs | Count | Gate range | Notes |
|---|---|---|---:|---|---|
| [`01-core.md`](01-core.md) | core | P001–P100 | 100 | A0–A2 | Identity, location, basic modality, questions. Start here. |
| [`02-interaction.md`](02-interaction.md) | interaction | P101–P180 | 80 | A0–B1 | Requests, clarification, booking, repair language. |
| [`03-narrative.md`](03-narrative.md) | narrative | P181–P240 | 60 | A2–B1 | Past tense, sequencing, reporting. Weak MCQ tier — favor production. |
| [`04-transactional.md`](04-transactional.md) | transactional | P241–P300 | 60 | A0–B1 | Food, shopping, lodging, payment. |
| [`05-travel-safety.md`](05-travel-safety.md) | travel_safety | P301–P380 | 80 | A0–B1 | **Highest priority** — transit, taxis, lost items, safety. |
| [`06-social.md`](06-social.md) | social | P381–P420 | 40 | A0–B1 | Introductions, small talk, invitations. |
| [`07-emergency-health-legal.md`](07-emergency-health-legal.md) | emergency_health_legal | P421–P460 | 40 | A0–B1 | Symptoms, pharmacy, police report, consular. **High priority.** |
| [`08-advanced-subjunctive-conditionals.md`](08-advanced-subjunctive-conditionals.md) | advanced_subjunctive_conditionals | P461–P500 | 40 | B1–C1 | Hedging, subjunctive, hypotheticals. Weak MCQ tier — favor production. |

500 patterns total, matching the source inventory
(`deep-research-report.md`) exactly — verified row-for-row, no gaps
or duplicates.

## Suggested order

Front-load the trip-critical tiers first, per
[PATTERN_DRILLING_PLAN.md](../PATTERN_DRILLING_PLAN.md)'s rollout:

1. `01-core.md` (core present-tense engine)
2. `05-travel-safety.md` + `07-emergency-health-legal.md` (safety
   micro-pack — see each file's "suggested first batch")
3. `02-interaction.md` + `04-transactional.md`
4. `03-narrative.md`, `06-social.md`, `08-advanced-...md` as time
   allows

Within any file, generate in batches of ~15–20 patterns per request
(see the generation guide) rather than asking for a whole tier at
once.
