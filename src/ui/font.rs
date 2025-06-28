// src/ui/font.rs
// Manages loading fonts, calculating text layouts, and rendering text.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::Window;

/// Holds a pre-calculated text layout for efficient rendering and scrolling.
pub struct TextLayout {
    pub lines: Vec<String>,
    pub total_height: i32,
    pub scroll_offset: i32,
}

pub struct FontManager<'a, 'b> {
    #[allow(dead_code)] // ttf_context must be kept alive, but is not read directly.
    ttf_context: &'a Sdl2TtfContext,
    font: Font<'a, 'b>,
}

impl<'a, 'b> FontManager<'a, 'b> {
    pub fn new(ttf_context: &'a Sdl2TtfContext, font_path: &str, font_size: u16) -> Result<Self, String> {
        let font = ttf_context.load_font(font_path, font_size)?;
        Ok(FontManager { ttf_context, font })
    }

    /// A new helper function to get the pixel dimensions of a string of text.
    pub fn size_of_text(&self, text: &str) -> Result<(u32, u32), String> {
        self.font.size_of(text).map_err(|e| e.to_string())
    }

    /// Calculates how to wrap text and returns a layout object.
    /// This uses character-by-character wrapping, suitable for CJK text.
    pub fn layout_text(&self, text: &str, max_width: u32) -> Result<TextLayout, String> {
        let mut lines = Vec::new();
        let line_height = self.font.height();
        let mut total_height = 0;

        for paragraph in text.lines() {
            if paragraph.is_empty() {
                lines.push("".to_string());
                total_height += line_height;
                continue;
            }

            let mut current_line = String::new();
            for character in paragraph.chars() {
                let mut test_line = current_line.clone();
                test_line.push(character);
                
                let (w, _h) = self.font.size_of(&test_line).map_err(|e| e.to_string())?;

                if w > max_width {
                    lines.push(current_line);
                    current_line = character.to_string();
                } else {
                    current_line = test_line;
                }
            }
            lines.push(current_line); // Add the last line of the paragraph
        }
        
        // A more accurate height calculation based on the number of lines generated.
        total_height = line_height * lines.len() as i32;

        Ok(TextLayout {
            lines,
            total_height,
            scroll_offset: 0,
        })
    }

    /// Renders a pre-calculated TextLayout to the screen.
    pub fn draw_layout(&self, canvas: &mut Canvas<Window>, layout: &TextLayout, x: i32, y: i32) -> Result<(), String> {
        let line_height = self.font.height() as i32;
        let mut current_y = y - layout.scroll_offset;

        for line in &layout.lines {
            // Only draw the line if it's within the visible area to improve performance.
            if current_y > -line_height && current_y < canvas.viewport().height() as i32 {
                self.draw_single_line(canvas, line, x, current_y)?;
            }
            current_y += line_height;
        }
        Ok(())
    }

    /// Renders a single, non-wrapping line of text.
    pub fn draw_single_line(&self, canvas: &mut Canvas<Window>, text: &str, x: i32, y: i32) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }
        let texture_creator = canvas.texture_creator();
        let surface = self.font
            .render(text)
            .blended(Color::RGBA(255, 255, 255, 255))
            .map_err(|e| e.to_string())?;
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string())?;
        let target_rect = Rect::new(x, y, surface.width(), surface.height());
        canvas.copy(&texture, None, Some(target_rect))?;
        Ok(())
    }
}
