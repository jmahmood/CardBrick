use std::sync::mpsc;
use std::thread;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::deck::html_parser;
use crate::scenes::main_menu::MainMenuState;
use crate::{AppState, GameState};

/// Handles input events for the deck selection scene.
pub fn handle_deck_selection_input(state: &mut AppState, event: Event) -> Result<(), String> {
    if let Event::KeyDown { keycode: Some(keycode), repeat: false, .. } = event {
        if let GameState::DeckSelection(deck_selection_state) = &mut state.game_state {
            match keycode {
                Keycode::Up => {
                    deck_selection_state.selected_index = deck_selection_state.selected_index.saturating_sub(1);
                }
                Keycode::Down => {
                    // Ensure we don't go out of bounds if there are decks.
                    if !deck_selection_state.decks.is_empty() {
                        deck_selection_state.selected_index = (deck_selection_state.selected_index + 1).min(deck_selection_state.decks.len() - 1);
                    }
                }
                Keycode::Backspace => {
                    state.game_state = GameState::MainMenu(MainMenuState::new());
                }
                Keycode::Return => {
                    // Guard against crashing if Enter is pressed when the deck list is empty.
                    if !deck_selection_state.decks.is_empty() {
                        let selected_deck = &deck_selection_state.decks[deck_selection_state.selected_index];
                        let deck_path = selected_deck.path.clone();
                        let deck_id = selected_deck.id.clone();
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || { crate::deck::loader::load_apkg(&deck_path, tx); });
                        let loading_spans = html_parser::parse_html_to_spans("Loading Deck...");
                        let loading_layout = state.font_manager.layout_text_binary(&loading_spans, 400, false)?;
                        state.game_state = GameState::Loading { rx, loading_layout, progress: 0.0, deck_id_to_load: deck_id };
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
