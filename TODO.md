# CardBrick — TODO / Backlog

Feature ideas and improvements for the learning app (primarily the active
`cardbrick-py` appliance). Items are intentionally kept at the "intent +
context" level; each should be broken into concrete tasks before
implementation.

## 1. Shrink photos by default, add a "look over your shoulder" maximize

**Problem:** Vocab card photos are now too large. Attachments are currently
mounted to span ~80% of the logical screen
(`cardbrick/app.py::_vocab_attachment_surface`, `target_mount_w = w * 0.80`),
so they dominate the paper reel and push text off-screen.

**Idea:** Display photos small inline, with a way to maximize on demand.
A fitting UI metaphor for the paper-roll/printer-feed model
(`cardbrick/paperroll.py`) is to "look over your shoulder at a photo book":
a right shift away from the printer feed reveals a larger view of the photo,
navigable with the left/right buttons. Small by default; deliberate action
to enlarge.

- Reduce the default mounted size on the reel.
- Add a photo-book / maximized view reachable via a right shift from the feed.
- Left/right buttons page through photos in that view.

## 2. Better sentence display — show per-word meanings

**Problem:** A word often has one formal/dictionary meaning, but the example
sentence uses it in a symbolic or figurative sense, so the shown meaning and
the sentence's actual usage don't line up.

**Idea:** Annotate example sentences with per-word meanings — e.g. arrows
from each word to its meaning in English or Japanese. This makes the
in-context sense explicit and should aid memory.

- Attach/derive per-word glosses for example sentences.
- Render them as arrows/callouts under or beside each word in the example
  phase (`cardbrick/app.py` vocab example rendering).

## 3. Mark sentences as wrong or inappropriate

**Problem:** Today the only recourse for a bad example sentence is to bury it
(`cardbrick/service.py::bury_card`), which just hides it temporarily rather
than flagging it for correction or removal.

**Idea:** Add an explicit way to mark a sentence as **wrong** or
**inappropriate**, distinct from burying. Marked sentences should be
recorded (flagged) so they can be reviewed, corrected, or excluded — not
merely deferred.

- Add a flag/report action alongside bury/suspend.
- Persist the flag (and reason) so it can feed the audit in item 5.

## 4. Memory helpers for frequently-missed words

**Problem:** Some words are missed repeatedly with no extra scaffolding to
help them stick.

**Idea:** For words a learner gets wrong a lot, surface memory helpers —
e.g. etymology/derivation, mnemonics, related forms, or component breakdowns.

- Identify high-miss words from the review log
  (`cardbrick/storage.py` / `review_log`).
- Attach and display memory-helper content for those words.

## 5. Audit feed to validate sentences against our goals

**Problem:** No systematic check that example sentences serve the learning
goals or that translations clarify usage.

**Idea:** Set up an audit feed that validates sentences, checking that:
- each sentence aligns with our stated learning goals, and
- the English and Japanese translations clarify **how the word is being
  used** in that sentence (ties into items 2 and 3).

Flags raised in item 3 should feed into this audit.

## 6. Show deck/topic maturity status at selection time

**Problem:** When selecting a topic (category) or a deck, there's no
at-a-glance sense of how far along that collection is.

**Idea:** On both the topic-selection and deck-selection screens, show a
status breakdown per entry: **unseen** vs **studying** vs **mature**.

- Derive counts from `review_state` (e.g. reps = 0 → unseen; young/in-learning
  → studying; stability past a threshold / FSRS state → mature).
- Surface the breakdown in the deck picker and the category/topic picker
  (`cardbrick/app.py` DECK_SELECT / CATEGORY_SELECT screens).

---

# Known Bugs

## B1. Suspended cards don't appear in the parent "Suspended cards" list

**Report:** Despite having suspended cards, they don't show up in the parent
mode "Suspended cards" list.

**Investigation (unresolved — could not reproduce in code):** The full path
was traced and exercised headlessly:
- `session.suspend_current` → `service.suspend_card` → `storage.set_suspended`
  writes `cards.suspended = 1` and commits.
- `storage.suspended_cards()` reads `SELECT * FROM cards WHERE suspended = 1`
  with no profile/deck/type filter.
- The parent menu ("Suspended cards") routes to `PARENT_SUSPENDED` →
  `screen_parent_suspended`, which reads `suspended_cards()`.
- `upsert_card`'s sync/import `ON CONFLICT` update does **not** reset
  `suspended`, so re-imports preserve it.
- A headless repro (suspend via `StudySession.suspend_current`, then read
  `suspended_cards()`) returns the card both same-connection and after a
  simulated app restart. `tests/test_bury_suspend.py::
  test_suspended_cards_visible_to_parent` also passes.

Because the in-code path is verified working, the trigger is likely
environmental — needs concrete reproduction details (card type,
same device vs another device, whether a sync/restore ran in between).
