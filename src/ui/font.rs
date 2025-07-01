// src/ui/font.rs

// Manages loading fonts, calculating text layouts, and rendering text.

use sdl2::surface::Surface;

use crate::Config;
use sdl2::pixels::Color;
use std::collections::VecDeque;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::ttf::{Font, Sdl2TtfContext, FontStyle};
use sdl2::video::Window;
use crate::debug::Tracer;
use crate::deck::html_parser::TextSpan;

/// Holds a pre-calculated text layout for efficient rendering and scrolling.
pub struct TextLayout {
    // Each inner Vec<TextSpan> represents a single line of text with its styled segments.
    pub lines: Vec<Vec<TextSpan>>,
    pub total_height: i32,
    pub scroll_offset: i32,
}

pub struct FontManager<'a, 'b> {
    #[allow(dead_code)] // ttf_context must be kept alive, but is not read directly.
    ttf_context: &'a Sdl2TtfContext,
    font: Font<'a, 'b>,
}

impl TextSpan {
    pub fn text_to_use(&self, use_ruby: bool) -> &str {
        if use_ruby {
            // Use the ruby text if available, otherwise fall back to the base text.
            self.ruby_text.as_deref().unwrap_or(&self.text)
        } else {
            &self.text
        }
    }
}

impl<'a, 'b> FontManager<'a, 'b> {
    pub fn new(ttf_context: &'a Sdl2TtfContext, font_path: &str, font_size: u16) -> Result<Self, String> {
        let font = ttf_context.load_font(font_path, font_size)?;
        Ok(FontManager { ttf_context, font })
    }


    /// Get the pixel dimensions of a string of text.
    /// This considers the current style the font is set to.
    pub fn size_of_text_with_style(&mut self, text: &str, is_bold: bool, is_italic: bool) -> Result<(u32, u32), String> {
        let original_style = self.font.get_style();
        let mut current_style = original_style;
        if is_bold { current_style = current_style | FontStyle::BOLD; }
        if is_italic { current_style = current_style | FontStyle::ITALIC; }
        self.font.set_style(current_style);

        let result = self.font.size_of(text).map_err(|e| e.to_string());
        self.font.set_style(original_style); // Reset style
        result
    }

    /// Finds the character index to split a TextSpan so it fits within the available width.
    /// This is the efficient binary search method.
    fn find_split_index(&mut self, span: &TextSpan, space_left: u32, use_ruby: bool) -> Result<usize, String> {
        let text = span.text_to_use(use_ruby);
        let mut current_width = 0;
        let mut last_valid_split_point = 0;

        // Iterate character by character to respect UTF-8 boundaries
        for (byte_index, char) in text.char_indices() {
            let char_str = char.to_string();
            let (char_width, _) = self.size_of_text_with_style(&char_str, span.is_bold, span.is_italic)?;
            
            if current_width + char_width > space_left {
                // This character does not fit, so the split point is before it.
                return Ok(last_valid_split_point);
            }
            
            current_width += char_width;
            // The split point is after the current character.
            last_valid_split_point = byte_index + char.len_utf8();
        }

        // If the whole string fits, the split point is at the end.
        Ok(last_valid_split_point)
    }


