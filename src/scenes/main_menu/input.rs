use sdl2::event::Event;
use sdl2::controller::Button as CtrlBtn;
use sdl2::joystick::HatState;
use sdl2::keyboard::Keycode;
use crate::state::{map_to_brick_input, BrickInput, BrickButton};

use crate::{AppState, GameState};


pub fn handle_main_menu_input(state: &mut AppState, event: Event) -> Result<(), String> {
    // Only run when we’re in the MainMenu state
    if let GameState::MainMenu(main_menu) = &mut state.game_state {
        // Your three menu options
        let options = ["Study", "Profile", "Quit"];

        if let Some(input) = map_to_brick_input(&event) {
            match input {
                BrickInput::ButtonDown(BrickButton::DPadDown) => {
                        main_menu.selected_index = (main_menu.selected_index + 1).min(options.len() - 1);
                }
                BrickInput::ButtonDown(BrickButton::DPadUp) => {
                        main_menu.selected_index = main_menu.selected_index.saturating_sub(1);
                },
                BrickInput::ButtonDown(BrickButton::A) => {
                    match main_menu.selected_index {
                        0 => state.game_state = GameState::GoToDeckSelection,
                        1 => { /* to Profile */ }
                        2 => return Err("User quit".into()),
                        _ => {}
                    }
                },
                _ => {}
            }
        } else {
            match event {
                // Keyboard
                Event::KeyDown { keycode: Some(key), repeat: false, .. } => {
                    match key {
                        Keycode::Up   => {
                            main_menu.selected_index = main_menu.selected_index.saturating_sub(1);
                        }
                        Keycode::Down => {
                            main_menu.selected_index = (main_menu.selected_index + 1).min(options.len() - 1);
                        }
                        Keycode::Return => {
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

                _ => {}
            }


        }
    }

    Ok(())
}
