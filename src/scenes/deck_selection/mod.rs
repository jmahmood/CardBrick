// src/scenes/deck_selection/mod.rs

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::config::Config;
use crate::DeckMetadata;
use crate::ui::{FontManager, font::TextLayout};

pub mod input;

/// Contains the state specific to the deck selection screen.
pub struct DeckSelectionState {
    pub decks: Vec<DeckMetadata>,
    pub deck_layouts: Vec<TextLayout>,
    pub selected_index: usize,
}

/// Draws the deck selection scene.
pub fn draw_deck_selection_scene(
    canvas: &mut Canvas<Window>,
    font_manager: &mut FontManager,
    small_font_manager: &mut FontManager,
    state: &DeckSelectionState,
    config: &Config,
) -> Result<(), String> {
    font_manager.draw_single_line(canvas, "Select a Deck", 20, 20)?;
    small_font_manager.draw_single_line(canvas, "Press Backspace to return to Main Menu", 20, 70)?;

    let mut y_pos = 150;
    let max_width = config.window_width - 40;

    for (i, layout) in state.deck_layouts.iter().enumerate() {
        if i == state.selected_index {
            let highlight_rect = Rect::new(18, y_pos, max_width, layout.total_height as u32);
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            canvas.fill_rect(highlight_rect)?;
        }
        small_font_manager.draw_layout(canvas, layout, 20, y_pos, false)?;
        y_pos += layout.total_height + 10;
    }
    Ok(())
}
