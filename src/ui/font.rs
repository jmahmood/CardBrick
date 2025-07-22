// src/ui/font.rs

// Manages loading fonts, calculating text layouts, and rendering text with macroquad.

use std::path::PathBuf;
use macroquad::prelude::*;
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

    /// Draw text with top-left corner positioning instead of baseline positioning
    pub fn draw_line_top_left(&self, text: &str, x: i32, top_y: i32, canvas_manager: &crate::ui::CanvasManager) -> Result<(), String> {
        let (ascent, _, _) = self.metrics();
        let baseline_y = top_y + ascent as i32;
        self.draw_text_span_segment(text, x, baseline_y, false, false, canvas_manager)?;
        Ok(())
    }
    
    pub fn size_of_text(&self, text: &str) -> Result<(u32, u32), String> {
        self.size_of_text_with_style(text, false, false)
    }

    /// Get font metrics (ascent, descent, line_gap) in logical pixels
    pub fn metrics(&self) -> (f32, f32, f32) {
        // For macroquad, we approximate font metrics based on font size
        // These are reasonable defaults for most fonts
        let ascent = self.font_size * 0.8;    // ~80% of font size is above baseline
        let descent = self.font_size * 0.2;   // ~20% of font size is below baseline  
        let line_gap = self.font_size * 0.1;  // ~10% for line spacing
        (ascent, descent, line_gap)
    }

    /// Get total line height (ascent + descent + line_gap)
    pub fn line_height(&self) -> f32 {
        let (ascent, descent, line_gap) = self.metrics();
        ascent + descent + line_gap
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

// Note: UI font tests removed due to macroquad graphics context dependency
// These tests require a full graphics initialization which is not available
// in headless test environments. Font layout and rendering functionality
// is tested through integration tests that run the full application.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_span_text_to_use_without_ruby() {
        let span = TextSpan {
            text: "Hello world".to_string(),
            ruby_text: Some("ハロー ワールド".to_string()),
            is_bold: false,
            is_italic: false,
            is_ruby_base: true,
            new_text_block: false,
            is_newline: false,
        };
        
        // When use_ruby is false, should return base text
        assert_eq!(span.text_to_use(false), "Hello world");
    }

    #[test]
    fn test_text_span_text_to_use_with_ruby() {
        let span = TextSpan {
            text: "漢字".to_string(),
            ruby_text: Some("かんじ".to_string()),
            is_bold: false,
            is_italic: false,
            is_ruby_base: true,
            new_text_block: false,
            is_newline: false,
        };
        
        // When use_ruby is true, should return ruby text
        assert_eq!(span.text_to_use(true), "かんじ");
    }

    #[test]
    fn test_text_span_text_to_use_no_ruby_available() {
        let span = TextSpan {
            text: "English text".to_string(),
            ruby_text: None,
            is_bold: false,
            is_italic: false,
            is_ruby_base: false,
            new_text_block: false,
            is_newline: false,
        };
        
        // When no ruby text is available, should fall back to base text
        assert_eq!(span.text_to_use(true), "English text");
        assert_eq!(span.text_to_use(false), "English text");
    }

    #[test]
    fn test_text_span_text_to_use_empty_ruby() {
        let span = TextSpan {
            text: "Base text".to_string(),
            ruby_text: Some("".to_string()),
            is_bold: false,
            is_italic: false,
            is_ruby_base: false,
            new_text_block: false,
            is_newline: false,
        };
        
        // When ruby text is empty, should use it if requested
        assert_eq!(span.text_to_use(true), "");
        assert_eq!(span.text_to_use(false), "Base text");
    }

    #[test]
    fn test_text_span_formatting_flags() {
        let span = TextSpan {
            text: "Formatted text".to_string(),
            ruby_text: None,
            is_bold: true,
            is_italic: true,
            is_ruby_base: false,
            new_text_block: true,
            is_newline: false,
        };
        
        assert!(span.is_bold);
        assert!(span.is_italic);
        assert!(span.new_text_block);
        assert!(!span.is_ruby_base);
        assert!(!span.is_newline);
    }

    #[test]
    fn test_text_span_newline_flag() {
        let span = TextSpan {
            text: "\n".to_string(),
            ruby_text: None,
            is_bold: false,
            is_italic: false,
            is_ruby_base: false,
            new_text_block: false,
            is_newline: true,
        };
        
        assert!(span.is_newline);
        assert_eq!(span.text, "\n");
    }

    #[test]
    fn test_text_span_default_values() {
        let span = TextSpan::default();
        
        assert_eq!(span.text, "");
        assert_eq!(span.ruby_text, None);
        assert!(!span.is_bold);
        assert!(!span.is_italic);
        assert!(!span.is_ruby_base);
        assert!(!span.new_text_block);
        assert!(!span.is_newline);
    }

    #[test]
    fn test_text_span_ruby_text_with_complex_characters() {
        let span = TextSpan {
            text: "今日".to_string(),
            ruby_text: Some("きょう".to_string()),
            is_bold: false,
            is_italic: false,
            is_ruby_base: true,
            new_text_block: false,
            is_newline: false,
        };
        
        // Test with Japanese characters
        assert_eq!(span.text_to_use(true), "きょう");
        assert_eq!(span.text_to_use(false), "今日");
    }

    #[test]
    fn test_text_layout_creation() {
        let layout = TextLayout {
            lines: vec![
                vec![TextSpan {
                    text: "Line 1".to_string(),
                    ruby_text: None,
                    is_bold: false,
                    is_italic: false,
                    is_ruby_base: false,
                    new_text_block: false,
                    is_newline: false,
                }]
            ],
            total_height: 20,
            scroll_offset: 0,
        };
        
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.lines[0].len(), 1);
        assert_eq!(layout.lines[0][0].text, "Line 1");
        assert_eq!(layout.total_height, 20);
    }

    #[test]
    fn test_font_manager_from_loaded_font() {
        let font_manager = FontManager::from_loaded_font(None, None, 16);
        
        assert!(font_manager.font.is_none());
        assert!(font_manager.fallback_font.is_none());
        assert_eq!(font_manager.font_size, 16.0);
    }
}
