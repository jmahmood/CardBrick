// src/scenes/studying/logic.rs

use crate::deck::{html_parser, Card};
use crate::ui::FontManager;
use crate::debug::Tracer;
use crate::scheduler::queue::{self};
use crate::scheduler::bandit;
use crate::scheduler::{points, Rating, sm2};
use crate::config::{PACK_SIZE_DEFAULT, DAILY_GOAL_POINTS};
use super::{StudyingState, StudyingScreenMode, haptics};
use chrono::{Utc, NaiveDate};
use macroquad::prelude::*;
// use rand::seq::SliceRandom; // We moved this logic into the db.
// use rand::thread_rng;

/// Loads the next card from the scheduler into the state.
/// Now integrates with the daily queue system from Core Learning Loop.
pub fn load_next_card(state: &mut StudyingState, font: &mut FontManager, small_font: &mut FontManager) {
    // Check for calendar day rollover and handle bandit updates
    let now = Utc::now().date_naive();
    if let Err(e) = check_calendar_rollover(now) {
        eprintln!("Warning: Failed to handle calendar rollover: {}", e);
    }
    
    // For Sprint 0: Try to ensure today's queue exists first
    if let Err(e) = queue::ensure_today(now) {
        eprintln!("Warning: Failed to ensure today's queue: {}", e);
    }
    
    state.current_card = state.scheduler.next_card();
    if let Some(card) = state.current_card.clone() {
        // Normal path - we have a card to show
        state.mode = StudyingScreenMode::InProgress;
        state.is_done = false;
        state.is_answer_revealed = false;
        
        // Reset flip time - will be set when answer is revealed
        state.card_flip_time = None;
        
        load_card_layouts(state, &card, font, small_font);
    } else {
        // ===== Session finished =====
        // Sprint 3: Finalize session with points, database updates, and bandit rewards
        if let Err(e) = finalize_study_session(state) {
            eprintln!("Warning: Failed to finalize study session: {}", e);
        }
        
        state.mode = StudyingScreenMode::SessionComplete;
        state.is_done = true;

        // 1. Purge obsolete layouts so nothing else draws
        state.front_layout_default = None;
        state.front_layout_ruby = None;
        state.back_layout_default = None;
        state.back_layout_ruby = None;
        state.small_front_layout_default = None;
        state.small_front_layout_ruby = None;
        state.hint_layout = None;
        state.done_layout = None;

        // 2. Build banner layout with points information
        let banner_text = if state.points_today >= DAILY_GOAL_POINTS {
            format!("Daily goal complete! 🎯\n{} points achieved!", state.points_today)
        } else {
            format!("Session complete!\n{} points earned", state.points_today)
        };
        let spans = html_parser::parse_html_to_spans(&banner_text);
        state.banner_layout = small_font.layout_text_binary(&spans, 380, false).ok();

        // 3. Kick off animation timer (seconds since app start)
        state.banner_started = Some(get_time() as f32);
    }
}

/// Generates and caches all text layouts for the current card.
pub fn load_card_layouts(state: &mut StudyingState, card: &Card, font: &mut FontManager, small_font: &mut FontManager) {
    #[cfg(debug_assertions)]
    let _layout_tracer = Tracer::new("Load Card Layout");
    state.is_answer_revealed = false;
    state.scroll_offset = 0;
    state.hint_layout = None;

    if let Some(note) = state.scheduler.get_note(card.note_id) {
        let content_width = 512 - 60;
        let front_html = note.fields.get(0).map_or("", |s| s.as_str());
        let back_html = note.fields.get(1).map_or("", |s| s.as_str());
        
        state.front_layout_default = font.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, false).ok();
        state.small_front_layout_default = small_font.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, false).ok();
        state.back_layout_default = font.layout_text_binary(&html_parser::parse_html_to_spans(back_html), content_width, false).ok();
        state.front_layout_ruby = font.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, true).ok();
        state.small_front_layout_ruby = small_font.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, true).ok();
        state.back_layout_ruby = font.layout_text_binary(&html_parser::parse_html_to_spans(back_html), content_width, true).ok();
    }
}

