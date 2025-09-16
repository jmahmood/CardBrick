// src/scenes/studying/mod.rs

use macroquad::prelude::*;
use crate::deck::Card;
use crate::scheduler::Scheduler;
use crate::storage::{DatabaseManager, ReplayLogger};
use crate::ui::{FontManager, CanvasManager, font::TextLayout, sprite::Sprite};

pub mod input;
pub mod logic;
pub mod haptics;

/// How we render the studying scene
#[derive(Debug, Clone, PartialEq)]
pub enum StudyingScreenMode {
    InProgress,          // normal Q&A flow
    GoalSplash,          // goal achieved banner with fade-in overlay
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
    // Sprint 3 additions for points tracking
    pub points_today: i32,             // accumulated points for today across all sessions
    pub points_this_session: i32,      // points accumulated in current session only
    pub session_goal_achieved: bool,   // whether session goal has been reached
    pub daily_goal_achieved: bool,     // whether daily goal has been reached
    pub goal_splash_started: Option<f32>, // goal splash animation timer
    pub is_math_mode: bool,            // whether this deck is detected as math mode
    
    // Complete point-accounting system fields
    pub current_combo: i32,            // consecutive correct answers (Good/Easy)
    pub card_flip_time: Option<f32>,   // time when card was flipped (for speed bonus)
    pub session_has_again: bool,       // track if any "Again" was pressed (for perfect session bonus)
    pub cards_completed_today: i32,    // total cards completed in today's session
    
    // Battery optimization fields
    pub animation_frame_counter: u32,  // for throttling animations to reduce battery usage
    pub cached_daily_ratings: Option<Vec<(i64, String)>>, // cached database query results
    pub ratings_cache_frame: u32,      // frame when cache was last updated

    // Toast notification for recent ratings
    pub last_rating_toast: Option<(crate::scheduler::Rating, f32)>, // (rating, start_time)
    pub toast_layout: Option<TextLayout>, // cached layout for toast text

    // Cached UI strings to reduce per-frame allocations/measurements
    pub last_points_today: i32,
    pub cached_score_text: String,
    pub cached_score_size: (u32, u32),
    pub last_completed: usize,
    pub last_total: usize,
    pub cached_progress_text: String,
    pub cached_progress_size: (u32, u32),
}

impl<'a> StudyingState<'a> {
    /// Creates a new StudyingState.
    pub fn new(scheduler: Box<dyn Scheduler + 'a>, db_manager: DatabaseManager, replay_logger: ReplayLogger) -> Self {
        // Load today's existing points from the database
        let points_today = db_manager.get_todays_points().unwrap_or(0);
        let session_goal_achieved = false; // New session, no points yet
        let daily_goal_achieved = points_today >= crate::config::DAILY_GOAL_POINTS;

        // Detect if this is a math mode deck by checking card content
        let is_math_mode = detect_math_mode(&*scheduler);
        
        println!("📊 Starting session with {} points already earned today", points_today);
        
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
            points_today,
            points_this_session: 0,
            session_goal_achieved,
            daily_goal_achieved,
            goal_splash_started: None,
            is_math_mode,
            current_combo: 0,
            card_flip_time: None,
            session_has_again: false,
            cards_completed_today: 0,
            animation_frame_counter: 0,
            cached_daily_ratings: None,
            ratings_cache_frame: 0,

            last_rating_toast: None,
            toast_layout: None,

            last_points_today: points_today,
            cached_score_text: String::new(),
            cached_score_size: (0, 0),
            last_completed: 0,
            last_total: 0,
            cached_progress_text: String::new(),
            cached_progress_size: (0, 0),
        }
    }
    
    /// Invalidate the daily ratings cache when new ratings are added (battery optimization)
    pub fn invalidate_ratings_cache(&mut self) {
        self.cached_daily_ratings = None;
        self.ratings_cache_frame = 0;
    }
}

