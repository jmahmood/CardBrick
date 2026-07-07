// src/scenes/progress/mod.rs
// Progress viewer scene for Sprint 3

use crate::ui::progress::{load_recent_progress, DailyProgress};
use crate::ui::{font::TextLayout, CanvasManager, FontManager};
use macroquad::prelude::*;

pub mod input;

/// Progress viewer state
pub struct ProgressState {
    pub is_done: bool,
    pub progress_data: Vec<DailyProgress>,
    pub scroll_offset: i32,
    pub header_layout: Option<TextLayout>,
    pub row_layouts: Vec<TextLayout>,
    pub error_layout: Option<TextLayout>,
}

impl ProgressState {
    /// Creates a new ProgressState and loads recent progress data
    pub fn new() -> Self {
        let mut state = Self {
            is_done: false,
            progress_data: Vec::new(),
            scroll_offset: 0,
            header_layout: None,
            row_layouts: Vec::new(),
            error_layout: None,
        };

        // Load last 14 days of progress
        match load_recent_progress(14) {
            Ok(data) => {
                state.progress_data = data;
                println!(
                    "📊 Loaded {} days of progress data",
                    state.progress_data.len()
                );
            }
            Err(e) => {
                eprintln!("Failed to load progress data: {}", e);
            }
        }

        state
    }
}

/// Draw the progress viewer scene
pub fn draw_progress_scene(
    progress_state: &mut ProgressState,
    font_manager: &FontManager,
    small_font_manager: &FontManager,
    canvas_manager: &CanvasManager,
) -> Result<(), String> {
    let (logical_width, logical_height) = canvas_manager.logical_size();
    let scale = canvas_manager.scale_factor;

    // Draw background
    let (bg_x, bg_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let bg_w = logical_width * scale;
    let bg_h = logical_height * scale;
    draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(25, 30, 40, 255));

    // Build layouts if not already built
    if progress_state.header_layout.is_none() && !progress_state.progress_data.is_empty() {
        build_progress_layouts(progress_state, small_font_manager)?;
    }

    let margin = 30;
    let mut y_pos = 50 - progress_state.scroll_offset;
    let font_ascent = small_font_manager.metrics().0;

    // Draw title
    let title_text = "📊 Progress Viewer";
    let (title_w, _) = font_manager.size_of_text(title_text)?;
    let title_x = (logical_width as i32 - title_w as i32) / 2;
    font_manager.draw_line_top_left(title_text, title_x, 20, canvas_manager)?;

    // Draw header if available
    if let Some(header_layout) = &progress_state.header_layout {
        if y_pos > -50 && y_pos < logical_height as i32 + 50 {
            small_font_manager.draw_layout(
                header_layout,
                margin,
                y_pos + font_ascent as i32,
                false,
                canvas_manager,
            )?;
        }
        y_pos += header_layout.total_height as i32 + 10;
    }

    // Draw progress rows
    for layout in &progress_state.row_layouts {
        if y_pos > -50 && y_pos < logical_height as i32 + 50 {
            small_font_manager.draw_layout(
                layout,
                margin,
                y_pos + font_ascent as i32,
                false,
                canvas_manager,
            )?;
        }
        y_pos += layout.total_height as i32 + 5;
    }

    // Draw error message if no data
    if progress_state.progress_data.is_empty() {
        if let Some(error_layout) = &progress_state.error_layout {
            let error_y = (logical_height as i32 - error_layout.total_height as i32) / 2;
            small_font_manager.draw_layout(
                error_layout,
                margin,
                error_y + font_ascent as i32,
                false,
                canvas_manager,
            )?;
        }
    }

    // Draw controls at bottom
    let controls_text = "↑/↓: Scroll  B: Return to Menu";
    let (controls_w, _) = small_font_manager.size_of_text(controls_text)?;
    let controls_x = (logical_width as i32 - controls_w as i32) / 2;
    let controls_y = logical_height as i32 - 30;
    small_font_manager.draw_line_top_left(controls_text, controls_x, controls_y, canvas_manager)?;

    Ok(())
}

/// Build text layouts for progress data
fn build_progress_layouts(
    progress_state: &mut ProgressState,
    font_manager: &FontManager,
) -> Result<(), String> {
    use crate::deck::html_parser;

    progress_state.row_layouts.clear();

    if progress_state.progress_data.is_empty() {
        // Build error message layout
        let error_text = "No progress data available yet.\nStart studying to see your progress!";
        let spans = html_parser::parse_html_to_spans(error_text);
        progress_state.error_layout = font_manager.layout_text_binary(&spans, 400, false).ok();
        return Ok(());
    }

    // Build header layout
    let header_text = "Date       | Cards | Points | Goal";
    let header_spans = html_parser::parse_html_to_spans(header_text);
    progress_state.header_layout = font_manager
        .layout_text_binary(&header_spans, 450, false)
        .ok();

    // Build row layouts
    for day in &progress_state.progress_data {
        let cards_studied = day.cards_studied.map_or("?".to_string(), |x| x.to_string());
        let points = day.points.map_or("?".to_string(), |x| x.to_string());
        let goal_achieved = day
            .reward_bin
            .map_or("?", |achieved| if achieved { "🎯" } else { "○" });

        let row_text = format!(
            "{} | {:>5} | {:>6} | {}",
            day.date, cards_studied, points, goal_achieved
        );
        let row_spans = html_parser::parse_html_to_spans(&row_text);

        if let Ok(layout) = font_manager.layout_text_binary(&row_spans, 450, false) {
            progress_state.row_layouts.push(layout);
        }
    }

    Ok(())
}
