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
    pub deck_layouts: Vec<TextLayout>, // This is no longer used for drawing but may be used elsewhere.
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

    // Handle the case where there are no decks to display.
    if state.decks.is_empty() {
        small_font_manager.draw_single_line(canvas, "No decks found.", 20, 150)?;
        small_font_manager.draw_single_line(canvas, "Please add .apkg files to the 'decks' directory.", 20, 180)?;
        return Ok(());
    }

    let mut y_pos = 150;
    let max_width = config.logical_window_width - 80; // max width for deck titles

    // Iterate over the actual deck metadata.
    for (i, deck) in state.decks.iter().enumerate() {
        let display_title = deck.name.replace('_', " ");
        let highlight_text_box = i == state.selected_index;
        let (_natural_w, natural_h) = small_font_manager.draw_text_in_box(
            canvas,
            &display_title,
            20,             // x
            y_pos,          // y
            max_width,      // box width
            80, // box height
            10,         // min point size
            72,         // max point size
            highlight_text_box
        )?;

        // Increment y_pos for the next deck title, using the actual height of the wrapped text.
        y_pos += natural_h as i32 + 10; // Add padding
    }
    Ok(())
}
