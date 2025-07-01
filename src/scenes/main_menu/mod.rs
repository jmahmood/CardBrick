// src/scenes/main_menu/mod.rs

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::ui::FontManager;

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

/// Draws the main menu scene.
/// This function was moved from main.rs.
pub fn draw_main_menu_scene(
    canvas: &mut Canvas<Window>,
    font_manager: &mut FontManager,
    state: &MainMenuState,
) -> Result<(), String> {
    let options = ["Study", "Profile", "Quit"];
    font_manager.draw_single_line(canvas, "CardBrick", 20, 20)?;

    let mut y_pos = 150;
    for (i, option) in options.iter().enumerate() {
        if i == state.selected_index {
            let (text_w, text_h) = font_manager.size_of_text(option)?;
            let highlight_rect = Rect::new(18, y_pos, text_w + 4, text_h);
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            canvas.fill_rect(highlight_rect)?;
        }
        font_manager.draw_single_line(canvas, option, 20, y_pos)?;
        y_pos += 40;
    }
    Ok(())
}
