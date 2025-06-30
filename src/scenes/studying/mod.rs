// src/scenes/studying/mod.rs

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas};
use sdl2::video::Window;

use crate::deck::Card;
use crate::scheduler::{Scheduler, Rating};
use crate::storage::{DatabaseManager, ReplayLogger};
use crate::ui::{FontManager, font::TextLayout, sprite::Sprite};

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

/// Draws the studying scene.
/// This function was moved from main.rs.
pub fn draw_studying_scene(
    canvas: &mut Canvas<Window>,
    studying_state: &mut StudyingState,
    font_manager: &mut FontManager,
    small_font_manager: &mut FontManager,
    hint_font_manager: &mut FontManager,
    sprite: &mut Sprite,
) -> Result<(), String> {
    let margin: u32 = 30;
    let total = studying_state.scheduler.total_session_cards();
    if total > 0 {
        let completed = studying_state.scheduler.reviews_complete();
        let bar_height = 25_u32;
        let bar_bg_rect = Rect::new(0, 0, 512, bar_height);
        canvas.set_draw_color(Color::RGB(60, 60, 60));
        canvas.fill_rect(bar_bg_rect)?;
        let progress = completed as f32 / total as f32;
        let progress_width = (512.0 * progress) as u32;
        let bar_fg_rect = Rect::new(0, 0, progress_width, bar_height);
        let r = (255.0 * (1.0 - progress)) as u8;
        let g = (255.0 * progress) as u8;
        canvas.set_draw_color(Color::RGB(r, g, 80));
        canvas.fill_rect(bar_fg_rect)?;
        let progress_text = format!("{}/{}", completed, total);
        let (text_w, text_h) = hint_font_manager.size_of_text(&progress_text)?;
        let text_x = (512 as i32 - text_w as i32 - 10).max(0);
        let text_y = (bar_height as i32 - text_h as i32) / 2;
        hint_font_manager.draw_single_line(canvas, &progress_text, text_x, text_y)?;
    }
    
    sprite.draw(canvas)?;
    let content_viewport = Rect::new(0, 25, 512, 305);
    canvas.set_clip_rect(Some(content_viewport));

    if !studying_state.is_answer_revealed {
        let layout_to_draw = if studying_state.show_ruby_text { &studying_state.front_layout_ruby } else { &studying_state.front_layout_default };
        if let Some(layout) = layout_to_draw {
            font_manager.draw_layout(canvas, layout, margin as i32, 40, studying_state.show_ruby_text)?;
        }
    } else {
        let mut y_pos = 40 - studying_state.scroll_offset;
        let small_front_layout_to_draw = if studying_state.show_ruby_text { &studying_state.small_front_layout_ruby } else { &studying_state.small_front_layout_default };
        let back_layout_to_draw = if studying_state.show_ruby_text { &studying_state.back_layout_ruby } else { &studying_state.back_layout_default };
        if let Some(layout) = small_front_layout_to_draw {
            small_font_manager.draw_layout(canvas, layout, margin as i32, y_pos, studying_state.show_ruby_text)?;
            y_pos += layout.total_height + 20;
        }
        if let Some(layout) = back_layout_to_draw {
            font_manager.draw_layout(canvas, layout, margin as i32, y_pos, studying_state.show_ruby_text)?;
        }
    }

    if studying_state.is_done {
        if let Some(layout) = &studying_state.done_layout {
            font_manager.draw_layout(canvas, layout, 150, 150, studying_state.show_ruby_text)?;
        }
    }
    
    canvas.set_clip_rect(None);
    if studying_state.is_answer_revealed {
        if let Some(hint_layout) = &studying_state.hint_layout {
            hint_font_manager.draw_layout(canvas, hint_layout, margin as i32, 335, studying_state.show_ruby_text)?;
        }
    }
    Ok(())
}
