// src/storage/replay_log.rs
// Manages the plain-text transaction log for recovery purposes.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::deck::Card;
use crate::scheduler::Rating;

pub struct ReplayLogger {
    log_path: PathBuf,
}

impl ReplayLogger {
    /// Creates a new logger for a specific deck.
    pub fn new(deck_id: &str) -> Result<Self, std::io::Error> {
        let path = Path::new("anki/history/txn");
        fs::create_dir_all(path)?;
        
        // For simplicity, we'll use one log file per deck for now.
        let log_path = path.join(format!("{}.log", deck_id));
        
        Ok(ReplayLogger { log_path })
    }

    /// Logs a single review action to the text file.
    pub fn log_action(&self, card: &Card, rating: Rating) -> Result<(), std::io::Error> {
        // Open the file in append mode, creating it if it doesn't exist.
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        // Format: timestamp_iso,card_id,event,ease,old_ivl,new_ivl
        // This is a simplified version of the spec for now.
        let timestamp = chrono::Utc::now().to_rfc3339();
        let log_entry = format!(
            "{},{},{:?},{},{}\n",
            timestamp, card.id, rating, card.ease_factor, card.interval
        );

        file.write_all(log_entry.as_bytes())?;
        Ok(())
    }
}
