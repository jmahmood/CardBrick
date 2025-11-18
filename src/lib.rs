// CardBrick Library
// Shared modules for both handheld and desktop versions

pub mod config;
pub mod deck;
pub mod scheduler;
pub mod storage;
pub mod debug;
pub mod state;
pub mod ui;
pub mod scenes;
pub mod perf;

// Desktop-specific modules
pub mod desktop;

// Re-exports for convenience
pub use deck::{Card, Deck, Note};
pub use scheduler::sm2::Rating;
pub use storage::db;
pub use state::{AppState, GameState, DeckMetadata, LoaderMessage, BackgroundFontMessage};
pub use config::assets;
