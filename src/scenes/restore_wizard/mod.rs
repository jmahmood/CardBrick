// src/scenes/restore_wizard/mod.rs

use macroquad::prelude::*;
use crate::ui::{FontManager, CanvasManager};

pub mod input;

#[derive(Debug, Clone)]
pub enum RestoreStep {
    Welcome,
    EnterCode,
    Downloading,
    Complete,
    Error(String),
}

pub struct RestoreWizardState {
    pub step: RestoreStep,
    pub wormhole_code: String,
    pub cursor_pos: usize,
    pub download_progress: f32,
    pub error_message: Option<String>,
}

impl RestoreWizardState {
    pub fn new() -> Self {
        Self {
            step: RestoreStep::Welcome,
            wormhole_code: String::new(),
            cursor_pos: 0,
            download_progress: 0.0,
            error_message: None,
        }
    }
}

pub fn draw_restore_wizard_scene(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &RestoreWizardState,
) -> Result<(), String> {
    // Draw background
    let (logical_width, logical_height) = canvas_manager.logical_size();
    let (screen_x, screen_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let screen_w = logical_width * canvas_manager.get_scale_factor();
    let screen_h = logical_height * canvas_manager.get_scale_factor();
    
    draw_rectangle(screen_x, screen_y, screen_w, screen_h, Color::from_rgba(30, 30, 35, 255));
    
    match &state.step {
        RestoreStep::Welcome => {
            draw_welcome_screen(font_manager, canvas_manager)?;
        }
        RestoreStep::EnterCode => {
            draw_code_entry_screen(font_manager, canvas_manager, state)?;
        }
        RestoreStep::Downloading => {
            draw_download_screen(font_manager, canvas_manager, state)?;
        }
        RestoreStep::Complete => {
            draw_complete_screen(font_manager, canvas_manager)?;
        }
        RestoreStep::Error(error) => {
            draw_error_screen(font_manager, canvas_manager, error)?;
        }
    }
    
    Ok(())
}

fn draw_welcome_screen(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
) -> Result<(), String> {
    font_manager.draw_single_line("CardBrick Restore", 20, 50, canvas_manager)?;
    font_manager.draw_single_line("Empty device detected", 20, 100, canvas_manager)?;
    font_manager.draw_single_line("", 20, 130, canvas_manager)?;
    font_manager.draw_single_line("Would you like to restore", 20, 160, canvas_manager)?;
    font_manager.draw_single_line("from a backup?", 20, 190, canvas_manager)?;
    font_manager.draw_single_line("", 20, 220, canvas_manager)?;
    font_manager.draw_single_line("A - Yes, restore backup", 20, 250, canvas_manager)?;
    font_manager.draw_single_line("B - Skip, start fresh", 20, 280, canvas_manager)?;
    
    Ok(())
}

fn draw_code_entry_screen(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &RestoreWizardState,
) -> Result<(), String> {
    font_manager.draw_single_line("Enter Wormhole Code", 20, 50, canvas_manager)?;
    font_manager.draw_single_line("", 20, 80, canvas_manager)?;
    font_manager.draw_single_line("Enter the 6-word code from", 20, 110, canvas_manager)?;
    font_manager.draw_single_line("the sending device:", 20, 140, canvas_manager)?;
    font_manager.draw_single_line("", 20, 170, canvas_manager)?;
    
    // Draw code input box
    let code_display = if state.wormhole_code.is_empty() {
        "_".to_string()
    } else {
        format!("{}_", state.wormhole_code)
    };
    
    // Highlight the input area
    let (text_w, text_h) = font_manager.size_of_text(&code_display)?;
    let (highlight_x, highlight_y) = canvas_manager.logical_to_screen(18.0, 200.0);
    let highlight_w = (text_w + 10) as f32 * canvas_manager.get_scale_factor();
    let highlight_h = text_h as f32 * canvas_manager.get_scale_factor();
    
    draw_rectangle(highlight_x, highlight_y - (text_h as f32), highlight_w, highlight_h, Color::from_rgba(60, 60, 65, 255));
    
    font_manager.draw_single_line(&code_display, 20, 200, canvas_manager)?;
    
    font_manager.draw_single_line("", 20, 230, canvas_manager)?;
    font_manager.draw_single_line("A - Confirm", 20, 260, canvas_manager)?;
    font_manager.draw_single_line("B - Cancel", 20, 290, canvas_manager)?;
    
    Ok(())
}

fn draw_download_screen(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &RestoreWizardState,
) -> Result<(), String> {
    font_manager.draw_single_line("Downloading Backup", 20, 50, canvas_manager)?;
    font_manager.draw_single_line("", 20, 80, canvas_manager)?;
    font_manager.draw_single_line("Please wait while your", 20, 110, canvas_manager)?;
    font_manager.draw_single_line("backup is downloaded...", 20, 140, canvas_manager)?;
    font_manager.draw_single_line("", 20, 170, canvas_manager)?;
    
    // Draw progress bar
    let progress_width = 200.0;
    let progress_height = 20.0;
    let progress_x = 20.0;
    let progress_y = 200.0;
    
    // Background
    let (bg_x, bg_y) = canvas_manager.logical_to_screen(progress_x, progress_y);
    let bg_w = progress_width * canvas_manager.get_scale_factor();
    let bg_h = progress_height * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(60, 60, 65, 255));
    
    // Progress fill
    let fill_w = (progress_width * state.download_progress) * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, fill_w, bg_h, Color::from_rgba(0, 150, 0, 255));
    
    // Progress text
    let progress_text = format!("{}%", (state.download_progress * 100.0) as i32);
    font_manager.draw_single_line(&progress_text, 20, 240, canvas_manager)?;
    
    Ok(())
}

