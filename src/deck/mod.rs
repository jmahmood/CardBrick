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
}
