// src/storage/db.rs
// Manages the SQLite database for storing card states.

use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use chrono;

use crate::storage::schema::*;
use std::path::PathBuf;

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    /// Creates a new DatabaseManager and opens a connection to the database file.
    /// If custom_path is provided, uses that instead of the default "anki/history" directory.
    pub fn new(deck_id: &str) -> Result<Self> {
        Self::new_with_path(deck_id, None)
    }

    /// Creates a new DatabaseManager with an optional custom path.
    /// If custom_path is None, uses the default "anki/history" directory.
    pub fn new_with_path(deck_id: &str, custom_path: Option<&Path>) -> Result<Self> {
        let db_path = if let Some(custom_path) = custom_path {
            custom_path.to_path_buf()
        } else {
            let path = Path::new("anki/history");
            fs::create_dir_all(path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
            path.join(format!("{}.db", deck_id))
        };
        
        let conn = Connection::open(db_path)?;
        let manager = DatabaseManager { conn };
        manager.init_schema()?;
        
        Ok(manager)
    }

    /// Creates the necessary tables if they don't already exist.
    fn init_schema(&self) -> Result<()> {
        create_deck_database_tables(&self.conn)
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

    // ===== Explore mode: adoption bookkeeping (deck-scoped DB) =====

    /// Get number of cards adopted today for this deck
    pub fn adopted_today(&self) -> Result<i64> {
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let count: i64 = self.conn
            .prepare("SELECT adopted_count FROM deck_daily_stats WHERE study_day = ?1")?
            .query_row([&today], |row| Ok(row.get::<_, i64>(0)?))
            .unwrap_or(0);
        Ok(count)
    }

    /// Increment today's adopted count and return the new total
    pub fn increment_adopted_today(&self) -> Result<i64> {
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        // Upsert row and increment
        self.conn.execute(
            "INSERT INTO deck_daily_stats (study_day, adopted_count) VALUES (?1, 1)
             ON CONFLICT(study_day) DO UPDATE SET adopted_count = adopted_count + 1",
            [&today],
        )?;
        // Read back value
        self.adopted_today()
    }

    /// Mark first_seen timestamp if missing
    pub fn mark_first_seen_if_missing(&self, card_id: i64, ts: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO card_state (card_id, first_seen_ts) VALUES (?1, ?2)
             ON CONFLICT(card_id) DO UPDATE SET first_seen_ts = COALESCE(first_seen_ts, excluded.first_seen_ts)",
            (card_id, ts),
        )?;
        Ok(())
    }

    /// Mark adopted timestamp (only set once)
    pub fn mark_adopted(&self, card_id: i64, ts: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO card_state (card_id, adopted_ts) VALUES (?1, ?2)
             ON CONFLICT(card_id) DO UPDATE SET adopted_ts = COALESCE(adopted_ts, excluded.adopted_ts)",
            (card_id, ts),
        )?;
        Ok(())
    }
}

/// Get the path to the progress database file
pub fn progress_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cardbrick")
        .join("progress.db")
}

/// Check if a card has been adopted into SRS (exists in progress srs_log)
pub fn is_card_adopted_in_progress(card_id: i64) -> Result<bool> {
    let path = progress_path();
    let conn = Connection::open(&path)?;
    let exists: i64 = conn
        .prepare("SELECT COUNT(1) FROM srs_log WHERE card_id = ?1")?
        .query_row([card_id], |row| Ok(row.get::<_, i64>(0)?))?;
    Ok(exists > 0)
}

