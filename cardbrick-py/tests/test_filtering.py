"""Tag/category filtering of the review queue."""

from conftest import seed_card

LIMITS = {"daily_new_cards": 100, "daily_review_cards": 100,
          "session_card_limit": 100}


def _ids(service, category_filter=None, profile=None):
    return sorted(row["id"] for row in service.get_due_cards(
        profile=profile, category_filter=category_filter, limits=LIMITS))


def test_only_active_tags_appear(storage, service):
    seed_card(storage, service, 1, tags="restaurant")
    seed_card(storage, service, 2, tags="numbers")
    seed_card(storage, service, 3, tags="restaurant greetings")
    assert _ids(service, ["restaurant"]) == [1, 3]


def test_untagged_cards_excluded_by_active_filter(storage, service):
    seed_card(storage, service, 1, tags="")
    seed_card(storage, service, 2, tags="food")
    assert _ids(service, ["food"]) == [2]


def test_no_filter_means_all_cards(storage, service):
    seed_card(storage, service, 1, tags="")
    seed_card(storage, service, 2, tags="food")
    assert _ids(service, None) == [1, 2]


def test_multi_category_group(storage, service):
    seed_card(storage, service, 1, tags="restaurant")
    seed_card(storage, service, 2, tags="numbers")
    seed_card(storage, service, 3, tags="verbs")
    assert _ids(service, ["restaurant", "numbers"]) == [1, 2]


def test_parent_selected_categories_respected_via_profile(storage, service,
                                                          profile):
    seed_card(storage, service, 1, tags="travel")
    seed_card(storage, service, 2, tags="grammar")
    storage.update_profile(profile["id"], active_categories=["travel"])
    profile = storage.get_profile(profile["id"])
    assert _ids(service, profile=profile) == [1]


def test_all_tags_surfaced_as_categories(storage, service):
    seed_card(storage, service, 1, tags="restaurant food")
    seed_card(storage, service, 2, tags="numbers")
    assert storage.all_tags() == ["food", "numbers", "restaurant"]