    pub fn layout_text_binary(&mut self, spans: &[TextSpan], max_width: u32, use_ruby: bool) -> Result<TextLayout, String> {
        #[cfg(debug_assertions)]
        let _layout_tracer = Tracer::new("Load Card Layout");

        // --- STAGE 1: Pre-processing (creating new spans if there is a newline) ---
        let mut processed_spans = VecDeque::new();
        for span in spans {
            let mut parts = span.text.split('\n').peekable();
            while let Some(part) = parts.next() {
                if !part.is_empty() {
                    let mut text_span = span.clone();
                    text_span.text = part.to_string();
                    text_span.is_newline = false;
                    processed_spans.push_back(text_span);
                }
                if parts.peek().is_some() {
                    let mut newline_span = span.clone();
                    newline_span.text = String::new();
                    newline_span.is_newline = true;
                    processed_spans.push_back(newline_span);
                }
            }
        }

        // --- STAGE 2: Corrected Layout Engine ---
        let mut lines: Vec<Vec<TextSpan>> = Vec::new();
        let mut current_line_spans: Vec<TextSpan> = Vec::new();
        let mut current_line_width = 0;
        let line_height = self.font.height();

        while let Some(span) = processed_spans.pop_front() {
            if span.is_newline {
                lines.push(current_line_spans);
                current_line_spans = Vec::new();
                current_line_width = 0;
                continue;
            }

            let text_for_layout = span.text_to_use(use_ruby);
            let space_left = max_width.saturating_sub(current_line_width);
            let (span_width, _) = self.size_of_text_with_style(text_for_layout, span.is_bold, span.is_italic)?;

            if span_width <= space_left {
                current_line_spans.push(span);
                current_line_width += span_width;
            } else {
                let split_byte_index = self.find_split_index(&span, space_left, use_ruby)?;

                if split_byte_index > 0 {
                    // FIX: By calling .to_string(), we create an owned String and drop the borrow on `span`.
                    // This allows `span` to be moved into `remaining_span` later without a borrow checker error.
                    let text_to_split = span.text_to_use(use_ruby).to_string();
                    let (fits, remaining) = text_to_split.split_at(split_byte_index);
                    
                    let mut fit_span = span.clone();
                    let mut remaining_span = span;

                    if use_ruby {
                        fit_span.ruby_text = Some(fits.to_string());
                        remaining_span.ruby_text = Some(remaining.to_string());
                        remaining_span.text = String::new();
                    } else {
                        fit_span.text = fits.to_string();
                        remaining_span.text = remaining.to_string();
                    }
                    
                    current_line_spans.push(fit_span);
                    processed_spans.push_front(remaining_span);

                } else {
                    // #################################################################
                    // ### BUG FIX STARTS HERE: PREVENTING THE INFINITE LOOP ###
                    // #################################################################
                    if !current_line_spans.is_empty() {
                        // The current line has content, so it's full.
                        // Finalize it and re-process the current span on a new line.
                        processed_spans.push_front(span);
                    } else {
                        // The line is empty, but the word is still too long.
                        // Force a split by taking at least one character to prevent an infinite loop.
                        let text_to_split = span.text_to_use(use_ruby).to_string();
                        let mut char_iter = text_to_split.chars();
                        if let Some(first_char) = char_iter.next() {
                            let split_at = first_char.len_utf8();
                            let (fits, remaining) = text_to_split.split_at(split_at);
                            
                            let mut fit_span = span.clone();
                            let mut remaining_span = span;

                            if use_ruby {
                                fit_span.ruby_text = Some(fits.to_string());
                                remaining_span.ruby_text = Some(remaining.to_string());
                                // We keep the base text with the first part and clear it for the rest.
                                remaining_span.text = String::new(); 
                            } else {
                                fit_span.text = fits.to_string();
                                remaining_span.text = remaining.to_string();
                            }

                            current_line_spans.push(fit_span);
                            if !remaining.is_empty() {
                                processed_spans.push_front(remaining_span);
                            }
                        } else {
                             // The span was empty, do nothing.
                        }
                    }
                    // #################################################################
                    // ### BUG FIX ENDS HERE ###
                    // #################################################################
                }

                lines.push(current_line_spans);
                current_line_spans = Vec::new();
                current_line_width = 0;
            }
        }

        if !current_line_spans.is_empty() {
            lines.push(current_line_spans);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        let total_height = line_height * lines.len() as i32;
        Ok(TextLayout { lines, total_height, scroll_offset: 0 })
    }

    /// Renders a pre-calculated TextLayout to the screen.
    pub fn draw_layout(&mut self, canvas: &mut Canvas<Window>, layout: &TextLayout, x: i32, y: i32, show_ruby: bool) -> Result<(), String> {
        let line_height = self.font.height() as i32;
        let mut current_y = y - layout.scroll_offset;

        for line_spans in &layout.lines {
            if current_y > -line_height && current_y < canvas.viewport().height() as i32 {
                let mut current_x = x;
                for span in line_spans {
                    let text_to_draw = span.text_to_use(show_ruby);
                    let (text_w, _) = self.draw_text_span_segment(canvas, text_to_draw, current_x, current_y, span.is_bold, span.is_italic)?;
                    current_x += text_w as i32;
                }
            }
            current_y += line_height;
        }
        Ok(())
    }

    /// Renders a single segment of text with specified bold/italic styles.
    fn draw_text_span_segment(&mut self, canvas: &mut Canvas<Window>, text: &str, x: i32, y: i32, is_bold: bool, is_italic: bool) -> Result<(u32, u32), String> {
        if text.is_empty() {
            return Ok((0, 0));
        }

        let original_style = self.font.get_style();
        let mut current_style = original_style;
        if is_bold { current_style = current_style | FontStyle::BOLD; }
        if is_italic { current_style = current_style | FontStyle::ITALIC; }
        self.font.set_style(current_style);

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

        self.font.set_style(original_style); // Reset style
        Ok((surface.width(), surface.height()))
    }
    
    pub fn draw_single_line(&mut self, canvas: &mut Canvas<Window>, text: &str, x: i32, y: i32, ) -> Result<(), String> {
        self.draw_text_span_segment(canvas, text, x, y, false, false)?;
        Ok(())
    }
    
    pub fn size_of_text(&mut self, text: &str) -> Result<(u32, u32), String> {
        self.size_of_text_with_style(text, false, false)
    }

    fn find_fitting_size(
        &self,
        text: &str,
        box_width: u32,
        box_height: u32,
        min_pt: u16,
        max_pt: u16,
    ) -> Result<u16, String> {
        let mut low = min_pt;
        let mut high = max_pt;
        let mut best = min_pt;
        let config = Config::new();
        while low <= high {
            let mid = (low + high) / 2;
            // load font at trial size
            let trial = self.ttf_context
                .load_font(&config.font_path, mid)
                .map_err(|e| e.to_string())?;
            // wrap & measure
            let surf = trial
                .render(text)
                .blended_wrapped(Color::RGBA(255,255,255,255), box_width)
                .map_err(|e| e.to_string())?;

            eprintln!(
              " pt={} → wrapped size: w={} h={}",
              mid,
              surf.width(),
              surf.height()
            );

            let h = surf.height();

            if h <= box_height {
                best = mid;       // fits, try larger
                low  = mid + 1;
            } else {
                if mid == 0 { break; }
                high = mid - 1;   // too tall, try smaller
            }
        }

        Ok(best)
    }

    pub fn render_text_to_surface(
        &self,
        text: &str,
        box_width: u32,
        box_height: u32,
        min_pt: u16,
        max_pt: u16,
    ) -> Result<(Surface<'static>, u32, u32), String> {
        let config = Config::new();
        let best_pt = self.find_fitting_size(text, box_width, box_height, min_pt, max_pt)?;
        let font = self
            .ttf_context
            .load_font(&config.font_path, best_pt)
            .map_err(|e| e.to_string())?;
        let surface = font
            .render(text)
            .blended_wrapped(Color::RGBA(255, 255, 255, 255), box_width)
            .map_err(|e| e.to_string())?;
        let (width, height) = (surface.width(), surface.height());
        Ok((surface, width, height))
    }


    pub fn draw_text_in_box(
        &mut self,
        canvas: &mut Canvas<Window>,
        text: &str,
        x: i32,
        y: i32,
        box_width: u32,
        box_height: u32,
        min_pt: u16,
        max_pt: u16,
        highlight: bool
    ) -> Result<(u32, u32), String> {
        if text.is_empty() {
            return Ok((0, 0));
        }

        // 1) pick best size (largest pt that wraps ≤ box_height)
        let best_pt = self.find_fitting_size(text, box_width, box_height, min_pt, max_pt)?;
        println!("{:?}", best_pt);
        let config = Config::new();

        // 2) reload font at that size
        let new_font = self.ttf_context
            .load_font(config.font_path, best_pt)
            .map_err(|e| e.to_string())?;

        // 3) wrap & render into a surface
        let surface = new_font
            .render(text)
            .blended_wrapped(Color::RGBA(255, 255, 255, 255), box_width)
            .map_err(|e| e.to_string())?;

        // 4) create the texture
        let texture_creator = canvas.texture_creator();
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string())?;

        let w = surface.width();
        let h = surface.height();

        if highlight {
            let highlight_rect = Rect::new(18, y - 2, w + 4, h + 4);
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            canvas.fill_rect(highlight_rect)?;

        }

        let target_rect = Rect::new(x, y, w, h);
        canvas.copy(&texture, None, Some(target_rect))?;
        Ok((w, h))
    }
}