/// Initialize the progress database with all required tables
pub fn init_progress_database() -> Result<()> {
    let path = progress_path();
    
    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
    }
    
    let conn = Connection::open(&path)?;
    create_progress_database_tables(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use chrono::{Utc, Duration};

    fn create_test_database_manager(test_name: &str) -> (DatabaseManager, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join(format!("{}.db", test_name));
        let manager = DatabaseManager::new_with_path("test_deck", Some(&db_path)).unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_record_and_retrieve_difficult_cards() {
        let (manager, _temp_dir) = create_test_database_manager("difficult_cards");
        
        // Test recording failed cards
        manager.record_difficult_card(123, "failed").unwrap();
        manager.record_difficult_card(456, "hard").unwrap();
        
        // Test retrieval by type
        let (failed, hard) = manager.get_todays_difficult_cards().unwrap();
        assert_eq!(failed, vec![123]);
        assert_eq!(hard, vec![456]);
    }

    #[test]
    fn test_daily_rating_chronological_order() {
        let (manager, _temp_dir) = create_test_database_manager("daily_rating_order");
        
        // Record ratings with specific order
        manager.record_daily_rating(100, "Good").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamps
        manager.record_daily_rating(200, "Hard").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.record_daily_rating(300, "Easy").unwrap();
        
        // Verify chronological order
        let ratings = manager.get_todays_ratings().unwrap();
        assert_eq!(ratings.len(), 3);
        assert_eq!(ratings[0], (100, "Good".to_string()));
        assert_eq!(ratings[1], (200, "Hard".to_string()));
        assert_eq!(ratings[2], (300, "Easy".to_string()));
    }

    #[test]
    fn test_study_event_points_calculation() {
        let (manager, _temp_dir) = create_test_database_manager("study_events");
        
        let timestamp = Utc::now().timestamp();
        
        // Record multiple study events
        manager.record_study_event(1, timestamp, 10, 2, 5, 3, 20).unwrap();
        manager.record_study_event(2, timestamp + 1, 15, 3, 0, 2, 20).unwrap();
        manager.record_study_event(3, timestamp + 2, 0, 1, 10, 0, 11).unwrap();
        
        // Verify points sum correctly (20 + 20 + 11 = 51)
        let total_points = manager.get_todays_points().unwrap();
        assert_eq!(total_points, 51);
    }

    #[test]
    fn test_profile_score_updates() {
        let (manager, _temp_dir) = create_test_database_manager("profile_scores");
        
        // Initial profile should have zero scores
        let (total_score, level_score, streak, date) = manager.get_profile().unwrap();
        assert_eq!(total_score, 0);
        assert_eq!(level_score, 0);
        assert_eq!(streak, 0);
        assert_eq!(date, None);
        
        // Add points multiple times
        manager.update_profile_scores(25).unwrap();
        manager.update_profile_scores(15).unwrap();
        
        // Verify total_score increments properly
        let (total_score, level_score, _, _) = manager.get_profile().unwrap();
        assert_eq!(total_score, 40);
        assert_eq!(level_score, 40);
    }

    #[test]
    fn test_difficult_cards_date_isolation() {
        let (manager, _temp_dir) = create_test_database_manager("date_isolation");
        
        // Record difficult cards
        manager.record_difficult_card(111, "failed").unwrap();
        manager.record_difficult_card(222, "hard").unwrap();
        
        // Get today's cards
        let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let failed_today = manager.get_difficult_cards_for_date(&today, Some("failed")).unwrap();
        let hard_today = manager.get_difficult_cards_for_date(&today, Some("hard")).unwrap();
        
        assert_eq!(failed_today, vec![111]);
        assert_eq!(hard_today, vec![222]);
        
        // Test with different date - should be empty
        let yesterday = (Utc::now().date_naive() - Duration::days(1)).format("%Y-%m-%d").to_string();
        let failed_yesterday = manager.get_difficult_cards_for_date(&yesterday, Some("failed")).unwrap();
        assert_eq!(failed_yesterday, Vec::<i64>::new());
    }

    #[test]
    fn test_daily_streak_logic() {
        let (manager, _temp_dir) = create_test_database_manager("daily_streak");
        
        // First study should set streak to 1
        let streak1 = manager.update_daily_streak().unwrap();
        assert_eq!(streak1, 1);
        
        // Same day study should not change streak
        let streak2 = manager.update_daily_streak().unwrap();
        assert_eq!(streak2, 1);
        
        // Verify profile was updated
        let (_, _, streak, last_date) = manager.get_profile().unwrap();
        assert_eq!(streak, 1);
        assert!(last_date.is_some());
    }
}
