"""Topic picker visibility follows the deck chosen for a sitting."""

from conftest import seed_card

from cardbrick.app import CardBrickApp


def app_for(storage, profile, deck_filter=None):
    """Build only the state used by the picker-resolution helpers."""
    app = object.__new__(CardBrickApp)
    app.storage = storage
    app.profile = profile
    app._session_deck_filter = deck_filter
    return app


def seed_two_decks(storage, service):
    seed_card(storage, service, 1, deck="Spanish", tags="food verbs")
    seed_card(storage, service, 2, deck="French", tags="food numbers")


def test_tags_in_decks_reads_all_cards_not_review_eligibility(storage, service):
    seed_card(storage, service, 1, deck="Spanish", tags="visible")
    storage.conn.execute("UPDATE cards SET suspended = 1 WHERE id = 1")
    storage.commit()

    assert storage.tags_in_decks(["Spanish"]) == ["visible"]
    assert storage.tags_in_decks([]) == []


def test_selected_deck_limits_topics_to_its_cards(storage, service, profile):
    seed_two_decks(storage, service)

    assert app_for(storage, profile, ["Spanish"])._resolve_available_categories() == [
        "food", "verbs"
    ]
    assert app_for(storage, profile, ["French"])._resolve_available_categories() == [
        "food", "numbers"
    ]


def test_parent_categories_intersect_selected_deck_topics(storage, service, profile):
    seed_two_decks(storage, service)
    storage.update_profile(profile["id"], active_categories=["food", "numbers"])
    profile = storage.get_profile(profile["id"])

    assert app_for(storage, profile, ["Spanish"])._resolve_available_categories() == [
        "food"
    ]
    assert app_for(storage, profile, ["French"])._resolve_available_categories() == [
        "food", "numbers"
    ]


def test_single_assigned_deck_scopes_topics_without_picker(storage, service, profile):
    seed_two_decks(storage, service)
    storage.update_profile(profile["id"], active_decks=["Spanish"])
    profile = storage.get_profile(profile["id"])
    app = app_for(storage, profile)

    assert app._resolve_available_categories() == ["food", "verbs"]
    assert app._next_after_deck_choice() == "CATEGORY_SELECT"


def test_all_assigned_decks_retains_topics_from_each_deck(storage, service, profile):
    seed_two_decks(storage, service)
    storage.update_profile(profile["id"], active_decks=["Spanish", "French"])
    profile = storage.get_profile(profile["id"])

    assert app_for(storage, profile)._resolve_available_categories() == [
        "food", "numbers", "verbs"
    ]


def test_zero_or_one_topics_skips_topic_picker(storage, service, profile):
    seed_card(storage, service, 1, deck="Spanish", tags="food")
    seed_card(storage, service, 2, deck="French", tags="numbers")

    assert app_for(storage, profile, ["Spanish"])._next_after_deck_choice() == "REVIEW"
    assert app_for(storage, profile, [])._next_after_deck_choice() == "REVIEW"
