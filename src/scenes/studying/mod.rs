// src/scenes/studying/mod.rs

use macroquad::prelude::*;
use crate::deck::Card;
use crate::scheduler::Scheduler;
use crate::storage::{DatabaseManager, ReplayLogger};
use crate::ui::{FontManager, CanvasManager, font::TextLayout, sprite::Sprite};

pub mod input;
pub mod logic;

/// How we render the studying scene
#[derive(Debug, Clone, PartialEq)]
pub enum StudyingScreenMode {
    InProgress,          // normal Q&A flow
    SessionComplete,     // daily goal banner shown
    ExhaustedDeck,       // no more cards in deck banner
    SessionDetails,      // detailed view of challenging cards from this session
}

/// Contains the state specific to the studying screen.
pub struct StudyingState<'a> {
    pub is_done: bool,
    pub mode: StudyingScreenMode,
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
    pub banner_layout: Option<TextLayout>,
    pub banner_started: Option<f32>,   // animation timer
    pub detail_layouts: Vec<TextLayout>,  // layouts for challenging cards detail view
    pub detail_scroll_offset: i32,       // scroll offset for detail view
}

impl<'a> StudyingState<'a> {
    /// Creates a new StudyingState.
    pub fn new(scheduler: Box<dyn Scheduler + 'a>, db_manager: DatabaseManager, replay_logger: ReplayLogger) -> Self {
        Self {
            is_done: false,
            mode: StudyingScreenMode::InProgress,
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
            banner_layout: None,
            banner_started: None,
            detail_layouts: Vec::new(),
            detail_scroll_offset: 0,
        }
    }
}

