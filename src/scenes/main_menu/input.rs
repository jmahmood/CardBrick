use crate::state::{BrickInput, BrickButton};
use crate::{AppState, GameState};

pub fn handle_main_menu_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    // Only run when we're in the MainMenu state
    if let GameState::MainMenu(main_menu) = &mut state.game_state {
        // Your three menu options
        let options = ["Study", "Profile", "Quit"];

        match input {
            BrickInput::ButtonDown(BrickButton::DPadDown) => {
                main_menu.selected_index = (main_menu.selected_index + 1).min(options.len() - 1);
                let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
            }
            BrickInput::ButtonDown(BrickButton::DPadUp) => {
                main_menu.selected_index = main_menu.selected_index.saturating_sub(1);
                let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
            }
            BrickInput::ButtonDown(BrickButton::A) => {
                let _ = state.audio.play_sound(crate::assets::OPEN_SOUND);
                match main_menu.selected_index {
                    0 => state.game_state = GameState::GoToDeckSelection,
                    1 => { /* to Profile */ }
                    2 => return Err("User quit".into()),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}
