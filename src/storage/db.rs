// src/storage/db.rs
// Manages the SQLite database for storing card states.

use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use chrono;

use crate::deck::Card;
use std::path::PathBuf;

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

        // Bandit state table for Thompson sampling
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
                param_id TEXT NOT NULL,
                arm_value REAL NOT NULL,
                alpha INTEGER NOT NULL,
                beta INTEGER NOT NULL,
                PRIMARY KEY (param_id, arm_value)
            )",
            [],
        )?;

        // Study events table for complete point-accounting system
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS study_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                bp INTEGER NOT NULL,      -- Base Points
                df INTEGER NOT NULL,      -- Difficulty Factor  
                cb INTEGER NOT NULL,      -- Combo Bonus
                sb INTEGER NOT NULL,      -- Speed Bonus
                pa INTEGER NOT NULL       -- Points Awarded (total)
            )",
            [],
        )?;

        // Profile table for user progress tracking
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS profile (
                id INTEGER PRIMARY KEY DEFAULT 1,
                total_score INTEGER DEFAULT 0,
                level_score INTEGER DEFAULT 0,
                daily_streak INTEGER DEFAULT 0,
                last_study_date TEXT,
                CHECK (id = 1)  -- Ensure only one profile row
            )",
            [],
        )?;

        // Set database version
        self.conn.execute("PRAGMA user_version = 3", [])?;

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
        
        // Read from progress database, not per-deck database
        let progress_db_path = progress_path();
        let progress_conn = Connection::open(&progress_db_path)?;
        
        let mut stmt = progress_conn.prepare(
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

    /// Record a study event with complete point breakdown
    pub fn record_study_event(
        &self,
        card_id: i64,
        timestamp: i64,
        bp: i32,
        df: i32,
        cb: i32,
        sb: i32,
        pa: i32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO study_events (card_id, timestamp, bp, df, cb, sb, pa)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (card_id, timestamp, bp, df, cb, sb, pa),
        )?;
        Ok(())
    }

    /// Get today's total points from study events
    pub fn get_todays_points(&self) -> Result<i32> {
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let today_end = today_start + 86400; // 24 hours

        let points: i32 = self.conn
            .prepare("SELECT COALESCE(SUM(pa), 0) FROM study_events WHERE timestamp >= ?1 AND timestamp < ?2")?
            .query_row([today_start, today_end], |row| Ok(row.get(0)?))?;

        Ok(points)
    }

    /// Update profile scores (total_score, level_score)
    pub fn update_profile_scores(&self, points_to_add: i32) -> Result<()> {
        // Initialize profile if it doesn't exist
        self.conn.execute(
            "INSERT OR IGNORE INTO profile (id, total_score, level_score, daily_streak, last_study_date)
             VALUES (1, 0, 0, 0, NULL)",
            [],
        )?;

        // Add points to both total and level scores
        self.conn.execute(
            "UPDATE profile SET 
                total_score = total_score + ?1,
                level_score = level_score + ?1
             WHERE id = 1",
            [points_to_add],
        )?;

        Ok(())
    }

    /// Get current profile data
    pub fn get_profile(&self) -> Result<(i32, i32, i32, Option<String>)> {
        // Initialize profile if it doesn't exist
        self.conn.execute(
            "INSERT OR IGNORE INTO profile (id, total_score, level_score, daily_streak, last_study_date)
             VALUES (1, 0, 0, 0, NULL)",
            [],
        )?;

        let (total_score, level_score, daily_streak, last_study_date) = self.conn
            .prepare("SELECT total_score, level_score, daily_streak, last_study_date FROM profile WHERE id = 1")?
            .query_row([], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;

        Ok((total_score, level_score, daily_streak, last_study_date))
    }

    /// Update daily streak
    pub fn update_daily_streak(&self) -> Result<i32> {
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        
        // Initialize profile if it doesn't exist
        self.conn.execute(
            "INSERT OR IGNORE INTO profile (id, total_score, level_score, daily_streak, last_study_date)
             VALUES (1, 0, 0, 0, NULL)",
            [],
        )?;

        let (current_streak, last_date) = self.conn
            .prepare("SELECT daily_streak, last_study_date FROM profile WHERE id = 1")?
            .query_row([], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })?;

        let new_streak = match last_date {
            Some(last) if last == today => current_streak, // Same day, no change
            Some(last) => {
                // Check if yesterday
                if let Ok(last_date) = chrono::NaiveDate::parse_from_str(&last, "%Y-%m-%d") {
                    let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
                    if last_date == yesterday {
                        current_streak + 1 // Continue streak
                    } else {
                        1 // Reset streak
                    }
                } else {
                    1 // Invalid date, reset
                }
            }
            None => 1, // First time studying
        };

        // Update streak and last study date
        self.conn.execute(
            "UPDATE profile SET daily_streak = ?1, last_study_date = ?2 WHERE id = 1",
            [new_streak.to_string(), today],
        )?;

        Ok(new_streak)
    }
}

/// Get the path to the progress database file
pub fn progress_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cardbrick")
        .join("progress.db")
}

/// Initialize the progress database with all required tables
pub fn init_progress_database() -> Result<()> {
    let path = progress_path();
    
    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
    }
    
    let conn = Connection::open(&path)?;
    
    // Create all required tables that scheduler functions expect
    conn.execute(
        "CREATE TABLE IF NOT EXISTS srs_log (
            card_id INTEGER PRIMARY KEY,
            next_due_ts INTEGER,
            interval INTEGER,
            ease_factor INTEGER,
            reps INTEGER,
            lapses INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT,
            arm_value INTEGER,
            alpha INTEGER,
            beta INTEGER,
            PRIMARY KEY (param_id, arm_value)
        )",
        [],
    )?;

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

    // Create daily_ratings table for progress bar functionality
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_ratings (
            card_id INTEGER,
            rating TEXT NOT NULL, -- 'Easy', 'Good', 'Hard', 'Again'
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL, -- YYYY-MM-DD format
            PRIMARY KEY (card_id, timestamp)
        )",
        [],
    )?;

    Ok(())
}
