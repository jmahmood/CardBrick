// src/state.rs

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::config::Config;
use crate::deck::Deck;
use crate::scenes::deck_selection::DeckSelectionState;
use crate::scenes::main_menu::MainMenuState;
use crate::scenes::studying::StudyingState;
use crate::ui::font::TextLayout;
use crate::ui::{CanvasManager, FontManager, sprite::Sprite};

/// Holds metadata about a single deck, used for selection screens.
#[derive(Clone)]
pub struct DeckMetadata {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// Messages sent from the deck loading thread to the main thread.
pub enum LoaderMessage {
    Progress(f32),
    Complete(Result<Deck, String>),
}

/// Represents the current screen or state of the application.
pub enum GameState<'a> {
    MainMenu(MainMenuState),
    GoToDeckSelection,
    DeckSelection(DeckSelectionState),
    Loading {
        rx: Receiver<LoaderMessage>,
        loading_layout: TextLayout,
        progress: f32,
        deck_id_to_load: String,
    },
    Studying(StudyingState<'a>),
    Error(String),
}

/// The top-level state for the entire application.
pub struct AppState<'a> {
    pub game_state: GameState<'a>,
    pub available_decks: Vec<DeckMetadata>,
    pub canvas_manager: CanvasManager<'a>,
    pub font_manager: FontManager<'a, 'a>,
    pub small_font_manager: FontManager<'a, 'a>,
    pub hint_font_manager: FontManager<'a, 'a>,
    pub sprite: Sprite,
    pub config: Config,
}
