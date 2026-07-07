"""Storage.purge_decks: the cascading delete behind `admin purge-decks`."""

from conftest import seed_card


def test_purge_all_decks_cascades_everything(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    service.answer_card(1, 3)  # creates a review_log row for card 1
    storage.upsert_card(card_id=3, note_id=3, deck="Spanish", front="w",
                        back="b", tags="", card_type="vocab")
    storage.upsert_vocab_card(3, "w", None, "", "def", None, "", None,
                              "", None, None)
    storage.init_review_state(service.scheduler.initial_state(3))
    storage.commit()

    deleted = storage.purge_decks(None)

    assert deleted == 3
    assert storage.conn.execute(
        "SELECT COUNT(*) AS n FROM cards").fetchone()["n"] == 0
    assert storage.conn.execute(
        "SELECT COUNT(*) AS n FROM review_state").fetchone()["n"] == 0
    assert storage.conn.execute(
        "SELECT COUNT(*) AS n FROM review_log").fetchone()["n"] == 0
    assert storage.conn.execute(
        "SELECT COUNT(*) AS n FROM vocab_cards").fetchone()["n"] == 0


def test_purge_single_deck_leaves_others_untouched(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")

    deleted = storage.purge_decks(["Spanish"])

    assert deleted == 1
    assert storage.get_card(1) is None
    assert storage.get_card(2) is not None
    assert storage.deck_names_list() == ["French"]


def test_purge_multiple_named_decks(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    seed_card(storage, service, 3, deck="German")

    deleted = storage.purge_decks(["Spanish", "German"])

    assert deleted == 2
    assert storage.deck_names_list() == ["French"]


def test_purge_leaves_profiles_and_settings_alone(storage, service):
    profile = storage.ensure_default_profile("Maya")
    seed_card(storage, service, 1, deck="Spanish")
    storage.purge_decks(None)
    assert storage.get_profile(profile["id"])["name"] == "Maya"


def test_count_cards_in_decks(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="Spanish")
    seed_card(storage, service, 3, deck="French")
    assert storage.count_cards_in_decks(None) == 3
    assert storage.count_cards_in_decks(["Spanish"]) == 2
    assert storage.count_cards_in_decks(["French"]) == 1
    assert storage.count_cards_in_decks(["Nonexistent"]) == 0


def test_deck_names_list_is_sorted_and_distinct(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="Spanish")
    seed_card(storage, service, 3, deck="Anki-French")
    assert storage.deck_names_list() == ["Anki-French", "Spanish"]
