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
        
        // Check if the provided path is a direct cached deck directory (for testing)
        // or if we need to look it up in the cache
        let db_path = if path.join("manifest.json").exists() {
            // Direct path to cached deck directory
            let manifest_path = path.join("manifest.json");
            let manifest = scanner::load_manifest(&manifest_path)
                .map_err(|e| format!("Failed to load manifest: {}", e))?;
            path.join(&manifest.db_file)
        } else {
            // Look up in cache using deck hash
            scanner::ensure_cached_deck(deck_hash)?
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use crate::testing::setup_cached_deck_structure;
    use tempfile::tempdir;

    #[test]
    fn test_load_apkg_progress_reporting() {
        // Create cached deck structure with proper manifest and database
        let (_temp_dir, test_deck_path) = setup_cached_deck_structure("test_deck");
        
        let (tx, rx) = mpsc::channel();
        
        // Spawn loader in background thread
        let test_path = test_deck_path.clone();
        thread::spawn(move || {
            load_apkg(&test_path, tx);
        });
        
        // Verify progress messages arrive in order
        let mut progress_values = Vec::new();
        let mut received_complete = false;
        
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(5)) {
            match msg {
                LoaderMessage::Progress(p) => {
                    progress_values.push(p);
                },
                LoaderMessage::Complete(_) => {
                    received_complete = true;
                    break;
                },
                _ => {}
            }
        }
        
        // Verify progress reporting
        assert!(progress_values.len() >= 5, "Should have multiple progress updates");
        assert!(progress_values.iter().all(|&p| p >= 0.0 && p <= 1.0), "Progress should be 0.0-1.0");
        assert!(progress_values.windows(2).all(|w| w[0] <= w[1]), "Progress should be non-decreasing");
        assert!(received_complete, "Should receive completion message");
    }

    #[test]
    fn test_load_apkg_minimum_card_selection() {
        // Create cached deck structure with proper manifest and database
        let (_temp_dir, test_deck_path) = setup_cached_deck_structure("min_cards_test");
        
        let (tx, rx) = mpsc::channel();
        
        // Load deck
        let test_path = test_deck_path.clone();
        thread::spawn(move || {
            load_apkg(&test_path, tx);
        });
        
        // Wait for completion
        let mut deck_result = None;
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(5)) {
            if let LoaderMessage::Complete(result) = msg {
                deck_result = Some(result);
                break;
            }
        }
        
        // Verify minimum 10 cards were selected
        let deck = deck_result.unwrap().unwrap();
        assert!(deck.cards.len() >= 10, "Should have at least 10 cards, got {}", deck.cards.len());
    }

    #[test]
    fn test_load_apkg_due_card_priority() {
        // Create cached deck structure with proper manifest and database
        let (_temp_dir, test_deck_path) = setup_cached_deck_structure("due_priority_test");
        
        let (tx, rx) = mpsc::channel();
        
        let test_path = test_deck_path.clone();
        thread::spawn(move || {
            load_apkg(&test_path, tx);
        });
        
        // Get result
        let mut deck_result = None;
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(5)) {
            if let LoaderMessage::Complete(result) = msg {
                deck_result = Some(result);
                break;
            }
        }
        
        let deck = deck_result.unwrap().unwrap();
        
        // Verify due/overdue cards appear first
        // Cards 1, 2, 3 should be in the selection (they're due/overdue)
        let card_ids: Vec<i64> = deck.cards.iter().map(|c| c.id).collect();
        assert!(card_ids.contains(&1), "Should include overdue card 1");
        assert!(card_ids.contains(&2), "Should include due card 2"); 
        assert!(card_ids.contains(&3), "Should include due card 3");
        
        // First few cards should be the due ones
        assert!(deck.cards[0].id <= 3, "First card should be due/overdue");
    }

    #[test]
    fn test_load_apkg_error_handling() {
        let temp_dir = tempdir().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent");
        
        let (tx, rx) = mpsc::channel();
        
        // Try to load nonexistent deck
        thread::spawn(move || {
            load_apkg(&nonexistent_path, tx);
        });
        
        // Should receive error result
        let mut received_error = false;
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(5)) {
            if let LoaderMessage::Complete(Err(_)) = msg {
                received_error = true;
                break;
            }
        }
        
        assert!(received_error, "Should receive error for nonexistent deck");
    }
}
