"""Storage-level support behind Parent Mode's "Decks" screen: the
active_decks profile field and queue_candidates' deck filter."""

import sqlite3

from conftest import seed_card
from cardbrick.storage import SCHEMA_VERSION, Storage


def test_queue_candidates_deck_filter(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    rows = storage.queue_candidates(
        service.now().isoformat(), new_cards=True, decks=["Spanish"])
    assert [r["id"] for r in rows] == [1]


def test_queue_candidates_no_deck_filter_is_all_decks(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    seed_card(storage, service, 2, deck="French")
    rows = storage.queue_candidates(
        service.now().isoformat(), new_cards=True, decks=None)
    assert sorted(r["id"] for r in rows) == [1, 2]


def test_queue_candidates_empty_deck_list_is_no_decks(storage, service):
    seed_card(storage, service, 1, deck="Spanish")
    rows = storage.queue_candidates(
        service.now().isoformat(), new_cards=True, decks=[])
    assert rows == []


def test_profile_active_decks_round_trips(storage):
    profile_id = storage.create_profile("Maya", active_decks=["Spanish"])
    profile = storage.get_profile(profile_id)
    assert profile["active_decks"] == ["Spanish"]


def test_profile_active_decks_defaults_to_none(storage):
    profile_id = storage.create_profile("Maya")
    profile = storage.get_profile(profile_id)
    assert profile["active_decks"] is None


def test_profile_active_decks_empty_list_round_trips(storage, profile):
    """An explicit empty list ("no decks selected") must be
    distinguishable from None ("all decks") after a round trip."""
    storage.update_profile(profile["id"], active_decks=[])
    reloaded = storage.get_profile(profile["id"])
    assert reloaded["active_decks"] == []


def test_update_profile_active_decks(storage, profile):
    storage.update_profile(profile["id"], active_decks=["Spanish", "French"])
    reloaded = storage.get_profile(profile["id"])
    assert reloaded["active_decks"] == ["Spanish", "French"]


def test_migration_adds_active_decks_to_old_profiles_table(tmp_path):
    """A database with the pre-active_decks child_profiles table
    migrates cleanly and the new column defaults to NULL (all decks)."""
    db = str(tmp_path / "old.db")
    conn = sqlite3.connect(db)
    conn.executescript("""
        CREATE TABLE child_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
            active_categories TEXT, daily_new_cards INTEGER NOT NULL
                DEFAULT 10,
            daily_review_cards INTEGER NOT NULL DEFAULT 40,
            session_card_limit INTEGER NOT NULL DEFAULT 50,
            session_time_minutes INTEGER NOT NULL DEFAULT 15,
            study_direction TEXT NOT NULL DEFAULT 'normal'
        );
    """)
    conn.execute("INSERT INTO child_profiles (name, active_categories) "
                 "VALUES ('Maya', '[\"travel\"]')")
    conn.commit()
    conn.close()

    storage = Storage(db)
    assert storage.schema_version() == SCHEMA_VERSION
    profile = storage.get_profile(1)
    assert profile["name"] == "Maya"
    assert profile["active_categories"] == ["travel"]  # preserved
    assert profile["active_decks"] is None  # new column, safe default
    storage.update_profile(1, active_decks=["Spanish"])
    assert storage.get_profile(1)["active_decks"] == ["Spanish"]
    storage.close()
