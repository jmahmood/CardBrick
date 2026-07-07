# Plan: Studying Ahead (sprint-based microstudying)

## The model

CardBrick is a microstudying app: the child is not supposed to sit down
once and clear a queue, they are supposed to chip away at a **daily card
goal** (say 150 cards) in **5–10 minute sprints** spread across the day.

Two numbers drive everything:

* `daily_goal_cards` — how many card answers the child should log today
  (parent-set, e.g. 150);
* a **sprint** — one `StudySession`, already bounded by
  `session_card_limit` (cards per sitting) and `session_time_minutes`
  (5–10 min).

From these we derive, always from the append-only review log (never
in-memory counters, matching the house style):

```
cards_done_today   = non-undone review-log entries since local midnight
cards_remaining    = max(daily_goal_cards - cards_done_today, 0)
sprints_remaining  = ceil(cards_remaining / session_card_limit)
sprints_planned    = ceil(daily_goal_cards / session_card_limit)
```

The child should always see **how many sprints they still owe today**,
and **"going ahead" — doing the next sprint right now instead of waiting
— decrements that count**. A child with unexpected spare time can knock
out three sprints back-to-back and owe three fewer later. Deriving the
count from *cards*, not from completed sessions, means a sprint cut short
by the time limit only earns partial credit and the arithmetic stays
honest across undo, crashes, and restarts.

## What's wrong today

The current code models exactly one queue per day and then stops:

* `ReviewService.get_due_cards` (`cardbrick/service.py`) caps the queue
  by `daily_review_cards` / `daily_new_cards` minus work already logged.
  Once the caps are consumed, the queue is empty for the rest of the day.
* `screen_child_start` (`cardbrick/app.py`) then shows *"All done for
  today! Come back tomorrow."* — a dead end that tells the child the
  opposite of what we want. There is no notion of a goal, a sprint, or
  "you're 3 of 8 through your day".
* `screen_summary` says "SESSION COMPLETE / ¡Buen trabajo!" and routes
  back to that dead end. Nothing invites the next sprint.

One thing does already work and must keep working: caps are log-derived,
so quitting mid-day and coming back resumes the remainder. The change is
the framing (goal/sprints) and what happens when today's due pool runs
dry before the goal is met.

## Design

### 1. Goal-driven queue budget (service)

`daily_goal_cards` becomes the day's review budget, replacing
`daily_review_cards` in the queue math (`get_due_cards`):

* `remaining_review = max(daily_goal_cards - cards_done_today, 0)` —
  answers, not unique cards: learning-step repeats within a sprint count
  toward the goal, which is correct for a "150 cards a day" contract.
* `daily_new_cards` keeps its current meaning — it paces how fast new
  material enters FSRS, which is orthogonal to the goal.
* `daily_review_cards` is retired from queue-building (column stays for
  migration compatibility; the CLI stops offering it).
* Each sprint's queue is then, as today, truncated to
  `session_card_limit` — one sprint's worth.

New service helper used by every screen:

```
sprint_status(profile, deck_filter=None) ->
    {cards_done, cards_remaining, sprints_remaining, sprints_planned,
     next_sprint_size}
```

### 2. Filling sprints when the due pool runs dry (storage + service)

A goal of 150 can exceed what FSRS has due today. When due reviews + new
cards can't fill the remaining budget, sprints are topped up with
**ahead cards** — cards due soon, pulled forward. FSRS makes this sound:
py-fsrs computes the next state from actual elapsed time, so an early
review is well-defined (just a smaller stability gain). No interval
hacks.

New storage query alongside `queue_candidates` (`storage.py:314`):

```
ahead_candidates(now_iso, horizon_iso, decks=None)
    -> reps > 0 AND due > :now AND due <= :horizon,
       not suspended, not buried, ordered by due (soonest first)
```

`get_due_cards` gains a final fill stage: due reviews → new cards →
ahead cards (horizon = next local midnight + `study_ahead_days`, default
1), all still truncated to the sprint size. Buried cards stay excluded —
bury means "not today", and no fill stage may resurrect them.

The ahead pool self-limits: every early review pushes that card's due
date out past the horizon, so back-to-back sprints drain it naturally.

### 3. Sprint-aware UI (`cardbrick/app.py`)

`screen_child_start` — replace the due-count framing with goal framing:

* Big line: **"5 sprints to go today"** (from `sprint_status`).
* Progress line: "72 / 150 cards" (a simple bar fits the existing
  drawing helpers).
