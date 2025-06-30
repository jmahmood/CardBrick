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

    /// Calculates how to wrap text and returns a layout object.
    /// This processes TextSpans, handles explicit newlines, and implements
    /// character-by-character wrapping to ensure content fits within max_width
    pub fn layout_text(&mut self, spans: &[TextSpan], max_width: u32) -> Result<TextLayout, String> {
        let mut lines: Vec<Vec<TextSpan>> = Vec::new();
        let mut current_line_spans: Vec<TextSpan> = Vec::new();
        let mut current_line_width = 0;
        let line_height = self.font.height();
        let mut overflow = false;

        // Ensure at least one line to start with if spans are empty or to handle leading newlines
        if spans.is_empty() {
            lines.push(Vec::new());
            return Ok(TextLayout { lines, total_height: line_height as i32, scroll_offset: 0 });
        }

        for span in spans {

            // Get the text from the span. We will add it step-by-step to the current list of spans in this line.
            // If the line gets too long, we will split this span into two and add the current "line spans" to the line array.

            if span.new_text_block {
                if !current_line_spans.is_empty() {
                    lines.push(current_line_spans);
                    current_line_spans = Vec::new();
                    current_line_width = 0;
                }
            }


            let mut remaining_text_in_span: String = span.text.clone().trim().to_string();

            // Loop while there's text left in the current span to process
            while !remaining_text_in_span.is_empty() {
                let mut line_segment_chars_to_add = String::new(); // Reset the chars to add to the line from the current span.
                let mut chars_consumed_from_remaining = 0; // Chars in remaining_text_in_span that were processed for this line
                let mut current_segment_width_temp = 0; // Width of `line_segment_chars_to_add`
                let mut hard_break_encountered = false;

                // Iterate through characters of the remaining span text to build a line segment, until we run out of characters or space.
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

                    let potential_line_width = current_line_width + current_segment_width_temp + char_width;
                    if potential_line_width <= max_width {
                        line_segment_chars_to_add.push(char_c);
                        current_segment_width_temp += char_width;
                        chars_consumed_from_remaining += 1; // Count char taken from remaining_text_in_span

                        println!("Characters to add; {:?}", line_segment_chars_to_add);
                    } else {
                        // Overflow.  Perform line processing and then return to the remaining characters via the for loop.
                        overflow = true;
                        break;
                    }
                }

                println!("{:?}", line_segment_chars_to_add);
                println!("{:?}", chars_consumed_from_remaining);
                println!("{:?}", current_segment_width_temp);
                println!("{:?}", hard_break_encountered);

                // This happens when a hard break (\n) is encountered, or a character is encountered, or we run out of characters in this current span.

                let requires_newline = hard_break_encountered || overflow;

                if requires_newline {
                    if hard_break_encountered {
                        println!("LINEBREAK: Hard Break encountered", );
                        if !line_segment_chars_to_add.is_empty() {
                            let span = TextSpan {
                                text: line_segment_chars_to_add.clone(),
                                is_bold: span.is_bold,
                                is_italic: span.is_italic,
                                is_ruby_base: span.is_ruby_base,
                                ruby_text: span.ruby_text.clone(),
                                new_text_block: false,
                            };
                            println!("{:?}", span);
                            current_line_spans.push(span);
                            current_line_width += current_segment_width_temp; // Use the accumulated width for efficiency
                        }
                        if !current_line_spans.is_empty() {
                            lines.push(current_line_spans);
                            current_line_spans = Vec::new();
                            current_line_width = 0;
                        } else {
                            lines.push(Vec::new());
                        }
                        overflow = false
                    }

                    if overflow {
                        // Create a span with the same properties and add it to the line.
                        println!("LINEBREAK: Overflow encountered", );

                        let span = TextSpan {
                            text: line_segment_chars_to_add.clone(),
                            is_bold: span.is_bold,
                            is_italic: span.is_italic,
                            is_ruby_base: span.is_ruby_base,
                            ruby_text: span.ruby_text.clone(),
                            new_text_block: false,
                        };
                        current_line_spans.push(span);
                        println!("Adding current line spans to lines: {:?}", current_line_spans);

                        lines.push(current_line_spans);
                        current_line_spans = Vec::new();
                        current_line_width = 0;
                        overflow = false
                    }
                }

                remaining_text_in_span = remaining_text_in_span.chars().skip(chars_consumed_from_remaining).collect();

                if remaining_text_in_span.is_empty() {
                    println!("No text left in span", );

                    // The new line may or may not start a newline, but we aren't sure yet.  
                    let span = TextSpan {
                        text: line_segment_chars_to_add.clone(),
                        is_bold: span.is_bold,
                        is_italic: span.is_italic,
                        is_ruby_base: span.is_ruby_base,
                        ruby_text: span.ruby_text.clone(),
                        new_text_block: false,
                    };

                    current_line_spans.push(span);
                    println!("Appending Span to current line spans: {:?}", current_line_spans);
                    current_line_width += current_segment_width_temp; // Use the accumulated width for efficiency
                }

            }

            // End the previously line and start a new one if we have a new text block span.
            if span.new_text_block {
                lines.push(current_line_spans);
                current_line_spans = Vec::new();
                current_line_width = 0;
            }

        }

        if !current_line_spans.is_empty() {
            lines.push(current_line_spans); // Always add the last line even if empty.
        }

        
        // Final check: if no lines were formed at all, ensure there's at least one empty line.
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        let total_height = line_height * lines.len() as i32;
        println!("{:?} Lines", lines);

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