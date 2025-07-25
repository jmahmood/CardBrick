// src/scenes/studying/logic.rs

use crate::deck::{html_parser, Card};
use crate::debug::Tracer;
use crate::ui::FontManager;
use crate::scheduler::queue::{self};
use crate::scheduler::bandit;
use crate::config::PACK_SIZE_DEFAULT;
use super::{StudyingState, StudyingScreenMode};
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
        load_card_layouts(state, &card, font, small_font);
    } else {
        // ===== Session finished =====
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

        // 2. Build banner layout once
        let spans = html_parser::parse_html_to_spans("Daily goal complete! 🎯");
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
