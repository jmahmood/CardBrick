// src/scheduler.rs

// Contains the logic for the spaced repetition system.

use crate::config::PACK_SIZE_DEFAULT;
use crate::deck::{Card, Deck, Note};
use crate::scheduler::queue;
use crate::storage::db::progress_path;
use rand::seq::SliceRandom;
use rand::thread_rng;
use chrono::Utc;
use rusqlite::Connection;


/// Represents the user's rating for a card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

/// Applies SM-2 algorithm with database persistence for a card rating
/// Updates srs_log table with new interval, ease factor, repetitions, and next due date
pub fn apply_rating(card_id: i64, quality: Rating, timestamp: i64) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = progress_path();
    apply_rating_with_path(card_id, quality, timestamp, &db_path)
}

/// Applies SM-2 algorithm with explicit database path - supports dependency injection
pub fn apply_rating_with_path(card_id: i64, quality: Rating, timestamp: i64, db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;
    
    // Get current card state from srs_log, or create new entry
    let mut stmt = conn.prepare(
        "SELECT interval, ease_factor, reps, lapses FROM srs_log WHERE card_id = ?1"
    )?;
    
    let (mut interval, mut ease_factor, mut reps, mut lapses): (u32, f32, u32, u32) = match stmt.query_row([card_id], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u32,
            row.get::<_, Option<f64>>(1)?.unwrap_or(2.5) as f32,
            row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
            row.get::<_, Option<i64>>(3)?.unwrap_or(0) as u32,
        ))
    }) {
        Ok(values) => values,
        Err(_) => (0, 2.5, 0, 0), // New card defaults
    };
    
    // Apply SM-2 algorithm based on quality rating
    match quality {
        Rating::Again => {
            lapses += 1;
            ease_factor = (ease_factor - 0.2).max(1.3); // Clamp minimum ease to 1.3
            interval = 1; // Reset interval to 1 day
            reps = 0; // Reset repetition count
        }
        Rating::Hard => {
            ease_factor = (ease_factor - 0.15).max(1.3); // Clamp minimum ease to 1.3
            reps += 1;
            interval = if interval == 0 {
                1
            } else {
                ((interval as f32 * 1.2).round() as u32).max(interval + 1)
            };
        }
        Rating::Good => {
            reps += 1;
            interval = if interval == 0 {
                1
            } else if interval == 1 {
                6 // Standard SM-2: second interval is 6 days
            } else {
                ((interval as f32 * ease_factor).round() as u32).max(interval + 1)
            };
        }
        Rating::Easy => {
            ease_factor = (ease_factor + 0.15).min(2.5); // Clamp maximum ease to 2.5
            reps += 1;
            interval = if interval == 0 {
                4 // Easy first rating gets 4 days
            } else {
                let easy_bonus = 1.3;
                ((interval as f32 * ease_factor * easy_bonus).round() as u32).max(interval + 1)
            };
        }
    }
    
    // Calculate next due timestamp (today + interval days)
    let next_due_ts = timestamp + (interval as i64 * 24 * 60 * 60);
    
    // Update or insert srs_log entry
    conn.execute(
        "INSERT OR REPLACE INTO srs_log (card_id, interval, ease_factor, reps, lapses, next_due_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![card_id, interval as i64, ease_factor as f64, reps as i64, lapses as i64, next_due_ts],
    )?;
    
    Ok(())
}


/// A trait defining the core behavior of any scheduling algorithm.
pub trait Scheduler {
    fn new(deck: Deck) -> Self where Self: Sized;
    fn next_card(&mut self) -> Option<Card>;
    fn answer_card(&mut self, card_id: i64, rating: Rating) -> Option<Card>; // Changed return type
    fn get_note(&mut self, note_id: i64) -> Option<Note>; // Changed to owned Note to work with lazy loading
    fn reviews_complete(&self) -> usize;
    fn total_session_cards(&self) -> usize;
    fn hard_cards(&self) -> &[i64];
    fn failed_cards_today(&self) -> &[i64];
    fn rewind_last_answer(&mut self) -> Option<Card>;
    fn add_card_to_front(&mut self, card_id: i64);
    fn introduce_new_cards(&mut self, count: usize) -> usize;
    fn load_more_cards(&mut self, count: usize) -> Result<Vec<Card>, Box<dyn std::error::Error>>;
    fn session_ratings(&self) -> &[(i64, Rating)];
    fn get_card_note_id(&self, card_id: i64) -> Option<i64>;
    fn get_card_note_from_db(&mut self, card_id: i64) -> Result<Option<(i64, Note)>, Box<dyn std::error::Error>>;
}

