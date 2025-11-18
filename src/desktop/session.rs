use crate::desktop::deck::{KartaCard, KartaDeck};
use crate::scheduler::sm2::Rating;
use crate::storage::db::progress_path;
use chrono::Utc;
use rusqlite::Connection;
use std::collections::{HashMap, VecDeque};

// Daily card limits
const DEFAULT_NEW_CARDS_PER_DAY: usize = 10;
const DEFAULT_REVIEW_CARDS_PER_DAY: usize = 50;
const MAX_DAILY_CARDS: usize = 60;

/// Represents which face of the card is currently shown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFace {
    Front,
    Back,
}

/// Events that occurred during the session
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub card_id: String,
    pub rating: Rating,
    pub timestamp: i64,
}

/// Main session state for the desktop application
pub struct Session {
    deck_name: String,
    full_deck: Vec<KartaCard>, // All cards in the deck
    due_cards: Vec<KartaCard>, // Only cards that are due or new today
    current_card_index: usize,
    current_face: CardFace,
    history: VecDeque<SessionEvent>,
    max_history: usize,
    db_conn: Connection,
    card_ids_to_internal: HashMap<String, i64>,
    next_internal_id: i64,
}

impl Session {
    /// Create a new session from a KARTA deck
    pub fn new(deck: KartaDeck) -> anyhow::Result<Self> {
        let db_path = progress_path();
        let db_conn = Connection::open(&db_path)?;

        // Initialize database schema
        Self::init_db(&db_conn)?;

        let deck_name = deck.name.clone();

        // Build a mapping of card UUIDs to internal IDs
        let mut card_ids_to_internal = HashMap::new();
        let mut next_internal_id = 1i64;

        // First, get or assign internal IDs for all cards in the deck
        for card in &deck.cards {
            // Check if we already have an internal ID for this card
            let internal_id: i64 = db_conn
                .query_row(
                    "SELECT card_id FROM deck_card_mapping WHERE uuid = ?1",
                    [&card.id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| {
                    // Assign new internal ID
                    let id = next_internal_id;
                    next_internal_id += 1;

                    // Store the mapping
                    let _ = db_conn.execute(
                        "INSERT INTO deck_card_mapping (uuid, card_id) VALUES (?1, ?2)",
                        [&card.id, &id.to_string()],
                    );

                    id
                });

            card_ids_to_internal.insert(card.id.clone(), internal_id);
            next_internal_id = next_internal_id.max(internal_id + 1);
        }

        // Filter cards to only those that are due or new
        let due_cards = Self::get_due_cards(&deck, &card_ids_to_internal, &db_conn)?;

        println!("Session created: {} cards due out of {} total", due_cards.len(), deck.cards.len());

        let full_deck = deck.cards.clone();

        Ok(Session {
            deck_name,
            full_deck,
            due_cards,
            current_card_index: 0,
            current_face: CardFace::Front,
            history: VecDeque::new(),
            max_history: 10,
            db_conn,
            card_ids_to_internal,
            next_internal_id,
        })
    }

    /// Get cards that are due today or are new
    fn get_due_cards(
        deck: &KartaDeck,
        card_ids_to_internal: &HashMap<String, i64>,
        conn: &Connection,
    ) -> anyhow::Result<Vec<KartaCard>> {
        let now = Utc::now().timestamp();
        let mut new_cards = Vec::new();
        let mut review_cards = Vec::new();

        for card in &deck.cards {
            let internal_id = card_ids_to_internal.get(&card.id).copied().unwrap_or(0);

            // Check if card is in srs_log
            let srs_state: Option<i64> = conn
                .query_row(
                    "SELECT next_due_ts FROM srs_log WHERE card_id = ?1",
                    [internal_id],
                    |row| row.get(0),
                )
                .ok();

            match srs_state {
                None => {
                    // New card (not in srs_log)
                    if new_cards.len() < DEFAULT_NEW_CARDS_PER_DAY {
                        new_cards.push(card.clone());
                    }
                }
                Some(due_ts) => {
                    // Card exists in srs_log, check if due
                    if due_ts <= now && review_cards.len() < DEFAULT_REVIEW_CARDS_PER_DAY {
                        review_cards.push(card.clone());
                    }
                }
            }

            // Stop if we've hit the daily limit
            if new_cards.len() + review_cards.len() >= MAX_DAILY_CARDS {
                break;
            }
        }

        // Combine review cards (prioritized) with new cards
        let mut due_cards = review_cards;
        due_cards.extend(new_cards);

        Ok(due_cards)
    }

    fn init_db(conn: &Connection) -> anyhow::Result<()> {
        // Create minimal schema for desktop SRS tracking
        conn.execute(
            "CREATE TABLE IF NOT EXISTS srs_log (
                card_id INTEGER PRIMARY KEY,
                next_due_ts INTEGER,
                interval INTEGER,
                ease_factor REAL,
                lapses INTEGER,
                reps INTEGER
            )",
            [],
        )?;

        // Drop old daily_ratings table if it has the wrong schema
        let _ = conn.execute("DROP TABLE IF EXISTS daily_ratings", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_ratings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id INTEGER,
                rating TEXT,
                timestamp INTEGER,
                date TEXT
            )",
            [],
        )?;

