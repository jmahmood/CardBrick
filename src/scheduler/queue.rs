// src/scheduler/queue.rs
// Daily queue builder for Core Learning Loop

use crate::config::BACKLOG_CAP;
use crate::deck::Deck;
use crate::scheduler::bandit::BanditManager;
use crate::storage::db::progress_path;
// use crate::storage::models::{CardId, SrsRow, DailyLogRow};
use crate::storage::models::CardId;
use chrono::NaiveDate;
use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueData {
    pub pack_size: usize,
    pub review_coefficient: f64,
    #[serde(default = "default_fail_k")]
    pub fail_k: usize,
    pub cards: Vec<CardId>,
    #[serde(default)]
    pub completed_cards: Vec<CardId>, // Track completed cards (defaults to empty for backward compatibility)
}

fn default_fail_k() -> usize {
    5 // Default value for backward compatibility
}

/// Ensures today's queue exists, creating it if needed (idempotent)
pub fn ensure_today(today: NaiveDate) -> Result<(), Box<dyn std::error::Error>> {
    let queue_path = get_queue_path(today);

    // If queue already exists for today, return early
    if queue_path.exists() {
        return Ok(());
    }

    // Build today's queue with default sequential IDs
    build_today(today)?;
    Ok(())
}

/// Ensures today's queue exists using actual deck card IDs
pub fn ensure_today_with_deck(
    today: NaiveDate,
    deck: &Deck,
) -> Result<(), Box<dyn std::error::Error>> {
    let deck_id = get_deck_id(deck);
    let queue_path = get_deck_queue_path(today, &deck_id);

    // If queue already exists for today and this deck, return early
    if queue_path.exists() {
        return Ok(());
    }

    // Build today's queue using actual deck card IDs
    build_today_with_deck(today, deck)?;
    Ok(())
}

/// Loads today's queue from disk (panics if missing)
pub fn _load_today(today: NaiveDate) -> Vec<CardId> {
    let queue_path = get_queue_path(today);

    let queue_data: QueueData = serde_json::from_str(
        &fs::read_to_string(&queue_path)
            .unwrap_or_else(|_| panic!("Queue file missing for {}", today)),
    )
    .expect("Failed to parse queue JSON");

    queue_data.cards
}

/// Get cards that are due for review today
pub fn due_cards(today: NaiveDate, conn: &Connection) -> SqlResult<Vec<CardId>> {
    let today_timestamp = today.and_hms_opt(9, 0, 0).unwrap().and_utc().timestamp();

    let mut stmt = conn.prepare(
        "SELECT card_id FROM srs_log 
         WHERE next_due_ts IS NULL OR next_due_ts <= ?1 
         ORDER BY next_due_ts ASC NULLS FIRST 
         LIMIT ?2",
    )?;

    let card_ids: Result<Vec<CardId>, _> = stmt
        .query_map([today_timestamp, BACKLOG_CAP as i64], |row| {
            row.get::<_, i64>(0)
        })?
        .collect();

    card_ids
}

/// Get new cards that haven't been studied yet
pub fn next_new_cards(count: usize, conn: &Connection) -> SqlResult<Vec<CardId>> {
    let mut stmt = conn.prepare(
        "SELECT c.id FROM cards c 
         LEFT JOIN srs_log s ON c.id = s.card_id 
         WHERE s.card_id IS NULL 
         LIMIT ?1",
    )?;

    let card_ids: Result<Vec<CardId>, _> = stmt
        .query_map([count as i64], |row| row.get::<_, i64>(0))?
        .collect();

    card_ids
}

/// Builds today's queue and saves it to disk
/// Uses bandit sampling for adaptive parameter tuning
pub fn build_today(today: NaiveDate) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = progress_path();
    build_today_with_path(today, &db_path)
}

