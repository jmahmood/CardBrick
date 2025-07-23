use std::sync::mpsc;
use std::thread;

use crate::deck::html_parser;
use crate::scenes::main_menu::MainMenuState;
use crate::state::{BrickInput, BrickButton};
use crate::{AppState, GameState};

/// Handles input events for the deck selection scene.
pub fn handle_deck_selection_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {

    if let GameState::DeckSelection(deck_selection_state) = &mut state.game_state {
        // Calculate visible items dynamically from layout constants
        let available_height = 384.0f32 - 90.0f32 - 40.0f32; // LOGICAL_HEIGHT - LIST_START_Y - FOOTER_AREA_HEIGHT
        let visible_items = (available_height / 26.0f32).floor() as usize; // available_height / LIST_ITEM_HEIGHT
        let total_decks = deck_selection_state.decks.len();

        match input {
            BrickInput::ButtonDown(BrickButton::DPadUp) => {
                if !deck_selection_state.decks.is_empty() {
                    if deck_selection_state.index_changes(-1, total_decks) {
                        let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
                    }
                    deck_selection_state.move_selection(-1, total_decks, visible_items);
                }
            },
            BrickInput::ButtonDown(BrickButton::DPadDown) => {
                // Ensure we don't go out of bounds if there are decks.
                if !deck_selection_state.decks.is_empty() {
                    if deck_selection_state.index_changes(1, total_decks) {
                        let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
                    }

                    deck_selection_state.move_selection(1, total_decks, visible_items);

                }
            }
            BrickInput::ButtonDown(BrickButton::A) => {
                if !deck_selection_state.decks.is_empty() {
                    let _ = state.audio.play_sound(crate::assets::OPEN_SOUND);
                    let selected_deck = &deck_selection_state.decks[deck_selection_state.selected_index];
                    let deck_path = selected_deck.path.clone();
                    let deck_id = selected_deck.id.clone();
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || { crate::deck::loader::load_apkg(&deck_path, tx); });
                    let loading_spans = html_parser::parse_html_to_spans("Loading Deck...");
                    let loading_layout = state.font_manager.layout_text_binary(&loading_spans, 400, false)?;
                    state.game_state = GameState::Loading { rx, loading_layout, progress: 0.0, deck_id_to_load: deck_id };
                }
            },
            BrickInput::ButtonDown(BrickButton::Back) => state.game_state = GameState::MainMenu(MainMenuState::new()),
            _ => {}
        }
    }
    Ok(())
}
