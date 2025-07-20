// src/deck/loader.rs
// This file contains the logic for parsing .apkg files.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::Sender;

// We need to bring our structs into scope from the parent module (deck/mod.rs)
use super::{Card, Deck, LazyDeck};
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
        tx.send(LoaderMessage::Progress(0.15)).unwrap(); // 15% - DB file located

        // Create our own temp directory to avoid system /tmp limitations on handheld devices
        let temp_dir = if std::path::Path::new("/storage").exists() {
            // RG35XX Plus - use storage partition with plenty of space
            let temp_path = std::path::PathBuf::from("/storage/tmp");
            if !temp_path.exists() {
                std::fs::create_dir_all(&temp_path)?;
                println!("Created custom temp directory: /storage/tmp");
            }
            temp_path
        } else if std::path::Path::new("/mnt/SDCARD").exists() {
            // TrimUI Brick - use SD card with plenty of space
            let temp_path = std::path::PathBuf::from("/mnt/SDCARD/tmp");
            if !temp_path.exists() {
                std::fs::create_dir_all(&temp_path)?;
                println!("Created custom temp directory: /mnt/SDCARD/tmp");
            }
            temp_path
        } else {
            // Desktop/development - use system temp
            std::env::temp_dir()
        };
        
        println!("Using temp directory: {:?}", temp_dir);
        
        // Stream database to a small temp file with progress reporting
        let mut temp_file = tempfile::NamedTempFile::new_in(&temp_dir)?;
        let mut total_bytes = 0;
        let mut buffer = vec![0u8; 8192]; // 8KB buffer for progress reporting
        
        loop {
            let bytes_read = db_file.read(&mut buffer)?;
            if bytes_read == 0 { break; }
            
            temp_file.write_all(&buffer[..bytes_read])?;
            total_bytes += bytes_read;
            
            // Report progress every 1MB
            if total_bytes % (1024 * 1024) == 0 {
                let progress = 0.15 + (total_bytes as f32 / 50_000_000.0 * 0.15).min(0.15); // Up to 30%
                tx.send(LoaderMessage::Progress(progress)).ok();
            }
        }
        
        let temp_path = temp_file.into_temp_path();
        println!("Extracted {} bytes to temporary database", total_bytes);
        tx.send(LoaderMessage::Progress(0.35)).unwrap(); // 35% - DB extracted
        
        let conn = rusqlite::Connection::open(&temp_path)?;
        println!("Successfully opened Anki database");
        tx.send(LoaderMessage::Progress(0.40)).unwrap(); // 40% - DB opened
        
        // Get current timestamp for scheduling (Anki uses days since epoch)
        let today = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / 86400; // Convert to days
        
        println!("Today (Anki format): {}", today);
        
        // Get total cards for progress tracking
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM cards")?;
        let total_cards: i64 = stmt.query_row([], |row| row.get(0))?;
        println!("Found {} total cards in deck", total_cards);
        tx.send(LoaderMessage::Progress(0.60)).unwrap(); // 60% - Basic info loaded
        
        // Step 1: Get cards that are due today or overdue
        let mut stmt = conn.prepare(
            "SELECT id, nid, due, ivl, factor, lapses FROM cards 
             WHERE due <= ? OR ivl = 0 
             ORDER BY due ASC, ivl ASC 
             LIMIT 100")?;
             
        let cards_iter = stmt.query_map([today as i64], |row| {
            Ok(Card {
                id: row.get(0)?, note_id: row.get(1)?,
                due: row.get(2)?, interval: row.get(3)?,
                ease_factor: row.get(4)?, lapses: row.get(5)?,
            })
        })?;

        let mut scheduled_cards = Vec::new();
        for card_result in cards_iter {
            scheduled_cards.push(card_result?);
        }
        println!("Loaded {} scheduled cards (due today or overdue)", scheduled_cards.len());
        tx.send(LoaderMessage::Progress(0.70)).unwrap(); // 70% - Due cards loaded
        
        // Step 2: If we have fewer than 10 cards, add more to reach the minimum
        let min_cards = 10;
        if scheduled_cards.len() < min_cards {
            let needed_cards = min_cards - scheduled_cards.len();
            println!("Need {} more cards to reach minimum of {}", needed_cards, min_cards);
            
            // Get additional cards that aren't already selected
            // For now, this is random selection, but we can improve this later with performance-based algorithms
            let (query, params): (String, Vec<rusqlite::types::Value>) = if scheduled_cards.is_empty() {
                // No cards selected yet, just get random cards
                (
                    "SELECT id, nid, due, ivl, factor, lapses FROM cards 
                     ORDER BY RANDOM() 
                     LIMIT ?".to_string(),
                    vec![(needed_cards as i64).into()]
                )
            } else {
                // Exclude already selected cards
                let selected_ids: Vec<i64> = scheduled_cards.iter().map(|c| c.id).collect();
                let placeholders = vec!["?"; selected_ids.len()].join(",");
                let mut params: Vec<rusqlite::types::Value> = selected_ids.iter().map(|&id| id.into()).collect();
                params.push((needed_cards as i64).into());
                
                (
                    format!(
                        "SELECT id, nid, due, ivl, factor, lapses FROM cards 
                         WHERE id NOT IN ({}) 
                         ORDER BY RANDOM() 
                         LIMIT ?", 
                        placeholders
                    ),
                    params
                )
            };
            
            let mut stmt = conn.prepare(&query)?;
            let additional_cards_iter = stmt.query_map(rusqlite::params_from_iter(params), |row| {
                Ok(Card {
                    id: row.get(0)?, note_id: row.get(1)?,
                    due: row.get(2)?, interval: row.get(3)?,
                    ease_factor: row.get(4)?, lapses: row.get(5)?,
                })
            })?;

            let mut additional_cards = Vec::new();
            for card_result in additional_cards_iter {
                additional_cards.push(card_result?);
            }
            
            let additional_count = additional_cards.len();
            scheduled_cards.extend(additional_cards);
            println!("Added {} additional cards (random selection) for total of {}", 
                     additional_count, scheduled_cards.len());
        }
        
        tx.send(LoaderMessage::Progress(0.80)).unwrap(); // 80% - All cards loaded
        
        // Create a deck with database connection for on-demand note loading
        let lazy_deck = LazyDeck {
            db_path: temp_path,
            initial_cards: scheduled_cards,
            total_card_count: total_cards,
        };
        
        tx.send(LoaderMessage::Progress(1.0)).unwrap(); // 100% - Deck ready

        Ok(lazy_deck.into_deck()?)
    })(); // Immediately-invoked function expression to handle errors cleanly

    // Send the final result (either the Deck or an Error) through the channel.
    tx.send(LoaderMessage::Complete(result.map_err(|e| e.to_string()))).unwrap();
}