fn draw_complete_screen(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
) -> Result<(), String> {
    font_manager.draw_single_line("Restore Complete!", 20, 50, canvas_manager)?;
    font_manager.draw_single_line("", 20, 80, canvas_manager)?;
    font_manager.draw_single_line("Your backup has been", 20, 110, canvas_manager)?;
    font_manager.draw_single_line("successfully restored.", 20, 140, canvas_manager)?;
    font_manager.draw_single_line("", 20, 170, canvas_manager)?;
    font_manager.draw_single_line("The device will restart", 20, 200, canvas_manager)?;
    font_manager.draw_single_line("with your restored data.", 20, 230, canvas_manager)?;
    font_manager.draw_single_line("", 20, 260, canvas_manager)?;
    font_manager.draw_single_line("Press A to continue", 20, 290, canvas_manager)?;
    
    Ok(())
}

fn draw_error_screen(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    error: &str,
) -> Result<(), String> {
    font_manager.draw_single_line("Restore Failed", 20, 50, canvas_manager)?;
    font_manager.draw_single_line("", 20, 80, canvas_manager)?;
    font_manager.draw_single_line("Error occurred:", 20, 110, canvas_manager)?;
    
    // Word wrap error message
    let words: Vec<&str> = error.split_whitespace().collect();
    let mut y_pos = 140;
    let mut current_line = String::new();
    
    for word in words {
        let test_line = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current_line, word)
        };
        
        if let Ok((text_w, _)) = font_manager.size_of_text(&test_line) {
            if text_w > 250 && !current_line.is_empty() {
                font_manager.draw_single_line(&current_line, 20, y_pos, canvas_manager)?;
                y_pos += 25;
                current_line = word.to_string();
            } else {
                current_line = test_line;
            }
        }
    }
    
    if !current_line.is_empty() {
        font_manager.draw_single_line(&current_line, 20, y_pos, canvas_manager)?;
    }
    
    font_manager.draw_single_line("", 20, y_pos + 30, canvas_manager)?;
    font_manager.draw_single_line("B - Back to main menu", 20, y_pos + 60, canvas_manager)?;
    
    Ok(())
}