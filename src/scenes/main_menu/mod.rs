// src/scenes/main_menu/mod.rs

use crate::ui::{CanvasManager, FontManager};
use macroquad::prelude::*;

// This line was missing. It tells the main_menu module
// that the input.rs file is part of it.
pub mod input;

/// Contains the state specific to the main menu screen.
pub struct MainMenuState {
    pub selected_index: usize,
}

impl MainMenuState {
    /// Creates a new MainMenuState with a default selection.
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Draws the main menu scene.
/// This function was moved from main.rs.
pub fn draw_main_menu_scene(
    font_manager: &FontManager,
    small_font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &mut MainMenuState,
) -> Result<(), String> {
    let options = ["Study", "Progress", "Send Backup", "Quit"];

    // Draw everything directly in screen space so it lines up with text
    let (logical_width, _logical_height) = canvas_manager.logical_size();
    let (screen_x, screen_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let scale = canvas_manager.get_scale_factor();
    let screen_w = logical_width * scale;
    let screen_h = 384.0 * scale;

    // Background base
    draw_rectangle(
        screen_x,
        screen_y,
        screen_w,
        screen_h,
        Color::from_rgba(40, 41, 46, 255),
    );

    // Header banner and accents
    let header_h = 64.0;
    draw_rectangle(
        screen_x,
        screen_y,
        512.0 * scale,
        header_h * scale,
        Color::from_rgba(35, 43, 68, 255),
    );
    draw_rectangle(
        screen_x,
        screen_y + (header_h - 2.0) * scale,
        512.0 * scale,
        2.0 * scale,
        Color::from_rgba(100, 180, 255, 180),
    );
    draw_rectangle(
        screen_x,
        screen_y + header_h * scale,
        4.0 * scale,
        16.0 * scale,
        Color::from_rgba(100, 180, 255, 140),
    );
    draw_rectangle(
        screen_x + (512.0 - 4.0) * scale,
        screen_y + header_h * scale,
        4.0 * scale,
        16.0 * scale,
        Color::from_rgba(100, 180, 255, 140),
    );

    // Title with drop shadow (smaller font)
    let title_x = 24;
    let title_y = 18;
    small_font_manager.draw_single_line("CardBrick", title_x + 1, title_y + 1, canvas_manager)?;
    small_font_manager.draw_single_line("CardBrick", title_x, title_y, canvas_manager)?;
    small_font_manager.draw_single_line("Manual sync available", 24, 42, canvas_manager)?;

    let mut y_pos = 140;
    let padding_x = 16.0;
    let padding_y = 8.0;
    for (i, option) in options.iter().enumerate() {
        let (text_w, text_h) = font_manager.size_of_text(option)?;
        let text_left = 24.0;
        if i == state.selected_index {
            // Highlight block aligned to text baseline using ascent, plus slight downward offset
            let ascent = font_manager.metrics().0; // logical px
            let y_top = (y_pos as f32 - ascent) - padding_y; // push 6px down
            let (hx, hy) = canvas_manager.logical_to_screen(text_left - padding_x, y_top);
            let hw = (text_w as f32 + padding_x * 2.0) * canvas_manager.get_scale_factor();
            let hh = (text_h as f32 + padding_y * 2.0) * canvas_manager.get_scale_factor();
            draw_rectangle(hx, hy, hw, hh, Color::from_rgba(57, 85, 165, 220));

            // Left accent bar
            let (ax, ay) = canvas_manager.logical_to_screen(text_left - padding_x, y_top);
            draw_rectangle(
                ax,
                ay,
                4.0 * canvas_manager.get_scale_factor(),
                hh,
                Color::from_rgba(100, 180, 255, 255),
            );
        }
        font_manager.draw_single_line(option, text_left as i32, y_pos, canvas_manager)?;
        y_pos += 40;
    }
    Ok(())
}