/// Load more cards for continued studying beyond the daily goal
pub fn continue_studying(state: &mut StudyingState, font: &mut FontManager, small_font: &mut FontManager) {
    // First try to load more cards from the deck
    if let Ok(new_cards) = state.scheduler.load_more_cards(PACK_SIZE_DEFAULT) {
        if new_cards.len() > 0 {
            // Reset the done state and load next card
            state.is_done = false;
            state.mode = StudyingScreenMode::InProgress;
            load_next_card(state, font, small_font);
            return;
        }
    }
    
    // If no new cards available, try to reorder existing cards
    let additional_count = state.scheduler.introduce_new_cards(PACK_SIZE_DEFAULT);
    
    if additional_count > 0 {
        // Reset the done state and load next card
        state.is_done = false;
        state.mode = StudyingScreenMode::InProgress;
        load_next_card(state, font, small_font);
    } else {
        // No more cards available - switch to exhausted deck mode
        state.mode = StudyingScreenMode::ExhaustedDeck;
        state.is_done = true;

        // Clear existing layouts
        state.front_layout_default = None;
        state.front_layout_ruby = None;
        state.back_layout_default = None;
        state.back_layout_ruby = None;
        state.small_front_layout_default = None;
        state.small_front_layout_ruby = None;
        state.hint_layout = None;
        state.done_layout = None;

        // Build exhausted deck banner
        let spans = html_parser::parse_html_to_spans("No more cards available in this deck! 🎯\nB: Return to menu");
        state.banner_layout = font.layout_text_binary(&spans, 380, false).ok();
        state.banner_started = Some(get_time() as f32);
    }
}

// Global state to track the last known day for calendar rollover detection
static mut LAST_KNOWN_DAY: Option<NaiveDate> = None;

/// Checks for calendar day rollover and applies bandit rewards if needed
pub fn check_calendar_rollover(today: NaiveDate) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let last_day = LAST_KNOWN_DAY;
        
        if let Some(prev_day) = last_day {
            if prev_day != today {
                // Calendar day has changed - apply bandit reward for previous day
                let daily_points = calculate_daily_points(prev_day)?;
                let reward = daily_points >= 10; // Binary reward: 10+ points = success
                
                // Update daily_log with points and reward
                finalize_daily_log(prev_day, daily_points, reward)?;
                
                // Apply reward to bandits
                bandit::apply_reward(prev_day, reward)?;
                
                println!("📅 Calendar rollover detected: {} -> {} (prev day points: {}, reward: {})", 
                         prev_day, today, daily_points, reward);
                
                // Ensure today's queue exists
                queue::ensure_today(today)?;
            }
        }
        
        // Update the last known day
        LAST_KNOWN_DAY = Some(today);
    }
    
    Ok(())
}

/// Calculate daily points from session ratings (Easy=4, Good=3, Hard=2, Again=1)
fn calculate_daily_points(date: NaiveDate) -> Result<i64, Box<dyn std::error::Error>> {
    use crate::storage::db::progress_path;
    use rusqlite::Connection;
    
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT rating FROM daily_ratings WHERE date = ?1 ORDER BY timestamp ASC"
    )?;
    
    let ratings: Result<Vec<String>, _> = stmt
        .query_map([&date_str], |row| Ok(row.get::<_, String>(0)?))?.collect();
    
    let total_points: i64 = ratings?.iter().map(|rating| {
        match rating.as_str() {
            "Easy" => 4,
            "Good" => 3, 
            "Hard" => 2,
            "Again" => 1,
            _ => 0,
        }
    }).sum();
    
    Ok(total_points)
}

