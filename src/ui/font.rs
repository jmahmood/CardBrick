// src/ui/font.rs
// Manages loading fonts, calculating text layouts, and rendering text.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::ttf::{Font, Sdl2TtfContext, FontStyle};
use sdl2::video::Window;

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

impl<'a, 'b> FontManager<'a, 'b> {
    pub fn new(ttf_context: &'a Sdl2TtfContext, font_path: &str, font_size: u16) -> Result<Self, String> {
        let font = ttf_context.load_font(font_path, font_size)?;
        Ok(FontManager { ttf_context, font })
    }

    /// A new helper function to get the pixel dimensions of a string of text.
    /// This now considers the current style the font is set to.
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

    /// Calculates how to wrap text and returns a layout object.
    /// This processes TextSpans, handles explicit newlines, and implements
    /// character-by-character wrapping to ensure content fits within max_width,
    /// with a small epsilon for minor measurement discrepancies.
    pub fn layout_text(&mut self, spans: &[TextSpan], max_width: u32) -> Result<TextLayout, String> {
        let mut lines: Vec<Vec<TextSpan>> = Vec::new();
        let mut current_line_spans: Vec<TextSpan> = Vec::new();
        let mut current_line_width = 0;
        let line_height = self.font.height();

        // A small pixel tolerance to prevent off-by-one line breaks.
        // If `current_line_width + char_width` is slightly over `max_width` due to rounding,
        // this allows it to fit.
        let epsilon: u32 = 2; // Increased slightly for potential impact

        // Ensure at least one line to start with if spans are empty or to handle leading newlines
        if spans.is_empty() {
            lines.push(Vec::new());
            return Ok(TextLayout { lines, total_height: line_height as i32, scroll_offset: 0 });
        }

        for span in spans {
            // Handle explicit newline spans immediately and directly, ensuring one line break.
            if span.text == "\n" {
                if !current_line_spans.is_empty() {
                    lines.push(current_line_spans);
                    current_line_spans = Vec::new();
                    current_line_width = 0;
                }
                lines.push(Vec::new()); // Push an explicit empty line for the newline character
                continue; // Move to the next TextSpan
            }

            let mut remaining_text_in_span = span.text.clone(); // Mutable copy of text to process
            
            // Loop while there's text left in the current span to process
            while !remaining_text_in_span.is_empty() {
                let mut line_segment_chars_to_add = String::new(); // Chars for THIS line's part of the span
                let mut chars_consumed_from_remaining = 0; // How many chars from remaining_text_in_span were processed for this line
                let mut current_segment_width_temp = 0; // Width of `line_segment_chars_to_add`
                let mut hard_break_encountered = false;

                // Iterate through characters of the remaining span text to build a line segment
                for char_c in remaining_text_in_span.chars() {
                    if char_c == '\n' {
                        // Found a hard newline character, break the line here
                        hard_break_encountered = true;
                        chars_consumed_from_remaining += 1; // Consume the newline char
                        break; // Stop accumulating chars for this line segment
                    }

                    let (char_width, _) = self.size_of_text_with_style(
                        &char_c.to_string(),
                        span.is_bold, // Use original span's style for width calculation
                        span.is_italic
                    )?;

                    // Check if adding this character makes the current line too wide with epsilon
                    // current_line_width: width of spans already committed to current_line_spans
                    // current_segment_width_temp: width of chars accumulated for line_segment_chars_to_add
                    // char_width: width of the character being considered
                    if current_line_width + current_segment_width_temp + char_width > (max_width + epsilon) {
                        // If line_segment_chars_to_add is empty, it means this single char won't fit on this line.
                        // We must still process it, but it will start a new line and possibly overflow.
                        if line_segment_chars_to_add.is_empty() {
                            // If current line has content, break it here. The char will start a new line.
                            // Otherwise, it's an empty line, and the char is too wide, so it will simply overflow.
                            if current_line_width > 0 || !current_line_spans.is_empty() {
                                break; // Break from inner loop, current char starts a new line (will be handled below)
                            }
                        } else {
                            // The accumulated `line_segment_chars_to_add` fits, but `char_c` won't.
                            // Break here, `char_c` starts a new segment on a new line.
                            break;
                        }
                    }
                    
                    line_segment_chars_to_add.push(char_c);
                    current_segment_width_temp += char_width;
                    chars_consumed_from_remaining += 1; // Count char taken from remaining_text_in_span
                }

                // --- CRITICAL FIX FOR INFINITE LOOP (from previous turn) ---
                if chars_consumed_from_remaining == 0 && !remaining_text_in_span.is_empty() && !hard_break_encountered {
                    let first_char = remaining_text_in_span.chars().next().unwrap();
                    let char_str = first_char.to_string();
                    let (char_width, _) = self.size_of_text_with_style(&char_str, span.is_bold, span.is_italic)?;

                    // If the current line has content, commit it first (this is the line that "lacks the last character").
                    if !current_line_spans.is_empty() {
                        println!("Newline: Critial Fix - Committing previous line before overflow char: {:?}", current_line_spans);
                        lines.push(current_line_spans);
                        current_line_spans = Vec::new();
                        current_line_width = 0;
                    }
                    
                    // Now, add the single problematic character to a new current_line_spans.
                    current_line_spans.push(TextSpan {
                        text: char_str.clone(),
                        is_bold: span.is_bold,
                        is_italic: span.is_italic,
                        is_ruby_base: span.is_ruby_base,
                        ruby_text: span.ruby_text.clone(),
                    });
                    current_line_width += char_width;

                    // Commit this new line (containing only the problematic character).
                    println!("Newline: Critial Fix - Committing new line with overflow char: {:?}", current_line_spans);
                    // lines.push(current_line_spans);
                    // current_line_spans = Vec::new();
                    // current_line_width = 0;

                    // Consume this single character from remaining_text_in_span.
                    remaining_text_in_span = remaining_text_in_span.chars().skip(1).collect();

                    continue; // Proceed to the next iteration of the outer while loop.
                }
                // --- END CRITICAL FIX ---


                // Add the accumulated line segment to the current line's spans (if any chars were added)
                if !line_segment_chars_to_add.is_empty() {
                    current_line_spans.push(TextSpan {
                        text: line_segment_chars_to_add.clone(), // Use clone as original span text is immutable
                        is_bold: span.is_bold,
                        is_italic: span.is_italic,
                        is_ruby_base: span.is_ruby_base,
                        ruby_text: span.ruby_text.clone(), // Corrected line
                    });
                    current_line_width += current_segment_width_temp; // Use the accumulated width for efficiency
                }

                // Consume the processed characters from `remaining_text_in_span`
                remaining_text_in_span = remaining_text_in_span.chars().skip(chars_consumed_from_remaining).collect();

                // Decide whether to commit the current line and start a new one
                // This happens if a hard break was found, or if we finished processing the span's text for this line segment.
                if hard_break_encountered || (chars_consumed_from_remaining > 0 && remaining_text_in_span.is_empty()) {
                    // Commit the current line if it has content
                    if !current_line_spans.is_empty() {
                        lines.push(current_line_spans);
                        current_line_spans = Vec::new();
                        current_line_width = 0;
                    } else if hard_break_encountered {
                        // If it was just a newline, and the line was empty, push an explicit empty line
                        lines.push(Vec::new());
                    }
                }
            }
        }

        // Add any remaining spans to the last line if not empty
        if !current_line_spans.is_empty() {
            lines.push(current_line_spans);
        }
        
        // Final check: if no lines were formed at all, ensure there's at least one empty line.
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        let total_height = line_height * lines.len() as i32;

        Ok(TextLayout {
            lines,
            total_height,
            scroll_offset: 0,
        })
    }

    /// Renders a pre-calculated TextLayout to the screen.
    pub fn draw_layout(&mut self, canvas: &mut Canvas<Window>, layout: &TextLayout, x: i32, y: i32) -> Result<(), String> {
        let line_height = self.font.height() as i32;
        let mut current_y = y - layout.scroll_offset;

        for line_spans in &layout.lines {
            // Only draw the line if it's within the visible area to improve performance.
            if current_y > -line_height && current_y < canvas.viewport().height() as i32 {
                let mut current_x = x;
                for span in line_spans {
                    let (text_w, _) = self.draw_text_span_segment(canvas, &span.text, current_x, current_y, span.is_bold, span.is_italic)?;
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
}