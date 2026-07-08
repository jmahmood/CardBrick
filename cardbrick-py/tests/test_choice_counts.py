"""counts_for_queue_many: the batched per-entry counts behind the
child-facing deck and topic pickers.

Opening a picker used to call counts_for_queue once per menu entry,
each call re-running the whole pool build (daily counts + due + new +
study-ahead queries) — O(entries × collection) work on the A press.
The batched method must return exactly what the per-entry calls would,
from a single set of storage queries.
"""

from datetime import timedelta

from conftest import seed_card


def overdue(clock):
    return clock.now() - timedelta(days=2)


def seed_mixed_collection(storage, service, clock):
    """Two decks, three tags, a mix of due reviews and new cards."""
    due = overdue(clock)
    seed_card(storage, service, 1, deck="Spanish", tags="food", reps=3,
              due=due)
    seed_card(storage, service, 2, deck="Spanish", tags="verbs", reps=2,
              due=due)
    seed_card(storage, service, 3, deck="Spanish", tags="food verbs")
    seed_card(storage, service, 4, deck="French", tags="food", reps=1,
              due=due)
    seed_card(storage, service, 5, deck="French", tags="numbers")
    seed_card(storage, service, 6, deck="French", tags="numbers")


def assert_matches_single_calls(service, profile, filters, bonus=False):
    batched = service.counts_for_queue_many(filters, profile=profile,
                                            bonus=bonus)
    singles = [
        service.counts_for_queue(profile=profile, deck_filter=decks,
                                 category_filter=cats, bonus=bonus)
        for decks, cats in filters
    ]
    assert batched == singles
    return batched


def test_deck_picker_entries_match_single_calls(storage, service, clock,
                                                profile):
    seed_mixed_collection(storage, service, clock)
    filters = [(None, None), (["Spanish"], None), (["French"], None)]
    counts = assert_matches_single_calls(service, profile, filters)
    # Sanity: the numbers themselves, not just agreement.
    assert counts[0] == (3, 3)  # all: 3 due reviews + 3 new
    assert counts[1] == (2, 1)
    assert counts[2] == (1, 2)


def test_category_picker_entries_match_single_calls(storage, service, clock,
                                                    profile):
    seed_mixed_collection(storage, service, clock)
    deck = ["Spanish"]  # the sitting's earlier deck choice
    filters = [(deck, None), (deck, ["food"]), (deck, ["verbs"]),
               (deck, ["numbers"])]
    counts = assert_matches_single_calls(service, profile, filters)
    assert counts[1] == (1, 1)  # food: card 1 review, card 3 new
    assert counts[2] == (1, 1)  # verbs: card 2 review, card 3 new
    assert counts[3] == (0, 0)  # numbers: only in French


def test_respects_profile_assignments_for_none_filters(storage, service,
                                                       clock, profile):
    seed_mixed_collection(storage, service, clock)
    storage.update_profile(profile["id"], active_decks=["Spanish"],
                           active_categories=["food"])
    profile = storage.get_profile(profile["id"])
    # None means "whatever the parent assigned", not literally all.
    counts = assert_matches_single_calls(service, profile, [(None, None)])
    assert counts == [(1, 1)]


def test_new_card_budget_is_consumed_in_id_order(storage, service, clock,
                                                 profile):
    # Cap new intake below what's available: the budget must be spent
    # walking cards in id order per entry, same as the single calls.
    for card_id in range(1, 7):
        seed_card(storage, service, card_id, deck="Spanish",
                  tags="even" if card_id % 2 == 0 else "odd")
    storage.update_profile(profile["id"], daily_new_cards=2)
    profile = storage.get_profile(profile["id"])
    filters = [(None, None), (None, ["even"]), (None, ["odd"])]
    counts = assert_matches_single_calls(service, profile, filters)
    assert counts == [(0, 2), (0, 2), (0, 2)]


def test_bonus_mode_matches_single_calls(storage, service, clock, profile):
    seed_mixed_collection(storage, service, clock)
    filters = [(None, None), (["French"], None), (None, ["food"])]
    assert_matches_single_calls(service, profile, filters, bonus=True)


def test_explicitly_empty_filters_count_zero(storage, service, clock,
                                             profile):
    seed_mixed_collection(storage, service, clock)
    filters = [([], None), (None, []), ([], [])]
    counts = assert_matches_single_calls(service, profile, filters)
    assert counts == [(0, 0), (0, 0), (0, 0)]


def test_cards_answered_today_match_single_calls(storage, service, clock,
                                                 profile):
    # Repeats already counted today can't advance the goal; make sure
    # the shared answered-set is applied per entry identically.
    seed_mixed_collection(storage, service, clock)
    service.answer_card(1, 3)
    service.answer_card(4, 3)
    clock.advance(minutes=30)
    filters = [(None, None), (["Spanish"], None), (["French"], None)]
    assert_matches_single_calls(service, profile, filters)


def test_batched_call_queries_storage_a_constant_number_of_times(
        storage, service, clock, profile, monkeypatch):
    # The point of the batch: adding entries must not add pool builds.
    seed_mixed_collection(storage, service, clock)
    calls = {"n": 0}
    real = storage.queue_candidates

    def counting(*args, **kwargs):
        calls["n"] += 1
        return real(*args, **kwargs)

    monkeypatch.setattr(storage, "queue_candidates", counting)

    service.counts_for_queue_many([(None, None)], profile=profile)
    per_single_entry = calls["n"]

    calls["n"] = 0
    many = [(None, None)] + [([d], None) for d in ("Spanish", "French")] \
        + [(None, [t]) for t in ("food", "verbs", "numbers")]
    service.counts_for_queue_many(many, profile=profile)
    assert calls["n"] == per_single_entry
