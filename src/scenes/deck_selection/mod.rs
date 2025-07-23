// src/scenes/deck_selection/mod.rs

use macroquad::prelude::*;

use crate::DeckMetadata;
use crate::ui::{CanvasManager, FontManager};
use crate::config::UiAssets;

pub mod input;

/// Contains the state specific to the deck selection screen.
/// The pre-caching of rendered surfaces has been removed in favor of
/// drawing directly each frame, which simplifies state management.
pub struct DeckSelectionState {
    pub decks: Vec<DeckMetadata>,
    pub selected_index: usize,
    first_visible: usize,
    pub character_anim_timer: f32,
    pub character_current_frame: usize,
    display_names: Vec<String>, // Pre-computed display names for performance
}

impl DeckSelectionState {
    /// Creates a new state for the deck selection screen.
    pub fn new(decks: Vec<DeckMetadata>) -> Result<Self, String> {
        // Pre-compute display names for performance
        let display_names: Vec<String> = decks.iter()
            .map(|deck| deck.name.replace('_', " "))
            .collect();
            
        Ok(Self {
            decks,
            selected_index: 0,
            first_visible: 0,
            character_anim_timer: 0.0,
            character_current_frame: 0,
            display_names,
        })
    }

    /// Updates the character animation state
    pub fn update(&mut self, delta_time: f32) {
        const FRAME_DURATION: f32 = 0.25;
        const NUM_FRAMES: usize = 4;
        
        self.character_anim_timer += delta_time;
        if self.character_anim_timer >= FRAME_DURATION {
            self.character_current_frame = (self.character_current_frame + 1) % NUM_FRAMES;
            self.character_anim_timer -= FRAME_DURATION;
        }
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
    _font_manager: &FontManager,
    small_font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &DeckSelectionState,
    ui_assets: &Option<UiAssets>,
) -> Result<(), String> {
    // Layout constants for 512x384 pixel-art design
    const LOGICAL_HEIGHT: f32 = 384.0;
    const TITLE_POS: (i32, i32) = (51, 38);
    const LIST_START_POS: (i32, i32) = (51, 90);
    const LIST_ITEM_HEIGHT: f32 = 26.0;
    const CHARACTER_POS: (f32, f32) = (312.0, 120.0);
    const FOOTER_POS: (i32, i32) = (51, 346);
    const FOOTER_AREA_HEIGHT: f32 = 40.0;
    const FRAME_WIDTH: f32 = 64.0;
    const FRAME_HEIGHT: f32 = 64.0;

    // Calculate screen coordinates once
    let (screen_x, screen_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let scale = canvas_manager.get_scale_factor();
    let screen_w = 512.0 * scale;
    let screen_h = 384.0 * scale;
    
    // 1. Draw Background (solid color fallback if assets not loaded)
    if let Some(assets) = ui_assets {
        draw_texture_ex(
            &assets.deck_selection_bg, 
            screen_x, 
            screen_y, 
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(screen_w, screen_h)),
                ..Default::default()
            }
        );
    } else {
        // Fallback: solid color background
        draw_rectangle(screen_x, screen_y, screen_w, screen_h, Color::from_rgba(45, 50, 65, 255));
    }

    // 2. Draw Animated Character (if assets are loaded)
    if let Some(assets) = ui_assets {
        let source_rect = Rect::new(
            state.character_current_frame as f32 * 4.0,  // Each frame is 4 pixels wide
            0.0,
            4.0,  // Source width: 4 pixels
            4.0,  // Source height: 4 pixels
        );
        
        let (char_screen_x, char_screen_y) = canvas_manager.logical_to_screen(CHARACTER_POS.0, CHARACTER_POS.1);
        let char_dest_size = Vec2::new(FRAME_WIDTH * scale, FRAME_HEIGHT * scale);
        
        draw_texture_ex(
            &assets.character_sprite,
            char_screen_x,
            char_screen_y,
            WHITE,
            DrawTextureParams {
                source: Some(source_rect),
                dest_size: Some(char_dest_size),
                ..Default::default()
            }
        );
    }

    // 3. Draw Title
    small_font_manager.draw_line_top_left("Select a Deck", TITLE_POS.0, TITLE_POS.1, canvas_manager)?;

    // 4. Draw Dynamic Footer (selected deck name if decks exist)
    if !state.decks.is_empty() {
        let display_name = &state.display_names[state.selected_index]; // Use pre-computed display name
        small_font_manager.draw_line_top_left(display_name, FOOTER_POS.0, FOOTER_POS.1, canvas_manager)?;
    }

    // 5. Draw Deck List
    if state.decks.is_empty() {
        // Empty list message
        small_font_manager.draw_line_top_left("No cached decks found.", LIST_START_POS.0, LIST_START_POS.1, canvas_manager)?;
        small_font_manager.draw_line_top_left("Run precache_decks.py to cache .apkg files.", LIST_START_POS.0, LIST_START_POS.1 + 30, canvas_manager)?;
        return Ok(());
    }

    // Calculate visible items dynamically
    let available_height = LOGICAL_HEIGHT - LIST_START_POS.1 as f32 - FOOTER_AREA_HEIGHT;
    let visible_items = (available_height / LIST_ITEM_HEIGHT).floor() as usize;

    for row in 0..visible_items {
        let idx = state.first_visible + row;
        if idx >= state.decks.len() {
            break;
        }

        let display_title = &state.display_names[idx]; // Use pre-computed display name
        let item_top = LIST_START_POS.1 as f32 + (row as f32 * LIST_ITEM_HEIGHT);

        // Selection Highlight with new blue color (fixed width for performance)
        if idx == state.selected_index {
            // Use fixed width to avoid text measurement every frame
            let rect_x = LIST_START_POS.0 as f32 - 2.0;
            let rect_y = item_top;
            let rect_w = 300.0; // Fixed reasonable width
            let rect_h = LIST_ITEM_HEIGHT;

            // Convert to screen space
            let (screen_x, screen_y) = canvas_manager.logical_to_screen(rect_x, rect_y);
            
            draw_rectangle(
                screen_x,
                screen_y,
                rect_w * scale,
                rect_h * scale,
                Color::from_rgba(57, 85, 165, 255), // New blue selection color
            );
        }

        // Draw deck name text
        small_font_manager.draw_line_top_left(display_title, LIST_START_POS.0, item_top as i32, canvas_manager)?;
    }

    Ok(())
}