/// Draw a centered retro panel behind the banner.
fn draw_retro_panel(canvas: &CanvasManager, layout: &TextLayout) {
    let (_, screen_h) = canvas.logical_size();
    // Use a reasonable fixed width based on typical banner text length
    let panel_w = 380.0 + 40.0;  // banner text width + padding
    let panel_h = layout.total_height as f32 + 40.0;
    let panel_x = (512.0 - panel_w) / 2.0;
    let panel_y = (screen_h - panel_h) / 2.0;

    // Convert to screen coordinates
    let (screen_x, screen_y) = canvas.logical_to_screen(panel_x, panel_y);
    let scale = canvas.get_scale_factor();

    // Backdrop (80% opaque black)
    draw_rectangle(
        screen_x, screen_y, 
        panel_w * scale, panel_h * scale,
        Color::from_rgba(0, 0, 0, 204),
    );

    // 1-px white border (scaled appropriately)
    draw_rectangle_lines(
        screen_x, screen_y, 
        panel_w * scale, panel_h * scale, 
        1.0 * scale, WHITE
    );
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
    // Layout constants
    const BAR_HEIGHT: f32 = 25.0;
    const CONTENT_TOP: f32 = BAR_HEIGHT + 15.0;
    
    let margin: u32 = 30;
    let total = studying_state.scheduler.total_session_cards();
    let (logical_width, logical_height) = canvas_manager.logical_size();
    let scale = canvas_manager.scale_factor;

    let mut clip_cam = Camera2D::from_display_rect(Rect::new(0., 0., logical_width * scale, logical_height * scale - (CONTENT_TOP + 10.0) * scale));

    // B) CRITICAL FIX: Invert the camera's Y-axis. This prevents the text from being flipped upside down.
    clip_cam.zoom.y = -clip_cam.zoom.y;

    // C) Define the viewport on the SCREEN where the scrolling content should appear.
    let (viewport_x, viewport_y) = canvas_manager.logical_to_screen(0.0, CONTENT_TOP);
    let viewport_w = logical_width;
    let viewport_h = logical_height - CONTENT_TOP;
    clip_cam.viewport = Some((viewport_x as i32, viewport_y as i32, (viewport_w * scale) as i32, (viewport_h * scale - CONTENT_TOP * scale) as i32));


    // Draw progress bar
    if total > 0 {
        let completed = studying_state.scheduler.reviews_complete();
        
        // Draw progress bar background
        let (bg_x, bg_y) = canvas_manager.logical_to_screen(0.0, 0.0);
        let bg_w = 512.0 * canvas_manager.get_scale_factor();
        let bg_h = BAR_HEIGHT * canvas_manager.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(60, 60, 60, 255));
        
        // Draw progress bar foreground with color gradient
        let progress = completed as f32 / total as f32;
        let progress_width = 512.0 * progress;
        let fg_w = progress_width * canvas_manager.get_scale_factor();
        let r = (255.0 * (1.0 - progress)) as u8;
        let g = (255.0 * progress) as u8;
        draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(r, g, 80, 255));
        
        // Draw progress text with proper top-left positioning
        let progress_text = format!("{}/{}", completed, total);
        let (tw, th) = hint_font_manager.size_of_text(&progress_text)?;
        
        // Centre the bounding box in the bar
        let text_x = 512.0 - tw as f32 - 10.0;
        let text_y = (BAR_HEIGHT - th as f32) / 2.0;
        hint_font_manager.draw_line_top_left(&progress_text, text_x as i32, text_y as i32, canvas_manager)?;
    }
    
    // Draw animated sprite
    sprite.draw(canvas_manager)?;
    
    // Set clipping rectangle for scrollable content area - start after progress bar
    // canvas_manager.set_clip_rect(0, BAR_HEIGHT as i32, 512, (logical_height - BAR_HEIGHT) as u32);
    
    // Calculate content origin with proper ascent handling
    let font_ascent = font_manager.metrics().0;
    let small_font_ascent = small_font_manager.metrics().0;

    // Branch on current screen mode
    match studying_state.mode {
        StudyingScreenMode::InProgress => {

            // Normal Q&A flow
            if !studying_state.is_answer_revealed {
                // Question mode: Show large front text only
                let layout_to_draw = if studying_state.show_ruby_text { 
                    &studying_state.front_layout_ruby 
                } else { 
                    &studying_state.front_layout_default 
                };
                
                if let Some(layout) = layout_to_draw {
                    let y_pos = CONTENT_TOP as i32 - studying_state.scroll_offset;
                    set_camera(&clip_cam);
                    font_manager.draw_layout(layout, margin as i32, y_pos + font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    set_default_camera(); // IMPORTANT – disable clipping afterwards
                }
            } else {
                // Answer mode: Show small front + full back text with scrolling
                let mut y_pos = CONTENT_TOP as i32 - studying_state.scroll_offset;
                
                // Draw small front text
                let small_front_layout_to_draw = if studying_state.show_ruby_text { 
                    &studying_state.small_front_layout_ruby 
                } else { 
                    &studying_state.small_front_layout_default 
                };
                
                if let Some(layout) = small_front_layout_to_draw {
                    set_camera(&clip_cam);
                    small_font_manager.draw_layout(layout, margin as i32, y_pos + small_font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    y_pos += layout.total_height + 20;
                    set_default_camera(); // IMPORTANT – disable clipping afterwards

                }
                
                // Draw back text (the answer)
                let back_layout_to_draw = if studying_state.show_ruby_text { 
                    &studying_state.back_layout_ruby 
                } else { 
                    &studying_state.back_layout_default 
                };
                
                if let Some(layout) = back_layout_to_draw {
                    set_camera(&clip_cam);
                    font_manager.draw_layout(layout, margin as i32, y_pos + font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    set_default_camera(); // IMPORTANT – disable clipping afterwards

                }
            }
            
            // Draw hint text at bottom for answer mode
            if studying_state.is_answer_revealed {
                if let Some(hint_layout) = &studying_state.hint_layout {
                    let hint_ascent = hint_font_manager.metrics().0;
                    let hint_y = logical_height - hint_layout.total_height as f32 - 10.0; // 10px from bottom
                    hint_font_manager.draw_layout(hint_layout, margin as i32, hint_y as i32 + hint_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                }
            }
            set_default_camera(); // IMPORTANT – disable clipping afterwards
        },
        
        StudyingScreenMode::SessionComplete | StudyingScreenMode::ExhaustedDeck => {
            // Banner display mode
            if let Some(layout) = &studying_state.banner_layout {
                // Draw retro panel first
                draw_retro_panel(canvas_manager, layout);

                // Calculate centered position for banner text
                let x_center = (512 - 380) / 2;  // Center based on layout width used in logic.rs
                let y_center = (logical_height - layout.total_height as f32) / 2.0;
                
                font_manager.draw_layout(
                    layout,
                    x_center as i32,
                    y_center as i32 + font_ascent as i32,
                    studying_state.show_ruby_text,
                    canvas_manager,
                )?;

                // Draw colored progress boxes for SessionComplete mode
                if studying_state.mode == StudyingScreenMode::SessionComplete {
                    println!("DEBUG: About to call draw_session_progress_boxes");
                    draw_session_progress_boxes(studying_state, canvas_manager, y_center + layout.total_height as f32 + 10.0)?;
                    println!("DEBUG: draw_session_progress_boxes completed");
                }

                // Draw flashing prompt below banner/progress
                let prompt_text = match studying_state.mode {
                    StudyingScreenMode::SessionComplete => "Press A to Continue, B for Menu, R for Details",
                    StudyingScreenMode::ExhaustedDeck => "Press B to Return to Menu",
                    _ => "",
                };
                
                // Flash the prompt every 0.5 seconds
                if (get_time() * 2.0).floor() as i32 % 2 == 0 {
                    let (prompt_w, _) = hint_font_manager.size_of_text(prompt_text)?;
                    let prompt_x = (512 - prompt_w) / 2;
                    let prompt_y = if studying_state.mode == StudyingScreenMode::SessionComplete {
                        y_center + layout.total_height as f32 + 50.0  // Extra space for progress boxes
                    } else {
                        y_center + layout.total_height as f32 + 24.0
                    };
                    let hint_ascent = hint_font_manager.metrics().0;
                    
                    hint_font_manager.draw_line_top_left(
                        prompt_text,
                        prompt_x as i32,
                        prompt_y as i32 + hint_ascent as i32,
                        canvas_manager,
                    )?;
                }
            }
        }
        
        StudyingScreenMode::SessionDetails => {
            // Detail view of challenging cards
            draw_session_details(studying_state, small_font_manager, canvas_manager)?;
        }
    }
    
    Ok(())
}

/// Draw colored progress boxes showing today's performance (persistent across sessions)
fn draw_session_progress_boxes(studying_state: &StudyingState, canvas_manager: &CanvasManager, y_position: f32) -> Result<(), String> {
    
    // Get daily ratings from database instead of session-only ratings
    let daily_ratings = match studying_state.db_manager.get_todays_ratings() {
        Ok(ratings) => ratings,
        Err(e) => {
            println!("DEBUG: Failed to get daily ratings: {}", e);
            return Ok(()); // No ratings to display
        }
    };
    
    let total_cards = daily_ratings.len();
    
    println!("DEBUG: Daily ratings count: {}", total_cards);
    for (i, (card_id, rating_str)) in daily_ratings.iter().enumerate() {
        println!("DEBUG: Card {}: ID={}, Rating={}", i, card_id, rating_str);
    }
    
    if total_cards == 0 {
        println!("DEBUG: No daily ratings found, not drawing progress bar");
        return Ok(()); // No ratings to display
    }
    
    // Calculate progressive compression based on total cards completed
    let box_width = if total_cards <= 12 {
        25.0  // Comfortable viewing for initial 12 cards
    } else if total_cards <= 24 {
        15.0  // More compact for 24 cards
    } else if total_cards <= 36 {
        10.0  // Even more compact for 36 cards
    } else {
        (400.0 / total_cards as f32).max(1.0)  // Minimum 1px per card, max 400px total width
    };
    
    let total_width = total_cards as f32 * box_width;
    let start_x = (512.0 - total_width) / 2.0;  // Center the progress bar
    let box_height = 20.0;
    
    // Draw background bar
    let (bg_x, bg_y) = canvas_manager.logical_to_screen(start_x, y_position);
    let bg_w = total_width * canvas_manager.get_scale_factor();
    let bg_h = box_height * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(40, 40, 40, 255));
    
    // Draw individual rating boxes
    for (i, (_card_id, rating_str)) in daily_ratings.iter().enumerate() {
        let box_x = start_x + (i as f32 * box_width);
        let (screen_x, screen_y) = canvas_manager.logical_to_screen(box_x, y_position);
        let screen_w = box_width * canvas_manager.get_scale_factor();
        let screen_h = bg_h;
        
        let color = match rating_str.as_str() {
            "Easy" => Color::from_rgba(100, 200, 100, 255),   // Green - target achievement
            "Good" => Color::from_rgba(255, 200, 100, 255),   // Yellow - solid performance  
            "Hard" | "Again" => Color::from_rgba(255, 120, 120, 255), // Red - challenging (not "failed")
            _ => Color::from_rgba(128, 128, 128, 255),        // Gray - unknown rating
        };
        
        draw_rectangle(screen_x, screen_y, screen_w, screen_h, color);
        
        // Draw subtle border between boxes if they're wide enough
        if box_width > 3.0 {
            draw_rectangle(
                screen_x + screen_w - canvas_manager.get_scale_factor(), 
                screen_y, 
                canvas_manager.get_scale_factor(), 
                screen_h, 
                Color::from_rgba(20, 20, 20, 255)
            );
        }
    }
    
    Ok(())
}

/// Draw the session details view showing challenging cards
fn draw_session_details(studying_state: &StudyingState, font_manager: &FontManager, canvas_manager: &CanvasManager) -> Result<(), String> {
    let (_logical_width, logical_height) = canvas_manager.logical_size();
    
    // Draw background
    let (bg_x, bg_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let bg_w = 512.0 * canvas_manager.get_scale_factor();
    let bg_h = logical_height * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(30, 30, 35, 255));
    
    // Draw layouts with scroll offset
    let mut current_y = 50 - studying_state.detail_scroll_offset;
    let margin = 30;
    
    let font_ascent = font_manager.metrics().0;
    
    for layout in &studying_state.detail_layouts {
        if current_y > -50 && current_y < logical_height as i32 + 50 { // Only draw visible layouts
            font_manager.draw_layout(
                layout,
                margin,
                current_y + font_ascent as i32,
                false, // ruby text
                canvas_manager,
            )?;
        }
        current_y += layout.total_height as i32 + 20; // Spacing between layouts
    }
    
    // Draw "Press B to return" at bottom
    let prompt_text = "Press B to return";
    let (prompt_w, _) = font_manager.size_of_text(prompt_text)?;
    let prompt_x = (512 - prompt_w) / 2;
    let prompt_y = logical_height as i32 - 40;
    let font_ascent = font_manager.metrics().0;
    
    font_manager.draw_line_top_left(
        prompt_text,
        prompt_x as i32,
        prompt_y + font_ascent as i32,
        canvas_manager,
    )?;
    
    Ok(())
}
