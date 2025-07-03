use sdl2::surface::Surface;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::config::Config;
use crate::DeckMetadata;
use crate::ui::FontManager;

pub mod input;

pub struct DeckRenderInfo {
    pub surface: Surface<'static>,
    pub width: u32,
    pub height: u32,
}

pub struct DeckSelectionState {
    pub decks: Vec<DeckMetadata>,
    pub rendered_decks: Vec<DeckRenderInfo>,
    pub selected_index: usize,
    first_visible: usize,
}


impl DeckSelectionState {
    /// This is where the caching happens. Call this once when switching to this scene.
    pub fn new(
        decks: Vec<DeckMetadata>,
        small_font_manager: &FontManager,
        config: &Config,
    ) -> Result<Self, String> {
        let mut rendered_decks = Vec::new();
        let max_width = config.logical_window_width - 80;

        for deck in &decks {
            let display_title = deck.name.replace('_', " ");
            
            // Perform the expensive rendering operation here.
            let (surface, width, height) = small_font_manager.render_text_to_surface(
                &display_title,
                max_width,
                80, // box_height
                10, // min_pt
                32, // max_pt
            )?;

            rendered_decks.push(DeckRenderInfo { surface, width, height });
        }

        Ok(DeckSelectionState {
            decks,
            rendered_decks,
            selected_index: 0,
            first_visible: 0,
        })
    }

    pub fn move_selection(&mut self, delta: isize, total: usize, visible: usize) {
        let new_index = (self.selected_index as isize + delta)
            .clamp(0, total as isize - 1) as usize;
        self.selected_index = new_index;

        // scroll window up or down in whole‐row steps
        if self.selected_index < self.first_visible {
            self.first_visible = self.selected_index;
        } else if self.selected_index >= self.first_visible + visible {
            self.first_visible = self.selected_index - visible + 1;
        }
    }
}


pub fn draw_deck_selection_scene(
    canvas: &mut Canvas<Window>,
    font_manager: &mut FontManager,
    small_font_manager: &mut FontManager,
    state: &DeckSelectionState,
) -> Result<(), String> {
    font_manager.draw_single_line(canvas, "Select a Deck", 20, 20)?;
    small_font_manager.draw_single_line(canvas, "Press Backspace to return to Main Menu", 20, 70)?;

    if state.decks.is_empty() {
        small_font_manager.draw_single_line(canvas, "No decks found.", 20, 150)?;
        small_font_manager.draw_single_line(canvas, "Please add .apkg files to the 'decks' directory.", 20, 180)?;
        return Ok(());
    }

    let texture_creator = canvas.texture_creator();
    let list_top = 150;
    // let list_bottom = canvas.window().size().1 as i32 - 20; // or whatever bottom margin
    let mut y_pos = list_top - 0;

    for row in 0..4 {
        let idx = state.first_visible + row;
        if idx >= state.rendered_decks.len() { break; }
        let info = &state.rendered_decks[idx];

        // draw highlight on the *cursor* row
        if idx == state.selected_index {
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            let r = Rect::new(18, y_pos - 2, info.width + 4, info.height as u32 + 4);
            canvas.fill_rect(r)?;
        }

        // draw the text
        let tex = texture_creator
            .create_texture_from_surface(&info.surface)
            .map_err(|e| e.to_string())?;
        canvas.copy(&tex, None, Rect::new(20, y_pos, info.width, info.height as u32))?;

        y_pos = y_pos + info.height as i32;
    }

    Ok(())
}
