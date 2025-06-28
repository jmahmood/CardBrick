// src/scheduler.rs
// Contains the logic for the spaced repetition system.

use crate::deck::{Card, Deck, Note};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;

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
    fn answer_card(&mut self, card_id: i64, rating: Rating);
    fn get_note(&self, note_id: i64) -> Option<&Note>;
    fn reviews_complete(&self) -> usize;
    fn total_session_cards(&self) -> usize;
    fn hard_cards(&self) -> &[i64];
    fn rewind_last_answer(&mut self) -> Option<Card>;
    fn add_card_to_front(&mut self, card_id: i64);
}

/// Implementation of the SM-2 algorithm.
pub struct Sm2Scheduler {
    cards: HashMap<i64, Card>,
    notes: HashMap<i64, Note>,
    review_queue: Vec<i64>,
    session_total: usize,
    session_reviews_complete: usize,
    hard_cards_this_session: Vec<i64>,
    last_answer: Option<(i64, Rating, Card)>, // Store a clone of the card state before modification
}

impl Scheduler for Sm2Scheduler {
    fn new(deck: Deck) -> Self {
        let cards_map: HashMap<i64, Card> = deck.cards.into_iter().map(|c| (c.id, c)).collect();
        let mut review_queue: Vec<i64> = cards_map.keys().cloned().collect();
        
        if cfg!(test) {
            // Sort ascending for predictable test order. .pop() will take from the end.
            review_queue.sort_unstable(); 
        } else {
            review_queue.shuffle(&mut thread_rng());
        }
        
        let session_total = review_queue.len();

        Sm2Scheduler {
            cards: cards_map,
            notes: deck.notes,
            review_queue,
            session_total,
            session_reviews_complete: 0,
            hard_cards_this_session: Vec::new(),
            last_answer: None,
        }
    }

    fn next_card(&mut self) -> Option<Card> {
        self.review_queue.pop().and_then(|id| self.cards.get(&id).cloned())
    }
    
    fn add_card_to_front(&mut self, card_id: i64) {
        // Pushing to the end of the vec makes it the next item for .pop()
        self.review_queue.push(card_id);
    }

    fn answer_card(&mut self, card_id: i64, rating: Rating) {
        let card = self.cards.get_mut(&card_id).unwrap();
        
        self.last_answer = Some((card_id, rating, card.clone()));

        match rating {
            Rating::Again => {
                card.lapses += 1;
                card.ease_factor = (card.ease_factor as i32 - 200).max(1300) as u32;
                card.interval = 0;
                
                let cooldown_distance = 5_u32.saturating_sub(card.lapses).max(2) as usize;
                let insertion_point = self.review_queue.len().saturating_sub(cooldown_distance);
                self.review_queue.insert(insertion_point, card.id);
            }
            _ => { // Hard, Good, or Easy
                self.session_reviews_complete += 1;
               
                match rating {
                    Rating::Good => { /* ... interval logic ... */ }
                    Rating::Hard => {
                        if !self.hard_cards_this_session.contains(&card_id) {
                            self.hard_cards_this_session.push(card_id);
                        }
                        card.ease_factor = (card.ease_factor as i32 - 150).max(1300) as u32;
                        let new_interval = (card.interval as f32 * 1.2).round() as u32;
                        card.interval = new_interval.max(card.interval + 1);
                    }
                    Rating::Easy => { /* ... interval logic ... */ }
                    Rating::Again => {} // Already handled
                }
            }
        }
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
            self.cards.insert(card_id, original_card_state);
            
            return self.cards.get(&card_id).cloned();
        }
        None
    }

    fn get_note(&self, note_id: i64) -> Option<&Note> { self.notes.get(&note_id) }
    fn reviews_complete(&self) -> usize { self.session_reviews_complete }
    fn total_session_cards(&self) -> usize { self.session_total }
    fn hard_cards(&self) -> &[i64] { &self.hard_cards_this_session }
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
        Deck { cards, notes }
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