* Detail: "next sprint: ~20 cards, up to 7 min".
* Bottom button always starts the next sprint while
  `sprints_remaining > 0` — never a dead end mid-goal.
* Goal met (`cards_remaining == 0`): celebrate — "Goal reached! 🎉
  8 sprints done" — and offer a **bonus sprint** from the ahead pool if
  it's non-empty ("keep going: N cards from tomorrow"). Only when the
  ahead pool is also empty show "Come back tomorrow."

`screen_summary` — this is where "going ahead" lives:

* "Sprint done! **4 sprints to go** today" instead of the generic
  session-complete banner (keep the stats lines).
* Bottom button = **next sprint now** (straight into `REVIEW`, reusing
  the deck filter from this sitting); east button = done for now (back to
  start screen). A child with momentum never bounces through a menu.
* After the last sprint: goal celebration + bonus-sprint offer, same as
  the start screen.

`_draw_review` header — show "Sprint 4/8" next to the existing
"{n} left" label so the child always knows where they are in the day.

`screen_deck_select` is unchanged except its counts come from the same
sprint-sized queue builder.

### 4. Parent controls

New profile columns, wired through the existing versioned migration
(`storage.py` ~124–137, schema-version bump) and both profile-update
allowlists (`storage.py:538, :554`) so the profile CLI can set them:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `daily_goal_cards` | INTEGER | 150 | Card answers per day; drives sprint count |
| `study_ahead_days` | INTEGER | 1 | How far forward the ahead fill may reach |
| `study_ahead_enabled` | INTEGER (bool) | 1 | Allow ahead fill + bonus sprints |

Sprint size/length stay on the existing `session_card_limit` /
`session_time_minutes` fields (parents should set them to the 5–10 min
range; suggested new defaults: 20 cards / 7 minutes, tuned so
`150 / 20 ≈ 8` sprints).

Migration note: initialize `daily_goal_cards` for existing profiles from
`daily_review_cards + daily_new_cards` so nobody's day suddenly triples.

### 5. Tests (`cardbrick-py/tests/test_study_ahead.py`)

Injected `now_fn`, log-derived assertions, following the house style:

1. **Sprint math** — `sprint_status` derives counts from the log; a
   partial sprint (ended early) decrements cards, not a whole sprint;
   undo restores the count.
2. **Going ahead decrements** — three back-to-back sprints reduce
   `sprints_remaining` by three; caps never block a sprint while the
   goal is unmet.
3. **Goal budget** — queue building stops at `daily_goal_cards`;
   `daily_new_cards` still paces new cards; `session_card_limit` still
   truncates each sprint.
4. **Ahead fill** — kicks in only when due + new can't fill the budget;
   respects horizon, ordering (soonest due first), suspended/buried
   exclusion, deck/category filters; disabled cleanly by
   `study_ahead_enabled=0` (day ends early instead).
5. **FSRS early review** — an ahead card answered early gets a valid
   state with `due > now`; Again brings it back within the sprint
   (LEARN_AHEAD path unchanged).
6. **Bonus sprint** — available after goal met while the ahead pool is
   non-empty; pool drains to empty across repeated bonus sprints (the
   loop terminates).
7. **Rollover** — a sprint straddling local midnight books its answers
   to the day they were logged; tomorrow's status starts clean.
8. **Migration** — existing profiles get a sane `daily_goal_cards`.

## Implementation order

Each step lands independently with the suite green:

1. **Storage**: profile columns + migration + allowlists;
   `ahead_candidates` query.
2. **Service**: goal-budget queue math, ahead fill stage,
   `sprint_status` (tests 1–5).
3. **Session**: no structural change expected — a sprint *is* a
   `StudySession`; verify summary/counters need nothing new (test 6–7
   land here).
4. **UI**: start screen, summary "next sprint now" flow, review header,
   deck-select counts. Manual smoke via desktop pygame
   (`cardbrick-py/main.py`).
5. **CLI + docs**: expose the new fields, retire `daily_review_cards`
   from the CLI, update `docs/CLI_API.md` / developer guide.

## Out of scope

* **The legacy Rust app** (`src/`): same dead end (daily queue file in
  `src/scheduler/queue.rs` built once per day), but its SM-2 scheduler
  has no sound notion of early review and active development has moved
  to `cardbrick-py`. An SM-2 review-ahead needs its own design
  (elapsed/scheduled interval adjustment) if ever wanted.
* Scheduled sprint reminders across the day (notifications/alarms) —
  the sprint count tells the child what they owe; nagging hardware
  timers are a separate feature.
* Parent Mode screens for the new fields (CLI-only in v1, like the
  other limits).