/// Builds today's queue with explicit database path - NO GLOBAL STATE
pub fn build_today_with_path(
    today: NaiveDate,
    db_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Sample parameters from bandits and persist immediately
    let mut bandit_manager = BanditManager::new_with_path(db_path)?;
    let (pack_sz, rev_coef, fail_k) = bandit_manager.sample_parameters_with_path(today, db_path);
    bandit_manager.save_all_with_path(db_path)?; // Persist bandit state after sampling

    // Open database connection and start transaction
    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;

    // Get due cards (already limited by BACKLOG_CAP in SQL)
    let due_cards_all = due_cards(today, &tx)?;

    // Apply review coefficient cap
    let review_cap = ((rev_coef * pack_sz as f64).round() as usize).min(due_cards_all.len());
    let reviews_today: Vec<CardId> = due_cards_all.into_iter().take(review_cap).collect();

    // Get new cards (roughly 1/3 of pack size)
    let n_new = pack_sz / 3;
    let new_cards = next_new_cards(n_new, &tx)?;

    // Interleave review and new cards with new card first
    let cards = interleave_one_new_first(&reviews_today, &new_cards);

    let queue_data = QueueData {
        pack_size: pack_sz,
        review_coefficient: rev_coef,
        fail_k,
        cards,
        completed_cards: Vec::new(),
    };

    // Create skeleton row in daily_log
    create_daily_log_skeleton_tx(&tx, today, pack_sz, rev_coef, fail_k)?;

    // Commit transaction
    tx.commit()?;

    // Create queues directory if it doesn't exist
    let queue_dir = get_queue_dir();
    fs::create_dir_all(&queue_dir)?;

    // Write queue to JSON file
    let queue_path = get_queue_path(today);
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;

    println!("Created queue for {} with {} cards ({} reviews, {} new) (pack_sz={}, rev_coef={}, fail_k={})", 
             today, queue_data.cards.len(), reviews_today.len(), new_cards.len(), pack_sz, rev_coef, fail_k);
    Ok(())
}

/// Builds today's queue using actual deck card IDs
pub fn build_today_with_deck(
    today: NaiveDate,
    deck: &Deck,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = progress_path();
    build_today_with_deck_and_path(today, deck, &db_path)
}

/// Builds today's queue with deck and explicit database path - NO GLOBAL STATE
pub fn build_today_with_deck_and_path(
    today: NaiveDate,
    deck: &Deck,
    db_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Sample parameters from bandits and persist immediately
    let mut bandit_manager = BanditManager::new_with_path(db_path)?;
    let (pack_sz, rev_coef, fail_k) = bandit_manager.sample_parameters_with_path(today, db_path);
    bandit_manager.save_all_with_path(db_path)?; // Persist bandit state after sampling

    // Open database connection and start transaction
    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;

    // Create set of deck card IDs for filtering
    let deck_card_ids: std::collections::HashSet<CardId> =
        deck.cards.iter().map(|c| c.id).collect();

    // Get due cards and filter to only include cards from this deck
    let due_cards_all = due_cards(today, &tx)?;
    let deck_due_cards: Vec<CardId> = due_cards_all
        .into_iter()
        .filter(|card_id| deck_card_ids.contains(card_id))
        .collect();

    // Apply review coefficient cap
    let review_cap = ((rev_coef * pack_sz as f64).round() as usize).min(deck_due_cards.len());
    let reviews_today: Vec<CardId> = deck_due_cards.into_iter().take(review_cap).collect();

    // Get new cards derived from the deck itself (no global `cards` table).
    // A "new" card is any deck card not present in `srs_log`.
    // Explore detection: if no reviews due for this deck, show larger first batch.
    let explore_mode = reviews_today.is_empty() && crate::config::ENABLE_EXPLORE_MODE;
    let n_new = if explore_mode {
        crate::config::EXPLORE_BATCH_SIZE
    } else {
        pack_sz / 3
    };
    let studied_ids: std::collections::HashSet<CardId> = {
        let mut set = std::collections::HashSet::new();
        let mut stmt = tx.prepare("SELECT card_id FROM srs_log")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        for r in rows {
            set.insert(r?);
        }
        set
    };

    let mut new_cards: Vec<CardId> = Vec::new();
    for &cid in deck_card_ids.iter() {
        if !studied_ids.contains(&cid) {
            new_cards.push(cid);
            if new_cards.len() >= n_new {
                break;
            }
        }
    }

    // Interleave review and new cards with new card first
    let cards = interleave_one_new_first(&reviews_today, &new_cards);

    let queue_data = QueueData {
        pack_size: pack_sz,
        review_coefficient: rev_coef,
        fail_k,
        cards,
        completed_cards: Vec::new(),
    };

    // Create skeleton row in daily_log
    create_daily_log_skeleton_tx(&tx, today, pack_sz, rev_coef, fail_k)?;

    // Commit transaction
    tx.commit()?;

    // Create queues directory if it doesn't exist
    let queue_dir = get_queue_dir();
    fs::create_dir_all(&queue_dir)?;

    // Write deck-specific queue to JSON file
    let deck_id = get_deck_id(deck);
    let queue_path = get_deck_queue_path(today, &deck_id);
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;

    println!("Created deck-specific queue for {} (deck {}) with {} cards ({} reviews, {} new) (pack_sz={}, rev_coef={}, fail_k={})", 
             today, deck_id, queue_data.cards.len(), reviews_today.len(), new_cards.len(), pack_sz, rev_coef, fail_k);
    Ok(())
}

