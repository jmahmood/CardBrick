# Plan: Studying Ahead

## The problem

When a child finishes their queue, the app says *"All done for today! Come
back tomorrow."* and there is nothing else to do — even if they have spare
time and want to keep going. Concretely, in `cardbrick-py`:

* `ReviewService.get_due_cards` (`cardbrick/service.py`) builds the queue
  from cards with `due <= now` plus new cards, capped by
  `daily_review_cards` / `daily_new_cards` **minus work already logged
  today**. Once the day's caps are consumed and nothing is overdue, the
  queue is empty.
* `screen_child_start` (`cardbrick/app.py`) only offers Start when
  `counts_for_queue` is non-zero; otherwise the child hits a dead end.
* `screen_summary` returns to `CHILD_START`, which then shows the dead end.

The goal: after finishing, the child can immediately start another session
— "studying ahead" — pulling in cards that are due soon (tomorrow, or the
next few days) and optionally a few bonus new cards, without breaking the
daily-cap safety rails for normal sessions.

Note that one case already works today and must keep working: if the child
ends a session early (time limit, "End session") while daily caps are not
yet exhausted, starting again simply resumes the remainder — the caps are
derived from the review log, so no change is needed there. This plan is
about the case where **nothing** is left for today.

## Why this is safe with FSRS

Scheduling is delegated to py-fsrs (`cardbrick/scheduler.py`). FSRS
computes the next state from the *actual* elapsed time since the last
review, so reviewing a card before its due date is well-defined: the
stability gain is simply smaller than it would have been on time. No
interval hacks or "pretend it was due" adjustments are needed — we just
show the card and rate it normally. (This is a real advantage over the
legacy Rust SM-2 app, where early reviews would corrupt intervals; see
"Out of scope" below.)

## Design

### 1. Ahead queue builder (service + storage)

New storage query, alongside `queue_candidates`
(`cardbrick/storage.py:314`):

```
ahead_candidates(now_iso, horizon_iso, decks=None)
    -> review cards with reps > 0 AND due > :now AND due <= :horizon,
       not suspended, not buried, ordered by due (soonest first)
```

New service method in `cardbrick/service.py`:

```
get_ahead_cards(profile=None, category_filter=None, deck_filter=None,
                limits=None)
```

Behaviour:

* Horizon = next local midnight + `study_ahead_days` days (profile field,
  default 1 → "tomorrow's cards"). Reuses `next_local_midnight`.
* **Ignores** `daily_review_cards` / `daily_new_cards` — those caps are
  exhausted by definition when this queue is offered. Bounding comes from:
  * `session_card_limit` and `session_time_minutes` (unchanged, still
    enforced by `StudySession`);
  * the horizon itself — each early review pushes the card's due date out,
    so the ahead pool shrinks as it is studied and repeated ahead sessions
    naturally dry up;
  * `study_ahead_extra_new` (profile field, default 0): a per-day bonus of
    new cards allowed beyond `daily_new_cards`. Computed from the review
    log like the other caps (`max(daily_new_cards + study_ahead_extra_new
    - new_done, 0)`), so it survives restarts and undo.
* Order: soon-due review cards first (most urgent first), then bonus new
  cards — same shape as the normal queue.
* Same category/deck filtering and profile fallbacks as `get_due_cards`.
* Companion `counts_for_ahead_queue(...)` mirroring `counts_for_queue`,
  for the start screen label.

Gate: `study_ahead_enabled` profile field (default 1). When off,
`get_ahead_cards` returns `[]` and the UI never shows the option.

### 2. Session plumbing

`StudySession` (`cardbrick/session.py`) takes an optional `ahead=False`
flag:

* When `ahead=True`, it seeds its queue from `get_ahead_cards` instead of
  `get_due_cards`. Everything else — answering, LEARN_AHEAD requeue of
  learning cards, undo, bury/suspend, summary, session row — is unchanged
  and works as-is, because answers flow through the same
  `ReviewService.answer_card` path.
* The session row records that it was an ahead session (see migration
  below) so the calendar/stats can distinguish it later if wanted. Ahead
  sessions still earn a calendar stamp — extra effort should be rewarded,
  and `sessions_per_day` already counts any session with reviews.

### 3. UI flow (`cardbrick/app.py`)

* `screen_child_start`: when the normal queue is empty **and**
  `counts_for_ahead_queue` > 0, replace the dead end with:

  * "All done for today! ⭐" (keep the celebration)
  * "Study ahead: N cards from tomorrow" + "Press the bottom button to
    keep going!"
  * Bottom button starts an ahead session (via `DECK_SELECT` when more
    than one deck is assigned, same as today, with ahead counts shown).

  When the ahead pool is also empty (deck truly exhausted or feature
  disabled), keep the current "Come back tomorrow." message.
