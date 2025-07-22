// src/storage/db.rs
// Manages the SQLite database for storing card states.

use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use chrono;

use crate::deck::Card;

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    /// Creates a new DatabaseManager and opens a connection to the database file.
    pub fn new(deck_id: &str) -> Result<Self> {
        let path = Path::new("anki/history");
        fs::create_dir_all(path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let db_path = path.join(format!("{}.db", deck_id));
        
        let conn = Connection::open(db_path)?;
        let manager = DatabaseManager { conn };
        manager.init_schema()?;
        
        Ok(manager)
    }

    /// Creates the necessary tables if they don't already exist.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY,
                front TEXT NOT NULL,
                back TEXT NOT NULL,
                media_id INTEGER,
                similarity_hash TEXT
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS srs_log (
                card_id INTEGER PRIMARY KEY,
                next_due_ts INTEGER,
                interval INTEGER,
                ease REAL,
                lapses INTEGER,
                reps INTEGER
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
                param_id TEXT,
                arm_value INTEGER,
                alpha INTEGER,
                beta INTEGER,
                PRIMARY KEY (param_id, arm_value)
            )",
            [],
        )?;

        self.conn.execute(
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

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT
            )",
            [],
        )?;

        // Legacy card_state table for compatibility
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS card_state (
                id INTEGER PRIMARY KEY,
                due INTEGER NOT NULL,
                interval INTEGER NOT NULL,
                ease_factor INTEGER NOT NULL,
                lapses INTEGER NOT NULL
            )",
            [],
        )?;

        // Table for tracking difficult cards for prioritization
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS difficult_cards (
                card_id INTEGER,
                difficulty_type TEXT NOT NULL, -- 'failed' or 'hard'
                timestamp INTEGER NOT NULL,
                date TEXT NOT NULL, -- YYYY-MM-DD format for easy daily queries
                PRIMARY KEY (card_id, difficulty_type, date)
            )",
            [],
        )?;

        // Table for tracking all daily card ratings for progress bar visualization
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_ratings (
                card_id INTEGER,
                rating TEXT NOT NULL, -- 'Easy', 'Good', 'Hard', 'Again'
                timestamp INTEGER NOT NULL,
                date TEXT NOT NULL, -- YYYY-MM-DD format
                PRIMARY KEY (card_id, timestamp)
            )",
            [],
        )?;

        // Set database version
        self.conn.execute("PRAGMA user_version = 1", [])?;

        Ok(())
    }

    /// Updates the state of a single card in the database.
    /// Uses `INSERT OR REPLACE` to handle both new and existing cards.
    pub fn update_card_state(&self, card: &Card) -> Result<()> {
        // Update legacy card_state table for compatibility
        self.conn.execute(
            "INSERT OR REPLACE INTO card_state (id, due, interval, ease_factor, lapses)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                card.id,
                card.due,
                card.interval,
                card.ease_factor,
                card.lapses,
            ),
        )?;
        
        // Also update the new srs_log table for Core Learning Loop
        self.conn.execute(
            "INSERT OR REPLACE INTO srs_log (card_id, next_due_ts, interval, ease, lapses, reps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                card.id,
                card.due as i64,
                card.interval as i64,
                card.ease_factor as f64 / 1000.0, // Convert to decimal
                card.lapses as i64,
                1i64, // For now, increment reps
            ),
        )?;
        
        Ok(())
    }
    

    /// Records a card as difficult (failed or hard) for today's date
    pub fn record_difficult_card(&self, card_id: i64, difficulty_type: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        
        self.conn.execute(
            "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
             VALUES (?1, ?2, ?3, ?4)",
            (card_id, difficulty_type, now, today),
        )?;
        
        Ok(())
    }

    /// Gets difficult cards for a specific date, ordered by most recent first
    #[allow(dead_code)]
    pub fn get_difficult_cards_for_date(&self, date: &str, difficulty_type: Option<&str>) -> Result<Vec<i64>> {
        let query = if let Some(_diff_type) = difficulty_type {
            "SELECT card_id FROM difficult_cards 
             WHERE date = ?1 AND difficulty_type = ?2 
             ORDER BY timestamp DESC"
        } else {
            "SELECT card_id FROM difficult_cards 
             WHERE date = ?1 
             ORDER BY timestamp DESC"
        };
        
        let mut stmt = self.conn.prepare(query)?;
        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<i64> {
            Ok(row.get::<_, i64>(0)?)
        };

        
        let card_iter = if let Some(diff_type) = difficulty_type {
            stmt.query_map([date, diff_type], map_row)?
        } else {
            stmt.query_map([date], map_row)?
        };
        
        let mut cards = Vec::new();
        for card in card_iter {
            cards.push(card?);
        }
        
        Ok(cards)
    }

    /// Gets all difficult cards for today (both failed and hard)
    #[allow(dead_code)]
    pub fn get_todays_difficult_cards(&self) -> Result<(Vec<i64>, Vec<i64>)> {
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        
        let failed_cards = self.get_difficult_cards_for_date(&today, Some("failed"))?;
        let hard_cards = self.get_difficult_cards_for_date(&today, Some("hard"))?;
        
        Ok((failed_cards, hard_cards))
    }
    
    /// Record a card rating for daily progress tracking
    pub fn record_daily_rating(&self, card_id: i64, rating: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        
        self.conn.execute(
            "INSERT OR REPLACE INTO daily_ratings (card_id, rating, timestamp, date)
             VALUES (?1, ?2, ?3, ?4)",
            (card_id, rating, now, today),
        )?;
        
        Ok(())
    }
    
    /// Get all daily ratings for today in chronological order
    pub fn get_todays_ratings(&self) -> Result<Vec<(i64, String)>> {
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        
        let mut stmt = self.conn.prepare(
            "SELECT card_id, rating FROM daily_ratings 
             WHERE date = ?1 
             ORDER BY timestamp ASC"
        )?;
        
        let rating_iter = stmt.query_map([today], |row| {
            Ok((
                row.get::<_, i64>(0)?,    // card_id
                row.get::<_, String>(1)?, // rating
            ))
        })?;
        
        let mut ratings = Vec::new();
        for rating in rating_iter {
            ratings.push(rating?);
        }
        
        Ok(ratings)
    }
}
