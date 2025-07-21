// src/deck/mod.rs
// This module handles loading and managing Anki decks.

// Make the loader module public so other parts of our application can use it.
pub mod loader;
pub mod html_parser;
pub mod scanner;

use std::collections::{HashMap, HashSet};

/// Represents a single Anki card.
/// We use `#[derive(Debug)]` to allow for easy printing to the console, which is great for debugging.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,         // Card ID
    pub note_id: i64,    // The ID of the note this card belongs to
    pub due: i64,        // Due date in a format Anki uses
    pub interval: u32,   // Interval in days
    pub ease_factor: u32, // The ease factor (stored as an integer in Anki DB)
    pub lapses: u32,     // Number of times the card has been forgotten
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
    // Optional database connection for lazy loading (legacy)
    pub db_connection: Option<LazyDeckConnection>,
    // Optional cached database connection for lazy loading
    pub cached_db_connection: Option<CachedDeckConnection>,
}

/// Database connection info for lazy loading cards and notes
#[derive(Debug)]
pub struct LazyDeckConnection {
    pub db_path: tempfile::TempPath,
    pub total_card_count: i64,
}

/// Cached deck connection info for lazy loading cards and notes
#[derive(Debug)]
pub struct CachedDeckConnection {
    pub db_path: std::path::PathBuf,
    pub total_card_count: i64,
}

/// Temporary structure for lazy deck loading (legacy)
pub struct LazyDeck {
    pub db_path: tempfile::TempPath,
    pub initial_cards: Vec<Card>,
    pub total_card_count: i64,
}

/// Structure for cached deck loading
pub struct CachedDeck {
    pub db_path: std::path::PathBuf,
    pub initial_cards: Vec<Card>,
    pub total_card_count: i64,
}

impl LazyDeck {
    pub fn into_deck(self) -> Result<Deck, Box<dyn std::error::Error>> {
        Ok(Deck {
            cards: self.initial_cards,
            notes: HashMap::new(), // Will be loaded on demand
            db_connection: Some(LazyDeckConnection {
                db_path: self.db_path,
                total_card_count: self.total_card_count,
            }),
            cached_db_connection: None,
        })
    }
}

impl CachedDeck {
    pub fn into_deck(self) -> Result<Deck, Box<dyn std::error::Error>> {
        Ok(Deck {
            cards: self.initial_cards,
            notes: HashMap::new(), // Will be loaded on demand
            db_connection: None,
            cached_db_connection: Some(CachedDeckConnection {
                db_path: self.db_path,
                total_card_count: self.total_card_count,
            }),
        })
    }
}

impl Deck {
    /// Get a note by ID, loading from database if not already cached
    pub fn get_note(&mut self, note_id: i64) -> Result<Option<Note>, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        
        // Check if note is already cached
        if let Some(note) = self.notes.get(&note_id) {
            println!("⏱️  Note {} retrieved from cache in {}ms", note_id, start_time.elapsed().as_millis());
            return Ok(Some(note.clone()));
        }
        
        // Load from database if we have a connection (legacy or cached)
        let db_path: &std::path::Path = if let Some(ref db_conn) = self.db_connection {
            db_conn.db_path.as_ref()
        } else if let Some(ref cached_db_conn) = self.cached_db_connection {
            cached_db_conn.db_path.as_ref()
        } else {
            return Ok(None);
        };
        
        println!("⏱️  Opening DB connection for note {} at {}ms", note_id, start_time.elapsed().as_millis());
        let conn = rusqlite::Connection::open(db_path)?;
        println!("⏱️  DB opened for note {} at {}ms", note_id, start_time.elapsed().as_millis());
        
        let mut stmt = conn.prepare("SELECT id, flds FROM notes WHERE id = ?")?;
        println!("⏱️  Statement prepared for note {} at {}ms", note_id, start_time.elapsed().as_millis());
        
