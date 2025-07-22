// src/scheduler/queue.rs
// Daily queue builder for Core Learning Loop

use crate::config::{BACKLOG_CAP, PACK_SIZE_DEFAULT};
use crate::deck::Deck;
// use crate::storage::models::{CardId, SrsRow, DailyLogRow};
use crate::storage::models::{CardId};
use chrono::{NaiveDate};
use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueData {
    pub pack_size: usize,
    pub review_coefficient: f64,
    pub cards: Vec<CardId>,
    #[serde(default)]
    pub completed_cards: Vec<CardId>, // Track completed cards (defaults to empty for backward compatibility)
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
pub fn ensure_today_with_deck(today: NaiveDate, deck: &Deck) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn load_today(today: NaiveDate) -> Vec<CardId> {
    let queue_path = get_queue_path(today);
    
    let queue_data: QueueData = serde_json::from_str(
        &fs::read_to_string(&queue_path)
            .unwrap_or_else(|_| panic!("Queue file missing for {}", today))
    ).expect("Failed to parse queue JSON");
    
    queue_data.cards
}

/// Get cards that are due for review today
pub fn due_cards(today: NaiveDate, conn: &Connection) -> SqlResult<Vec<CardId>> {
    let today_timestamp = today.and_hms_opt(9, 0, 0).unwrap().and_utc().timestamp();
    
    let mut stmt = conn.prepare(
        "SELECT card_id FROM srs_log 
         WHERE next_due_ts IS NULL OR next_due_ts <= ?1 
         ORDER BY next_due_ts ASC NULLS FIRST 
         LIMIT ?2"
    )?;
    
    let card_ids: Result<Vec<CardId>, _> = stmt
        .query_map([today_timestamp, BACKLOG_CAP as i64], |row| {
            Ok(row.get::<_, i64>(0)?)
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
         LIMIT ?1"
    )?;
    
    let card_ids: Result<Vec<CardId>, _> = stmt
        .query_map([count as i64], |row| {
            Ok(row.get::<_, i64>(0)?)
        })?
        .collect();
    
    card_ids
}

/// Builds today's queue and saves it to disk
/// For Sprint 0: Creates a simple queue with first N cards from available decks
pub fn build_today(today: NaiveDate) -> Result<(), Box<dyn std::error::Error>> {
    // For Sprint 0, we'll use hard-coded parameters
    let pack_sz = PACK_SIZE_DEFAULT;
    let rev_coef = 2.0; // Hard-coded for Sprint 0
    
    // For Sprint 0: Generate a simple queue with sequential card IDs
    // This will be replaced with database queries in Sprint 1
    let mut cards = Vec::new();
    
    // Create a basic queue with card IDs 1 through pack_sz
    // In Sprint 1, this will be replaced with actual due cards + new cards
    for i in 1..=pack_sz as i64 {
        cards.push(i);
    }
    
    let queue_data = QueueData {
        pack_size: pack_sz,
        review_coefficient: rev_coef,
        cards,
        completed_cards: Vec::new(),
    };
    
    // Create queues directory if it doesn't exist
    let queue_dir = get_queue_dir();
    fs::create_dir_all(&queue_dir)?;
    
    // Write queue to JSON file
    let queue_path = get_queue_path(today);
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;
    
    // Create skeleton row in daily_log
    create_daily_log_skeleton(today, pack_sz, rev_coef)?;
    
    println!("Created queue for {} with {} cards", today, queue_data.cards.len());
    Ok(())
}

/// Builds today's queue using actual deck card IDs
pub fn build_today_with_deck(today: NaiveDate, deck: &Deck) -> Result<(), Box<dyn std::error::Error>> {
    // For Sprint 0, we'll use hard-coded parameters
    let pack_sz = PACK_SIZE_DEFAULT;
    let rev_coef = 2.0; // Hard-coded for Sprint 0
    
    // Use actual card IDs from the deck
    let cards: Vec<CardId> = deck.cards.iter()
        .take(pack_sz) // Limit to pack size
        .map(|c| c.id)
        .collect();
    
    let queue_data = QueueData {
        pack_size: pack_sz,
        review_coefficient: rev_coef,
        cards,
        completed_cards: Vec::new(),
    };
    
    // Create queues directory if it doesn't exist
    let queue_dir = get_queue_dir();
    fs::create_dir_all(&queue_dir)?;
    
    // Write deck-specific queue to JSON file
    let deck_id = get_deck_id(deck);
    let queue_path = get_deck_queue_path(today, &deck_id);
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;
    
    // Create skeleton row in daily_log
    create_daily_log_skeleton(today, pack_sz, rev_coef)?;
    
    println!("Created deck-specific queue for {} (deck {}) with {} cards", today, deck_id, queue_data.cards.len());
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

/// Creates a skeleton row in daily_log table
fn create_daily_log_skeleton(today: NaiveDate, pack_sz: usize, rev_coef: f64) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Connect to actual database
    // This is a stub for Sprint 0
    println!("Would create daily_log skeleton: {} pack_sz={} rev_coef={}", today, pack_sz, rev_coef);
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
fn get_queue_path(date: NaiveDate) -> PathBuf {
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
    let base_id = format!("{:x}", id_sample.iter().fold(0u64, |acc, &x| acc.wrapping_add(x as u64)));
    
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
pub fn mark_card_completed_for_deck_id(today: NaiveDate, card_id: CardId, deck_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let queue_path = get_deck_queue_path(today, deck_id);
    
    if !queue_path.exists() {
        return Ok(()); // No queue exists, nothing to mark
    }
    
    // Load existing queue
    let mut queue_data: QueueData = serde_json::from_str(
        &fs::read_to_string(&queue_path)?
    )?;
    
    // Add to completed cards if not already there
    if !queue_data.completed_cards.contains(&card_id) {
        queue_data.completed_cards.push(card_id);
        println!("✅ Marked card {} as completed in deck {}", card_id, deck_id);
    }
    
    // Save updated queue
    let json_data = serde_json::to_string_pretty(&queue_data)?;
    fs::write(&queue_path, json_data)?;
    
    Ok(())
}

/// Mark a card as completed in today's queue (legacy function - needs deck)
pub fn mark_card_completed(today: NaiveDate, card_id: CardId) -> Result<(), Box<dyn std::error::Error>> {
    // This is a fallback for compatibility - will need deck parameter in the future
    println!("⚠️ Using legacy mark_card_completed - consider updating to mark_card_completed_for_deck");
    let queue_path = get_queue_path(today);
    
    if !queue_path.exists() {
        return Ok(()); // No queue exists, nothing to mark
    }
    
    // Load existing queue
    let mut queue_data: QueueData = serde_json::from_str(
        &fs::read_to_string(&queue_path)?
    )?;
    
    // Add to completed cards if not already there
    if !queue_data.completed_cards.contains(&card_id) {
        queue_data.completed_cards.push(card_id);
        println!("✅ Marked card {} as completed", card_id);
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
        println!("⚠️ No deck-specific queue file exists for {} (deck {})", today, deck_id);
        return Vec::new();
    }
    
    match fs::read_to_string(&queue_path) {
        Ok(content) => {
            match serde_json::from_str::<QueueData>(&content) {
                Ok(queue_data) => {
                    let total_cards = queue_data.cards.len();
                    let completed_count = queue_data.completed_cards.len();
                    let remaining: Vec<CardId> = queue_data.cards.into_iter()
                        .filter(|card_id| !queue_data.completed_cards.contains(card_id))
                        .collect();
                    println!("📋 Deck {} queue has {} total cards, {} completed, {} remaining", 
                            deck_id, total_cards, completed_count, remaining.len());
                    remaining
                }
                Err(e) => {
                    println!("⚠️ Failed to parse deck queue file: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            println!("⚠️ Failed to read deck queue file: {}", e);
            Vec::new()
        }
    }
}

/// Get remaining (uncompleted) cards from today's queue (legacy function)
pub fn get_remaining_cards(today: NaiveDate) -> Vec<CardId> {
    let queue_path = get_queue_path(today);
    
    if !queue_path.exists() {
        println!("⚠️ No queue file exists for {}", today);
        return Vec::new();
    }
    
    match fs::read_to_string(&queue_path) {
        Ok(content) => {
            match serde_json::from_str::<QueueData>(&content) {
                Ok(queue_data) => {
                    let total_cards = queue_data.cards.len();
                    let completed_count = queue_data.completed_cards.len();
                    let remaining: Vec<CardId> = queue_data.cards.into_iter()
                        .filter(|card_id| !queue_data.completed_cards.contains(card_id))
                        .collect();
                    println!("📋 Queue has {} total cards, {} completed, {} remaining", 
                            total_cards, completed_count, remaining.len());
                    remaining
                }
                Err(e) => {
                    println!("⚠️ Failed to parse queue file: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            println!("⚠️ Failed to read queue file: {}", e);
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
}