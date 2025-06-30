// src/scenes/main_menu/input.rs

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::{AppState, GameState};
use crate::deck::html_parser;

/// Handles input events for the main menu.
/// This function was moved from main.rs.
pub fn handle_main_menu_input(state: &mut AppState, event: Event) -> Result<(), String> {
    if let Event::KeyDown { keycode: Some(keycode), repeat: false, .. } = event {
        if let GameState::MainMenu(main_menu_state) = &mut state.game_state {
            let options = ["Study", "Profile", "Quit"];
            match keycode {
                Keycode::Up => main_menu_state.selected_index = main_menu_state.selected_index.saturating_sub(1),
                Keycode::Down => main_menu_state.selected_index = (main_menu_state.selected_index + 1).min(options.len() - 1),
                Keycode::Return => {
                    match main_menu_state.selected_index {
                        0 => { // Study
                            // Pre-calculate layouts on state transition
                            let max_width = state.config.window_width - 40;
                            let layouts = state.available_decks.iter().map(|deck| {
                                let spans = html_parser::parse_html_to_spans(&deck.name);
                                state.small_font_manager.layout_text_binary(&spans, max_width, false)
                            }).collect::<Result<Vec<_>, _>>()?;

                            state.game_state = GameState::DeckSelection {
                                decks: state.available_decks.clone(),
                                deck_layouts: layouts,
                                selected_index: 0,
                            };
                        }
                        1 => { /* Go to Profile state */ }
                        2 => return Err("User quit".to_string()),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