        match stmt.query_row([note_id], |row| {
            let id: i64 = row.get(0)?;
            let fields_str: String = row.get(1)?;
            let fields: Vec<String> = fields_str.split('\x1f').map(String::from).collect();
            Ok(Note { id, fields })
        }) {
            Ok(note_row) => {
                println!("⏱️  Note {} loaded from DB in {}ms", note_id, start_time.elapsed().as_millis());
                // Cache the note for future use
                self.notes.insert(note_id, note_row.clone());
                Ok(Some(note_row))
            },
            Err(_) => {
                println!("⏱️  Note {} not found after {}ms", note_id, start_time.elapsed().as_millis());
                Ok(None) // Note not found
            }
        }
    }
    
    /// Load more cards from the database (for extending the session)
    /// Uses intelligent random selection: prioritizes uncovered cards and cards the user found difficult TODAY
    pub fn load_more_cards(&mut self, limit: usize, failed_today: &[i64], hard_today: &[i64]) -> Result<Vec<Card>, Box<dyn std::error::Error>> {
        // Load from database if we have a connection (legacy or cached)
        let db_path: &std::path::Path = if let Some(ref db_conn) = self.db_connection {
            db_conn.db_path.as_ref()
        } else if let Some(ref cached_db_conn) = self.cached_db_connection {
            cached_db_conn.db_path.as_ref()
        } else {
            return Ok(Vec::new());
        };
        
        let conn = rusqlite::Connection::open(db_path)?;
        let mut new_cards = Vec::new();
        
        // Get IDs of cards we already have loaded to avoid duplicates
        let existing_ids: HashSet<i64> = self.cards.iter().map(|c| c.id).collect();
        
        let existing_ids_str = if existing_ids.is_empty() {
            "(-1)".to_string() // Placeholder that won't match any real IDs
        } else {
            format!("({})", existing_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","))
        };
        
        // Strategy 1: Prioritize cards the user found difficult TODAY (failed or hard) (25% of requested cards max)
        if new_cards.len() < limit && (!failed_today.is_empty() || !hard_today.is_empty()) {
            let difficult_cards_needed = (limit - new_cards.len()).min(limit / 4); // Up to 25% from today's difficult cards
            
            // Combine failed and hard cards, with preference for failed cards
            let mut difficult_card_ids = failed_today.to_vec();
            difficult_card_ids.extend_from_slice(hard_today);
            
            // Remove duplicates while preserving order (failed cards first)
            difficult_card_ids.sort();
            difficult_card_ids.dedup();
            
            let difficult_ids_str = if difficult_card_ids.is_empty() {
                "(-1)".to_string() // Placeholder that won't match any real IDs
            } else {
                format!("({})", difficult_card_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","))
            };
            
            let query = format!(
                "SELECT id, nid, due, ivl, factor, lapses 
                 FROM cards 
                 WHERE id NOT IN {} AND id IN {}
                 ORDER BY RANDOM() 
                 LIMIT ?", 
                existing_ids_str, difficult_ids_str
            );
            
            let mut stmt = conn.prepare(&query)?;
            let cards_iter = stmt.query_map([difficult_cards_needed as i64], |row| {
                Ok(Card {
                    id: row.get(0)?, note_id: row.get(1)?,
                    due: row.get(2)?, interval: row.get(3)?,
                    ease_factor: row.get(4)?, lapses: row.get(5)?,
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
                format!("({})", all_existing_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","))
            };
            
            let query = format!(
                "SELECT c.id, c.nid, c.due, c.ivl, c.factor, c.lapses 
                 FROM cards c 
                 LEFT JOIN revlog r ON c.id = r.cid 
                 WHERE c.id NOT IN {} AND r.cid IS NULL 
                 ORDER BY RANDOM() 
                 LIMIT ?", 
                all_ids_str
            );
            
            let mut stmt = conn.prepare(&query)?;
            let cards_iter = stmt.query_map([uncovered_needed as i64], |row| {
                Ok(Card {
                    id: row.get(0)?, note_id: row.get(1)?,
                    due: row.get(2)?, interval: row.get(3)?,
                    ease_factor: row.get(4)?, lapses: row.get(5)?,
                })
            })?;

            let initial_count = new_cards.len();
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
                format!("({})", all_existing_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","))
            };
            
            let query = format!(
                "SELECT id, nid, due, ivl, factor, lapses 
                 FROM cards 
                 WHERE id NOT IN {} 
                 ORDER BY RANDOM() 
                 LIMIT ?", 
                all_ids_str
            );
            
            let mut stmt = conn.prepare(&query)?;
            let cards_iter = stmt.query_map([random_needed as i64], |row| {
                Ok(Card {
                    id: row.get(0)?, note_id: row.get(1)?,
                    due: row.get(2)?, interval: row.get(3)?,
                    ease_factor: row.get(4)?, lapses: row.get(5)?,
                })
            })?;

            let initial_count = new_cards.len();
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
        let cards = vec![
            create_test_card(1, 100),
            create_test_card(2, 101),
        ];
        
        let mut notes = HashMap::new();
        notes.insert(100, create_test_note(100, vec!["Front 1", "Back 1"]));
        notes.insert(101, create_test_note(101, vec!["Front 2", "Back 2"]));
        
        let deck = Deck {
            cards,
            notes,
            db_connection: None,
            cached_db_connection: None,
        };
        
        assert_eq!(deck.cards.len(), 2);
        assert_eq!(deck.notes.len(), 2);
        assert!(deck.db_connection.is_none());
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
        assert!(deck.db_connection.is_none());
        assert!(deck.cached_db_connection.is_some());
        
        let cached_conn = deck.cached_db_connection.unwrap();
        assert_eq!(cached_conn.db_path, PathBuf::from("/test/path"));
        assert_eq!(cached_conn.total_card_count, 10);
    }

    #[test]
    fn test_deck_get_note_from_cache() {
        let mut notes = HashMap::new();
        notes.insert(100, create_test_note(100, vec!["Cached front", "Cached back"]));
        
        let mut deck = Deck {
            cards: vec![create_test_card(1, 100)],
            notes,
            db_connection: None,
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
            db_connection: None,
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
            db_connection: None,
            cached_db_connection: None,
        };
        
        // Should return empty Vec when no database connection
        let result = deck.load_more_cards(10, &[], &[]).unwrap();
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
}
