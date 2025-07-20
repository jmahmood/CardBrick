// src/scenes/deck_selection/mod.rs

use macroquad::prelude::*;

use crate::DeckMetadata;
use crate::ui::{CanvasManager, FontManager};

pub mod input;

/// Contains the state specific to the deck selection screen.
/// The pre-caching of rendered surfaces has been removed in favor of
/// drawing directly each frame, which simplifies state management.
pub struct DeckSelectionState {
    pub decks: Vec<DeckMetadata>,
    pub selected_index: usize,
    first_visible: usize,
}

impl DeckSelectionState {
    /// Creates a new state for the deck selection screen.
    pub fn new(decks: Vec<DeckMetadata>) -> Result<Self, String> {
        Ok(Self {
            decks,
            selected_index: 0,
            first_visible: 0,
        })
    }

    pub fn index_changes(&mut self, delta: isize, total: usize) -> bool {

        if total == 0 { return false; }
        let new_index = (self.selected_index as isize + delta)
            .clamp(0, total as isize - 1) as usize;

        return self.selected_index != new_index

    }

    /// Moves the selection cursor up or down, scrolling the list if necessary.
    pub fn move_selection(&mut self, delta: isize, total: usize, visible: usize) {
        if total == 0 { return; }
        let new_index = (self.selected_index as isize + delta)
            .clamp(0, total as isize - 1) as usize;
        self.selected_index = new_index;

        // scroll window up or down
        if self.selected_index < self.first_visible {
            self.first_visible = self.selected_index;
        } else if self.selected_index >= self.first_visible + visible {
            self.first_visible = self.selected_index - visible + 1;
        }
    }
}


pub fn draw_deck_selection_scene(
    font_manager: &FontManager,
    small_font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &DeckSelectionState,
) -> Result<(), String> {
    // Draw static text instructions
    font_manager.draw_single_line("Select a Deck", 20, 20, canvas_manager)?;
    small_font_manager.draw_single_line("Press Backspace to return to Main Menu", 20, 70, canvas_manager)?;

    // Handle case where no decks are found
    if state.decks.is_empty() {
        small_font_manager.draw_single_line("No decks found.", 20, 150, canvas_manager)?;
        small_font_manager.draw_single_line("Please add .apkg files to the 'decks' directory.", 20, 180, canvas_manager)?;
        return Ok(());
    }

    let list_top = 150;
    let mut y_pos = list_top;
    let item_height = 40; // Use a fixed height for uniform list items
    let visible_items = 4; // Max number of items to show at once

    for row in 0..visible_items {
        let idx = state.first_visible + row;
        if idx >= state.decks.len() {
            break;
        }

        let deck = &state.decks[idx];
        let display_title = deck.name.replace('_', " ");

        // Draw highlight rectangle for the selected item
        if idx == state.selected_index {
            let (text_w, text_h) = small_font_manager.size_of_text(&display_title)?;
            
            // Note: The x-coordinate for the highlight is hardcoded to match the text's logical x-position.
            let (highlight_x, highlight_y) = canvas_manager.logical_to_screen(18.0, y_pos as f32);
            let highlight_w = (text_w as f32 + 4.0) * canvas_manager.get_scale_factor();
            // Center the highlight vertically around the text baseline
            let highlight_h = (text_h as f32 + 4.0) * canvas_manager.get_scale_factor();
            let highlight_y_centered = highlight_y - highlight_h / 2.0 - (text_h as f32 / 4.0);
            
            draw_rectangle(
                highlight_x,
                highlight_y_centered,
                highlight_w,
                highlight_h,
                Color::from_rgba(80, 80, 80, 255),
            );
        }

        // Draw the deck name
        // We ignore the result since an error would have been caught by the highlight logic above.
        let _ = small_font_manager.draw_single_line(&display_title, 20, y_pos, canvas_manager);

        y_pos += item_height;
    }

    Ok(())
}