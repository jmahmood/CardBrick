// src/storage/schema.rs
// Centralized database schema definitions to eliminate duplication

use rusqlite::{Connection, Result};

/// Creates the SRS (Spaced Repetition System) log table
/// Stores card learning progress with SM-2 algorithm data
pub fn create_srs_log_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS srs_log (
            card_id INTEGER PRIMARY KEY,
            next_due_ts INTEGER,
            interval INTEGER,
            ease_factor REAL,
            reps INTEGER,
            lapses INTEGER
        )",
        [],
    )?;
    Ok(())
}

/// Creates the bandit state table for Thompson sampling algorithm
/// Stores Beta distribution parameters for adaptive parameter tuning
pub fn create_bandit_state_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT NOT NULL,
            arm_value REAL NOT NULL,
            alpha INTEGER NOT NULL,
            beta INTEGER NOT NULL,
            PRIMARY KEY (param_id, arm_value)
        )",
        [],
    )?;
    Ok(())
}

/// Creates the daily log table for study session tracking
/// Records daily parameters and performance metrics
pub fn create_daily_log_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_log (
            date TEXT PRIMARY KEY,
            pack_sz INTEGER,
            rev_coef REAL,
            fail_k INTEGER,
            cards_studied INTEGER,
            points INTEGER,
            reward_scaled REAL,
            reward_bin INTEGER
        )",
        [],
    )?;
    Ok(())
}

/// Creates the daily ratings table for progress bar visualization
/// Stores all card ratings for daily progress tracking
pub fn create_daily_ratings_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_ratings (
            card_id INTEGER,
            rating TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL,
            PRIMARY KEY (card_id, timestamp)
        )",
        [],
    )?;
    Ok(())
}

/// Creates the meta table for key-value storage
/// Used for bandit algorithm metadata and configuration
pub fn create_meta_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Creates the cards table for basic card information
/// Used in tests and deck management
pub fn create_cards_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY
        )",
        [],
    )?;
    Ok(())
}

/// Creates the difficult cards table for challenge tracking
/// Stores cards marked as Hard or Again for prioritization
pub fn create_difficult_cards_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS difficult_cards (
            card_id INTEGER,
            difficulty_type TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL,
            PRIMARY KEY (card_id, difficulty_type, date)
        )",
        [],
    )?;
    Ok(())
}

/// Creates the study events table for point accounting
/// Records detailed point breakdown for each card interaction
pub fn create_study_events_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS study_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            bp INTEGER NOT NULL,
            df INTEGER NOT NULL,
            cb INTEGER NOT NULL,
            sb INTEGER NOT NULL,
            pa INTEGER NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Creates the profile table for user progress tracking
/// Stores total scores, level progression, and daily streaks
pub fn create_profile_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS profile (
            id INTEGER PRIMARY KEY DEFAULT 1,
            total_score INTEGER DEFAULT 0,
            level_score INTEGER DEFAULT 0,
            daily_streak INTEGER DEFAULT 0,
            last_study_date TEXT,
            CHECK (id = 1)
        )",
        [],
    )?;
    Ok(())
}

/// Creates the bandit log table for testing and debugging
/// Used in integration tests to track bandit learning
pub fn create_bandit_log_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bandit_log (
            date TEXT,
            param_id TEXT,
            arm INTEGER,
            reward INTEGER,
            PRIMARY KEY (date, param_id)
        )",
        [],
    )?;
    Ok(())
}

/// Creates all core tables required for the progress database
/// This is the standard set used by the main application
pub fn create_progress_database_tables(conn: &Connection) -> Result<()> {
    create_srs_log_table(conn)?;
    create_bandit_state_table(conn)?;
    create_daily_log_table(conn)?;
    create_daily_ratings_table(conn)?;
    Ok(())
}

/// Creates all tables required for deck-specific databases
/// Used by DatabaseManager for individual deck storage
pub fn create_deck_database_tables(conn: &Connection) -> Result<()> {
    // Cards table with full schema for deck management
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY,
            front TEXT NOT NULL,
            back TEXT NOT NULL,
            media_id INTEGER,
            similarity_hash TEXT
        )",
        [],
    )?;
    
    create_srs_log_table(conn)?;
    create_bandit_state_table(conn)?;
    create_daily_log_table(conn)?;
    create_meta_table(conn)?;
    create_difficult_cards_table(conn)?;
    create_daily_ratings_table(conn)?;
    create_study_events_table(conn)?;
    create_profile_table(conn)?;
    
    // Set database version for deck databases
    conn.execute("PRAGMA user_version = 3", [])?;
    
    Ok(())
}

/// Creates all tables required for comprehensive testing
/// Used by test setup functions across the codebase
pub fn create_test_database_tables(conn: &Connection) -> Result<()> {
    create_srs_log_table(conn)?;
    create_bandit_state_table(conn)?;
    create_daily_log_table(conn)?;
    create_daily_ratings_table(conn)?;
    create_meta_table(conn)?;
    create_cards_table(conn)?;
    create_bandit_log_table(conn)?;
    Ok(())
}