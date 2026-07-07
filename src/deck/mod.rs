// src/deck/mod.rs
// This module handles loading and managing Anki decks.

// Make the loader module public so other parts of our application can use it.
pub mod html_parser;
pub mod loader;
pub mod scanner;

use chrono;
use std::collections::{HashMap, HashSet};

/// Represents a single Anki card.
/// We use `#[derive(Debug)]` to allow for easy printing to the console, which is great for debugging.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,          // Card ID
    pub note_id: i64,     // The ID of the note this card belongs to
    pub due: i64,         // Due date in a format Anki uses
    pub interval: u32,    // Interval in days
    pub ease_factor: u32, // The ease factor (stored as an integer in Anki DB)
    pub lapses: u32,      // Number of times the card has been forgotten
}

/// Represents a single Anki note, which contains the actual content (front, back, etc.).
#[derive(Debug, Clone)]
pub struct Note {
    pub id: i64,
    // A vector of strings, where each string is a field (e.g., fields[0] is Front, fields[1] is Back).
    pub fields: Vec<String>,
}

/// Represents the entire deck collection.
#[derive(Debug)]
pub struct Deck {
    // For now, we won't read the deck name, just the cards and notes.
    pub cards: Vec<Card>,
    // We use a HashMap to quickly look up a note by its ID.
    pub notes: HashMap<i64, Note>,
    // Optional cached database connection for lazy loading
    pub cached_db_connection: Option<CachedDeckConnection>,
}

/// Cached deck connection info for lazy loading cards and notes
#[derive(Debug)]
pub struct CachedDeckConnection {
    pub db_path: std::path::PathBuf,
    pub total_card_count: i64,
}

/// Structure for cached deck loading
pub struct CachedDeck {
    pub db_path: std::path::PathBuf,
    pub initial_cards: Vec<Card>,
    pub total_card_count: i64,
}

impl CachedDeck {
    pub fn into_deck(self) -> Result<Deck, Box<dyn std::error::Error>> {
        Ok(Deck {
            cards: self.initial_cards,
            notes: HashMap::new(), // Will be loaded on demand
            cached_db_connection: Some(CachedDeckConnection {
                db_path: self.db_path,
                total_card_count: self.total_card_count,
            }),
        })
    }
}

/// Creates the complete database schema used throughout the application
/// This includes both Anki-compatible tables and CardBrick-specific tables
pub fn ensure_database_schema(
    conn: &rusqlite::Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    // Standard Anki tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY,
            nid INTEGER,
            due INTEGER,
            ivl INTEGER,
            factor INTEGER,
            lapses INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            flds TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS revlog (
            id INTEGER PRIMARY KEY,
            cid INTEGER,
            usn INTEGER,
            ease INTEGER,
            ivl INTEGER,
            lastIvl INTEGER,
            factor INTEGER,
            time INTEGER,
            type INTEGER
        )",
        [],
    )?;

    // CardBrick-specific tables

    conn.execute(
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

    // Additional CardBrick tables from storage/db.rs
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT
        )",
        [],
    )?;

    Ok(())
}

impl Deck {
    /// Get a note by ID, loading from database if not already cached
    pub fn get_note(&mut self, note_id: i64) -> Result<Option<Note>, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();

        // Check if note is already cached
        if let Some(note) = self.notes.get(&note_id) {
            println!(
                "⏱️  Note {} retrieved from cache in {}ms",
                note_id,
                start_time.elapsed().as_millis()
            );
            return Ok(Some(note.clone()));
        }

        // Load from database if we have a cached connection
        let db_path: &std::path::Path = if let Some(ref cached_db_conn) = self.cached_db_connection
        {
            cached_db_conn.db_path.as_ref()
        } else {
            return Ok(None);
        };

        println!(
            "⏱️  Opening DB connection for note {} at {}ms",
            note_id,
            start_time.elapsed().as_millis()
        );
        let conn = rusqlite::Connection::open(db_path)?;
        println!(
            "⏱️  DB opened for note {} at {}ms",
            note_id,
            start_time.elapsed().as_millis()
        );