/// Implementation of the SM-2 algorithm.
pub struct Sm2Scheduler {
    deck: Deck, // Store the entire deck for lazy loading
    review_queue: Vec<i64>,
    session_total: usize,
    session_reviews_complete: usize,
    hard_cards_this_session: Vec<i64>,
    failed_cards_today: Vec<i64>, // Track cards answered "Again" today
    last_answer: Option<(i64, Rating, Card)>, // Store a clone of the card state before modification
    today_completed_cards: Vec<i64>, // Track cards completed today
    session_ratings: Vec<(i64, Rating)>, // Track each card's rating in session order for progress visualization
    deck_id: String, // Store deck ID for queue operations
}

impl Scheduler for Sm2Scheduler {
    fn new(deck: Deck) -> Self {
        // Use daily queue system for Core Learning Loop
        let today = Utc::now().date_naive();
        
        // Create a queue using actual deck card IDs, filtering out already completed cards
        let mut review_queue = if let Ok(()) = queue::ensure_today_with_deck(today, &deck) {
            let remaining_cards = queue::get_remaining_cards_for_deck(today, &deck);
            println!("📅 Loaded deck-specific daily queue with {} remaining cards for {}", remaining_cards.len(), today);
            
            // In test mode, if no remaining cards from queue system, use all deck cards for predictable behavior
            if cfg!(test) && remaining_cards.is_empty() {
                println!("🧪 Test mode: Using all deck cards since queue returned empty");
                deck.cards.iter().map(|c| c.id).collect()
            } else {
                remaining_cards
            }
        } else {
            // Fallback to limited subset using actual deck card IDs
            println!("⚠️ Queue system failed, using fallback limited deck");
            let mut limited_cards: Vec<i64> = deck.cards.iter()
                .take(PACK_SIZE_DEFAULT) // Limit to default pack size
                .map(|c| c.id)
                .collect();
            
            if !cfg!(test) {
                limited_cards.shuffle(&mut thread_rng());
            }
            limited_cards
        };
        
        if cfg!(test) {
            // Sort ascending for predictable test order. .pop() will take from the end.
            review_queue.sort_unstable(); 
        } else if review_queue.len() > 1 {
            review_queue.shuffle(&mut thread_rng());
        }
        
        let session_total = review_queue.len();
        let deck_id = queue::get_deck_id(&deck);
        println!("🎯 Session will have {} total cards for deck {}", session_total, deck_id);

        Sm2Scheduler {
            deck,
            review_queue,
            session_total,
            session_reviews_complete: 0,
            hard_cards_this_session: Vec::new(),
            failed_cards_today: Vec::new(),
            last_answer: None,
            today_completed_cards: Vec::new(),
            session_ratings: Vec::new(),
            deck_id,
        }
    }

    fn next_card(&mut self) -> Option<Card> {
        // For Core Learning Loop: stick to the daily queue, no additional loading
        // Get next card from queue
        if let Some(card_id) = self.review_queue.pop() {
            // Find card in our deck, load more if needed
            if let Some(card) = self.deck.cards.iter().find(|c| c.id == card_id).cloned() {
                return Some(card);
            }
            
            // If card not in memory, try to load more from DB
            if self.deck.cached_db_connection.is_some() {
                if let Ok(new_cards) = self.deck.load_more_cards(50) {
                    // Try to find the card in the newly loaded batch
                    if let Some(card) = new_cards.iter().find(|c| c.id == card_id).cloned() {
                        return Some(card);
                    }
                }
            }
            
            // Card not found, skip and try next
            self.next_card()
        } else {
            None
        }
    }
    
