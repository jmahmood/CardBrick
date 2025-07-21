// src/deck/mod.rs
// This module handles loading and managing Anki decks.

// Make the loader module public so other parts of our application can use it.
pub mod loader;
pub mod html_parser;

use std::collections::HashMap;

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
    // Optional database connection for lazy loading
    pub db_connection: Option<LazyDeckConnection>,
}

/// Database connection info for lazy loading cards and notes
#[derive(Debug)]
pub struct LazyDeckConnection {
    pub db_path: tempfile::TempPath,
    pub total_card_count: i64,
}

/// Temporary structure for lazy deck loading
pub struct LazyDeck {
    pub db_path: tempfile::TempPath,
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
        })
    }
}

impl Deck {
    /// Get a note by ID, loading from database if not already cached
    pub fn get_note(&mut self, note_id: i64) -> Result<Option<Note>, Box<dyn std::error::Error>> {
        // Check if note is already cached
        if let Some(note) = self.notes.get(&note_id) {
            return Ok(Some(note.clone()));
        }
        
        // Load from database if we have a connection
        if let Some(ref db_conn) = self.db_connection {
            let conn = rusqlite::Connection::open(&db_conn.db_path)?;
            let mut stmt = conn.prepare("SELECT id, flds FROM notes WHERE id = ?")?;
            
            match stmt.query_row([note_id], |row| {
                let id: i64 = row.get(0)?;
                let fields_str: String = row.get(1)?;
                let fields: Vec<String> = fields_str.split('\x1f').map(String::from).collect();
                Ok(Note { id, fields })
            }) {
                Ok(note_row) => {
                    // Cache the note for future use
                    self.notes.insert(note_id, note_row.clone());
                    Ok(Some(note_row))
                },
                Err(_) => Ok(None), // Note not found
            }
        } else {
            Ok(None)
        }
    }
    
    /// Load more cards from the database (for extending the session)
    pub fn load_more_cards(&mut self, limit: usize) -> Result<Vec<Card>, Box<dyn std::error::Error>> {
        if let Some(ref db_conn) = self.db_connection {
            let conn = rusqlite::Connection::open(&db_conn.db_path)?;
            let offset = self.cards.len();
            let mut stmt = conn.prepare("SELECT id, nid, due, ivl, factor, lapses FROM cards LIMIT ? OFFSET ?")?;
            
            let cards_iter = stmt.query_map([limit as i64, offset as i64], |row| {
                Ok(Card {
                    id: row.get(0)?, note_id: row.get(1)?,
                    due: row.get(2)?, interval: row.get(3)?,
                    ease_factor: row.get(4)?, lapses: row.get(5)?,
                })
            })?;

            let mut new_cards = Vec::new();
            for card_result in cards_iter {
                new_cards.push(card_result?);
            }
            
            // Add to our existing cards
            self.cards.extend(new_cards.clone());
            return Ok(new_cards);
        }
        
        Ok(Vec::new())
    }
}
