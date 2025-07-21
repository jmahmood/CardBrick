// src/ui/font.rs

// Manages loading fonts, calculating text layouts, and rendering text with macroquad.

use std::path::PathBuf;
use macroquad::prelude::*;
use crate::Config;
use std::collections::VecDeque;
use crate::debug::Tracer;
use crate::deck::html_parser::TextSpan;

/// Holds a pre-calculated text layout for efficient rendering and scrolling.
pub struct TextLayout {
    // Each inner Vec<TextSpan> represents a single line of text with its styled segments.
    pub lines: Vec<Vec<TextSpan>>,
    pub total_height: i32,
    pub scroll_offset: i32,
}

pub struct FontManager {
    font: Option<Font>,
    fallback_font: Option<Font>,
    font_size: f32,
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

impl FontManager {
    pub fn new_with_fallback(
        primary_path: &PathBuf,
        fallback_path: Option<&PathBuf>,
        font_size: u16
    ) -> Result<Self, String> {
        let primary_font = if primary_path.exists() {
            Some(load_ttf_font_from_bytes(
                &std::fs::read(primary_path).map_err(|e| e.to_string())?
            ).map_err(|e| format!("Failed to load primary font: {:?}", e))?)
        } else {
            None
        };
        
        let fallback_font = match fallback_path {
            Some(path) if path.exists() => {
                Some(load_ttf_font_from_bytes(
                    &std::fs::read(path).map_err(|e| e.to_string())?
                ).map_err(|e| format!("Failed to load fallback font: {:?}", e))?)
            },
            _ => None,
        };
        
        Ok(FontManager {
            font: primary_font,
            fallback_font,
            font_size: font_size as f32,
        })
    }
    
    pub fn from_loaded_font(font: Option<Font>, fallback_font: Option<Font>, font_size: u16) -> Self {
        FontManager {
            font,
            fallback_font,
            font_size: font_size as f32,
        }
    }
    
    pub fn new(font_path: &PathBuf, font_size: u16) -> Result<Self, String> {
        let font = if font_path.exists() {
            Some(load_ttf_font_from_bytes(
                &std::fs::read(font_path).map_err(|e| e.to_string())?
            ).map_err(|e| format!("Failed to load font: {:?}", e))?)
        } else {
            None
        };
        
        Ok(FontManager { 
            font, 
            fallback_font: None, 
            font_size: font_size as f32 
        })
    }


    /// Get the pixel dimensions of a string of text.
    pub fn size_of_text_with_style(&self, text: &str, _is_bold: bool, _is_italic: bool) -> Result<(u32, u32), String> {
        let text_params = TextParams {
            font: self.font.as_ref(),
            font_size: self.font_size as u16,
            color: WHITE,
            ..Default::default()
        };
        
        let dimensions = measure_text(text, self.font.as_ref(), self.font_size as u16, 1.0);
        Ok((dimensions.width as u32, dimensions.height as u32))
    }