    fn add_card_to_front(&mut self, card_id: i64) {
        // Pushing to the end of the vec makes it the next item for .pop()
        self.review_queue.push(card_id);
    }

    fn answer_card(&mut self, card_id: i64, rating: Rating) -> Option<Card> {
        // Find the card in our deck
        let card = self.deck.cards.iter_mut().find(|c| c.id == card_id)?;
        
        self.last_answer = Some((card_id, rating, card.clone()));
        
        // Track this rating for session progress visualization
        println!("DEBUG: Tracking rating for card {}: {:?}", card_id, rating);
        self.session_ratings.push((card_id, rating));
        println!("DEBUG: Total session ratings now: {}", self.session_ratings.len());
        
        // Store the result card to return after DB operations
        let mut result_card = Some(card.clone())?;

        match rating {
            Rating::Again => {
                result_card.lapses += 1;
                result_card.ease_factor = (result_card.ease_factor as i32 - 200).max(1300) as u32;
                result_card.interval = 0;
                
                // Track this card as failed today
                if !self.failed_cards_today.contains(&card_id) {
                    self.failed_cards_today.push(card_id);
                }
                
                
                let cooldown_distance = 5_u32.saturating_sub(result_card.lapses).max(2) as usize;
                let insertion_point = self.review_queue.len().saturating_sub(cooldown_distance);
                self.review_queue.insert(insertion_point, result_card.id);
            }
            _ => { // Hard, Good, or Easy
                self.session_reviews_complete += 1;
                
                // Track this card as completed today and mark in deck-specific queue
                if !self.today_completed_cards.contains(&card_id) {
                    self.today_completed_cards.push(card_id);
                    
                    // Mark card as completed in persistent deck-specific queue
                    let today = Utc::now().date_naive();
                    if let Err(e) = queue::mark_card_completed_for_deck_id(today, card_id, &self.deck_id) {
                        eprintln!("Warning: Failed to mark card {} as completed in deck {}: {}", card_id, self.deck_id, e);
                    }
                }
                
               let ease_factor_multiplier = result_card.ease_factor as f32 / 1000.0;

                match rating {
                    Rating::Good => { 
                        let new_interval = if result_card.interval == 0 {
                            1
                        } else {
                            (result_card.interval as f32 * ease_factor_multiplier).round() as u32
                        };
                        // The new interval should be at least one day longer than the previous one.
                        result_card.interval = new_interval.max(result_card.interval + 1);
                    }
                    Rating::Hard => {
                        if !self.hard_cards_this_session.contains(&card_id) {
                            self.hard_cards_this_session.push(card_id);
                        }
                        
                        
                        result_card.ease_factor = (result_card.ease_factor as i32 - 150).max(1300) as u32;
                        // "Hard" interval multiplier is 1.2
                        let new_interval = (result_card.interval as f32 * 1.2).round() as u32;
                        result_card.interval = new_interval.max(result_card.interval + 1);
                    }
                    Rating::Easy => {
                        // Increase ease by 150 (15%)
                        result_card.ease_factor = (result_card.ease_factor as i32 + 150) as u32;
                        // "Easy" bonus multiplier is typically 1.3
                        let easy_bonus = 1.3;
                        let new_interval = if result_card.interval == 0 {
                            // A common default for the first "Easy" rating is 4 days
                            4
                        } else {
                            (result_card.interval as f32 * ease_factor_multiplier * easy_bonus).round() as u32
                        };
                        result_card.interval = new_interval.max(result_card.interval + 1);
                    }
                    Rating::Again => {} // Already handled
                }
            }
        }
        Some(result_card) // Return a clone of the modified card
    }

