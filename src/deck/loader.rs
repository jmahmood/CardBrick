// src/deck/loader.rs
// This file contains the logic for loading cached deck databases.

use std::path::Path;
use std::sync::mpsc::Sender;

// We need to bring our structs into scope from the parent module (deck/mod.rs)
use super::{Card, Deck, CachedDeck, scanner};
use crate::state::LoaderMessage; // Import the message enum from state.rs


/// The main function for this module. It takes a path to a cached deck directory and a
/// channel sender to report progress.
pub fn load_apkg(path: &Path, tx: Sender<LoaderMessage>) {
    // This function now sends its result through the channel instead of returning it.
    let result = (|| -> Result<Deck, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        println!("Attempting to load cached deck from: {:?}", path);
        
        // Extract deck hash from path
        let deck_hash = path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid deck path")?;
        
        tx.send(LoaderMessage::Progress(0.10)).unwrap(); // 10% - Starting
        println!("⏱️  [{}ms] Extracted deck hash: {}", start_time.elapsed().as_millis(), deck_hash);
        
        // Ensure cached deck exists and get database path
        let db_path = scanner::ensure_cached_deck(deck_hash)?;
        println!("⏱️  [{}ms] Found cached database at: {:?}", start_time.elapsed().as_millis(), db_path);
        tx.send(LoaderMessage::Progress(0.20)).unwrap(); // 20% - Cache validated
        
        // Open database with read-only flags for performance
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        println!("⏱️  [{}ms] Database opened", start_time.elapsed().as_millis());
        
        // Apply performance PRAGMAs
        conn.pragma_update(None, "journal_mode", "OFF")?;
        conn.pragma_update(None, "synchronous", "OFF")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        println!("⏱️  [{}ms] PRAGMAs applied", start_time.elapsed().as_millis());
        
        println!("Successfully opened Anki database in read-only mode");
        tx.send(LoaderMessage::Progress(0.30)).unwrap(); // 30% - DB opened
        
        // Get current timestamp for scheduling (Anki uses days since epoch)
        let today = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / 86400; // Convert to days
        
        println!("Today (Anki format): {}", today);
        
        // Get total cards for progress tracking
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM cards")?;
        let total_cards: i64 = stmt.query_row([], |row| row.get(0))?;
        println!("⏱️  [{}ms] Card count query completed: {} cards", start_time.elapsed().as_millis(), total_cards);
        tx.send(LoaderMessage::Progress(0.50)).unwrap(); // 50% - Basic info loaded
        
        // Step 1: Get cards that are due today or overdue (using indexed query)
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
        println!("⏱️  [{}ms] Loaded {} scheduled cards (due today or overdue)", start_time.elapsed().as_millis(), scheduled_cards.len());
        tx.send(LoaderMessage::Progress(0.70)).unwrap(); // 70% - Due cards loaded
        
        // Step 2: If we have fewer than 10 cards, add more to reach the minimum
        let min_cards = 10;
        if scheduled_cards.len() < min_cards {
            let needed_cards = min_cards - scheduled_cards.len();
            println!("Need {} more cards to reach minimum of {}", needed_cards, min_cards);
            
            // Get additional cards that aren't already selected
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
            println!("⏱️  [{}ms] Added {} additional cards (random selection) for total of {}", 
                     start_time.elapsed().as_millis(), additional_count, scheduled_cards.len());
        }
        
        tx.send(LoaderMessage::Progress(0.90)).unwrap(); // 90% - All cards loaded
        println!("⏱️  [{}ms] Card loading phase complete", start_time.elapsed().as_millis());
        
        // Create a deck with database connection for on-demand note loading
        // We can directly use the cached database path without copying
        let lazy_deck = CachedDeck {
            db_path,
            initial_cards: scheduled_cards,
            total_card_count: total_cards,
        };
        println!("⏱️  [{}ms] CachedDeck created", start_time.elapsed().as_millis());
        
        tx.send(LoaderMessage::Progress(1.0)).unwrap(); // 100% - Deck ready

        let deck = lazy_deck.into_deck()?;
        println!("⏱️  [{}ms] TOTAL LOADING TIME - Deck ready", start_time.elapsed().as_millis());
        Ok(deck)
    })(); // Immediately-invoked function expression to handle errors cleanly

    // Send the final result (either the Deck or an Error) through the channel.
    tx.send(LoaderMessage::Complete(result.map_err(|e| e.to_string()))).unwrap();
}
