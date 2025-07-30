use crate::state::{BrickInput, BrickButton};
use crate::{AppState, GameState};
use std::process::Command;

pub fn handle_main_menu_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    // Only run when we're in the MainMenu state
    if let GameState::MainMenu(main_menu) = &mut state.game_state {
        // Your menu options
        let options = ["Study", "Progress", "Send Backup", "Quit"];

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
                    1 => state.game_state = GameState::Progress(crate::scenes::progress::ProgressState::new()),
                    2 => {
                        // Trigger manual backup sync
                        if let Err(e) = trigger_manual_backup() {
                            log::warn!("Manual backup failed: {}", e);
                        }
                    }
                    3 => return Err("User quit".into()),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Trigger a manual backup sync operation
fn trigger_manual_backup() -> Result<(), String> {
    log::info!("Starting manual backup sync...");
    
    // Use the sync_ring binary to trigger backup
    let result = Command::new("sync_ring")
        .arg("--force-backup")
        .output();
        
    match result {
        Ok(output) => {
            if output.status.success() {
                log::info!("Manual backup completed successfully");
                Ok(())
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                log::error!("Manual backup failed: {}", error);
                Err(format!("Backup failed: {}", error))
            }
        }
        Err(e) => {
            log::error!("Failed to start backup process: {}", e);
            Err(format!("Failed to start backup: {}", e))
        }
    }
}
