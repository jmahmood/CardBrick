// src/scenes/studying/mod.rs

use macroquad::prelude::*;
use crate::deck::Card;
use crate::scheduler::Scheduler;
use crate::storage::{DatabaseManager, ReplayLogger};
use crate::ui::{FontManager, CanvasManager, font::TextLayout, sprite::Sprite};

pub mod input;
pub mod logic;

/// Contains the state specific to the studying screen.
pub struct StudyingState<'a> {
    pub is_done: bool,
    pub scheduler: Box<dyn Scheduler + 'a>,
    pub db_manager: DatabaseManager,
    pub replay_logger: ReplayLogger,
    pub current_card: Option<Card>,
    pub is_answer_revealed: bool,
    pub scroll_offset: i32,
    pub show_ruby_text: bool,
    pub front_layout_default: Option<TextLayout>,
    pub front_layout_ruby: Option<TextLayout>,
    pub back_layout_default: Option<TextLayout>,
    pub back_layout_ruby: Option<TextLayout>,
    pub small_front_layout_default: Option<TextLayout>,
    pub small_front_layout_ruby: Option<TextLayout>,
    pub hint_layout: Option<TextLayout>,
    pub done_layout: Option<TextLayout>,
}

impl<'a> StudyingState<'a> {
    /// Creates a new StudyingState.
    pub fn new(scheduler: Box<dyn Scheduler + 'a>, db_manager: DatabaseManager, replay_logger: ReplayLogger) -> Self {
        Self {
            is_done: false,
            scheduler,
            db_manager,
            replay_logger,
            current_card: None,
            is_answer_revealed: false,
            scroll_offset: 0,
            show_ruby_text: false,
            front_layout_default: None,
            front_layout_ruby: None,
            back_layout_default: None,
            back_layout_ruby: None,
            small_front_layout_default: None,
            small_front_layout_ruby: None,
            hint_layout: None,
            done_layout: None,
        }
    }
}

/// Draws the studying scene using macroquad rendering
/// Maintains all Japanese learning functionality including ruby text support
pub fn draw_studying_scene(
    studying_state: &mut StudyingState,
    font_manager: &FontManager,
    small_font_manager: &FontManager,
    hint_font_manager: &FontManager,
    sprite: &mut Sprite,
    canvas_manager: &CanvasManager,
) -> Result<(), String> {
    let margin: u32 = 30;
    let total = studying_state.scheduler.total_session_cards();
    
    // Draw progress bar
    if total > 0 {
        let completed = studying_state.scheduler.reviews_complete();
        let bar_height = 25.0;
        
        // Draw progress bar background
        let (bg_x, bg_y) = canvas_manager.logical_to_screen(0.0, 0.0);
        let bg_w = 512.0 * canvas_manager.get_scale_factor();
        let bg_h = bar_height * canvas_manager.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(60, 60, 60, 255));
        
        // Draw progress bar foreground with color gradient
        let progress = completed as f32 / total as f32;
        let progress_width = 512.0 * progress;
        let fg_w = progress_width * canvas_manager.get_scale_factor();
        let r = (255.0 * (1.0 - progress)) as u8;
        let g = (255.0 * progress) as u8;
        draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(r, g, 80, 255));
        
        // Draw progress text
        let progress_text = format!("{}/{}", completed, total);
        let (text_w, text_h) = hint_font_manager.size_of_text(&progress_text)?;
        let text_x = (512 as i32 - text_w as i32 - 10).max(0);
        let text_y = (bar_height as i32 - text_h as i32) / 2;
        hint_font_manager.draw_single_line(&progress_text, text_x, text_y, canvas_manager)?;
    }
    
    // Draw animated sprite
    sprite.draw(canvas_manager)?;
    
    // Set clipping rectangle for scrollable content area
    canvas_manager.set_clip_rect(0, 25, 512, 305);

    if !studying_state.is_answer_revealed {
        // Question mode: Show large front text only
        let layout_to_draw = if studying_state.show_ruby_text { 
            &studying_state.front_layout_ruby 
        } else { 
            &studying_state.front_layout_default 
        };
        
        if let Some(layout) = layout_to_draw {
            font_manager.draw_layout(layout, margin as i32, 40, studying_state.show_ruby_text, canvas_manager)?;
        }
    } else {
        // Answer mode: Show small front + full back text with scrolling
        let mut y_pos = 40 - studying_state.scroll_offset;
        
        // Draw small front text
        let small_front_layout_to_draw = if studying_state.show_ruby_text { 
            &studying_state.small_front_layout_ruby 
        } else { 
            &studying_state.small_front_layout_default 
        };
        
        if let Some(layout) = small_front_layout_to_draw {
            small_font_manager.draw_layout(layout, margin as i32, y_pos, studying_state.show_ruby_text, canvas_manager)?;
            y_pos += layout.total_height + 20;
        }
        
        // Draw back text (the answer)
        let back_layout_to_draw = if studying_state.show_ruby_text { 
            &studying_state.back_layout_ruby 
        } else { 
            &studying_state.back_layout_default 
        };
        
        if let Some(layout) = back_layout_to_draw {
            font_manager.draw_layout(layout, margin as i32, y_pos, studying_state.show_ruby_text, canvas_manager)?;
        }
    }

    // Draw "Deck Complete!" message if done
    if studying_state.is_done {
        if let Some(layout) = &studying_state.done_layout {
            font_manager.draw_layout(layout, 150, 150, studying_state.show_ruby_text, canvas_manager)?;
        }
    }
    
    // Clear clipping and draw hint text at bottom
    canvas_manager.clear_clip_rect();
    if studying_state.is_answer_revealed {
        if let Some(hint_layout) = &studying_state.hint_layout {
            hint_font_manager.draw_layout(hint_layout, margin as i32, 335, studying_state.show_ruby_text, canvas_manager)?;
        }
    }
    
    Ok(())
}