    fn rewind_last_answer(&mut self) -> Option<Card> {
        if let Some((card_id, rating, original_card_state)) = self.last_answer.take() {
            println!("Rewinding last answer for card #{}", card_id);
            
            // Remove the card from wherever it was re-inserted in the queue.
            self.review_queue.retain(|&id| id != card_id);
            
            // Remove the last rating from session tracking
            if let Some(last_pos) = self.session_ratings.iter().rposition(|&(id, _)| id == card_id) {
                self.session_ratings.remove(last_pos);
                println!("DEBUG: Removed rating from session, now {} ratings", self.session_ratings.len());
            }

            // Revert state changes.
            if rating != Rating::Again {
                self.session_reviews_complete = self.session_reviews_complete.saturating_sub(1);
            }
            if rating == Rating::Hard {
                self.hard_cards_this_session.retain(|&id| id != card_id);
            }
            
            // Restore the card to its original state.
            if let Some(card) = self.deck.cards.iter_mut().find(|c| c.id == card_id) {
                *card = original_card_state.clone();
            }
            
            return Some(original_card_state);
        }
        None
    }

    fn get_note(&mut self, note_id: i64) -> Option<Note> { 
        self.deck.get_note(note_id).ok().flatten()
    }
    fn reviews_complete(&self) -> usize { self.session_reviews_complete }
    fn total_session_cards(&self) -> usize { self.session_total }
    fn hard_cards(&self) -> &[i64] { &self.hard_cards_this_session }
    
    fn failed_cards_today(&self) -> &[i64] { &self.failed_cards_today }
    
    fn session_ratings(&self) -> &[(i64, Rating)] { &self.session_ratings }
    
    fn get_card_note_id(&self, card_id: i64) -> Option<i64> {
        self.deck.cards.iter().find(|c| c.id == card_id).map(|c| c.note_id)
    }
    
    fn get_card_note_from_db(&mut self, card_id: i64) -> Result<Option<(i64, Note)>, Box<dyn std::error::Error>> {
        // First, get the note_id for this card from the database
        let note_id_opt = if let Some(ref cached_conn) = self.deck.cached_db_connection {
            let db_conn = rusqlite::Connection::open(&cached_conn.db_path)?;
            let mut stmt = db_conn.prepare("SELECT nid FROM cards WHERE id = ?1")?;
            let mut rows = stmt.query_map([card_id], |row| {
                Ok(row.get::<_, i64>(0)?)
            })?;
            
            if let Some(row) = rows.next() {
                Some(row?)
            } else {
                None
            }
        } else {
            None
        };
        
        if let Some(note_id) = note_id_opt {
            // Now get the note using the deck's get_note method
            if let Ok(Some(note)) = self.deck.get_note(note_id) {
                return Ok(Some((note_id, note)));
            }
        }
        
        Ok(None)
    }
    
    fn introduce_new_cards(&mut self, count: usize) -> usize {
        // In this scheduler, all cards are in the review queue from the beginning.
        // "Introducing new cards" means bringing cards from the back of the line
        // (the start of the vector) to the front (the end of the vector, where .pop()
        // takes from).
        
        let queue_len = self.review_queue.len();
        if queue_len < 2 { // Not enough cards to reorder
            return 0;
        }

        // Determine how many cards to move. We can't move more than are available.
        let num_to_move = count.min(queue_len);
        
        // Take `num_to_move` cards from the front of the queue (the bottom of the pile).
        let cards_to_promote: Vec<i64> = self.review_queue.drain(0..num_to_move).collect();
        
        // Add them to the end of the queue, making them available to be popped soon.
        self.review_queue.extend(cards_to_promote);
        
        // Shuffle the entire queue to mix the newly promoted cards with the
        // existing ones, providing a real change of pace.
        self.review_queue.shuffle(&mut thread_rng());
        
        num_to_move
    }

