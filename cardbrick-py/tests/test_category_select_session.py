"""StudySession's per-sitting category_filter override (the child's
pick in the CATEGORY_SELECT screen), independent of the parent's
active_categories — mirrors test_deck_select_session.py.

Picking a tag is non-destructive filtering, not a deck reassignment
(the same idea as Anki's filtered decks): a card's own tags never
change, so the general "all assigned categories" pick always includes
it again.
"""

from conftest import seed_card

from cardbrick.session import StudySession


def test_category_filter_overrides_profile_default(storage, service,
                                                    profile):
    seed_card(storage, service, 1, tags="verbs")
    seed_card(storage, service, 2, tags="nouns")
    # Parent has assigned both categories (active_categories stays None).
    session = StudySession(storage, service, profile,
                           category_filter=["nouns"])
    assert list(session.queue) == [2]


def test_none_category_filter_falls_back_to_profile(storage, service,
                                                    profile):
    seed_card(storage, service, 1, tags="verbs")
    seed_card(storage, service, 2, tags="nouns")
    storage.update_profile(profile["id"], active_categories=["verbs"])
    profile = storage.get_profile(profile["id"])
    # "All assigned categories" in the picker passes category_filter=None,
    # meaning "whatever the parent assigned" — not literally every tag.
    session = StudySession(storage, service, profile, category_filter=None)
    assert list(session.queue) == [1]


def test_deck_and_category_filters_combine_in_a_session(storage, service,
                                                        profile):
    seed_card(storage, service, 1, deck="Spanish", tags="food")
    seed_card(storage, service, 2, deck="Spanish", tags="numbers")
    seed_card(storage, service, 3, deck="French", tags="food")
    session = StudySession(storage, service, profile,
                           deck_filter=["Spanish"],
                           category_filter=["food"])
    assert list(session.queue) == [1]


def test_a_focused_tag_does_not_touch_the_cards_tags(storage, service,
                                                     profile):
    # Choosing "nouns" for one sitting is pure session-level filtering:
    # the card keeps its tag afterward and reappears under "All
    # assigned categories" — nothing like Anki's filtered-deck
    # temporary re-homing happens to the underlying data.
    seed_card(storage, service, 1, tags="nouns")
    StudySession(storage, service, profile, category_filter=["nouns"])
    card = storage.get_card(1)
    assert card["tags"] == "nouns"

    everything = StudySession(storage, service, profile,
                              category_filter=None)
    assert list(everything.queue) == [1]


def test_session_row_records_the_effective_category_filter(storage, service,
                                                           profile):
    seed_card(storage, service, 1, tags="verbs")
    session = StudySession(storage, service, profile,
                           category_filter=["verbs"])
    row = storage.session_row(session.session_id)
    assert row["category_filter"] == '["verbs"]'


def test_session_row_records_profile_default_when_unset(storage, service,
                                                        profile):
    seed_card(storage, service, 1, tags="verbs")
    storage.update_profile(profile["id"], active_categories=["verbs"])
    profile = storage.get_profile(profile["id"])
    session = StudySession(storage, service, profile, category_filter=None)
    row = storage.session_row(session.session_id)
    assert row["category_filter"] == '["verbs"]'
