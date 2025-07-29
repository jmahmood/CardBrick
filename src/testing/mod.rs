// src/testing/mod.rs
// Test infrastructure and utilities (only available in test builds)

#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use tempfile::{TempDir, tempdir};
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use crate::storage::schema::*;

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
        conn.execute(
            "INSERT INTO cards (id) VALUES (?1)",
            [card.id],
        ).unwrap();
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
        conn.execute(
            "INSERT INTO cards (id) VALUES (?1)", 
            [i],
        ).unwrap();
        
        // Insert SRS log entry (card is due)
        conn.execute(
            "INSERT INTO srs_log (card_id, next_due_ts, interval, ease_factor, reps, lapses) 
             VALUES (?1, ?2, 1, 2.5, 0, 0)",
            [i as i64, past_timestamp],
        ).unwrap();
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
        conn.execute(
            "INSERT INTO cards (id) VALUES (?1)", 
            [i],
        ).unwrap();
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