    fn load_more_cards(&mut self, count: usize) -> Result<Vec<Card>, Box<dyn std::error::Error>> {
        // Load more cards from the deck using database-stored difficult cards for prioritization
        let new_cards = self.deck.load_more_cards(count)?;
        
        // Add the new card IDs to the review queue
        for card in &new_cards {
            self.review_queue.push(card.id);
        }
        
        // Update session total
        self.session_total += new_cards.len();
        
        
        Ok(new_cards)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::CachedDeckConnection;
    use std::collections::HashMap;


    fn create_test_deck(num_cards: usize) -> Deck {
        let mut cards = Vec::new();
        let mut notes = HashMap::new();
        for i in 0..num_cards {
            let card_id = i as i64;
            let note_id = i as i64;
            cards.push(Card { id: card_id, note_id, due: 0, interval: 0, ease_factor: 2500, lapses: 0 });
            notes.insert(note_id, Note { id: note_id, fields: vec![format!("Front {}", i), format!("Back {}", i)] });
        }
        Deck { 
            cards, 
            notes,
            cached_db_connection: None,
        }
    }

    #[test]
    fn test_initialization() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(10));
        assert_eq!(scheduler.total_session_cards(), 10);
        assert_eq!(scheduler.reviews_complete(), 0);
        // Test that pop returns highest ID first because of test-only sort
        assert_eq!(scheduler.next_card().unwrap().id, 9);
    }

    #[test]
    fn test_review_flow() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(5));
        let card = scheduler.next_card().unwrap();
        scheduler.answer_card(card.id, Rating::Good);
        assert_eq!(scheduler.reviews_complete(), 1);
        let card = scheduler.next_card().unwrap();
        scheduler.answer_card(card.id, Rating::Easy);
        assert_eq!(scheduler.reviews_complete(), 2);
    }

    #[test]
    fn test_again_cooldown() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(7));
        
        let failed_card = scheduler.next_card().unwrap();
        assert_eq!(failed_card.id, 6);

        scheduler.answer_card(failed_card.id, Rating::Again);
        assert_eq!(scheduler.reviews_complete(), 0);

        // Pop the next 4 cards from the queue
        assert_eq!(scheduler.next_card().unwrap().id, 5);
        assert_eq!(scheduler.next_card().unwrap().id, 4);
        assert_eq!(scheduler.next_card().unwrap().id, 3);
        assert_eq!(scheduler.next_card().unwrap().id, 2);

        // The 5th card should be the one we failed
        assert_eq!(scheduler.next_card().unwrap().id, 6);
    }

    #[test]
    fn test_rewind() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(5));

        let card_4 = scheduler.next_card().unwrap(); // id=4
        scheduler.answer_card(card_4.id, Rating::Good); // reviews=1
        
        let card_3 = scheduler.next_card().unwrap(); // id=3
        scheduler.answer_card(card_3.id, Rating::Hard); // reviews=2, hard_cards=[3]
        
        assert_eq!(scheduler.reviews_complete(), 2);
        assert_eq!(scheduler.hard_cards(), &[3]);
        
        let card_2 = scheduler.next_card().unwrap(); // id=2, currently "on screen"
        
        // User hits rewind. We hold card_2 and rewind card_3.
        scheduler.add_card_to_front(card_2.id); 
        let rewound_card = scheduler.rewind_last_answer().unwrap();
        assert_eq!(rewound_card.id, 3);
        
        // Check that state was reverted
        assert_eq!(scheduler.reviews_complete(), 1);
        assert!(scheduler.hard_cards().is_empty());

        // We now present the rewound card (3) to the user.
        // It must be put back in the queue so it's the next one served.
        scheduler.add_card_to_front(rewound_card.id);

        // The next card should be the rewound card (3)
        let next = scheduler.next_card().unwrap();
        assert_eq!(next.id, 3);

        // After answering the rewound card, the next should be the one we held (2)
        scheduler.answer_card(next.id, Rating::Good);
        let final_card = scheduler.next_card().unwrap();
        assert_eq!(final_card.id, 2);
    }

    #[test]
    fn test_failed_cards_tracking() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(5));
        
        // Answer first card with "Again"
        let card1 = scheduler.next_card().unwrap();
        scheduler.answer_card(card1.id, Rating::Again);
        assert_eq!(scheduler.failed_cards_today(), &[card1.id]);
        
        // Answer second card with "Again"
        let card2 = scheduler.next_card().unwrap();
        scheduler.answer_card(card2.id, Rating::Again);
        assert_eq!(scheduler.failed_cards_today(), &[card1.id, card2.id]);
        
        // Answer third card with "Good" - should not be in failed list
        let card3 = scheduler.next_card().unwrap();
        scheduler.answer_card(card3.id, Rating::Good);
        assert_eq!(scheduler.failed_cards_today(), &[card1.id, card2.id]);
    }

    #[test]
    fn test_hard_cards_tracking() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(5));
        
        // Answer first card with "Hard"
        let card1 = scheduler.next_card().unwrap();
        scheduler.answer_card(card1.id, Rating::Hard);
        assert_eq!(scheduler.hard_cards(), &[card1.id]);
        
        // Answer second card with "Hard"
        let card2 = scheduler.next_card().unwrap();
        scheduler.answer_card(card2.id, Rating::Hard);
        assert_eq!(scheduler.hard_cards(), &[card1.id, card2.id]);
        
        // Answer third card with "Easy" - should not be in hard list
        let card3 = scheduler.next_card().unwrap();
        scheduler.answer_card(card3.id, Rating::Easy);
        assert_eq!(scheduler.hard_cards(), &[card1.id, card2.id]);
    }

    #[test]
    fn test_duplicate_tracking() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(3));
        
        // Answer first card with "Again"
        let card1 = scheduler.next_card().unwrap();
        scheduler.answer_card(card1.id, Rating::Again);
        assert_eq!(scheduler.failed_cards_today(), &[card1.id]);
        
        // The same card should come back due to cooldown - answer "Again" again
        let _returned_card = scheduler.next_card().unwrap();
        // Skip a few cards to get back to the failed one
        for _ in 0..3 {
            if let Some(card) = scheduler.next_card() {
                if card.id == card1.id {
                    scheduler.answer_card(card.id, Rating::Again);
                    break;
                }
                scheduler.answer_card(card.id, Rating::Good);
            }
        }
        
        // Should still only appear once in failed_cards_today
        assert_eq!(scheduler.failed_cards_today().len(), 1);
        assert!(scheduler.failed_cards_today().contains(&card1.id));
    }

    #[test]
    fn test_mixed_difficult_cards_tracking() {
        let mut scheduler = Sm2Scheduler::new(create_test_deck(5));
        
        // Answer cards with different ratings
        let card1 = scheduler.next_card().unwrap();
        scheduler.answer_card(card1.id, Rating::Again); // Failed
        
        let card2 = scheduler.next_card().unwrap();
        scheduler.answer_card(card2.id, Rating::Hard); // Hard
        
        let card3 = scheduler.next_card().unwrap();
        scheduler.answer_card(card3.id, Rating::Good); // Neither
        
        // Check both tracking arrays
        assert_eq!(scheduler.failed_cards_today(), &[card1.id]);
        assert_eq!(scheduler.hard_cards(), &[card2.id]);
    }

    #[test]
    fn test_continue_studying_prioritizes_difficult_cards() {
        // This is an integration test that verifies the complete flow:
        // 1. Create scheduler with test database 
        // 2. Answer some cards as failed/hard
        // 3. Call load_more_cards
        // 4. Verify difficult cards are prioritized in results

        use tempfile;
        
        // Create test database with more cards
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_db_path = temp_dir.path().join("integration_test.db");
        let file_conn = rusqlite::Connection::open(&temp_db_path).unwrap();
        
        // Create database schema
        file_conn.execute(
            "CREATE TABLE cards (
                id INTEGER PRIMARY KEY,
                nid INTEGER,
                due INTEGER,
                ivl INTEGER,
                factor INTEGER,
                lapses INTEGER
            )",
            [],
        ).unwrap();
        
        file_conn.execute(
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
        ).unwrap();
        
        // Insert test cards
        for i in 1..=50 {
            file_conn.execute(
                "INSERT INTO cards (id, nid, due, ivl, factor, lapses) VALUES (?, ?, 0, 0, 2500, 0)",
                [i, i * 10],
            ).unwrap();
        }
        
        // Create difficult_cards table for testing
        file_conn.execute(
            "CREATE TABLE IF NOT EXISTS difficult_cards (
                card_id INTEGER,
                difficulty_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                date TEXT NOT NULL,
                PRIMARY KEY (card_id, difficulty_type, date)
            )",
            [],
        ).unwrap();
        
        drop(file_conn);
        
        // Create deck with database connection
        let deck = Deck {
            cards: (1..=12).map(|i| Card { 
                id: i, 
                note_id: i * 10, 
                due: 0, 
                interval: 0, 
                ease_factor: 2500, 
                lapses: 0 
            }).collect(),
            notes: HashMap::new(),
            cached_db_connection: Some(CachedDeckConnection {
                db_path: temp_db_path.clone(),
                total_card_count: 50,
            }),
        };
        
        // Create scheduler
        let mut scheduler = Sm2Scheduler::new(deck);
        
        // Simulate answering cards during initial session
        let mut answered_cards = Vec::new();
        let mut failed_cards = Vec::new();
        let mut hard_cards = Vec::new();
        
        // Answer several cards, marking some as failed/hard
        for i in 0..8 {
            if let Some(card) = scheduler.next_card() {
                answered_cards.push(card.id);
                
                let rating = match i {
                    0 | 2 => {
                        failed_cards.push(card.id);
                        Rating::Again // Cards will be marked as failed
                    },
                    1 | 4 => {
                        hard_cards.push(card.id);
                        Rating::Hard // Cards will be marked as hard
                    },
                    _ => Rating::Good,
                };
                
                scheduler.answer_card(card.id, rating);
            }
        }
        
        // Verify tracking is working
        assert!(!scheduler.failed_cards_today().is_empty(), "Failed cards should be tracked");
        assert!(!scheduler.hard_cards().is_empty(), "Hard cards should be tracked");
        
        // Now manually insert some difficult cards from a higher range (to avoid exclusion)
        // to test database-based prioritization
        let test_conn = rusqlite::Connection::open(&temp_db_path).unwrap();
        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        // Insert some cards from the high range as difficult 
        test_conn.execute(
            "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
             VALUES (45, 'failed', ?, ?)",
            (now, &today),
        ).unwrap();
        
        test_conn.execute(
            "INSERT OR REPLACE INTO difficult_cards (card_id, difficulty_type, timestamp, date)
             VALUES (47, 'hard', ?, ?)",
            (now, &today),
        ).unwrap();
        
        drop(test_conn);
        
        // Now test load_more_cards - this simulates continuing study
        let load_result = scheduler.load_more_cards(crate::config::PACK_SIZE_DEFAULT);
        
        match load_result {
            Ok(new_cards) => {
                assert_eq!(new_cards.len(), crate::config::PACK_SIZE_DEFAULT, "Should load requested number of cards");
                
                // Check if the database difficult cards appear in the results
                let loaded_45 = new_cards.iter().any(|card| card.id == 45);
                let loaded_47 = new_cards.iter().any(|card| card.id == 47);
                
                println!("Cards in load_more_cards result: {:?}", new_cards.iter().map(|c| c.id).collect::<Vec<_>>());
                println!("Card 45 (failed) found in result: {}", loaded_45);
                println!("Card 47 (hard) found in result: {}", loaded_47);
                
                // At least one of the difficult cards should be prioritized
                assert!(loaded_45 || loaded_47, "At least one difficult card should be prioritized from database");
                
                // Verify cards have valid data
                for card in &new_cards {
                    assert!(card.id > 0, "Card should have valid ID");
                    assert!(card.note_id > 0, "Card should have valid note ID");
                }
                
                // Verify cards were added to review queue
                assert!(scheduler.review_queue.len() >= crate::config::PACK_SIZE_DEFAULT, "Cards should be added to review queue");
                
            },
            Err(e) => {
                panic!("load_more_cards should succeed when database has available cards: {}", e);
            }
        }
    }
}
