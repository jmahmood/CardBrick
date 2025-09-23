// src/scenes/deck_selection/mod.rs

use macroquad::prelude::*;

use crate::DeckMetadata;
use crate::ui::{CanvasManager, FontManager};
use crate::ui::font::TextLayout;
use crate::deck::html_parser;
use crate::config::UiAssets;

pub mod input;

/// Contains the state specific to the deck selection screen.
/// The pre-caching of rendered surfaces has been removed in favor of
/// drawing directly each frame, which simplifies state management.
pub struct DeckSelectionState {
    pub decks: Vec<DeckMetadata>,
    pub selected_index: usize,
    first_visible: usize,
    display_names: Vec<String>, // Pre-computed display names for performance
    name_layouts: Option<Vec<TextLayout>>, // Cached layouts for deck names
    // Offscreen cache for background + title
    bg_rt: Option<RenderTarget>,
    bg_dirty: bool,
    bg_used_assets: bool,
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
            display_names,
            name_layouts: None,
            bg_rt: None,
            bg_dirty: true,
            bg_used_assets: false,
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
    _font_manager: &FontManager,
    small_font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &mut DeckSelectionState,
    ui_assets: &Option<UiAssets>,
) -> Result<(), String> {
    // Layout constants for 512x384 pixel-art design
    const LOGICAL_HEIGHT: f32 = 384.0;
    const TITLE_POS: (i32, i32) = (51, 38);
    const LIST_START_POS: (i32, i32) = (51, 90);
    const LIST_ITEM_HEIGHT: f32 = 26.0;
    const FOOTER_POS: (i32, i32) = (51, 346);
    const FOOTER_AREA_HEIGHT: f32 = 40.0;

    // Calculate screen coordinates once
    let (screen_x, screen_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let scale = canvas_manager.get_scale_factor();
    let screen_w = 512.0 * scale;
    let screen_h = 384.0 * scale;
    
    // 1. Draw Background + Title directly in screen space (aligns with logical canvas)
    // Base background (full logical area)
    draw_rectangle(screen_x, screen_y, screen_w, screen_h, Color::from_rgba(40, 41, 46, 255));

    // Optional large background texture stretched to logical area
    if let Some(assets) = ui_assets {
        draw_texture_ex(
            &assets.deck_selection_bg,
            screen_x,
            screen_y,
            WHITE,
            DrawTextureParams { dest_size: Some(Vec2::new(screen_w, screen_h)), ..Default::default() },
        );
    }

    // Header banner and accents (logical coordinates scaled by canvas)
    let header_h = 64.0;
    let (hx, hy) = canvas_manager.logical_to_screen(0.0, 0.0);
    draw_rectangle(hx, hy, 512.0 * scale, header_h * scale, Color::from_rgba(35, 43, 68, 255));
    draw_rectangle(hx, hy + (header_h - 2.0) * scale, 512.0 * scale, 2.0 * scale, Color::from_rgba(100, 180, 255, 180));
    draw_rectangle(hx, hy + header_h * scale, 4.0 * scale, 16.0 * scale, Color::from_rgba(100, 180, 255, 140));
    draw_rectangle(hx + (512.0 - 4.0) * scale, hy + header_h * scale, 4.0 * scale, 16.0 * scale, Color::from_rgba(100, 180, 255, 140));

    // List panel background and border (logical coords)
    let panel_x = 44.0f32;
    let panel_y = 84.0f32;
    let panel_w = 420.0f32;
    let panel_h = 240.0f32;
    let (px, py) = canvas_manager.logical_to_screen(panel_x, panel_y);
    draw_rectangle(px, py, panel_w * scale, panel_h * scale, Color::from_rgba(28, 30, 34, 220));
    draw_rectangle_lines(px, py, panel_w * scale, panel_h * scale, 1.0 * scale, Color::from_rgba(90, 100, 120, 200));

    // Title + footer hint
    small_font_manager.draw_line_top_left("Select a Deck", TITLE_POS.0, TITLE_POS.1, canvas_manager)?;
    small_font_manager.draw_line_top_left("A: Select    B: Back", 44, 360, canvas_manager)?;


    // Title is part of offscreen background

    // 4. Draw Dynamic Footer (selected deck name if decks exist)
    if !state.decks.is_empty() {
        let display_name = &state.display_names[state.selected_index]; // Use pre-computed display name
        small_font_manager.draw_line_top_left(display_name, FOOTER_POS.0, FOOTER_POS.1, canvas_manager)?;
    }

    // Prepare cached layouts for deck names (one-time or when decks change)
    if state.name_layouts.as_ref().map_or(true, |v| v.len() != state.display_names.len()) {
        let mut layouts = Vec::with_capacity(state.display_names.len());
        for name in &state.display_names {
            let spans = html_parser::parse_html_to_spans(name);
            // Fixed reasonable width to avoid per-frame measurement
            let layout = small_font_manager.layout_text_binary(&spans, 380, false).unwrap_or(TextLayout { lines: vec![], total_height: 0, scroll_offset: 0 });
            layouts.push(layout);
        }
        state.name_layouts = Some(layouts);
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

    let (hint_ascent, _, _) = small_font_manager.metrics();
    let list_panel_w = 420.0f32; // width of the panel drawn in RT
    let text_left = 44.0 + 10.0; // panel_x + inner padding
    for row in 0..visible_items {
        let idx = state.first_visible + row;
        if idx >= state.decks.len() {
            break;
        }
        // Use precomputed layout for name
        let layout_opt = state.name_layouts.as_ref().and_then(|v| v.get(idx));
        let item_top = LIST_START_POS.1 as f32 + (row as f32 * LIST_ITEM_HEIGHT);

        // Selection highlight inside the panel
        if idx == state.selected_index {
            let rect_x = text_left;
            let rect_y = item_top;
            let rect_w = list_panel_w - 2.0 * 10.0; // padding on both sides
            // If the item spans multiple lines, expand the highlight to cover them.
            // Fallback to a single row height if layout is missing.
            let rect_h = layout_opt
                .map(|l| l.total_height as f32)
                .unwrap_or(LIST_ITEM_HEIGHT)
                .max(LIST_ITEM_HEIGHT);
            let (screen_x, screen_y) = canvas_manager.logical_to_screen(rect_x, rect_y);
            draw_rectangle(screen_x, screen_y, rect_w * scale, rect_h * scale, Color::from_rgba(57, 85, 165, 220));
            // Left accent bar
            draw_rectangle(screen_x - 4.0 * scale, screen_y, 4.0 * scale, rect_h * scale, Color::from_rgba(100, 180, 255, 255));
        }

        // Draw deck name text using cached layout
        if let Some(layout) = layout_opt {
            small_font_manager.draw_layout(layout, text_left as i32, item_top as i32 + hint_ascent as i32, false, canvas_manager)?;
        } else {
            // Fallback
            small_font_manager.draw_line_top_left(&state.display_names[idx], text_left as i32, item_top as i32, canvas_manager)?;
        }
    }

    Ok(())
}
