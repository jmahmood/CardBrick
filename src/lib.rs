#![allow(dead_code)]

// CardBrick Library
// Shared modules for both handheld and desktop versions

pub mod config;
pub mod debug;
pub mod deck;
pub mod perf;
pub mod scenes;
pub mod scheduler;
pub mod state;
pub mod storage;
#[cfg(test)]
pub mod testing;
pub mod ui;

// Desktop-specific modules
pub mod desktop;

// Re-exports for convenience
pub use config::assets;
pub use deck::{Card, Deck, Note};
pub use scheduler::sm2::Rating;
pub use state::{AppState, BackgroundFontMessage, DeckMetadata, GameState, LoaderMessage};
pub use storage::db;