* `screen_summary`: add a third action — when the normal queue is empty
  but ahead cards exist, show "… = Study ahead" in the footer and start
  the next session directly, so a child with momentum never has to bounce
  through the start screen. (When normal cards remain — e.g. the session
  ended on the time limit — the existing "Done → start screen → Start"
  path already covers "go again", so the summary shortcut can offer that
  too, but that is polish, not core.)
* A small visual marker during ahead sessions (e.g. "STUDYING AHEAD" in
  the header where `_draw_review` puts the deck/remaining labels) so the
  log, the child, and a watching parent all know these are bonus reviews.
* State passed the same way as the existing deck filter
  (`self._session_deck_filter`): an `self._session_ahead` flag cleared by
  `screen_child_start`.

### 4. Parent controls

New profile columns, wired through the existing machinery:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `study_ahead_enabled` | INTEGER (bool) | 1 | Offer ahead sessions at all |
| `study_ahead_days` | INTEGER | 1 | How far past midnight the horizon reaches |
| `study_ahead_extra_new` | INTEGER | 0 | Bonus new cards/day during ahead sessions |

* Schema: add to `CREATE TABLE profiles` and to the ALTER-TABLE migration
  map (`storage.py` ~line 124–137) with a schema-version bump — the
  versioned in-place migration path already exists.
* Add the fields to the two profile-update allowlists
  (`storage.py:538, :554`) so the profile CLI can set them
  (`cardbrick-cli`), matching how `session_card_limit` etc. are managed.
* Parent Mode screens: not required for v1 (parents manage limits via the
  CLI today); can be added to the parent menu later alongside the other
  limit editing.

### 5. Tests (`cardbrick-py/tests/test_study_ahead.py`)

Following the house style (injected `now_fn`, log-derived counters):

1. **Pool selection** — `get_ahead_cards` returns only cards with
   `now < due <= horizon`, soonest first; suspended and buried cards are
   excluded; deck/category filters apply.
2. **Horizon** — `study_ahead_days=1` picks up tomorrow's cards but not
   the day after; `study_ahead_days=2` picks up both.
3. **Caps** — daily review/new caps exhausted → normal queue empty, ahead
   queue non-empty; `session_card_limit` still truncates; bonus new cards
   appear only up to `study_ahead_extra_new` and are counted from the log
   (undo restores the allowance).
4. **Gate** — `study_ahead_enabled=0` → empty ahead queue.
5. **FSRS early review** — answering an ahead card produces a valid state
   with `due > now` and doesn't reduce reps/stability bookkeeping;
   answering it Again brings it back within the session (LEARN_AHEAD path
   unchanged).
6. **Shrinking pool** — after an ahead session covers the whole horizon,
   a second ahead session finds (almost) nothing: the loop terminates.
7. **Session row** — ahead sessions are flagged, still counted by
   `sessions_per_day`, and `summary()` numbers are correct.
8. **Regression guard** — mid-day resume with unexhausted caps still
   works exactly as before (normal queue, no ahead flag).

## Implementation order

Each step lands independently and keeps the suite green:

1. **Storage**: `ahead_candidates` query + profile columns + migration +
   allowlists (+ tests 1, 2 partially).
2. **Service**: `get_ahead_cards`, `counts_for_ahead_queue`, bonus-new
   accounting (+ tests 1–4).
3. **Session**: `ahead=` flag, session-row flag (+ tests 5–8).
4. **UI**: start-screen offer, summary shortcut, deck-select counts,
   review-header marker. Manual smoke on desktop pygame
   (`cardbrick-py/main.py`) since screens are loop-driven.
5. **CLI**: expose the three fields in the profile CLI; update
   `docs/CLI_API.md` / developer guide if they enumerate profile fields.

## Edge cases called out

* **Buried cards** stay excluded from ahead sessions — bury means "not
  today", and studying ahead must not resurrect them.
* **Learning-state cards due later today** (rated Again/Hard earlier) fall
  inside any horizon and are the most valuable content — they come first
  because ordering is by due date.
* **Midnight rollover mid-session**: all accounting is log-derived, so an
  ahead session straddling midnight just means tomorrow starts with those
  cards already pushed out. No special handling.
* **Undo** in an ahead session restores state exactly (existing snapshot
  mechanism) — including the bonus-new allowance, since it's log-derived.

## Out of scope

* **The legacy Rust app** (`src/`). It has the same dead end (the daily
  queue file in `src/scheduler/queue.rs` is built once per day, and
  `continue_studying` in `src/scenes/studying/logic.rs` only reshuffles or
  previews new cards), but its SM-2 scheduler has no sound notion of early
  review, and active development has moved to `cardbrick-py`. If wanted
  later, the equivalent would be a separate design (SM-2 needs an
  elapsed/scheduled interval adjustment for early reviews).
* Parent Mode screens for the new fields (CLI-only in v1).
* Any change to points/gamification — `cardbrick-py` has no points
  system; the stamp calendar already rewards extra sessions.