    /// Finds the character index to split a TextSpan so it fits within the available width.
    /// This is the efficient binary search method.
    fn find_split_index(&self, span: &TextSpan, space_left: u32, use_ruby: bool) -> Result<usize, String> {
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


    pub fn layout_text_binary(&self, spans: &[TextSpan], max_width: u32, use_ruby: bool) -> Result<TextLayout, String> {
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
        let line_height = self.font_size as i32;

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
    pub fn draw_layout(&self, layout: &TextLayout, x: i32, y: i32, show_ruby: bool, canvas_manager: &crate::ui::CanvasManager) -> Result<(), String> {
        let line_height = self.font_size as i32;
        let mut current_y = y - layout.scroll_offset;

        for line_spans in &layout.lines {
            // Simple clipping check
            if current_y > -line_height && current_y < canvas_manager.logical_size().1 as i32 {
                let mut current_x = x;
                for span in line_spans {
                    let text_to_draw = span.text_to_use(show_ruby);
                    let (text_w, _) = self.draw_text_span_segment(text_to_draw, current_x, current_y, span.is_bold, span.is_italic, canvas_manager)?;
                    current_x += text_w as i32;
                }
            }
            current_y += line_height;
        }
        Ok(())
    }

    fn draw_text_span_segment(&self, text: &str, x: i32, y: i32, _is_bold: bool, _is_italic: bool, canvas_manager: &crate::ui::CanvasManager) -> Result<(u32, u32), String> {
        if text.is_empty() {
            return Ok((0, 0));
        }

        let (screen_x, screen_y) = canvas_manager.logical_to_screen(x as f32, y as f32);
        let scaled_font_size = self.font_size * canvas_manager.get_scale_factor();
        
        // Try primary font first, then fallback
        let font_to_use = self.font.as_ref().or(self.fallback_font.as_ref());
        
        let text_params = TextParams {
            font: font_to_use,
            font_size: scaled_font_size as u16,
            color: WHITE,
            ..Default::default()
        };

        draw_text_ex(text, screen_x, screen_y, text_params);
        
        // Measure text to return dimensions
        let dimensions = measure_text(text, font_to_use, self.font_size as u16, 1.0);
        Ok((dimensions.width as u32, dimensions.height as u32))
    }


    pub fn draw_single_line(&self, text: &str, x: i32, y: i32, canvas_manager: &crate::ui::CanvasManager) -> Result<(), String> {
        self.draw_text_span_segment(text, x, y, false, false, canvas_manager)?;
        Ok(())
    }
    
    pub fn size_of_text(&self, text: &str) -> Result<(u32, u32), String> {
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
            // Simulate text wrapping manually for the trial font size
            let wrapped_height = self.calculate_wrapped_text_height(text, box_width, mid)?;

            eprintln!(
              " pt={} → wrapped size: w={} h={}",
              mid,
              box_width,
              wrapped_height
            );

            let h = wrapped_height;

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

    /// Calculate the height that text would take when wrapped to fit within the given width
    fn calculate_wrapped_text_height(&self, text: &str, box_width: u32, font_size: u16) -> Result<u32, String> {
        let mut lines = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_line = String::new();
        
        for word in words {
            let test_line = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };
            
            let dimensions = measure_text(&test_line, self.font.as_ref(), font_size, 1.0);
            if dimensions.width <= box_width as f32 {
                current_line = test_line;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = word.to_string();
                } else {
                    // Single word is too long, it becomes its own line
                    lines.push(word.to_string());
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        
        // Calculate total height with line spacing
        let line_height = font_size as f32 * 1.2; // Standard line spacing
        let total_height = lines.len() as f32 * line_height;
        
        Ok(total_height as u32)
    }

    pub fn get_fitting_text_info(
        &self,
        text: &str,
        box_width: u32,
        box_height: u32,
        min_pt: u16,
        max_pt: u16,
    ) -> Result<(u16, u32, u32), String> {
        let best_pt = self.find_fitting_size(text, box_width, box_height, min_pt, max_pt)?;
        let height = self.calculate_wrapped_text_height(text, box_width, best_pt)?;
        Ok((best_pt, box_width, height))
    }
}

// #################################################################
// ### UNIT TESTS TO PREVENT REGRESSIONS ###
// #################################################################
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // Test helper to create a FontManager.
    fn setup_font_manager() -> FontManager {
        // NOTE: This test requires a font file at the specified path.
        // A common font like DejaVuSans is used here, which is often found on Linux.
        // For other systems, you may need to change this path or place a font at `tests/font.ttf`.
        let font_path = Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        FontManager::new(&font_path.to_path_buf(), 16).expect("Failed to load font for testing")
    }

    #[test]
    fn test_simple_ascii_wrapping() {
        let fm = setup_font_manager();
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
        let fm = setup_font_manager();
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
        let fm = setup_font_manager();
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
