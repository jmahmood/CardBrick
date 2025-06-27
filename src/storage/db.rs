// src/storage/db.rs
// Manages the SQLite database for storing card states.

use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;

use crate::deck::Card;

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    /// Creates a new DatabaseManager and opens a connection to the database file.
    pub fn new(deck_id: &str) -> Result<Self> {
        let path = Path::new("anki/history");
        fs::create_dir_all(path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let db_path = path.join(format!("{}.db", deck_id));
        
        let conn = Connection::open(db_path)?;
        let manager = DatabaseManager { conn };
        manager.init_schema()?;
        
        Ok(manager)
    }

    /// Creates the necessary tables if they don't already exist.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS card_state (
                id              INTEGER PRIMARY KEY,
                due             INTEGER NOT NULL,
                interval        INTEGER NOT NULL,
                ease_factor     INTEGER NOT NULL,
                lapses          INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// Updates the state of a single card in the database.
    /// Uses `INSERT OR REPLACE` to handle both new and existing cards.
    pub fn update_card_state(&self, card: &Card) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO card_state (id, due, interval, ease_factor, lapses)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                card.id,
                card.due,
                card.interval,
                card.ease_factor,
                card.lapses,
            ),
        )?;
        Ok(())
    }
}
