// src/storage/mod.rs
// This module handles all data persistence, including the database and replay log.

pub mod db;
pub mod replay_log;

// Re-export the main structs for easier access.
pub use self::db::DatabaseManager;
pub use self.replay_log::ReplayLogger;
