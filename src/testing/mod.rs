// src/testing/mod.rs
// Test infrastructure and utilities (only available in test builds)

#[cfg(test)]
use crate::deck::scanner::DeckManifest;
#[cfg(test)]
use crate::storage::schema::*;
#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use tempfile::{tempdir, TempDir};

/// Test helper structure for test card data
#[cfg(test)]
pub struct TestCard {
    pub id: i64,
    pub front: String,
    pub back: String,
}

/// Sets up a temporary test database with all required tables
/// Returns (temp_dir, db_path) - keep temp_dir alive to prevent cleanup
#[cfg(test)]
pub fn setup_test_database() -> (TempDir, PathBuf) {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create database and initialize schema
    let conn = Connection::open(&db_path).unwrap();
    create_test_database_tables(&conn).unwrap();
    drop(conn);

    (temp_dir, db_path)
}

/// Sets up a test database with sample card data
/// Returns (temp_dir, db_path) - keep temp_dir alive to prevent cleanup
#[cfg(test)]
pub fn setup_test_database_with_cards(cards: Vec<TestCard>) -> (TempDir, PathBuf) {
    let (temp_dir, db_path) = setup_test_database();

    // Insert test cards
    let conn = Connection::open(&db_path).unwrap();
    for card in cards {
        conn.execute("INSERT INTO cards (id) VALUES (?1)", [card.id])
            .unwrap();
    }
    drop(conn);

    (temp_dir, db_path)
}

/// Sets up a test database with due cards (past timestamps)
/// Returns (temp_dir, db_path) - keep temp_dir alive to prevent cleanup
#[cfg(test)]
pub fn setup_test_database_with_due_cards(card_count: i32) -> (TempDir, PathBuf) {
    let (temp_dir, db_path) = setup_test_database();

    let conn = Connection::open(&db_path).unwrap();
    let past_timestamp = 1000000000i64; // Timestamp in the past

    for i in 1..=card_count {
        // Insert card
        conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
            .unwrap();

        // Insert SRS log entry (card is due)
        conn.execute(
            "INSERT INTO srs_log (card_id, next_due_ts, interval, ease_factor, reps, lapses) 
             VALUES (?1, ?2, 1, 2.5, 0, 0)",
            [i as i64, past_timestamp],
        )
        .unwrap();
    }
    drop(conn);

    (temp_dir, db_path)
}

/// Sets up a test database with new cards (no SRS entries)
/// Returns (temp_dir, db_path) - keep temp_dir alive to prevent cleanup
#[cfg(test)]
pub fn setup_test_database_with_new_cards(card_count: i32) -> (TempDir, PathBuf) {
    let (temp_dir, db_path) = setup_test_database();

    let conn = Connection::open(&db_path).unwrap();
    for i in 1..=card_count {
        conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
            .unwrap();
    }
    drop(conn);

    (temp_dir, db_path)
}

/// Sets up a minimal test database with just core tables
/// Used for lightweight tests that don't need full schema
#[cfg(test)]
pub fn setup_minimal_test_database() -> (TempDir, PathBuf) {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let conn = Connection::open(&db_path).unwrap();
    create_srs_log_table(&conn).unwrap();
    create_cards_table(&conn).unwrap();
    create_daily_log_table(&conn).unwrap();
    create_bandit_state_table(&conn).unwrap();
    create_meta_table(&conn).unwrap();
    drop(conn);

    (temp_dir, db_path)
}

/// Creates a cached deck structure for testing deck loader functionality.
/// This creates the proper directory structure with manifest.json and Anki database
/// that load_apkg expects to find.
/// Returns (temp_dir, deck_path) - keep temp_dir alive to prevent cleanup
#[cfg(test)]
pub fn setup_cached_deck_structure(deck_hash: &str) -> (TempDir, PathBuf) {
    let temp_dir = tempdir().unwrap();
    let deck_path = temp_dir.path().join(deck_hash);
    std::fs::create_dir_all(&deck_path).unwrap();

    // Create manifest.json
    let manifest = DeckManifest {
        apkg_name: format!("{}.apkg", deck_hash),
        sha256: deck_hash.to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        db_file: "collection.anki2".to_string(),
        card_count: 15,
        notes_count: 15,
        deck_name: "Test Deck".to_string(),
        anki_version: 2,
    };

    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let manifest_path = deck_path.join("manifest.json");
    std::fs::write(&manifest_path, manifest_json).unwrap();

    // Create the Anki database
    let db_path = deck_path.join("collection.anki2");
    create_test_anki_database(&db_path).unwrap();

    (temp_dir, deck_path)
}

/// Creates a test Anki database with sample cards for deck loader tests
#[cfg(test)]
fn create_test_anki_database(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;

    // Create Anki-compatible cards table
    conn.execute(
        "CREATE TABLE cards (
            id INTEGER PRIMARY KEY,
            nid INTEGER NOT NULL,
            due INTEGER NOT NULL,
            ivl INTEGER NOT NULL,
            factor INTEGER NOT NULL,
            lapses INTEGER NOT NULL
        )",
        [],
    )?;

    // Create Anki-compatible notes table
    conn.execute(
        "CREATE TABLE notes (
            id INTEGER PRIMARY KEY,
            flds TEXT NOT NULL
        )",
        [],
    )?;

    // Insert test cards - some due today, some overdue, some future
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;

    // Due/overdue cards (should be selected first)
    conn.execute(
        "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (1, 1, ?1, 1, 2500, 0)",
        [today - 1],
    )?; // overdue
    conn.execute(
        "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (2, 2, ?1, 0, 2500, 0)",
        [today],
    )?; // due today
    conn.execute(
        "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (3, 3, ?1, 2, 2500, 1)",
        [today],
    )?; // due today

    // Future cards (for minimum card selection)
    for i in 4..=15 {
        conn.execute(
            "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?1, ?1, ?2, 5, 2500, 0)",
            [i, today + 10],
        )?;
    }

    // Create corresponding notes
    for i in 1..=15 {
        conn.execute(
            "INSERT INTO notes (id, flds) VALUES (?1, ?2)",
            rusqlite::params![i, format!("Front text {}\x1fBack text {}", i, i)],
        )?;
    }

    Ok(())
}
