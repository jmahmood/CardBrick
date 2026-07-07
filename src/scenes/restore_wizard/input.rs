use super::{RestoreStep, RestoreWizardState};
use crate::state::{BrickButton, BrickInput};
use crate::{AppState, GameState};
use std::process::Command;

pub fn handle_restore_wizard_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::RestoreWizard(wizard_state) = &mut state.game_state {
        let step = wizard_state.step.clone();
        match step {
            RestoreStep::Welcome => handle_welcome_input(state, input),
            RestoreStep::EnterCode => handle_code_entry_input(state, input),
            RestoreStep::Downloading => {
                // No input during download
                Ok(())
            }
            RestoreStep::Complete => handle_complete_input(state, input),
            RestoreStep::Error(_) => handle_error_input(state, input),
        }
    } else {
        Ok(())
    }
}

fn handle_welcome_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    match input {
        BrickInput::ButtonDown(BrickButton::A) => {
            // User wants to restore backup
            if let GameState::RestoreWizard(wizard_state) = &mut state.game_state {
                wizard_state.step = RestoreStep::EnterCode;
            }
            let _ = state.audio.play_sound(crate::assets::OPEN_SOUND);
        }
        BrickInput::ButtonDown(BrickButton::B) => {
            // User wants to skip restore
            state.game_state = GameState::MainMenu(crate::scenes::main_menu::MainMenuState::new());
            let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
        }
        _ => {}
    }
    Ok(())
}

fn handle_code_entry_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::RestoreWizard(wizard_state) = &mut state.game_state {
        match input {
            BrickInput::ButtonDown(BrickButton::A) => {
                // Confirm and start download
                if !wizard_state.wormhole_code.trim().is_empty() {
                    start_restore_download(wizard_state)?;
                }
            }
            BrickInput::ButtonDown(BrickButton::B) => {
                // Cancel and go back
                wizard_state.step = RestoreStep::Welcome;
                wizard_state.wormhole_code.clear();
            }
            // Text input would need to be handled differently since BrickInput::Text doesn't exist
            // For now, users will need to input the code through other means
            BrickInput::ButtonDown(BrickButton::DPadLeft) => {
                // Backspace
                wizard_state.wormhole_code.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_complete_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    match input {
        BrickInput::ButtonDown(BrickButton::A) => {
            // Continue to main menu (app restart would happen here)
            state.game_state = GameState::MainMenu(crate::scenes::main_menu::MainMenuState::new());
            let _ = state.audio.play_sound(crate::assets::OPEN_SOUND);
        }
        _ => {}
    }
    Ok(())
}

fn handle_error_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    match input {
        BrickInput::ButtonDown(BrickButton::B) => {
            // Back to main menu
            state.game_state = GameState::MainMenu(crate::scenes::main_menu::MainMenuState::new());
            let _ = state.audio.play_sound(crate::assets::CLICK_SOUND);
        }
        BrickInput::ButtonDown(BrickButton::A) => {
            // Try again - go back to code entry
            if let GameState::RestoreWizard(wizard_state) = &mut state.game_state {
                wizard_state.step = RestoreStep::EnterCode;
                wizard_state.wormhole_code.clear();
                wizard_state.error_message = None;
            }
        }
        _ => {}
    }
    Ok(())
}

fn start_restore_download(wizard_state: &mut RestoreWizardState) -> Result<(), String> {
    log::info!(
        "Starting wormhole restore with code: {}",
        wizard_state.wormhole_code
    );

    wizard_state.step = RestoreStep::Downloading;
    wizard_state.download_progress = 0.0;

    // In a real implementation, this would start an async process
    // For now, we'll simulate it with a background command
    let code = wizard_state.wormhole_code.clone();

    std::thread::spawn(move || {
        // Simulate calling the CLI tool
        let result = Command::new("cardbrick-cli")
            .arg("receive-backup")
            .arg(&code)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    log::info!("Wormhole restore completed successfully");
                    // In reality, we'd need to communicate this back to the main thread
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    log::error!("Wormhole restore failed: {}", error);
                }
            }
            Err(e) => {
                log::error!("Failed to start wormhole restore: {}", e);
            }
        }
    });

    // For demo purposes, we'll simulate progress
    // In a real implementation, this would be driven by actual download progress
    simulate_download_progress(wizard_state);

    Ok(())
}

fn simulate_download_progress(wizard_state: &mut RestoreWizardState) {
    // This is a simplified simulation
    // In reality, you'd want proper async communication between threads
    wizard_state.download_progress = 0.5; // 50% for demo

    // After some time, mark as complete
    // (In real code, this would be event-driven)
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3));
        // Would send completion message to main thread here
    });
}
