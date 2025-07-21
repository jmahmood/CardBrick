// src/scheduler.rs
// Contains the logic for the spaced repetition system.

use crate::deck::{Card, Deck, Note};
use crate::scheduler::queue;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use chrono::Utc;


/// Represents the user's rating for a card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
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
                .take(queue::PACK_SIZE_DEFAULT) // Limit to default pack size
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
            if self.deck.db_connection.is_some() || self.deck.cached_db_connection.is_some() {
                if let Ok(new_cards) = self.deck.load_more_cards(50, &[], &[]) {
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

        match rating {
            Rating::Again => {
                card.lapses += 1;
                card.ease_factor = (card.ease_factor as i32 - 200).max(1300) as u32;
                card.interval = 0;
                
                // Track this card as failed today
                if !self.failed_cards_today.contains(&card_id) {
                    self.failed_cards_today.push(card_id);
                }
                
                let cooldown_distance = 5_u32.saturating_sub(card.lapses).max(2) as usize;
                let insertion_point = self.review_queue.len().saturating_sub(cooldown_distance);
                self.review_queue.insert(insertion_point, card.id);
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
                
               let ease_factor_multiplier = card.ease_factor as f32 / 1000.0;

                match rating {
                    Rating::Good => { 
                        let new_interval = if card.interval == 0 {
                            1
                        } else {
                            (card.interval as f32 * ease_factor_multiplier).round() as u32
                        };
                        // The new interval should be at least one day longer than the previous one.
                        card.interval = new_interval.max(card.interval + 1);
                    }
                    Rating::Hard => {
                        if !self.hard_cards_this_session.contains(&card_id) {
                            self.hard_cards_this_session.push(card_id);
                        }
                        card.ease_factor = (card.ease_factor as i32 - 150).max(1300) as u32;
                        // "Hard" interval multiplier is 1.2
                        let new_interval = (card.interval as f32 * 1.2).round() as u32;
                        card.interval = new_interval.max(card.interval + 1);
                    }
                    Rating::Easy => {
                    // Increase ease by 150 (15%)
                    card.ease_factor = (card.ease_factor as i32 + 150) as u32;
                    // "Easy" bonus multiplier is typically 1.3
                    let easy_bonus = 1.3;
                    let new_interval = if card.interval == 0 {
                        // A common default for the first "Easy" rating is 4 days
                        4
                    } else {
                        (card.interval as f32 * ease_factor_multiplier * easy_bonus).round() as u32
                    };
                    card.interval = new_interval.max(card.interval + 1);
                }
                    Rating::Again => {} // Already handled
                }
            }
        }
        Some(card.clone()) // Return a clone of the modified card
    }

    fn rewind_last_answer(&mut self) -> Option<Card> {
        if let Some((card_id, rating, original_card_state)) = self.last_answer.take() {
            println!("Rewinding last answer for card #{}", card_id);
            
            // Remove the card from wherever it was re-inserted in the queue.
            self.review_queue.retain(|&id| id != card_id);
            
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
        // Load more cards from the deck, passing today's failed and hard cards for prioritization
        let new_cards = self.deck.load_more_cards(count, &self.failed_cards_today, &self.hard_cards_this_session)?;
        
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
            db_connection: None,
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
}