        let mut stmt = conn.prepare("SELECT id, flds FROM notes WHERE id = ?")?;
        println!(
            "⏱️  Statement prepared for note {} at {}ms",
            note_id,
            start_time.elapsed().as_millis()
        );

        match stmt.query_row([note_id], |row| {
            let id: i64 = row.get(0)?;
            let fields_str: String = row.get(1)?;
            let fields: Vec<String> = fields_str.split('\x1f').map(String::from).collect();
            Ok(Note { id, fields })
        }) {
            Ok(note_row) => {
                println!(
                    "⏱️  Note {} loaded from DB in {}ms",
                    note_id,
                    start_time.elapsed().as_millis()
                );
                // Cache the note for future use
                self.notes.insert(note_id, note_row.clone());
                Ok(Some(note_row))
            }
            Err(_) => {
                println!(
                    "⏱️  Note {} not found after {}ms",
                    note_id,
                    start_time.elapsed().as_millis()
                );
                Ok(None) // Note not found
            }
        }
    }

    // Load more cards from the database (for extending the session)
    // Uses intelligent random selection: prioritizes uncovered cards and cards the user found difficult TODAY

    pub fn load_more_cards(
        &mut self,
        limit: usize,
    ) -> Result<Vec<Card>, Box<dyn std::error::Error>> {
        // Load from database if we have a cached connection
        let db_path: &std::path::Path = if let Some(ref cached_db_conn) = self.cached_db_connection
        {
            cached_db_conn.db_path.as_ref()
        } else {
            return Ok(Vec::new());
        };

        let conn = rusqlite::Connection::open(db_path)?;

        // Ensure the complete database schema exists
        ensure_database_schema(&conn)?;

        let mut new_cards = Vec::new();

        // Get IDs of cards we already have loaded to avoid duplicates
        let existing_ids: HashSet<i64> = self.cards.iter().map(|c| c.id).collect();

        let existing_ids_str = if existing_ids.is_empty() {
            "(-1)".to_string() // Placeholder that won't match any real IDs
        } else {
            format!(
                "({})",
                existing_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };

        // Define today's date for all strategies
        let today = chrono::Utc::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();

        // Strategy 1: Prioritize cards the user found difficult TODAY from database table (25% of requested cards max)
        if new_cards.len() < limit {
            let difficult_cards_needed = (limit - new_cards.len()).min(limit / 4); // Up to 25% from today's difficult cards

            // Query difficult cards from database table, prioritizing failed over hard
            let query = format!(
                "SELECT DISTINCT c.id, c.nid, c.due, c.ivl, c.factor, c.lapses 
                 FROM cards c 
                 INNER JOIN difficult_cards dc ON c.id = dc.card_id 
                 WHERE c.id NOT IN {} AND dc.date = ?
                 ORDER BY 
                     CASE dc.difficulty_type 
                         WHEN 'failed' THEN 1 
                         WHEN 'hard' THEN 2 
                         ELSE 3 
                     END,
                     dc.timestamp DESC,
                     RANDOM()
                 LIMIT ?",
                existing_ids_str
            );

            let mut stmt = conn.prepare(&query)?;
            let cards_iter =
                stmt.query_map([&today, &difficult_cards_needed.to_string()], |row| {
                    Ok(Card {
                        id: row.get(0)?,
                        note_id: row.get(1)?,
                        due: row.get(2)?,
                        interval: row.get(3)?,
                        ease_factor: row.get(4)?,
                        lapses: row.get(5)?,
                    })
                })?;

            for card_result in cards_iter {
                let card = card_result?;
                new_cards.push(card);
            }
        }

        // Strategy 2: Fill remaining with random uncovered cards (haven't been studied yet)
        if new_cards.len() < limit {
            let uncovered_needed = limit - new_cards.len();
            let new_card_ids: HashSet<i64> = new_cards.iter().map(|c| c.id).collect();
            let mut all_existing_ids = existing_ids.clone();
            all_existing_ids.extend(new_card_ids);

            let all_ids_str = if all_existing_ids.is_empty() {
                "(-1)".to_string()
            } else {
                format!(
                    "({})",
                    all_existing_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };

            let query = format!(
                "SELECT c.id, c.nid, c.due, c.ivl, c.factor, c.lapses 
                 FROM cards c 
                 LEFT JOIN revlog r ON c.id = r.cid 
                 LEFT JOIN difficult_cards dc ON c.id = dc.card_id AND dc.date = ?
                 WHERE c.id NOT IN {} AND r.cid IS NULL AND dc.card_id IS NULL 
                 ORDER BY RANDOM() 
                 LIMIT ?",
                all_ids_str
            );

            let mut stmt = conn.prepare(&query)?;
            let cards_iter =
                stmt.query_map([&today, &(uncovered_needed as i64).to_string()], |row| {
                    Ok(Card {
                        id: row.get(0)?,
                        note_id: row.get(1)?,
                        due: row.get(2)?,
                        interval: row.get(3)?,
                        ease_factor: row.get(4)?,
                        lapses: row.get(5)?,
                    })
                })?;

            let _initial_count = new_cards.len();
            for card_result in cards_iter {
                let card = card_result?;
                new_cards.push(card);
            }
        }

        // Strategy 3: If still need more cards, get random cards from anywhere in the database
        if new_cards.len() < limit {
            let random_needed = limit - new_cards.len();
            let new_card_ids: HashSet<i64> = new_cards.iter().map(|c| c.id).collect();
            let mut all_existing_ids = existing_ids.clone();
            all_existing_ids.extend(new_card_ids);

            let all_ids_str = if all_existing_ids.is_empty() {
                "(-1)".to_string()
            } else {
                format!(
                    "({})",
                    all_existing_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };

            let query = format!(
                "SELECT c.id, c.nid, c.due, c.ivl, c.factor, c.lapses 
                 FROM cards c 
                 LEFT JOIN revlog r ON c.id = r.cid 
                 LEFT JOIN difficult_cards dc ON c.id = dc.card_id AND dc.date = ?
                 WHERE c.id NOT IN {} AND r.cid IS NULL AND dc.card_id IS NULL 
                 ORDER BY RANDOM() 
                 LIMIT ?",
                all_ids_str
            );

            let mut stmt = conn.prepare(&query)?;
            let cards_iter =
                stmt.query_map([&today, &(random_needed as i64).to_string()], |row| {
                    Ok(Card {
                        id: row.get(0)?,
                        note_id: row.get(1)?,
                        due: row.get(2)?,
                        interval: row.get(3)?,
                        ease_factor: row.get(4)?,
                        lapses: row.get(5)?,
                    })
                })?;

            let _initial_count = new_cards.len();
            for card_result in cards_iter {
                let card = card_result?;
                new_cards.push(card);
            }
        }

        // Add to our existing cards
        self.cards.extend(new_cards.clone());

        Ok(new_cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_card(id: i64, note_id: i64) -> Card {
        Card {
            id,
            note_id,
            due: 0,
            interval: 0,
            ease_factor: 2500,
            lapses: 0,
        }
    }

    fn create_test_note(id: i64, fields: Vec<&str>) -> Note {
        Note {
            id,
            fields: fields.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_card_creation() {
        let card = create_test_card(1, 100);
        assert_eq!(card.id, 1);
        assert_eq!(card.note_id, 100);
        assert_eq!(card.due, 0);
        assert_eq!(card.interval, 0);
        assert_eq!(card.ease_factor, 2500);
        assert_eq!(card.lapses, 0);
    }

    #[test]
    fn test_note_creation() {
        let note = create_test_note(100, vec!["Front text", "Back text"]);
        assert_eq!(note.id, 100);
        assert_eq!(note.fields.len(), 2);
        assert_eq!(note.fields[0], "Front text");
        assert_eq!(note.fields[1], "Back text");
    }

    #[test]
    fn test_deck_creation() {
        let cards = vec![create_test_card(1, 100), create_test_card(2, 101)];

        let mut notes = HashMap::new();
        notes.insert(100, create_test_note(100, vec!["Front 1", "Back 1"]));
        notes.insert(101, create_test_note(101, vec!["Front 2", "Back 2"]));

        let deck = Deck {
            cards,
            notes,
            cached_db_connection: None,
        };

        assert_eq!(deck.cards.len(), 2);
        assert_eq!(deck.notes.len(), 2);
        assert!(deck.cached_db_connection.is_none());
    }

    #[test]
    fn test_cached_deck_into_deck() {
        let cached_deck = CachedDeck {
            db_path: PathBuf::from("/test/path"),
            initial_cards: vec![create_test_card(1, 100)],
            total_card_count: 10,
        };

        let deck = cached_deck.into_deck().unwrap();
        assert_eq!(deck.cards.len(), 1);
        assert_eq!(deck.cards[0].id, 1);
        assert!(deck.cached_db_connection.is_some());

        let cached_conn = deck.cached_db_connection.unwrap();
        assert_eq!(cached_conn.db_path, PathBuf::from("/test/path"));
        assert_eq!(cached_conn.total_card_count, 10);
    }

    #[test]
    fn test_deck_get_note_from_cache() {
        let mut notes = HashMap::new();
        notes.insert(
            100,
            create_test_note(100, vec!["Cached front", "Cached back"]),
        );

        let mut deck = Deck {
            cards: vec![create_test_card(1, 100)],
            notes,
            cached_db_connection: None,
        };

        // Should retrieve from cache (no database connection needed)
        let result = deck.get_note(100).unwrap();
        assert!(result.is_some());

        let note = result.unwrap();
        assert_eq!(note.id, 100);
        assert_eq!(note.fields[0], "Cached front");
        assert_eq!(note.fields[1], "Cached back");
    }

    #[test]
    fn test_deck_get_note_no_connection() {
        let mut deck = Deck {
            cards: vec![create_test_card(1, 100)],
            notes: HashMap::new(),
            cached_db_connection: None,
        };

        // Should return None when no database connection and not in cache
        let result = deck.get_note(100).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_deck_load_more_cards_no_connection() {
        let mut deck = Deck {
            cards: vec![create_test_card(1, 100)],
            notes: HashMap::new(),
            cached_db_connection: None,
        };

        // Should return empty Vec when no database connection
        let result = deck.load_more_cards(10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_card_clone() {
        let original = create_test_card(1, 100);
        let cloned = original.clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.note_id, cloned.note_id);
        assert_eq!(original.due, cloned.due);
        assert_eq!(original.interval, cloned.interval);
        assert_eq!(original.ease_factor, cloned.ease_factor);
        assert_eq!(original.lapses, cloned.lapses);
    }

    #[test]
    fn test_note_clone() {
        let original = create_test_note(100, vec!["Front", "Back"]);
        let cloned = original.clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.fields, cloned.fields);
        // Ensure they are separate instances
        assert_ne!(original.fields.as_ptr(), cloned.fields.as_ptr());
    }

    #[test]
    fn test_card_field_modifications() {
        let mut card = create_test_card(1, 100);

        // Test modifying card fields (simulating SM-2 algorithm)
        card.interval = 5;
        card.ease_factor = 2300;
        card.lapses = 1;
        card.due = 12345;

        assert_eq!(card.interval, 5);
        assert_eq!(card.ease_factor, 2300);
        assert_eq!(card.lapses, 1);
        assert_eq!(card.due, 12345);
    }

    #[test]
    fn test_note_empty_fields() {
        let note = create_test_note(100, vec![]);
        assert_eq!(note.fields.len(), 0);
        assert!(note.fields.is_empty());
    }

    #[test]
    fn test_note_single_field() {
        let note = create_test_note(100, vec!["Single field"]);
        assert_eq!(note.fields.len(), 1);
        assert_eq!(note.fields[0], "Single field");
    }

    #[test]
    fn test_note_multiple_fields() {
        let note = create_test_note(100, vec!["Field 1", "Field 2", "Field 3", "Field 4"]);
        assert_eq!(note.fields.len(), 4);
        assert_eq!(note.fields[0], "Field 1");
        assert_eq!(note.fields[1], "Field 2");
        assert_eq!(note.fields[2], "Field 3");
        assert_eq!(note.fields[3], "Field 4");
    }

    // Helper function to create an in-memory test database with cards
    fn create_test_database_with_cards(card_count: i64) -> rusqlite::Connection {
        let conn =
            rusqlite::Connection::open_in_memory().expect("Failed to create in-memory database");

        // Create cards table
        conn.execute(
            "CREATE TABLE cards (
                id INTEGER PRIMARY KEY,
                nid INTEGER,
                due INTEGER,
                ivl INTEGER,
                factor INTEGER,
                lapses INTEGER
            )",
            [],
        )
        .expect("Failed to create cards table");

        // Create revlog table (for tracking reviews)
        conn.execute(
            "CREATE TABLE revlog (
                id INTEGER PRIMARY KEY,
                cid INTEGER,
                usn INTEGER,
                ease INTEGER,
                ivl INTEGER,
                lastIvl INTEGER,
                factor INTEGER,
                time INTEGER,
                type INTEGER
            )",
            [],
        )
        .expect("Failed to create revlog table");

        // Insert test cards
        for i in 1..=card_count {
            conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10], // card id, note id
            ).expect("Failed to insert test card");
        }

        conn
    }

    #[test]
    fn test_load_more_cards_with_failed_today() {
        // Create an in-memory deck with database connection
        // let db = create_test_database_with_cards(20);
        // let db_path = std::path::PathBuf::from(":memory:");

        // We need to save the database to a temp file since we need a file path
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        // Use centralized database schema creation
        ensure_database_schema(&file_conn).unwrap();

        // Insert test cards
        for i in 1..=20 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn); // Close connection to file

        let mut deck = Deck {
            cards: vec![], // Start with no cards loaded
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path.clone(),
                total_card_count: 20,
            }),
        };

        // Test loading cards with some marked as failed today
        let failed_today = vec![1, 3, 5]; // Cards 1, 3, 5 failed today
        let hard_today: Vec<i64> = vec![];

        // Populate the difficult_cards table instead of relying on parameters
        let conn = rusqlite::Connection::open(&temp_db_path).unwrap();
        let today = chrono::Utc::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add failed cards to database
        for card_id in &failed_today {
            conn.execute(
                "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, "failed", now, &today),
            )
            .unwrap();
        }
        drop(conn);

        let result = deck.load_more_cards(PACK_SIZE_DEFAULT).unwrap();

        // Should load requested number of cards
        assert_eq!(result.len(), PACK_SIZE_DEFAULT);

        // At least some of the failed cards should be included (up to 25% of PACK_SIZE_DEFAULT = 3)
        let failed_in_result = result
            .iter()
            .filter(|card| failed_today.contains(&card.id))
            .count();
        assert!(failed_in_result > 0, "No failed cards were prioritized");
        assert!(
            failed_in_result <= PACK_SIZE_DEFAULT / 4,
            "Too many failed cards (should be max 25%)"
        );

        // All returned cards should have valid data
        for card in result {
            assert!(card.id > 0);
            assert!(card.note_id > 0);
            assert_eq!(card.ease_factor, 2500);
            assert_eq!(card.lapses, 0);
        }
    }

    #[test]
    fn test_load_more_cards_with_hard_today() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        // Create test database
        file_conn.execute(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, due INTEGER, ivl INTEGER, factor INTEGER, lapses INTEGER)",
            [],
        ).unwrap();

        file_conn.execute(
            "CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER, lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER)",
            [],
        ).unwrap();

        // Create difficult_cards table
        file_conn
            .execute(
                "CREATE TABLE IF NOT EXISTS difficult_cards (
                card_id INTEGER,
                difficulty_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                date TEXT NOT NULL,
                PRIMARY KEY (card_id, difficulty_type, date)
            )",
                [],
            )
            .unwrap();

        for i in 1..=15 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path.clone(),
                total_card_count: 15,
            }),
        };

        // Test with hard cards
        let failed_today: Vec<i64> = vec![];
        let hard_today = vec![2, 4, 6, 8]; // Cards 2, 4, 6, 8 were hard

        // Populate the difficult_cards table instead of relying on parameters
        let conn = rusqlite::Connection::open(&temp_db_path).unwrap();
        let today = chrono::Utc::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add hard cards to database
        for card_id in &hard_today {
            conn.execute(
                "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, "hard", now, &today),
            )
            .unwrap();
        }
        drop(conn);

        let result = deck.load_more_cards(8).unwrap();
        assert_eq!(result.len(), 8);

        // Should prioritize some hard cards (up to 25% of 8 = 2)
        let hard_in_result = result
            .iter()
            .filter(|card| hard_today.contains(&card.id))
            .count();
        assert!(hard_in_result > 0, "No hard cards were prioritized");
        assert!(
            hard_in_result <= 2,
            "Too many hard cards (should be max 25%)"
        );
    }

    #[test]
    fn test_load_more_cards_fallback_strategies() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        file_conn.execute(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, due INTEGER, ivl INTEGER, factor INTEGER, lapses INTEGER)",
            [],
        ).unwrap();

        file_conn.execute(
            "CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER, lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER)",
            [],
        ).unwrap();

        for i in 1..=10 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path,
                total_card_count: 10,
            }),
        };

        // Test with no difficult cards - should fall back to uncovered cards
        let failed_today: Vec<i64> = vec![];
        let hard_today: Vec<i64> = vec![];

        let result = deck.load_more_cards(5).unwrap();
        assert_eq!(result.len(), 5);

        // All cards should be returned since none are difficult and none are covered (no revlog entries)
        for card in result {
            assert!(card.id >= 1 && card.id <= 10);
        }
    }

    #[test]
    fn test_load_more_cards_no_difficult_cards() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        file_conn.execute(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, due INTEGER, ivl INTEGER, factor INTEGER, lapses INTEGER)",
            [],
        ).unwrap();

        file_conn.execute(
            "CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER, lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER)",
            [],
        ).unwrap();

        for i in 1..=10 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path,
                total_card_count: 10,
            }),
        };

        // Test with empty failed/hard arrays
        let failed_today: Vec<i64> = vec![];
        let hard_today: Vec<i64> = vec![];

        let result = deck.load_more_cards(6).unwrap();
        assert_eq!(result.len(), 6);

        // Should use Strategy 2 (uncovered cards) since no difficult cards
        for card in result {
            assert!(card.id >= 1 && card.id <= 10);
        }
    }

    #[test]
    fn test_load_more_cards_more_difficult_than_allocation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        file_conn.execute(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, due INTEGER, ivl INTEGER, factor INTEGER, lapses INTEGER)",
            [],
        ).unwrap();

        file_conn.execute(
            "CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER, lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER)",
            [],
        ).unwrap();

        // Create difficult_cards table
        file_conn
            .execute(
                "CREATE TABLE IF NOT EXISTS difficult_cards (
                card_id INTEGER,
                difficulty_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                date TEXT NOT NULL,
                PRIMARY KEY (card_id, difficulty_type, date)
            )",
                [],
            )
            .unwrap();

        for i in 1..=20 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path.clone(),
                total_card_count: 20,
            }),
        };

        // More difficult cards than can fit in 25% allocation
        let failed_today = vec![1, 2, 3, 4, 5, 6, 7, 8]; // 8 failed cards
        let hard_today = vec![9, 10, 11, 12]; // 4 hard cards

        // Populate the difficult_cards table instead of relying on parameters
        let conn = rusqlite::Connection::open(&temp_db_path).unwrap();
        let today = chrono::Utc::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add failed cards to database
        for card_id in &failed_today {
            conn.execute(
                "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, "failed", now, &today),
            )
            .unwrap();
        }

        // Add hard cards to database
        for card_id in &hard_today {
            conn.execute(
                "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
                 VALUES (?1, ?2, ?3, ?4)",
                (card_id, "hard", now, &today),
            )
            .unwrap();
        }
        drop(conn);

        let result = deck.load_more_cards(12).unwrap(); // 25% of 12 = 3

        // With 20 total cards and 12 difficult cards, we can only get at most 11 cards
        // (3 difficult + 8 non-difficult = 11, since we have only 8 non-difficult cards available)
        assert!(result.len() >= 9, "Should load at least some cards");
        assert!(result.len() <= 12, "Should not exceed requested amount");

        // Should include at most 3 difficult cards total (25% allocation respected)
        let failed_in_result = result
            .iter()
            .filter(|card| failed_today.contains(&card.id))
            .count();
        let hard_in_result = result
            .iter()
            .filter(|card| hard_today.contains(&card.id))
            .count();
        let total_difficult = failed_in_result + hard_in_result;

        assert!(
            total_difficult <= 3,
            "Should respect 25% allocation limit even with many difficult cards"
        );
        assert!(
            total_difficult > 0,
            "Should still include some difficult cards"
        );
    }

    #[test]
    fn test_load_more_cards_all_cards_already_loaded() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        file_conn.execute(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, due INTEGER, ivl INTEGER, factor INTEGER, lapses INTEGER)",
            [],
        ).unwrap();

        file_conn.execute(
            "CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER, lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER)",
            [],
        ).unwrap();

        // Only create 5 cards in database
        for i in 1..=5 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            // All cards are already loaded
            cards: (1..=5).map(|i| create_test_card(i, i * 10)).collect(),
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path,
                total_card_count: 5,
            }),
        };

        let failed_today = vec![1, 2]; // Some cards are difficult
        let hard_today = vec![3];

        let result = deck.load_more_cards(10).unwrap();

        // Should return 0 cards since all are already in existing_ids and excluded
        assert_eq!(
            result.len(),
            0,
            "Should return 0 cards when all database cards are already loaded"
        );
    }

    #[test]
    fn test_load_more_cards_database_query_error_handling() {
        // Test with invalid database path
        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: std::path::PathBuf::from("/nonexistent/path/to/database.db"),
                total_card_count: 100,
            }),
        };

        let failed_today = vec![1, 2];
        let hard_today = vec![3, 4];

        // Should return an error for invalid database path
        let result = deck.load_more_cards(12);
        assert!(
            result.is_err(),
            "Should return error for invalid database path"
        );
    }

    #[test]
    fn test_load_more_cards_with_reviewed_cards() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();

        // Use centralized database schema creation
        ensure_database_schema(&file_conn).unwrap();

        // Create 10 cards
        for i in 1..=10 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }

        // Mark cards 1-5 as reviewed (add entries to revlog)
        for i in 1..=5 {
            file_conn.execute(
                "INSERT INTO revlog (cid, usn, ease, ivl, lastIvl, factor, time, type) VALUES (?, 1, 2, 1, 0, 2500, 1234567890, 1)",
                [i],
            ).unwrap();
        }

        drop(file_conn);

        let mut deck = Deck {
            cards: vec![],
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path,
                total_card_count: 10,
            }),
        };

        let failed_today: Vec<i64> = vec![];
        let hard_today: Vec<i64> = vec![];

        let result = deck.load_more_cards(8).unwrap();

        // Should return only uncovered cards (6-10), so max 5 cards
        assert!(result.len() <= 5, "Should only return uncovered cards");

        // All returned cards should be from the uncovered set (6-10)
        for card in result {
            assert!(
                card.id >= 6 && card.id <= 10,
                "Should only return uncovered cards (6-10)"
            );
        }
    }
}
