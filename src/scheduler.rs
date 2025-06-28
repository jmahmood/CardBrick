// src/scheduler.rs
// Contains the logic for the spaced repetition system.

use crate::deck::{Card, Deck, Note};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;

/// Represents the user's rating for a card.
#[derive(Debug, Clone, Copy)]
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
    // New method to get the list of hard cards from the session.
    fn hard_cards(&self) -> &[i64];
}

/// Implementation of the SM-2 algorithm.
pub struct Sm2Scheduler {
    cards: HashMap<i64, Card>,
    notes: HashMap<i64, Note>,
    review_queue: Vec<i64>,
    session_total: usize,
    session_reviews_complete: usize,
    // A new vector to track cards rated as "Hard" during the session.
    hard_cards_this_session: Vec<i64>,
}

impl Scheduler for Sm2Scheduler {
    fn new(deck: Deck) -> Self {
        let cards_map: HashMap<i64, Card> = deck.cards.into_iter().map(|c| (c.id, c)).collect();
        let mut review_queue: Vec<i64> = cards_map.keys().cloned().collect();
        review_queue.shuffle(&mut thread_rng());
        let session_total = review_queue.len();

        Sm2Scheduler {
            cards: cards_map,
            notes: deck.notes,
            review_queue,
            session_total,
            session_reviews_complete: 0,
            hard_cards_this_session: Vec::new(), // Initialize the new vector.
        }
    }

    fn next_card(&mut self) -> Option<Card> {
        // Get the next card from the end of the queue.
        let card_id = self.review_queue.pop()?;
        self.cards.get(&card_id).cloned()
    }

    fn answer_card(&mut self, card_id: i64, rating: Rating) {
        let card = match self.cards.get_mut(&card_id) {
            Some(c) => c,
            None => return,
        };
        
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
                    Rating::Good => {
                        if card.interval == 0 { card.interval = 1; } 
                        else {
                            let new_interval = (card.interval as f32 * (card.ease_factor as f32 / 1000.0)).round() as u32;
                            card.interval = new_interval.max(card.interval + 1);
                        }
                    }
                    Rating::Hard => {
                        // Add the card to our tracking list if it's not already there.
                        if !self.hard_cards_this_session.contains(&card_id) {
                            self.hard_cards_this_session.push(card_id);
                        }
                        
                        card.ease_factor = (card.ease_factor as i32 - 150).max(1300) as u32;
                        let new_interval = (card.interval as f32 * 1.2).round() as u32;
                        card.interval = new_interval.max(card.interval + 1);
                    }
                    Rating::Easy => {
                        card.ease_factor += 150;
                        let new_interval = (card.interval as f32 * (card.ease_factor as f32 / 1000.0) * 1.3).round() as u32;
                        card.interval = new_interval.max(card.interval + 1);
                    }
                    Rating::Again => {} // Already handled
                }
            }
        }
    }

    fn get_note(&self, note_id: i64) -> Option<&Note> {
        self.notes.get(&note_id)
    }

    fn reviews_complete(&self) -> usize {
        self.session_reviews_complete
    }

    fn total_session_cards(&self) -> usize {
        self.session_total
    }

    /// Returns a slice of the card IDs that were marked as "Hard".
    fn hard_cards(&self) -> &[i64] {
        &self.hard_cards_this_session
    }
}