/// Finalize the daily_log entry with points and reward
fn finalize_daily_log(date: NaiveDate, points: i64, reward: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::storage::db::progress_path;
    use rusqlite::Connection;
    
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    let date_str = date.format("%Y-%m-%d").to_string();
    let reward_bin = if reward { 1 } else { 0 };
    let reward_scaled = points as f64 / 20.0; // Scale for potential future use
    
    // Get current cards_studied count if it exists
    let cards_studied: i64 = conn
        .prepare("SELECT COUNT(*) FROM daily_ratings WHERE date = ?1")?
        .query_row([&date_str], |row| Ok(row.get::<_, i64>(0)?))?;
    
    conn.execute(
        "UPDATE daily_log SET cards_studied = ?1, points = ?2, reward_scaled = ?3, reward_bin = ?4 WHERE date = ?5",
        [&cards_studied.to_string(), &points.to_string(), &reward_scaled.to_string(), &reward_bin.to_string(), &date_str],
    )?;
    
    println!("💾 Finalized daily_log for {}: {} cards, {} points, reward: {}", 
             date, cards_studied, points, reward);
    
    Ok(())
}

/// Handles card rating with complete point-accounting system
pub fn handle_card_rating(
    state: &mut StudyingState, 
    card_id: i64, 
    rating: Rating, 
    font: &mut FontManager
) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = chrono::Utc::now().timestamp();
    
    // Get current ease factor for difficulty calculation
    let ease_factor = if let Some(card) = &state.current_card {
        card.ease_factor
    } else {
        2500 // Default ease factor if no current card
    };
    
    // Apply SM-2 rating to database (updates srs_log table)
    sm2::apply_rating(card_id, rating, timestamp)?;
    
    // Apply rating to scheduler (updates in-memory card)
    if let Some(_updated_card) = state.scheduler.answer_card(card_id, rating) {
        // Calculate response time for speed bonus
        let response_time = if let Some(flip_time) = state.card_flip_time {
            (get_time() - flip_time as f64) as f32
        } else {
            10.0 // Default to slow if no flip time recorded
        };
        
        // Calculate combo bonus and update combo state
        let (new_combo, cb) = points::combo_bonus(state.current_combo, rating);
        state.current_combo = new_combo;
        
        // Calculate all point components
        let bp = points::base_points(rating);
        let df = points::difficulty_factor(ease_factor);
        let sb = points::speed_bonus(response_time);
        let pa = points::calculate_points_awarded(rating, ease_factor, cb, sb);
        
        // Update session state
        state.points_today += pa;
        state.cards_completed_today += 1;
        if rating == Rating::Again {
            state.session_has_again = true;
        }
        
        // Record complete study event in database
        state.db_manager.record_study_event(card_id, timestamp, bp, df, cb, sb, pa)?;
        
        // Update profile scores
        state.db_manager.update_profile_scores(pa)?;
        
        println!("📊 Points: BP={} × DF={} + CB={} + SB={} = PA={} (combo={}, time={:.1}s)", 
                bp, df, cb, sb, pa, new_combo, response_time);
        
        // Check if goal is achieved for the first time
        if !state.goal_achieved && state.points_today >= DAILY_GOAL_POINTS {
            state.goal_achieved = true;
            trigger_goal_splash(state, font)?;
        }
        
        // Record rating in database for progress tracking
        let rating_str = match rating {
            Rating::Again => "Again",
            Rating::Hard => "Hard", 
            Rating::Good => "Good",
            Rating::Easy => "Easy",
        };
        state.db_manager.record_daily_rating(card_id, rating_str)?;
        
        // Invalidate the daily ratings cache since we just added a new rating (battery optimization)
        state.invalidate_ratings_cache();
        
        // Auto-save progress (S3-7 requirement)
        flush_database()?;
        
        println!("✅ Card {} rated {:?}, PA={}, total today: {}, combo: {}", 
                card_id, rating, pa, state.points_today, state.current_combo);
    }
    
    Ok(())
}

