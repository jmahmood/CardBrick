// src/deck/loader.rs
// This file contains the logic for parsing .apkg files.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::collections::HashMap;
use std::sync::mpsc::Sender;

// We need to bring our structs into scope from the parent module (deck/mod.rs)
use super::{Card, Deck, Note};
use crate::LoaderMessage; // Import the message enum from main.rs

/// The main function for this module. It takes a path to an .apkg file and a
/// channel sender to report progress.
pub fn load_apkg(path: &Path, tx: Sender<LoaderMessage>) {
    // This function now sends its result through the channel instead of returning it.
    let result = (|| -> Result<Deck, Box<dyn std::error::Error>> {
        println!("Attempting to load deck from: {:?}", path);

        let file = fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        let db_filename = if archive.file_names().any(|name| name == "collection.anki21") {
            "collection.anki21"
        } else {
            "collection.anki2"
        };

        let mut db_file = archive.by_name(db_filename)?;
        let mut db_data = Vec::new();
        db_file.read_to_end(&mut db_data)?;
        tx.send(LoaderMessage::Progress(0.25)).unwrap(); // 25% - DB extracted

        let mut temp_file = tempfile::NamedTempFile::new()?;
        temp_file.write_all(&db_data)?;
        let temp_path = temp_file.into_temp_path();
        
        let conn = rusqlite::Connection::open(&temp_path)?;
        println!("Successfully opened Anki database.");
        
        // --- Load Notes ---
        let mut stmt = conn.prepare("SELECT id, flds FROM notes")?;
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
        tx.send(LoaderMessage::Progress(0.75)).unwrap(); // 75% - Notes loaded
        
        // --- Load Cards ---
        let mut stmt = conn.prepare("SELECT id, nid, due, ivl, factor, lapses FROM cards")?;
        let cards_iter = stmt.query_map([], |row| {
            Ok(Card {
                id: row.get(0)?, note_id: row.get(1)?,
                due: row.get(2)?, interval: row.get(3)?,
                ease_factor: row.get(4)?, lapses: row.get(5)?,
            })
        })?;

        let mut cards_vec = Vec::new();
        for card_result in cards_iter {
            cards_vec.push(card_result?);
        }
        println!("Loaded {} cards.", cards_vec.len());
        tx.send(LoaderMessage::Progress(1.0)).unwrap(); // 100% - Cards loaded

        Ok(Deck { cards: cards_vec, notes: notes_map })
    })(); // Immediately-invoked function expression to handle errors cleanly

    // Send the final result (either the Deck or an Error) through the channel.
    tx.send(LoaderMessage::Complete(result.map_err(|e| e.to_string()))).unwrap();
}