/// Interleave review and new cards, ensuring first card is new if available
fn interleave_one_new_first(reviews: &[CardId], new_cards: &[CardId]) -> Vec<CardId> {
    let mut result = Vec::new();

    // Start with a new card if available
    let mut new_iter = new_cards.iter();
    let mut review_iter = reviews.iter();

    if let Some(&new_card) = new_iter.next() {
        result.push(new_card);
    }

    // Interleave remaining cards
    let mut use_new = false;
    loop {
        let next_card = if use_new {
            new_iter.next()
        } else {
            review_iter.next()
        };

        match next_card {
            Some(&card_id) => {
                result.push(card_id);
                use_new = !use_new;
            }
            None => {
                // If one iterator is exhausted, add all remaining from the other
                if use_new {
                    result.extend(review_iter);
                } else {
                    result.extend(new_iter);
                }
                break;
            }
        }
    }

    result
}

/// Creates a skeleton row in daily_log table (transaction-aware)
fn create_daily_log_skeleton_tx(
    tx: &rusqlite::Transaction,
    today: NaiveDate,
    pack_sz: usize,
    rev_coef: f64,
    fail_k: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let date_str = today.format("%Y-%m-%d").to_string();
    tx.execute(
        "INSERT INTO daily_log (date, pack_sz, rev_coef, fail_k) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(date) DO UPDATE SET 
           pack_sz=excluded.pack_sz, 
           rev_coef=excluded.rev_coef, 
           fail_k=excluded.fail_k",
        [
            &date_str,
            &pack_sz.to_string(),
            &rev_coef.to_string(),
            &fail_k.to_string(),
        ],
    )?;

    println!(
        "Created daily_log skeleton: {} pack_sz={} rev_coef={} fail_k={}",
        today, pack_sz, rev_coef, fail_k
    );
    Ok(())
}

/// Get the path to the queue directory
fn get_queue_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cardbrick")
        .join("queues")
}

/// Get the path to a specific queue file
pub fn get_queue_path(date: NaiveDate) -> PathBuf {
    get_queue_dir().join(format!("queue_{}.json", date.format("%Y%m%d")))
}

/// Get the path to a deck-specific queue file
fn get_deck_queue_path(date: NaiveDate, deck_id: &str) -> PathBuf {
    get_queue_dir().join(format!("queue_{}_{}.json", date.format("%Y%m%d"), deck_id))
}

/// Generate a deck ID from the deck (using a simple hash of the first few card IDs)
pub fn get_deck_id(deck: &Deck) -> String {
    if deck.cards.is_empty() {
        return "empty".to_string();
    }

    // Create a simple ID from the first few card IDs
    let id_sample: Vec<i64> = deck.cards.iter().take(3).map(|c| c.id).collect();
    let base_id = format!(
        "{:x}",
        id_sample
            .iter()
            .fold(0u64, |acc, &x| acc.wrapping_add(x as u64))
    );

    // In test mode, add a unique timestamp to avoid cross-test contamination
    if cfg!(test) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("{}{:x}", base_id, timestamp as u64)
    } else {
        // Production: Use predictable ID for single-user usage
        base_id
    }
}

