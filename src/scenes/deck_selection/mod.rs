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
}


impl DeckSelectionState {
    /// **NEW**: Creates a new state by pre-rendering all deck titles into textures.
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
                72, // max_pt
            )?;

            rendered_decks.push(DeckRenderInfo { surface, width, height });
        }

        Ok(DeckSelectionState {
            decks,
            rendered_decks,
            selected_index: 0,
        })
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
    let mut y_pos = 150;

    for (i, info) in state.rendered_decks.iter().enumerate() {
        if i == state.selected_index {
            let highlight_rect = Rect::new(18, y_pos - 2, info.width + 4, info.height + 4);
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            canvas.fill_rect(highlight_rect)?;
        }

        // Create a temporary texture from the cached surface. This is a fast GPU upload.
        let texture = texture_creator
            .create_texture_from_surface(&info.surface)
            .map_err(|e| e.to_string())?;

        let target_rect = Rect::new(20, y_pos, info.width, info.height);
        canvas.copy(&texture, None, Some(target_rect))?;
        
        y_pos += info.height as i32 + 10;
    }
    Ok(())
}