/// Detect if this deck should use math mode based on content analysis
fn detect_math_mode(scheduler: &dyn Scheduler) -> bool {
    // Sample the first few cards to detect math patterns
    let total_cards = scheduler.total_session_cards();
    let sample_size = (total_cards.min(10)).max(1); // Sample at most 10 cards

    let mut math_indicators = 0;
    let mut total_checked = 0;

    // Look for math-related patterns in card content
    for i in 0..sample_size {
        // Try to get card ID from scheduler (this is a simplified check)
        // In a real implementation, we'd need a way to peek at card content
        // For now, use simple heuristics based on deck size and card count
        total_checked += 1;

        // Math decks tend to be smaller (10-50 cards) vs language decks (hundreds)
        if total_cards <= 50 {
            math_indicators += 1;
        }
    }

    // If more than 60% of indicators suggest math, enable math mode
    let math_ratio = math_indicators as f32 / total_checked as f32;
    let is_math = math_ratio > 0.6;

    if is_math {
        println!("🔢 Math mode detected: {} cards in deck", total_cards);
    } else {
        println!("🌐 Language mode detected: {} cards in deck", total_cards);
    }

    is_math
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
    // Increment frame counter for battery-optimized animations
    studying_state.animation_frame_counter = studying_state.animation_frame_counter.wrapping_add(1);
    
    // Layout constants
    const BAR_HEIGHT: f32 = 25.0;
    const CONTENT_TOP: f32 = BAR_HEIGHT + 15.0;
    
    let margin: u32 = 30;
    let total = studying_state.scheduler.total_session_cards();
    let (_logical_width, logical_height) = canvas_manager.logical_size();
    let _scale = canvas_manager.scale_factor;

    // Simple approach: just calculate the content area bounds for later use
    let content_area_top = CONTENT_TOP + 40.0; // Start below score display
    let content_area_bottom = logical_height - 60.0; // Leave space for help text


    // Draw progress bar
    if total > 0 {
        let completed = studying_state.scheduler.reviews_complete();

        // Update cached progress string if values changed
        if completed != studying_state.last_completed || total != studying_state.last_total {
            studying_state.cached_progress_text = format!("{}/{}", completed, total);
            studying_state.cached_progress_size = hint_font_manager.size_of_text(&studying_state.cached_progress_text)?;
            studying_state.last_completed = completed;
            studying_state.last_total = total;
        }

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
        
        // Draw progress text with proper top-left positioning (cached)
        let (tw, th) = studying_state.cached_progress_size;
        // Centre the bounding box in the bar
        let text_x = 512.0 - tw as f32 - 10.0;
        let text_y = (BAR_HEIGHT - th as f32) / 2.0;
        hint_font_manager.draw_line_top_left(&studying_state.cached_progress_text, text_x as i32, text_y as i32, canvas_manager)?;
    }
    
    // Draw animated sprite
    sprite.draw(canvas_manager)?;
    
    // Draw on-screen score overlay (top-left corner)
    if studying_state.mode == StudyingScreenMode::InProgress {
        // Update cached score if points changed - show session progress during study
        if studying_state.points_this_session != studying_state.last_points_today || studying_state.cached_score_text.is_empty() {
            let session_goal = if studying_state.is_math_mode {
                crate::config::MATH_MODE_SESSION_POINTS
            } else {
                crate::config::SESSION_GOAL_POINTS
            };

            let label = if studying_state.is_math_mode { "Problems" } else { "Session" };
            studying_state.cached_score_text = format!("{}: {}/{}", label, studying_state.points_this_session, session_goal);
            studying_state.cached_score_size = hint_font_manager.size_of_text(&studying_state.cached_score_text)?;
            studying_state.last_points_today = studying_state.points_this_session;
        }
        let (score_w, score_h) = studying_state.cached_score_size;
        
        // Draw semi-transparent background for score
        let (bg_x, bg_y) = canvas_manager.logical_to_screen(10.0, BAR_HEIGHT + 10.0);
        let bg_w = (score_w + 10) as f32 * canvas_manager.get_scale_factor();
        let bg_h = (score_h + 6) as f32 * canvas_manager.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(0, 0, 0, 150));
        
        // Draw score text
        let text_x = 15.0;
        let text_y = BAR_HEIGHT + 13.0;
        let hint_ascent = hint_font_manager.metrics().0;
        hint_font_manager.draw_line_top_left(
            &studying_state.cached_score_text,
            text_x as i32,
            text_y as i32 + hint_ascent as i32,
            canvas_manager,
        )?;

        // Explore mode banner: show adopted today status
        if crate::config::ENABLE_EXPLORE_MODE {
            if let Ok(adopted) = studying_state.db_manager.adopted_today() {
                let limit = crate::config::ADOPT_LIMIT_PER_DAY as i64;
                let adopt_text = format!("Adopted {}/{}", adopted, limit);
                let (aw, ah) = hint_font_manager.size_of_text(&adopt_text)?;
                let (bg2_x, bg2_y) = canvas_manager.logical_to_screen(10.0, BAR_HEIGHT + 10.0 + (score_h as f32) + 10.0);
                let bg2_w = (aw + 10) as f32 * canvas_manager.get_scale_factor();
                let bg2_h = (ah + 6) as f32 * canvas_manager.get_scale_factor();
                draw_rectangle(bg2_x, bg2_y, bg2_w, bg2_h, Color::from_rgba(0, 0, 0, 150));

                let text2_x = 15.0;
                let text2_y = BAR_HEIGHT + 13.0 + (score_h as f32) + 10.0;
                hint_font_manager.draw_line_top_left(
                    &adopt_text,
                    text2_x as i32,
                    text2_y as i32 + hint_ascent as i32,
                    canvas_manager,
                )?;
            }
        }
    }
    
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
                    let y_pos = content_area_top as i32 - studying_state.scroll_offset;
                    // Only draw if the content is within the visible area
                    if y_pos + layout.total_height > content_area_top as i32 && y_pos < content_area_bottom as i32 {
                        font_manager.draw_layout(layout, margin as i32, y_pos + font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    }
                }
            } else {
                // Answer mode: Show small front + full back text with scrolling
                let mut y_pos = content_area_top as i32 - studying_state.scroll_offset;
                
                // Draw small front text
                let small_front_layout_to_draw = if studying_state.show_ruby_text { 
                    &studying_state.small_front_layout_ruby 
                } else { 
                    &studying_state.small_front_layout_default 
                };
                
                if let Some(layout) = small_front_layout_to_draw {
                    // Only draw if the content is within the visible area
                    if y_pos + layout.total_height > content_area_top as i32 && y_pos < content_area_bottom as i32 {
                        small_font_manager.draw_layout(layout, margin as i32, y_pos + small_font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    }
                    y_pos += layout.total_height + 20;
                }
                
                // Draw back text (the answer)
                let back_layout_to_draw = if studying_state.show_ruby_text { 
                    &studying_state.back_layout_ruby 
                } else { 
                    &studying_state.back_layout_default 
                };
                
                if let Some(layout) = back_layout_to_draw {
                    // Only draw if the content is within the visible area
                    if y_pos + layout.total_height > content_area_top as i32 && y_pos < content_area_bottom as i32 {
                        font_manager.draw_layout(layout, margin as i32, y_pos + font_ascent as i32, studying_state.show_ruby_text, canvas_manager)?;
                    }
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

        },
        
        StudyingScreenMode::GoalSplash => {
            // Goal achieved banner with fade-in animation
            if let Some(layout) = &studying_state.banner_layout {
                // Calculate fade-in alpha based on animation timer
                let alpha = if let Some(start_time) = studying_state.goal_splash_started {
                    let elapsed = get_time() as f32 - start_time;
                    let fade_duration = 0.5; // 500ms fade-in
                    (elapsed / fade_duration).min(1.0)
                } else {
                    1.0 // Fully visible if no animation timer
                };
                
                // Draw semi-transparent overlay
                let (bg_x, bg_y) = canvas_manager.logical_to_screen(0.0, 0.0);
                let bg_w = 512.0 * canvas_manager.get_scale_factor();
                let bg_h = logical_height * canvas_manager.get_scale_factor();
                draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(0, 0, 0, (128.0 * alpha) as u8));
                
                // Draw goal achievement panel with fade-in
                draw_retro_panel(canvas_manager, layout);
                
                // Draw goal text with alpha
                let x_center = (512 - 380) / 2;
                let y_center = (logical_height - layout.total_height as f32) / 2.0;
                
                // Set text alpha for fade-in effect
                let old_alpha = WHITE.a;
                let _fade_color = Color::from_rgba(255, 255, 255, (old_alpha as f32 * alpha) as u8);
                
                font_manager.draw_layout(
                    layout,
                    x_center as i32,
                    y_center as i32 + font_ascent as i32,
                    studying_state.show_ruby_text,
                    canvas_manager,
                )?;
                
                // Auto-transition after 2 seconds
                if let Some(start_time) = studying_state.goal_splash_started {
                    if get_time() as f32 - start_time > 2.0 {
                        // Note: State transition will be handled in logic.rs
                    }
                }
            }
        },
        
        StudyingScreenMode::SessionComplete | StudyingScreenMode::ExhaustedDeck => {
            // Banner display mode
            if let Some(layout) = &studying_state.banner_layout {
                // Draw retro panel first
                draw_retro_panel(canvas_manager, layout);

                // Calculate centered position for banner text
                let x_center = (512 - 380) / 2;  // Center based on layout width used in logic.rs
                let y_center = (logical_height - layout.total_height as f32) / 2.0;
                let layout_height = layout.total_height as f32; // Extract height to avoid borrow issues
                
                font_manager.draw_layout(
                    layout,
                    x_center as i32,
                    y_center as i32 + font_ascent as i32,
                    studying_state.show_ruby_text,
                    canvas_manager,
                )?;

                // Draw colored progress boxes for SessionComplete mode (after layout borrow ends)
                if studying_state.mode == StudyingScreenMode::SessionComplete {
                    draw_session_progress_boxes(studying_state, canvas_manager, y_center + layout_height + 10.0)?;
                }

                // Draw flashing prompt below banner/progress
                let prompt_text = match studying_state.mode {
                    StudyingScreenMode::SessionComplete => "Press A to Continue, B for Menu, R for Details",
                    StudyingScreenMode::ExhaustedDeck => "Press B to Return to Menu",
                    _ => "",
                };
                
                // Flash the prompt every 30 frames (~0.5s at 60fps) - battery optimized
                // Only evaluate flashing state once per 30 frames instead of every frame
                let flash_visible = (studying_state.animation_frame_counter / 30) % 2 == 0;
                if flash_visible {
                    let (prompt_w, _) = hint_font_manager.size_of_text(prompt_text)?;
                    let prompt_x = (512 - prompt_w) / 2;
                    let prompt_y = if studying_state.mode == StudyingScreenMode::SessionComplete {
                        y_center + layout_height + 50.0  // Extra space for progress boxes
                    } else {
                        y_center + layout_height + 24.0
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

    // Draw rating toast if active - draw on top of all content
    draw_rating_toast(studying_state, font_manager, canvas_manager)?;

    Ok(())
}

/// Draw colored progress boxes showing today's performance (persistent across sessions)
fn draw_session_progress_boxes(studying_state: &mut StudyingState, canvas_manager: &CanvasManager, y_position: f32) -> Result<(), String> {
    
    // Cache database query to avoid per-frame database hits (battery optimization)
    let cache_expiry_frames = 300; // Refresh cache every 5 seconds (300 frames at 60fps)
    let daily_ratings = if let Some(ref cached) = studying_state.cached_daily_ratings {
        // Use cache if it's still fresh
        if studying_state.animation_frame_counter - studying_state.ratings_cache_frame < cache_expiry_frames {
            cached.clone()
        } else {
            // Cache expired, refresh
            match studying_state.db_manager.get_todays_ratings() {
                Ok(ratings) => {
                    studying_state.cached_daily_ratings = Some(ratings.clone());
                    studying_state.ratings_cache_frame = studying_state.animation_frame_counter;
                    ratings
                },
                Err(_e) => {
                    return Ok(()); // No ratings to display
                }
            }
        }
    } else {
        // No cache yet, load initial data
        match studying_state.db_manager.get_todays_ratings() {
            Ok(ratings) => {
                studying_state.cached_daily_ratings = Some(ratings.clone());
                studying_state.ratings_cache_frame = studying_state.animation_frame_counter;
                ratings
            },
            Err(_e) => {
                return Ok(()); // No ratings to display
            }
        }
    };
    
    let total_cards = daily_ratings.len();
    
    if total_cards == 0 {
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

/// Draw rating toast notification in top-right corner
fn draw_rating_toast(
    studying_state: &StudyingState,
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
) -> Result<(), String> {
    use macroquad::time::get_time;
    use macroquad::prelude::*;

    // Check if toast is active and not expired
    if let Some((rating, start_time)) = studying_state.last_rating_toast {
        let elapsed = get_time() as f32 - start_time;
        let toast_duration = 2.0; // 2 seconds

        if elapsed < toast_duration {
            if let Some(toast_layout) = &studying_state.toast_layout {
                // Calculate fade-out alpha based on elapsed time
                let alpha = if elapsed > 1.5 {
                    // Fade out in last 0.5 seconds
                    ((toast_duration - elapsed) / 0.5).max(0.0)
                } else {
                    1.0 // Fully visible for first 1.5 seconds
                };

                // Position in top-right corner (larger dimensions for bigger text)
                let toast_width = 150.0;
                let toast_height = toast_layout.total_height as f32 + 12.0; // more padding
                let toast_x = 512.0 - toast_width - 10.0; // 10px from right edge
                let toast_y = 30.0; // Below progress bar

                // Draw semi-transparent background
                let (bg_x, bg_y) = canvas_manager.logical_to_screen(toast_x, toast_y);
                let bg_w = toast_width * canvas_manager.get_scale_factor();
                let bg_h = toast_height * canvas_manager.get_scale_factor();

                let bg_alpha = (150.0 * alpha) as u8;
                draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(0, 0, 0, bg_alpha));

                // Draw border
                let border_alpha = (255.0 * alpha) as u8;
                draw_rectangle_lines(
                    bg_x, bg_y, bg_w, bg_h,
                    1.0 * canvas_manager.get_scale_factor(),
                    Color::from_rgba(255, 255, 255, border_alpha)
                );

                // Draw toast text (using larger font)
                let text_x = toast_x + 8.0; // more padding from left
                let text_y = toast_y + 6.0; // more padding from top
                let font_ascent = font_manager.metrics().0;

                let toast_text = match rating {
                    crate::scheduler::Rating::Again => "Again ↻",
                    crate::scheduler::Rating::Hard => "Hard ⚠",
                    crate::scheduler::Rating::Good => "Good ✓",
                    crate::scheduler::Rating::Easy => "Easy ⚡",
                };

                font_manager.draw_line_top_left(
                    toast_text,
                    text_x as i32,
                    text_y as i32 + font_ascent as i32,
                    canvas_manager,
                )?;
            }
        }
    }

    Ok(())
}