        // Mapping from UUID strings to integer IDs for SM-2 scheduler
        conn.execute(
            "CREATE TABLE IF NOT EXISTS deck_card_mapping (
                uuid TEXT PRIMARY KEY,
                card_id INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    /// Get an internal card ID for use with the SM-2 scheduler
    /// Maps string UUIDs from KARTA to integer IDs expected by the scheduler
    fn get_internal_card_id(&mut self, card_id: &str) -> i64 {
        if let Some(&id) = self.card_ids_to_internal.get(card_id) {
            id
        } else {
            let id = self.next_internal_id;
            self.card_ids_to_internal.insert(card_id.to_string(), id);
            self.next_internal_id += 1;
            id
        }
    }

    /// Get the current card
    pub fn current_card(&self) -> Option<&KartaCard> {
        self.due_cards.get(self.current_card_index)
    }

    /// Get the current face
    pub fn current_face(&self) -> CardFace {
        self.current_face
    }

    /// Get deck name
    pub fn deck_name(&self) -> &str {
        &self.deck_name
    }

    /// Get progress (current index, total cards)
    pub fn progress(&self) -> (usize, usize) {
        (self.current_card_index + 1, self.due_cards.len())
    }

    /// Flip to the back of the current card
    pub fn flip_to_back(&mut self) {
        if self.current_face == CardFace::Front {
            self.current_face = CardFace::Back;
        }
    }

    /// Flip back to front (without grading)
    pub fn flip_to_front(&mut self) {
        if self.current_face == CardFace::Back {
            self.current_face = CardFace::Front;
        }
    }

    /// Grade the current card and move to the next
    pub fn grade_and_next(&mut self, rating: Rating) -> anyhow::Result<()> {
        // Clone card and card ID first to avoid borrowing issues
        let card = self.current_card().cloned();

        if let Some(card) = card {
            let internal_id = self.get_internal_card_id(&card.id);
            // Use microsecond precision to avoid timestamp collisions
            let timestamp = Utc::now().timestamp_micros();
            let timestamp_seconds = (timestamp / 1_000_000) as i64;

            // Record the event in history
            self.history.push_back(SessionEvent {
                card_id: card.id.clone(),
                rating,
                timestamp: timestamp_seconds,
            });

            // Limit history size
            if self.history.len() > self.max_history {
                self.history.pop_front();
            }

            // Apply SM-2 rating
            crate::scheduler::sm2::apply_rating_with_path(
                internal_id,
                rating,
                timestamp_seconds,
                &progress_path(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to apply rating: {}", e))?;

            // Record in daily ratings
            let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let rating_str = match rating {
                Rating::Again => "Again",
                Rating::Hard => "Hard",
                Rating::Good => "Good",
                Rating::Easy => "Easy",
            };

            self.db_conn.execute(
                "INSERT INTO daily_ratings (card_id, rating, timestamp, date) VALUES (?1, ?2, ?3, ?4)",
                [&internal_id.to_string(), rating_str, &timestamp.to_string(), &date],
            )?;

            // If rated "Again", add card back to deck for review
            if rating == Rating::Again {
                // Insert card 3 positions ahead (or at end if near the end)
                let insert_position = (self.current_card_index + 3).min(self.due_cards.len());
                self.due_cards.insert(insert_position, card);
            }
        }

        // Move to next card
        self.next_card();

        Ok(())
    }

    /// Move to the next card (manual navigation)
    pub fn next_card(&mut self) {
        if self.current_card_index < self.due_cards.len() {
            self.current_card_index += 1;
            self.current_face = CardFace::Front;
        }
    }

    /// Move to the previous card (manual navigation)
    pub fn prev_card(&mut self) {
        if self.current_card_index > 0 {
            self.current_card_index -= 1;
            self.current_face = CardFace::Front;
        }
    }

    /// Undo the last grading
    pub fn undo_last_grade(&mut self) -> anyhow::Result<bool> {
        if let Some(event) = self.history.pop_back() {
            // Find the card index for this event
            if let Some(index) = self.due_cards.iter().position(|c| c.id == event.card_id) {
                self.current_card_index = index;
                self.current_face = CardFace::Back;

                // TODO: Ideally we'd reverse the SM-2 rating here, but that's complex
                // For now, we just revert the UI state
                // The user can re-grade the card

                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if we're at the end of the deck
    pub fn is_complete(&self) -> bool {
        self.current_card_index >= self.due_cards.len()
    }

    /// Get today's ratings for progress visualization
    pub fn get_todays_ratings(&self) -> anyhow::Result<Vec<Rating>> {
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let mut stmt = self.db_conn.prepare(
            "SELECT rating FROM daily_ratings WHERE date = ?1 ORDER BY timestamp ASC"
        )?;

        let ratings: Vec<Rating> = stmt
            .query_map([&date], |row| {
                let rating_str: String = row.get(0)?;
                let rating = match rating_str.as_str() {
                    "Again" => Rating::Again,
                    "Hard" => Rating::Hard,
                    "Good" => Rating::Good,
                    "Easy" => Rating::Easy,
                    _ => Rating::Good,
                };
                Ok(rating)
            })?
            .filter_map(Result::ok)
            .collect();

        Ok(ratings)
    }

    /// Restart the current session (replay the same due cards)
    pub fn restart_session(&mut self) {
        self.current_card_index = 0;
        self.current_face = CardFace::Front;
        self.history.clear();
    }

    /// Continue studying beyond daily limit (add 10 more cards)
    pub fn continue_studying(&mut self) -> usize {
        let now = Utc::now().timestamp();
        // Clone the IDs to avoid borrow checker issues
        let current_card_ids: std::collections::HashSet<String> =
            self.due_cards.iter().map(|c| c.id.clone()).collect();

        let mut added = 0;
        for card in &self.full_deck {
            // Skip cards already in due_cards
            if current_card_ids.contains(&card.id) {
                continue;
            }

            // Check if this card has been studied
            let internal_id = self.card_ids_to_internal.get(&card.id).copied().unwrap_or(0);
            let srs_state: Option<i64> = self.db_conn
                .query_row(
                    "SELECT next_due_ts FROM srs_log WHERE card_id = ?1",
                    [internal_id],
                    |row| row.get(0),
                )
                .ok();

            // Add new cards or cards not yet due
            match srs_state {
                None => {
                    // New card - add it
                    self.due_cards.push(card.clone());
                    added += 1;
                }
                Some(due_ts) if due_ts > now => {
                    // Not yet due, but add anyway
                    self.due_cards.push(card.clone());
                    added += 1;
                }
                _ => {
                    // Skip cards that are overdue (should already be in due_cards)
                    continue;
                }
            }

            // Add 10 cards at a time
            if added >= 10 {
                break;
            }
        }

        added
    }

    /// Get statistics about remaining cards
    pub fn remaining_cards_info(&self) -> (usize, usize) {
        let available = self.full_deck.len() - self.due_cards.len();
        (available, self.full_deck.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_deck() -> KartaDeck {
        KartaDeck {
            id: "test-deck".to_string(),
            name: "Test Deck".to_string(),
            description: None,
            fields: vec!["front".to_string(), "back".to_string()],
            cards: vec![
                KartaCard {
                    id: "card-1".to_string(),
                    fields: HashMap::from([
                        ("front".to_string(), "test".to_string()),
                        ("back".to_string(), "definition".to_string()),
                    ]),
                    tags: vec![],
                    created_at: Utc::now(),
                },
                KartaCard {
                    id: "card-2".to_string(),
                    fields: HashMap::from([
                        ("front".to_string(), "another".to_string()),
                        ("back".to_string(), "another def".to_string()),
                    ]),
                    tags: vec![],
                    created_at: Utc::now(),
                },
            ],
            media: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_session_creation() {
        let deck = create_test_deck();
        let session = Session::new(deck).unwrap();

        assert_eq!(session.current_face(), CardFace::Front);
        // All cards should be new/due in test
        assert!(session.progress().1 > 0);
    }

    #[test]
    fn test_flip_card() {
        let deck = create_test_deck();
        let mut session = Session::new(deck).unwrap();

        assert_eq!(session.current_face(), CardFace::Front);
        session.flip_to_back();
        assert_eq!(session.current_face(), CardFace::Back);
        session.flip_to_front();
        assert_eq!(session.current_face(), CardFace::Front);
    }

    #[test]
    fn test_navigation() {
        let deck = create_test_deck();
        let mut session = Session::new(deck).unwrap();

        let (start_idx, total) = session.progress();
        assert_eq!(start_idx, 1);
        assert!(total > 0);

        if total > 1 {
            session.next_card();
            assert_eq!(session.progress().0, 2);
            session.prev_card();
            assert_eq!(session.progress().0, 1);
        }
    }
}
