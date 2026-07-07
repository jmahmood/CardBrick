// src/scenes/progress/input.rs
// Input handling for Progress viewer scene

use super::ProgressState;
use crate::scenes::main_menu::MainMenuState;
use crate::state::{AppState, BrickButton, BrickInput, GameState};

/// Handles input events for the progress viewer scene
pub fn handle_progress_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::Progress(progress_state) = &mut state.game_state {
        match input {
            // DPad Up: Scroll up
            BrickInput::ButtonDown(BrickButton::DPadUp) => {
                progress_state.scroll_offset = (progress_state.scroll_offset - 30).max(0);
            }

            // DPad Down: Scroll down
            BrickInput::ButtonDown(BrickButton::DPadDown) => {
                let max_scroll = calculate_max_scroll(progress_state);
                progress_state.scroll_offset = (progress_state.scroll_offset + 30).min(max_scroll);
            }

            // B Button: Return to main menu
            BrickInput::ButtonDown(BrickButton::B) => {
                state.game_state = GameState::MainMenu(MainMenuState::new());
            }

            // Back button: Return to main menu
            BrickInput::ButtonDown(BrickButton::Back) => {
                state.game_state = GameState::MainMenu(MainMenuState::new());
            }

            _ => {}
        }
    }
    Ok(())
}

/// Calculate maximum scroll offset based on content height
fn calculate_max_scroll(progress_state: &ProgressState) -> i32 {
    let viewport_height = 350; // Available height for scrolling content
    let header_height = progress_state
        .header_layout
        .as_ref()
        .map_or(0, |l| l.total_height as i32);
    let rows_height: i32 = progress_state
        .row_layouts
        .iter()
        .map(|layout| layout.total_height as i32 + 5) // 5px spacing between rows
        .sum();
    let total_height = header_height + rows_height + 100; // Extra padding

    (total_height - viewport_height).max(0)
}