// #################################################################
// ### UNIT TESTS TO PREVENT REGRESSIONS ###
// #################################################################
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    // FIX: Use a static OnceLock to ensure the TTF context is initialized exactly once for all tests.
    static TTF_CONTEXT: OnceLock<Sdl2TtfContext> = OnceLock::new();

    // Test helper to create a FontManager.
    fn setup_font_manager() -> FontManager<'static, 'static> {
        // This will initialize the context on the first call and simply return
        // the existing context on all subsequent calls from other tests.
        let ttf_context = TTF_CONTEXT.get_or_init(|| {
            sdl2::ttf::init().expect("Failed to initialize SDL2 TTF context for tests")
        });

        // NOTE: This test requires a font file at the specified path.
        // A common font like DejaVuSans is used here, which is often found on Linux.
        // For other systems, you may need to change this path or place a font at `tests/font.ttf`.
        let font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        FontManager::new(ttf_context, font_path, 16).expect("Failed to load font for testing")
    }

    #[test]
    fn test_simple_ascii_wrapping() {
        let mut fm = setup_font_manager();
        let spans = vec![TextSpan {
            text: "This is a simple test.".to_string(),
            is_bold: false, is_italic: false, is_newline: false, is_ruby_base: false, ruby_text: None, new_text_block: false,
        }];
        let layout = fm.layout_text_binary(&spans, 80, false).unwrap();
        println!("{:?}", layout.lines);
        assert_eq!(layout.lines.len(), 2, "Text should wrap to 2 lines");
        assert_eq!(layout.lines[0][0].text, "This is a ");
        assert_eq!(layout.lines[1][0].text, "simple test.");
    }

    #[test]
    fn test_japanese_wrapping_no_panic() {
        let mut fm = setup_font_manager();
        let spans = vec![TextSpan {
            text: "これは長い日本の文章です。".to_string(),
            is_bold: false, is_italic: false, is_newline: false, is_ruby_base: false, ruby_text: None, new_text_block: false,
        }];
        // A narrow width to force wrapping
        let layout = fm.layout_text_binary(&spans, 100, false).unwrap();
        assert!(layout.lines.len() > 1, "Japanese text should wrap");
    }

    #[test]
    fn test_long_word_does_not_inf_loop() {
        let mut fm = setup_font_manager();
        let spans = vec![TextSpan {
            text: "Supercalifragilisticexpialidocious".to_string(),
            is_bold: false, is_italic: false, is_newline: false, is_ruby_base: false, ruby_text: None, new_text_block: false,
        }];
        // Use a width smaller than the first character
        let layout = fm.layout_text_binary(&spans, 5, false).unwrap();
        // The first line should contain just the first character, and the rest should wrap.
        assert!(layout.lines.len() > 1, "Very long word should wrap to multiple lines");
        assert_eq!(layout.lines[0][0].text, "S");
    }
}