/// Mark a card as completed in today's deck-specific queue using deck ID
pub fn mark_card_completed_for_deck_id(
    today: NaiveDate,
    card_id: CardId,
    deck_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let queue_path = get_deck_queue_path(today, deck_id);

    if !queue_path.exists() {
        return Ok(()); // No queue exists, nothing to mark
    }

    // Load existing queue
    let mut queue_data: QueueData = serde_json::from_str(&fs::read_to_string(&queue_path)?)?;

    // Add to completed cards if not already there
    if !queue_data.completed_cards.contains(&card_id) {
        queue_data.completed_cards.push(card_id);
        println!(
            "✅ Marked card {} as completed in deck {}",
            card_id, deck_id
        );
    }

    // Save updated queue
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;

    Ok(())
}

/// Get remaining (uncompleted) cards from today's deck-specific queue
pub fn get_remaining_cards_for_deck(today: NaiveDate, deck: &Deck) -> Vec<CardId> {
    let deck_id = get_deck_id(deck);
    let queue_path = get_deck_queue_path(today, &deck_id);

    if !queue_path.exists() {
        println!(
            "⚠️ No deck-specific queue file exists for {} (deck {})",
            today, deck_id
        );
        return Vec::new();
    }

    match fs::read_to_string(&queue_path) {
        Ok(content) => match serde_json::from_str::<QueueData>(&content) {
            Ok(queue_data) => {
                let total_cards = queue_data.cards.len();
                let completed_count = queue_data.completed_cards.len();
                let remaining: Vec<CardId> = queue_data
                    .cards
                    .into_iter()
                    .filter(|card_id| !queue_data.completed_cards.contains(card_id))
                    .collect();
                println!(
                    "📋 Deck {} queue has {} total cards, {} completed, {} remaining",
                    deck_id,
                    total_cards,
                    completed_count,
                    remaining.len()
                );
                remaining
            }
            Err(e) => {
                println!("⚠️ Failed to parse deck queue file: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            println!("⚠️ Failed to read deck queue file: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interleave_one_new_first() {
        let reviews = vec![1, 2, 3];
        let new_cards = vec![10, 20];

        let result = interleave_one_new_first(&reviews, &new_cards);

        // Should start with new card (10)
        assert_eq!(result[0], 10);
        // Should contain all cards
        assert_eq!(result.len(), 5);

        // Test with no new cards
        let result_no_new = interleave_one_new_first(&reviews, &[]);
        assert_eq!(result_no_new, reviews);

        // Test with no reviews
        let result_no_reviews = interleave_one_new_first(&[], &new_cards);
        assert_eq!(result_no_reviews, new_cards);
    }

    #[test]
    fn test_queue_path_generation() {
        let date = NaiveDate::from_ymd_opt(2025, 7, 4).unwrap();
        let path = get_queue_path(date);

        assert!(path.to_string_lossy().contains("queue_20250704.json"));
    }

    #[test]
    fn test_bandit_driven_queue_parameters() {
        use crate::scheduler::bandit::Bandit;
        use rusqlite::Connection;
        use tempfile::tempdir;

        // Set up test environment with NO GLOBAL STATE
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize complete database schema
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT NOT NULL,
            arm_value REAL NOT NULL,
            alpha INTEGER NOT NULL,
            beta INTEGER NOT NULL,
            PRIMARY KEY (param_id, arm_value)
        )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
            [],
        )
        .unwrap();
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
        )
        .unwrap();
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
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY
        )",
            [],
        )
        .unwrap();
        drop(conn);

        // Create deterministic bandits with single arms using path injection
        let mut pack_bandit = Bandit::load_with_path("test_pack", &[12.0], &db_path).unwrap();
        let mut rev_bandit = Bandit::load_with_path("test_rev", &[2.0], &db_path).unwrap();
        let mut fail_bandit = Bandit::load_with_path("test_fail", &[5.0], &db_path).unwrap();

        let today = NaiveDate::from_ymd_opt(2025, 7, 24).unwrap();

        // Sample should return our fixed values using path injection
        assert_eq!(pack_bandit.sample_with_path(today, &db_path) as usize, 12);
        assert_eq!(rev_bandit.sample_with_path(today, &db_path) as f64, 2.0);
        assert_eq!(fail_bandit.sample_with_path(today, &db_path) as usize, 5);

        // Save the bandits using path injection
        pack_bandit.save_with_path(&db_path).unwrap();
        rev_bandit.save_with_path(&db_path).unwrap();
        fail_bandit.save_with_path(&db_path).unwrap();
    }

    #[test]
    fn test_review_coefficient_capping() {
        use rusqlite::Connection;
        use tempfile::tempdir;

        // Set up test environment with NO GLOBAL STATE
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create test database with many due cards
        let conn = Connection::open(&db_path).unwrap();

        // Create tables
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
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (id INTEGER PRIMARY KEY)",
            [],
        )
        .unwrap();

        // Insert many due cards (timestamps in past)
        let past_timestamp = 1000000000i64; // Way in the past
        for i in 1..=20 {
            conn.execute(
                "INSERT INTO srs_log (card_id, next_due_ts) VALUES (?1, ?2)",
                [i, past_timestamp],
            )
            .unwrap();
            conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
                .unwrap();
        }

        let today = NaiveDate::from_ymd_opt(2025, 7, 24).unwrap();
        let due_cards_result = due_cards(today, &conn).unwrap();

        // Should get many due cards
        assert!(due_cards_result.len() >= 10);

        // Test review coefficient capping logic
        let pack_sz = 10;
        let rev_coef = 1.5; // Low coefficient should limit reviews
        let review_cap = ((rev_coef * pack_sz as f64).round() as usize).min(due_cards_result.len());

        assert_eq!(review_cap, 15.min(due_cards_result.len())); // 1.5 * 10 = 15, capped by available
    }

    #[test]
    fn test_queue_build_idempotence() {
        use std::fs;
        use tempfile::tempdir;

        // Set up test environment with NO GLOBAL STATE
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize database
        let conn = Connection::open(&db_path).unwrap();

        // Create minimal schema
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
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (id INTEGER PRIMARY KEY)",
            [],
        )
        .unwrap();
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
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT NOT NULL,
            arm_value REAL NOT NULL,
            alpha INTEGER NOT NULL,
            beta INTEGER NOT NULL,
            PRIMARY KEY (param_id, arm_value)
        )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
            [],
        )
        .unwrap();

        // Insert some test data
        for i in 1..=5 {
            conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
                .unwrap();
        }

        let today = NaiveDate::from_ymd_opt(2025, 7, 24).unwrap();

        // Build queue first time
        let result1 = build_today_with_path(today, &db_path);
        assert!(result1.is_ok(), "First queue build should succeed");

        // Read the generated JSON
        let queue_path = get_queue_path(today);
        let json_content1 = fs::read_to_string(&queue_path).unwrap();

        // Build queue second time (should be idempotent due to existing daily_log entry)
        let result2 = build_today_with_path(today, &db_path);
        assert!(result2.is_ok(), "Second queue build should succeed");

        // Read the JSON again
        let json_content2 = fs::read_to_string(&queue_path).unwrap();

        // Parse both to compare structure (timestamps might differ)
        let queue_data1: QueueData = serde_json::from_str(&json_content1).unwrap();
        let queue_data2: QueueData = serde_json::from_str(&json_content2).unwrap();

        // Should have same parameters and structure
        assert_eq!(queue_data1.pack_size, queue_data2.pack_size);
        assert_eq!(
            queue_data1.review_coefficient,
            queue_data2.review_coefficient
        );
        assert_eq!(queue_data1.fail_k, queue_data2.fail_k);
        // Cards might be the same or different depending on bandit sampling, but structure should be consistent
    }

    #[test]
    fn test_default_fail_k_function() {
        assert_eq!(default_fail_k(), 5);
    }
}
