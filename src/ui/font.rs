// src/ui/font.rs
// Manages loading fonts, calculating text layouts, and rendering text.

use sdl2::pixels::Color;
use std::collections::VecDeque;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::ttf::{Font, Sdl2TtfContext, FontStyle};
use sdl2::video::Window;
use crate::Tracer;
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
    fn find_split_index(&mut self, span: &TextSpan, available_width: u32, use_ruby: bool) -> Result<usize, String> {
        let text = &span.text_to_use(use_ruby);
        let mut low = 0;
        let mut high = text.chars().count();
        let mut best_fit_char_index = 0;

        // Binary search to find the maximum number of characters that fit.
        while low <= high {
            let mid = low + (high - low) / 2;
            if mid == 0 {
                low = mid + 1;
                continue;
            }

            let substring: String = text.chars().take(mid).collect();
            let (substr_width, _) = self.size_of_text_with_style(&substring, span.is_bold, span.is_italic)?;

            if substr_width <= available_width {
                best_fit_char_index = mid; // This substring fits.
                low = mid + 1;             // Try a longer one.
            } else {
                high = mid - 1;            // Substring is too long.
            }
        }

        // Convert the character count back to a byte index for `split_at`.
        Ok(text.char_indices().nth(best_fit_char_index).map_or(0, |(idx, _)| idx))
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

        while let Some(mut span) = processed_spans.pop_front() {
            // Handle dedicated newline spans from our pre-processing.
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
                // The whole span fits on the current line.
                current_line_spans.push(span);
                current_line_width += span_width;
            } else {
                // The span does NOT fit. We must split it.
                let split_byte_index = self.find_split_index(&span, space_left, use_ruby)?;

                if split_byte_index > 0 {
                    // Part of the span fits.
                    let (fits, remaining) = span.text.split_at(split_byte_index);
                    
                    // Add the part that fits to the current line.
                    let mut fit_span = span.clone();
                    fit_span.text = fits.to_string();
                    current_line_spans.push(fit_span);
                    
                    // Put the remainder back at the front of the queue to be processed next.
                    span.text = remaining.to_string();
                    processed_spans.push_front(span);

                } else {
                    // Not even a single character fits on the current line.
                    // (This happens if `current_line_width` is > 0).
                    // So, we put the whole span back and start a new line.
                    processed_spans.push_front(span);
                }

                // Finalize the current line and start fresh.
                lines.push(current_line_spans);
                current_line_spans = Vec::new();
                current_line_width = 0;
            }
        }

        // Add the very last line.
        if !current_line_spans.is_empty() {
            lines.push(current_line_spans);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        let total_height = line_height * lines.len() as i32;
        Ok(TextLayout { lines, total_height, scroll_offset: 0 })
    }


    // /// Calculates how to wrap text and returns a layout object.
    // /// This processes TextSpans, handles explicit newlines, and implements
    // /// character-by-character wrapping to ensure content fits within max_width
    // pub fn layout_text(&mut self, spans: &[TextSpan], max_width: u32) -> Result<TextLayout, String> {
    //     let mut lines: Vec<Vec<TextSpan>> = Vec::new();
    //     let mut current_line_spans: Vec<TextSpan> = Vec::new();
    //     let mut current_line_width = 0;
    //     let line_height = self.font.height();
    //     let mut overflow = false;

    //     // Ensure at least one line to start with if spans are empty or to handle leading newlines
    //     if spans.is_empty() {
    //         lines.push(Vec::new());
    //         return Ok(TextLayout { lines, total_height: line_height as i32, scroll_offset: 0 });
    //     }

    //     for span in spans {

    //         // Get the text from the span. We will add it step-by-step to the current list of spans in this line.
    //         // If the line gets too long, we will split this span into two and add the current "line spans" to the line array.

    //         if span.new_text_block {
    //             if !current_line_spans.is_empty() {
    //                 lines.push(current_line_spans);
    //                 current_line_spans = Vec::new();
    //                 current_line_width = 0;
    //             }
    //         }


    //         let mut remaining_text_in_span: String = span.text.clone().trim().to_string();

    //         // Loop while there's text left in the current span to process
    //         while !remaining_text_in_span.is_empty() {
    //             let mut line_segment_chars_to_add = String::new(); // Reset the chars to add to the line from the current span.
    //             let mut chars_consumed_from_remaining = 0; // Chars in remaining_text_in_span that were processed for this line
    //             let mut current_segment_width_temp = 0; // Width of `line_segment_chars_to_add`
    //             let mut hard_break_encountered = false;

    //             // Iterate through characters of the remaining span text to build a line segment, until we run out of characters or space.
    //             for char_c in remaining_text_in_span.chars() {
    //                 if char_c == '\n' {
    //                     // Found a hard newline character, break the line here
    //                     hard_break_encountered = true;
    //                     chars_consumed_from_remaining += 1; // Consume the newline char
    //                     break; // Stop accumulating chars for this line segment
    //                 }

    //                 let (char_width, _) = self.size_of_text_with_style(
    //                     &char_c.to_string(),
    //                     span.is_bold, // Use original span's style for width calculation
    //                     span.is_italic
    //                 )?;

    //                 let potential_line_width = current_line_width + current_segment_width_temp + char_width;
    //                 if potential_line_width <= max_width {
    //                     line_segment_chars_to_add.push(char_c);
    //                     current_segment_width_temp += char_width;
    //                     chars_consumed_from_remaining += 1; // Count char taken from remaining_text_in_span

    //                     // println!("Characters to add; {:?}", line_segment_chars_to_add);
    //                 } else {
    //                     // Overflow.  Perform line processing and then return to the remaining characters via the for loop.
    //                     overflow = true;
    //                     break;
    //                 }
    //             }

    //             // println!("{:?}", line_segment_chars_to_add);
    //             // println!("{:?}", chars_consumed_from_remaining);
    //             // println!("{:?}", current_segment_width_temp);
    //             // println!("{:?}", hard_break_encountered);

    //             // This happens when a hard break (\n) is encountered, or a character is encountered, or we run out of characters in this current span.

    //             let requires_newline = hard_break_encountered || overflow;

    //             if requires_newline {
    //                 if hard_break_encountered {
    //                     // println!("LINEBREAK: Hard Break encountered", );
    //                     if !line_segment_chars_to_add.is_empty() {
    //                         let span = TextSpan {
    //                             text: line_segment_chars_to_add.clone(),
    //                             is_bold: span.is_bold,
    //                             is_italic: span.is_italic,
    //                             is_ruby_base: span.is_ruby_base,
    //                             ruby_text: span.ruby_text.clone(),
    //                             new_text_block: false,
    //                             is_newline: true,
    //                         };
    //                         // println!("{:?}", span);
    //                         current_line_spans.push(span);
    //                         current_line_width += current_segment_width_temp; // Use the accumulated width for efficiency
    //                     }
    //                     if !current_line_spans.is_empty() {
    //                         lines.push(current_line_spans);
    //                         current_line_spans = Vec::new();
    //                         current_line_width = 0;
    //                     } else {
    //                         lines.push(Vec::new());
    //                     }
    //                     overflow = false
    //                 }

    //                 if overflow {
    //                     // Create a span with the same properties and add it to the line.
    //                     // println!("LINEBREAK: Overflow encountered", );

    //                     let span = TextSpan {
    //                         text: line_segment_chars_to_add.clone(),
    //                         is_bold: span.is_bold,
    //                         is_italic: span.is_italic,
    //                         is_ruby_base: span.is_ruby_base,
    //                         ruby_text: span.ruby_text.clone(),
    //                         new_text_block: false,
    //                         is_newline: false,
    //                     };
    //                     current_line_spans.push(span);
    //                     // println!("Adding current line spans to lines: {:?}", current_line_spans);

    //                     lines.push(current_line_spans);
    //                     current_line_spans = Vec::new();
    //                     current_line_width = 0;
    //                     overflow = false
    //                 }
    //             }

    //             remaining_text_in_span = remaining_text_in_span.chars().skip(chars_consumed_from_remaining).collect();

    //             if remaining_text_in_span.is_empty() {
    //                 // println!("No text left in span", );

    //                 // The new line may or may not start a newline, but we aren't sure yet.  
    //                 let span = TextSpan {
    //                     text: line_segment_chars_to_add.clone(),
    //                     is_bold: span.is_bold,
    //                     is_italic: span.is_italic,
    //                     is_ruby_base: span.is_ruby_base,
    //                     ruby_text: span.ruby_text.clone(),
    //                     new_text_block: false,
    //                     is_newline: false,
    //                 };

    //                 current_line_spans.push(span);
    //                 // println!("Appending Span to current line spans: {:?}", current_line_spans);
    //                 current_line_width += current_segment_width_temp; // Use the accumulated width for efficiency
    //             }

    //         }

    //         // End the previously line and start a new one if we have a new text block span.
    //         if span.new_text_block {
    //             lines.push(current_line_spans);
    //             current_line_spans = Vec::new();
    //             current_line_width = 0;
    //         }

    //     }

    //     if !current_line_spans.is_empty() {
    //         lines.push(current_line_spans); // Always add the last line even if empty.
    //     }

        
    //     // Final check: if no lines were formed at all, ensure there's at least one empty line.
    //     if lines.is_empty() {
    //         lines.push(Vec::new());
    //     }

    //     let total_height = line_height * lines.len() as i32;
    //     // println!("{:?} Lines", lines);

    //     Ok(TextLayout {
    //         lines,
    //         total_height,
    //         scroll_offset: 0,
    //     })
    // }

    /// Renders a pre-calculated TextLayout to the screen.
    pub fn draw_layout(&mut self, canvas: &mut Canvas<Window>, layout: &TextLayout, x: i32, y: i32, show_ruby: bool) -> Result<(), String> {
        let line_height = self.font.height() as i32;
        let mut current_y = y - layout.scroll_offset;

        for line_spans in &layout.lines {
            // Only draw the line if it's within the visible area to improve performance.
            if current_y > -line_height && current_y < canvas.viewport().height() as i32 {
                let mut current_x = x;
                for span in line_spans {
                    // Decide which text to draw based on the show_ruby flag.
                    let text_to_draw = if show_ruby && span.is_ruby_base {
                        // If show_ruby is true and this is a ruby span,
                        // use the ruby_text. Fall back to the base text if for some
                        // reason ruby_text is None.
                        span.ruby_text.as_deref().unwrap_or(&span.text)
                    } else {
                        // Otherwise, draw the normal base text.
                        &span.text
                    };

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
}