/// Triggers the goal splash screen with haptic feedback and audio
fn trigger_goal_splash(state: &mut StudyingState, font: &mut FontManager) -> Result<(), Box<dyn std::error::Error>> {
    // Switch to goal splash mode
    state.mode = StudyingScreenMode::GoalSplash;
    state.goal_splash_started = Some(get_time() as f32);
    
    // Create goal achievement banner
    let goal_text = format!("🎯 Goal Met! {} points achieved!", state.points_today);
    let spans = html_parser::parse_html_to_spans(&goal_text);
    state.banner_layout = font.layout_text_binary(&spans, 380, false).ok();
    
    // Trigger haptic feedback
    haptics::short_rumble();
    
    // TODO: Play goal chime audio (requires audio module integration)
    // audio::play_sound(GOAL_CHIME_WAV)?;
    
    println!("🎯 GOAL ACHIEVED! {} points reached - triggering splash screen", state.points_today);
    
    Ok(())
}


/// Flushes database changes to disk for auto-save protection
fn flush_database() -> Result<(), Box<dyn std::error::Error>> {
    use crate::storage::db::progress_path;
    use rusqlite::Connection;
    
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    // Force WAL checkpoint to ensure data is written to disk
    // Note: wal_checkpoint returns results, so we use query_row to handle them properly
    let _: (i32, i32, i32) = conn.query_row("PRAGMA wal_checkpoint(FULL)", [], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    
    Ok(())
}

/// Updates the goal splash screen state and handles auto-transition
pub fn update_goal_splash(state: &mut StudyingState) {
    if let Some(start_time) = state.goal_splash_started {
        let elapsed = get_time() as f32 - start_time;
        
        // Auto-transition back to studying after 2 seconds
        if elapsed > 2.0 {
            state.mode = StudyingScreenMode::InProgress;
            state.goal_splash_started = None;
            
            // Clear goal splash layouts
            if state.mode == StudyingScreenMode::InProgress {
                // Banner layout will be cleared when next card loads
            }
        }
    }
}

/// Finalizes the study session by updating daily_log and applying bandit rewards
pub fn finalize_study_session(state: &mut StudyingState) -> Result<(), Box<dyn std::error::Error>> {
    let today = chrono::Utc::now().date_naive();
    
    // Calculate final session metrics
    let cards_studied = state.scheduler.reviews_complete();
    let points_earned = state.points_today;
    let reward_achieved = points_earned >= DAILY_GOAL_POINTS;
    
    println!("🏁 Finalizing session: {} cards studied, {} points, goal achieved: {}", 
             cards_studied, points_earned, reward_achieved);
    
    // Update daily_log with final session data
    let date_str = today.format("%Y-%m-%d").to_string();
    let reward_bin = if reward_achieved { 1 } else { 0 };
    let reward_scaled = (points_earned as f64 / DAILY_GOAL_POINTS as f64).min(1.0);
    
    use crate::storage::db::progress_path;
    use rusqlite::Connection;
    
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    // Update daily_log with session completion data
    conn.execute(
        "UPDATE daily_log SET cards_studied = ?1, points = ?2, reward_scaled = ?3, reward_bin = ?4 WHERE date = ?5",
        [cards_studied as i64, points_earned as i64, reward_scaled as i64, reward_bin as i64, 0], // Note: will be converted to strings in the actual call
    )?;
    
    // Alternative approach using string parameters to match existing schema
    conn.execute(
        "INSERT OR REPLACE INTO daily_log (date, cards_studied, points, reward_scaled, reward_bin) 
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(date) DO UPDATE SET 
           cards_studied = excluded.cards_studied,
           points = excluded.points,
           reward_scaled = excluded.reward_scaled,
           reward_bin = excluded.reward_bin",
        [
            &date_str,
            &cards_studied.to_string(),
            &points_earned.to_string(), 
            &reward_scaled.to_string(),
            &reward_bin.to_string()
        ],
    )?;
    
    // Apply bandit rewards for today's chosen parameters
    bandit::apply_reward(today, reward_achieved)?;
    
    // Force database flush for persistence 
    flush_database()?;
    
    println!("💾 Session finalized: daily_log updated, bandit rewards applied");
    
    Ok(())
}
