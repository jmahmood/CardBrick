// src/deck/loader.rs
// This file contains the logic for parsing .apkg files.

use std::fs;
use std::io::{Read, Write}; // Removed unused `self` import
use std::path::Path;
use std::collections::HashMap;

// We need to bring our structs into scope from the parent module (deck/mod.rs)
use super::{Card, Deck, Note};

// The main function for this module. It takes a path to an .apkg file and returns
// a Result containing either our Deck or an error.
pub fn load_apkg(path: &Path) -> Result<Deck, Box<dyn std::error::Error>> {
    println!("Attempting to load deck from: {:?}", path);

    // Open the .apkg file. The `?` will propagate any errors.
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // **FIXED (Robustly)**: First, determine the correct filename. This avoids borrowing `archive` multiple times.
    let db_filename = if archive.file_names().any(|name| name == "collection.anki21") {
        "collection.anki21"
    } else {
        "collection.anki2"
    };

    // Now, open the file by its determined name.
    let mut db_file = archive.by_name(db_filename)?;
    
    // Read the database into a byte vector in memory.
    let mut db_data = Vec::new();
    db_file.read_to_end(&mut db_data)?;

    // Create a temporary file to write the database to.
    // This is often easier than working with an in-memory database with rusqlite.
    let mut temp_file = tempfile::NamedTempFile::new()?;
    temp_file.write_all(&db_data)?;
    let temp_path = temp_file.into_temp_path();
    
    // Connect to the temporary database file.
    let conn = rusqlite::Connection::open(&temp_path)?;
    println!("Successfully opened Anki database.");

    // --- Load Notes ---
    let mut stmt = conn.prepare("SELECT id, flds FROM notes")?;
    // The 'flds' field in Anki's database is a single string with fields separated by a special character (0x1f).
    let notes_iter = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let fields_str: String = row.get(1)?;
        let fields: Vec<String> = fields_str.split('\x1f').map(String::from).collect();
        Ok(Note { id, fields })
    })?;

    let mut notes_map = HashMap::new();
    for note_result in notes_iter {
        let note = note_result?;
        notes_map.insert(note.id, note);
    }
    println!("Loaded {} notes.", notes_map.len());

    // --- Load Cards ---
    // Anki stores the ease factor as an integer (e.g., 2500 for 250%), so we read it as u32.
    let mut stmt = conn.prepare("SELECT id, nid, due, ivl, factor, lapses FROM cards")?;
    let cards_iter = stmt.query_map([], |row| {
        Ok(Card {
            id: row.get(0)?,
            note_id: row.get(1)?,
            due: row.get(2)?,
            interval: row.get(3)?,
            ease_factor: row.get(4)?,
            lapses: row.get(5)?,
        })
    })?;

    let mut cards_vec = Vec::new();
    for card_result in cards_iter {
        cards_vec.push(card_result?);
    }
    println!("Loaded {} cards.", cards_vec.len());
    
    // Assemble the final Deck object and return it inside Ok()
    Ok(Deck {
        cards: cards_vec,
        notes: notes_map,
    })
}
