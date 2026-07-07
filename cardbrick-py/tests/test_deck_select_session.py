"""StudySession's per-sitting deck_filter override (the child's pick
in the DECK_SELECT screen), independent of the parent's active_decks."""

from conftest import seed_card
from cardbrick.session import StudySession


def test_deck_filter_overrides_profile_default(storage, service, profile):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    # Parent has assigned both decks (active_decks stays None = all).
    session = StudySession(storage, service, profile,
                           deck_filter=["French"])
    assert list(session.queue) == [2]


def test_none_deck_filter_falls_back_to_profile(storage, service, profile):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    storage.update_profile(profile["id"], active_decks=["Spanish"])
    profile = storage.get_profile(profile["id"])
    # "All assigned decks" in the picker passes deck_filter=None,
    # meaning "whatever the parent assigned" — not literally everything.
    session = StudySession(storage, service, profile, deck_filter=None)
    assert list(session.queue) == [1]
