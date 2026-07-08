"""Storage support for the four-phase 'vocab' card type."""

import sqlite3

from cardbrick.storage import SCHEMA_VERSION, Storage


def test_basic_cards_default_to_card_type_basic(storage, service):
    from conftest import seed_card
    seed_card(storage, service, 1)
    card = storage.get_card(1)
    assert card["card_type"] == "basic"
    assert storage.get_vocab_detail(1) is None


def test_vocab_card_round_trips(storage):
    storage.upsert_card(card_id=1, note_id=1, deck="Vocabulario",
                        front="gato", back="cat", tags="animals",
                        audio_filename="gato.mp3", audio_side="front",
                        card_type="vocab")
    storage.upsert_vocab_card(
        card_id=1, word="gato", word_en="cat", word_jp="猫",
        gendered_forms="la gata",
        definitions="Animal doméstico felino.", image_filename="gato.jpg",
        example_es="El gato duerme.", example_audio="ex1.mp3",
        example_en="The cat sleeps.", example_jp="猫は眠る。",
        report_link="https://example.com/report/1")
    from cardbrick.scheduler import ReviewScheduler
    storage.init_review_state(ReviewScheduler().initial_state(1))
    storage.commit()

    card = storage.get_card(1)
    assert card["card_type"] == "vocab"
    detail = storage.get_vocab_detail(1)
    assert detail["word"] == "gato"
    assert detail["word_en"] == "cat"
    assert detail["gendered_forms"] == "la gata"
    assert detail["image_filename"] == "gato.jpg"
    assert detail["example_audio"] == "ex1.mp3"
    assert detail["report_link"] == "https://example.com/report/1"


def test_reimporting_vocab_card_updates_content_not_review_state(storage,
                                                                  service):
    from conftest import seed_card
    seed_card(storage, service, 1, tags="animals")
    storage.upsert_card(card_id=1, note_id=1, deck="Vocabulario",
                        front="gato", back="cat", tags="animals",
                        card_type="vocab")
    storage.upsert_vocab_card(
        card_id=1, word="gato", word_jp=None, gendered_forms="",
        definitions="v1", image_filename=None, example_es="",
        example_audio=None, example_en="", example_jp=None,
        report_link=None)
    storage.commit()
    service.answer_card(1, 3)
    state_before = dict(storage.get_review_state(1))

    # Re-import: content changes, review progress must not reset.
    storage.upsert_vocab_card(
        card_id=1, word="gato", word_en="cat", word_jp=None,
        gendered_forms="",
        definitions="v2 (corrected)", image_filename=None, example_es="",
        example_audio=None, example_en="", example_jp=None,
        report_link=None)
    storage.init_review_state(service.scheduler.initial_state(1))
    storage.commit()

    assert storage.get_vocab_detail(1)["definitions"] == "v2 (corrected)"
    assert dict(storage.get_review_state(1)) == state_before


def test_migration_adds_vocab_support_to_old_database(tmp_path):
    """A pre-vocab-card database (schema v2, no card_type/vocab_cards)
    migrates cleanly and gains both."""
    db = str(tmp_path / "old.db")
    conn = sqlite3.connect(db)
    conn.executescript("""
        CREATE TABLE cards (
            id INTEGER PRIMARY KEY, note_id INTEGER NOT NULL,
            deck TEXT NOT NULL, front TEXT NOT NULL, back TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '', audio_filename TEXT,
            audio_side TEXT, suspended INTEGER NOT NULL DEFAULT 0,
            buried_until TEXT, created_at TEXT, updated_at TEXT
        );
        CREATE TABLE review_state (
            card_id INTEGER PRIMARY KEY, due TEXT NOT NULL,
            stability REAL, difficulty REAL,
            elapsed_days INTEGER NOT NULL DEFAULT 0,
            scheduled_days INTEGER NOT NULL DEFAULT 0,
            reps INTEGER NOT NULL DEFAULT 0,
            lapses INTEGER NOT NULL DEFAULT 0,
            state INTEGER NOT NULL, fsrs_json TEXT NOT NULL
        );
    """)
    conn.execute("INSERT INTO cards VALUES (1, 1, 'Spanish', 'hola', "
                 "'hello', '', NULL, NULL, 0, NULL, NULL, NULL)")
    conn.execute("INSERT INTO review_state VALUES (1, "
                 "'2026-01-01T00:00:00+00:00', 2.5, 5.0, 0, 0, 3, 0, 2, "
                 "'{}')")
    conn.commit()
    conn.close()

    storage = Storage(db)
    assert storage.schema_version() == SCHEMA_VERSION
    card = storage.get_card(1)
    assert card["card_type"] == "basic"  # migrated column, safe default
    assert card["reps"] == 3             # progress preserved
    # New table works immediately.
    storage.upsert_card(card_id=2, note_id=2, deck="Vocabulario",
                        front="perro", back="dog", tags="", card_type="vocab")
    storage.upsert_vocab_card(2, "perro", None, "", "Canino.", None, "",
                              None, "", None, None)
    storage.commit()
    assert storage.get_vocab_detail(2)["word"] == "perro"
    storage.close()


def test_migration_adds_word_en_to_existing_vocab_table(tmp_path):
    db = str(tmp_path / "v7.db")
    conn = sqlite3.connect(db)
    conn.executescript("""
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
        INSERT INTO meta VALUES ('schema_version', '7');
        CREATE TABLE cards (
            id INTEGER PRIMARY KEY, note_id INTEGER NOT NULL,
            deck TEXT NOT NULL, front TEXT NOT NULL, back TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '', audio_filename TEXT,
            audio_side TEXT, suspended INTEGER NOT NULL DEFAULT 0,
            buried_until TEXT, created_at TEXT, updated_at TEXT,
            card_type TEXT NOT NULL DEFAULT 'basic');
        CREATE TABLE vocab_cards (
            card_id INTEGER PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
            word TEXT NOT NULL, word_jp TEXT, gendered_forms TEXT,
            definitions TEXT, image_filename TEXT, example_es TEXT,
            example_audio TEXT, example_en TEXT, example_jp TEXT,
            report_link TEXT);
    """)
    conn.commit()
    conn.close()

    storage = Storage(db)
    try:
        assert storage.schema_version() == SCHEMA_VERSION
        columns = {row["name"] for row in
                   storage.conn.execute("PRAGMA table_info(vocab_cards)")}
        assert "word_en" in columns
    finally:
        storage.close()
