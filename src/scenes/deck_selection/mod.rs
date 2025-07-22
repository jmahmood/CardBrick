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
    // Use font-derived layout constants
    const MARGIN_TOP: f32 = 10.0;
    const TEXT_X: f32 = 20.0;
    const PADDING: f32 = 2.0;

    let (_screen_width, screen_height) = canvas_manager.logical_size();
    let title_line_height = font_manager.line_height();
    let item_line_height = small_font_manager.line_height();
    let list_spacing = 8.0;
    
    // Calculate layout positions using font metrics
    let title_top = MARGIN_TOP;
    let instruction_top = title_top + title_line_height + 8.0;
    let list_top = instruction_top + item_line_height + 20.0;
    let item_step = item_line_height + list_spacing;

    // Draw static text instructions using top-left positioning
    font_manager.draw_line_top_left("Select a Deck", TEXT_X as i32, title_top as i32, canvas_manager)?;
    small_font_manager.draw_line_top_left("Press Backspace to return to Main Menu", TEXT_X as i32, instruction_top as i32, canvas_manager)?;

    // Handle case where no decks are found
    if state.decks.is_empty() {
        let no_decks_top = list_top;
        let instructions_top = no_decks_top + item_line_height + 8.0;
        small_font_manager.draw_line_top_left("No cached decks found.", TEXT_X as i32, no_decks_top as i32, canvas_manager)?;
        small_font_manager.draw_line_top_left("Run precache_decks.py to cache .apkg files.", TEXT_X as i32, instructions_top as i32, canvas_manager)?;
        return Ok(());
    }

    // Calculate how many items can fit on screen
    let available_height = screen_height - list_top;
    let visible_items = (available_height / item_step).floor() as usize;

    for row in 0..visible_items {
        let idx = state.first_visible + row;
        if idx >= state.decks.len() {
            break;
        }

        let deck = &state.decks[idx];
        let display_title = deck.name.replace('_', " ");
        let item_top = list_top + (row as f32 * item_step);

        // Draw highlight rectangle for the selected item (in logical space)
        if idx == state.selected_index {
            let (text_w, text_h) = small_font_manager.size_of_text(&display_title)?;
            
            // Compute rectangle in logical space
            let rect_x = TEXT_X - PADDING;
            let rect_y = item_top;
            let rect_w = text_w as f32 + PADDING * 2.0;
            let rect_h = text_h as f32 + PADDING * 2.0;

            // Convert to screen space once
            let (screen_x, screen_y) = canvas_manager.logical_to_screen(rect_x, rect_y);
            let scale = canvas_manager.get_scale_factor();
            
            draw_rectangle(
                screen_x,
                screen_y,
                rect_w * scale,
                rect_h * scale,
                Color::from_rgba(80, 80, 80, 255),
            );
        }

        // Draw the deck name using top-left positioning
        small_font_manager.draw_line_top_left(&display_title, TEXT_X as i32, item_top as i32, canvas_manager)?;
    }

    Ok(())
}