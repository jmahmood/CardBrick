// src/ui/font.rs
// Manages loading fonts and rendering text.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::Window;

pub struct FontManager<'a, 'b> {
    ttf_context: &'a Sdl2TtfContext,
    // We'll store our loaded fonts here. For now, just one.
    font: Font<'a, 'b>,
}

impl<'a, 'b> FontManager<'a, 'b> {
    // Loads a font from a file path.
    pub fn new(ttf_context: &'a Sdl2TtfContext, font_path: &str, font_size: u16) -> Result<Self, String> {
        let font = ttf_context.load_font(font_path, font_size)?;
        Ok(FontManager { ttf_context, font })
    }

    // Renders text to the canvas at a given position.
    pub fn draw_text(&self, canvas: &mut Canvas<Window>, text: &str, x: i32, y: i32) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }

        let texture_creator = canvas.texture_creator();
        
        // Create a surface (an in-memory image) from the text.
        let surface = self.font
            .render(text)
            .blended(Color::RGBA(255, 255, 255, 255))
            .map_err(|e| e.to_string())?;
        
        // Create a hardware-accelerated texture from the surface.
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string())?;
        
        // Get the dimensions of the rendered text surface to create the destination rectangle.
        let target_rect = Rect::new(x, y, surface.width(), surface.height());
        
        // Copy the texture to the canvas.
        canvas.copy(&texture, None, Some(target_rect))?;

        Ok(())
    }
}
