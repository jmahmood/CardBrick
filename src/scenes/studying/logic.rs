// src/scenes/studying/logic.rs

use super::{haptics, StudyingScreenMode, StudyingState};
use crate::config::ADOPT_LIMIT_PER_DAY;
use crate::config::{DAILY_GOAL_POINTS, PACK_SIZE_DEFAULT};
use crate::debug::Tracer;
use crate::deck::{html_parser, Card};
use crate::scheduler::bandit;
use crate::scheduler::{points, sm2, Rating};
use crate::storage::db::is_card_adopted_in_progress;
use crate::ui::FontManager;
use chrono::{NaiveDate, Utc};
use macroquad::prelude::*;
// use rand::seq::SliceRandom; // We moved this logic into the db.
// use rand::thread_rng;

/// Loads the next card from the scheduler into the state.
/// Now integrates with the daily queue system from Core Learning Loop.
pub fn load_next_card(
    state: &mut StudyingState,
    font: &mut FontManager,
    small_font: &mut FontManager,
) {
    // Check for calendar day rollover and handle bandit updates
    let now = Utc::now().date_naive();
    if let Err(e) = check_calendar_rollover(now) {
        eprintln!("Warning: Failed to handle calendar rollover: {}", e);
    }

    // Queue creation is handled by the scheduler using the active deck.
    // Avoid calling the generic ensure_today() which expects a global
    // `cards` table in the progress DB and can log spurious errors.

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

        // 2. Build banner layout with session completion information
        let session_goal = if state.is_math_mode {
            crate::config::MATH_MODE_SESSION_POINTS
        } else {
            crate::config::SESSION_GOAL_POINTS
        };

        let sessions_completed = state.points_today / session_goal;
        let mode_label = if state.is_math_mode {
            "problem set"
        } else {
            "session"
        };

        let banner_text = if state.daily_goal_achieved {
            if state.is_math_mode {
                format!(
                    "Daily goal achieved! 🏆\nProblem set complete!\n{} total points",
                    state.points_today
                )
            } else {
                format!(
                    "Daily goal achieved! 🏆\n{} sessions completed today\n{} total points",
                    sessions_completed, state.points_today
                )
            }
        } else if state.session_goal_achieved {
            format!(
                "{} complete! 🎯\n{} points this {}\n{}",
                if state.is_math_mode {
                    "Problem set"
                } else {
                    "Session"
                },
                state.points_this_session,
                mode_label,
                if state.is_math_mode {
                    "Great work!"
                } else {
                    "Continue for daily goal?"
                }
            )
        } else {
            format!(
                "Queue complete!\n{} points this {}\nNeed {} more for {} goal",
                state.points_this_session,
                mode_label,
                session_goal - state.points_this_session,
                mode_label
            )
        };
        let spans = html_parser::parse_html_to_spans(&banner_text);
        state.banner_layout = small_font.layout_text_binary(&spans, 380, false).ok();

        // 3. Kick off animation timer (seconds since app start)
        state.banner_started = Some(get_time() as f32);
    }
}

/// Generates and caches all text layouts for the current card.
pub fn load_card_layouts(
    state: &mut StudyingState,
    card: &Card,
    font: &mut FontManager,
    small_font: &mut FontManager,
) {
    #[cfg(debug_assertions)]
    let _layout_tracer = Tracer::new("Load Card Layout");
    state.is_answer_revealed = false;
    state.scroll_offset = 0;
    state.hint_layout = None;

    if let Some(note) = state.scheduler.get_note(card.note_id) {
        let content_width = 512 - 60;
        let front_html = note.fields.first().map_or("", |s| s.as_str());
        let back_html = note.fields.get(1).map_or("", |s| s.as_str());

        state.front_layout_default = font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(front_html),
                content_width,
                false,
            )
            .ok();
        state.small_front_layout_default = small_font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(front_html),
                content_width,
                false,
            )
            .ok();
        state.back_layout_default = font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(back_html),
                content_width,
                false,
            )
            .ok();
        state.front_layout_ruby = font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(front_html),
                content_width,
                true,
            )
            .ok();
        state.small_front_layout_ruby = small_font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(front_html),
                content_width,
                true,
            )
            .ok();
        state.back_layout_ruby = font
            .layout_text_binary(
                &html_parser::parse_html_to_spans(back_html),
                content_width,
                true,
            )
            .ok();
    }
}

/// Load more cards for continued studying beyond the daily goal
pub fn continue_studying(
    state: &mut StudyingState,
    font: &mut FontManager,
    small_font: &mut FontManager,
) {
    // First try to load more cards from the deck
    if let Ok(new_cards) = state.scheduler.load_more_cards(PACK_SIZE_DEFAULT) {
        if !new_cards.is_empty() {
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
        let spans = html_parser::parse_html_to_spans(
            "No more cards available in this deck! 🎯\nB: Return to menu",
        );
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

                println!(
                    "📅 Calendar rollover detected: {} -> {} (prev day points: {}, reward: {})",
                    prev_day, today, daily_points, reward
                );

                // Deck-specific queues are created when starting a session.
                // No generic queue creation here to avoid DB schema mismatches.
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
    let mut stmt =
        conn.prepare("SELECT rating FROM daily_ratings WHERE date = ?1 ORDER BY timestamp ASC")?;

    let ratings: Result<Vec<String>, _> = stmt
        .query_map([&date_str], |row| row.get::<_, String>(0))?
        .collect();

    let total_points: i64 = ratings?
        .iter()
        .map(|rating| match rating.as_str() {
            "Easy" => 4,
            "Good" => 3,
            "Hard" => 2,
            "Again" => 1,
            _ => 0,
        })
        .sum();

    Ok(total_points)
}

/// Finalize the daily_log entry with points and reward
fn finalize_daily_log(
    date: NaiveDate,
    points: i64,
    reward: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
        .query_row([&date_str], |row| row.get::<_, i64>(0))?;

    conn.execute(
        "UPDATE daily_log SET cards_studied = ?1, points = ?2, reward_scaled = ?3, reward_bin = ?4 WHERE date = ?5",
        [&cards_studied.to_string(), &points.to_string(), &reward_scaled.to_string(), &reward_bin.to_string(), &date_str],
    )?;

    println!(
        "💾 Finalized daily_log for {}: {} cards, {} points, reward: {}",
        date, cards_studied, points, reward
    );

    Ok(())
}

/// Handles card rating with complete point-accounting system
pub fn handle_card_rating(
    state: &mut StudyingState,
    card_id: i64,
    rating: Rating,
    font: &mut FontManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = chrono::Utc::now().timestamp();

    // Get current ease factor for difficulty calculation
    let ease_factor = if let Some(card) = &state.current_card {
        card.ease_factor
    } else {
        2500 // Default ease factor if no current card
    };

    // Explore Mode adoption guard: Only adopt into SRS if under daily cap or already adopted
    let already_adopted = is_card_adopted_in_progress(card_id).unwrap_or(false);
    if already_adopted {
        // Normal path: card is in SRS; update scheduling
        sm2::apply_rating(card_id, rating, timestamp)?;
    } else {
        // New/unadopted card: enforce daily adoption cap
        let adopted_today = state.db_manager.adopted_today().unwrap_or(0) as usize;
        if adopted_today < ADOPT_LIMIT_PER_DAY {
            // Adopt now: schedule first time and increment deck counter
            sm2::apply_rating(card_id, rating, timestamp)?;
            let _ = state.db_manager.mark_adopted(card_id, timestamp);
            let _ = state.db_manager.increment_adopted_today();
        } else {
            // Preview-only: record first_seen but DO NOT schedule in SRS
            let _ = state
                .db_manager
                .mark_first_seen_if_missing(card_id, timestamp);
            // No call to sm2::apply_rating here (prevents adoption)
            println!(
                "🔎 Preview only: adoption limit reached ({} per day)",
                ADOPT_LIMIT_PER_DAY
            );
        }
    }

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
        state.points_this_session += pa;
        state.cards_completed_today += 1;
        if rating == Rating::Again {
            state.session_has_again = true;
        }

        // Record complete study event in database
        state
            .db_manager
            .record_study_event(card_id, timestamp, bp, df, cb, sb, pa)?;

        // Update profile scores
        state.db_manager.update_profile_scores(pa)?;

        println!(
            "📊 Points: BP={} × DF={} + CB={} + SB={} = PA={} (combo={}, time={:.1}s)",
            bp, df, cb, sb, pa, new_combo, response_time
        );

        // Get appropriate goal values based on mode
        let session_goal = if state.is_math_mode {
            crate::config::MATH_MODE_SESSION_POINTS
        } else {
            crate::config::SESSION_GOAL_POINTS
        };

        let daily_goal = if state.is_math_mode {
            crate::config::MATH_MODE_DAILY_POINTS
        } else {
            crate::config::DAILY_GOAL_POINTS
        };

        // Check for session goal achievement (primary milestone)
        if !state.session_goal_achieved && state.points_this_session >= session_goal {
            state.session_goal_achieved = true;
            // Only trigger splash if we're not already in a splash state
            if state.mode != StudyingScreenMode::GoalSplash {
                trigger_session_goal_splash(state, font)?;
            }
        }

        // Check for daily goal achievement (bonus milestone) - only trigger in special circumstances
        if !state.daily_goal_achieved && state.points_today >= daily_goal {
            state.daily_goal_achieved = true;
            // Only trigger daily splash if:
            // 1. We haven't triggered session splash this card AND
            // 2. We're not already in a splash state AND
            // 3. The session goal was achieved earlier (not this card)
            if state.session_goal_achieved
                && state.points_this_session > session_goal
                && state.mode != StudyingScreenMode::GoalSplash
            {
                trigger_daily_goal_splash(state, font)?;
            }
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

        // Keep daily_log up to date for Progress Viewer after each rating (live update)
        // This ensures cards/points/goal are not blank mid-session
        {
            use crate::storage::db::progress_path;
            use rusqlite::Connection;
            let db_path = progress_path();
            let conn = Connection::open(&db_path)?;
            let date_str = chrono::Utc::now()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string();
            // Count ratings for today as cards_studied proxy
            let cards_count: i64 = conn
                .prepare("SELECT COUNT(*) FROM daily_ratings WHERE date = ?1")?
                .query_row([&date_str], |row| row.get::<_, i64>(0))?;
            let points_today = state.points_today as i64;
            let reward_bin_i64 = if points_today >= DAILY_GOAL_POINTS as i64 {
                1
            } else {
                0
            };
            let reward_scaled = (points_today as f64 / DAILY_GOAL_POINTS as f64).min(1.0);
            conn.execute(
                "INSERT INTO daily_log (date, cards_studied, points, reward_scaled, reward_bin)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(date) DO UPDATE SET
                   cards_studied = excluded.cards_studied,
                   points = excluded.points,
                   reward_scaled = excluded.reward_scaled,
                   reward_bin = excluded.reward_bin",
                rusqlite::params![
                    &date_str,
                    cards_count,
                    points_today,
                    reward_scaled,
                    reward_bin_i64
                ],
            )?;
        }

        // Auto-save progress (S3-7 requirement)
        flush_database()?;

        println!(
            "✅ Card {} rated {:?}, PA={}, total today: {}, combo: {}",
            card_id, rating, pa, state.points_today, state.current_combo
        );
    }

    Ok(())
}

/// Triggers the session goal splash screen with haptic feedback and audio
fn trigger_session_goal_splash(
    state: &mut StudyingState,
    font: &mut FontManager,
) -> Result<(), Box<dyn std::error::Error>> {
    // Switch to goal splash mode
    state.mode = StudyingScreenMode::GoalSplash;
    state.goal_splash_started = Some(get_time() as f32);

    // Create session achievement banner
    let goal_text = format!(
        "🎯 Session Complete!\n{} points achieved!",
        state.points_this_session
    );
    let spans = html_parser::parse_html_to_spans(&goal_text);
    state.banner_layout = font.layout_text_binary(&spans, 380, false).ok();

    // Trigger haptic feedback
    haptics::short_rumble();

    // TODO: Play session chime audio (requires audio module integration)
    // audio::play_sound(SESSION_CHIME_WAV)?;

    println!(
        "🎯 SESSION COMPLETE! {} points reached - triggering splash screen",
        state.points_this_session
    );

    Ok(())
}

/// Triggers the daily goal splash screen with bigger celebration
fn trigger_daily_goal_splash(
    state: &mut StudyingState,
    font: &mut FontManager,
) -> Result<(), Box<dyn std::error::Error>> {
    // Switch to goal splash mode
    state.mode = StudyingScreenMode::GoalSplash;
    state.goal_splash_started = Some(get_time() as f32);

    // Create daily achievement banner (bigger celebration)
    let sessions_completed = state.points_today / crate::config::SESSION_GOAL_POINTS;
    let goal_text = format!(
        "🏆 Daily Goal Achieved!\n{} sessions completed today!",
        sessions_completed
    );
    let spans = html_parser::parse_html_to_spans(&goal_text);
    state.banner_layout = font.layout_text_binary(&spans, 380, false).ok();

    // Trigger stronger haptic feedback for daily achievement
    haptics::short_rumble();
    haptics::short_rumble(); // Double rumble for daily goal

    // TODO: Play achievement fanfare audio (requires audio module integration)
    // audio::play_sound(DAILY_ACHIEVEMENT_WAV)?;

    println!(
        "🏆 DAILY GOAL ACHIEVED! {} total points reached - triggering celebration",
        state.points_today
    );

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

        // Auto-transition based on achievement type:
        // - Session goal: 1.5 seconds (quick celebration, continue studying)
        // - Daily goal: 3 seconds (bigger celebration, likely ending session)
        let splash_duration = if state.daily_goal_achieved && !state.session_goal_achieved {
            3.0 // Daily goal hit without session goal (rare case)
        } else if state.session_goal_achieved {
            1.5 // Session goal celebration
        } else {
            2.0 // Default fallback
        };

        if elapsed > splash_duration {
            state.mode = StudyingScreenMode::InProgress;
            state.goal_splash_started = None;

            // Clear goal splash layouts
            if state.mode == StudyingScreenMode::InProgress {
                // Banner layout will be cleared when next card loads
            }
        }
    }
}

/// Updates the rating toast state and handles auto-cleanup
pub fn update_rating_toast(state: &mut StudyingState) {
    if let Some((_, start_time)) = state.last_rating_toast {
        let elapsed = get_time() as f32 - start_time;
        let toast_duration = 2.0; // 2 seconds

        // Clear toast after duration expires
        if elapsed > toast_duration {
            state.last_rating_toast = None;
            state.toast_layout = None;
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

    println!(
        "🏁 Finalizing session: {} cards studied, {} points, goal achieved: {}",
        cards_studied, points_earned, reward_achieved
    );

    // Update daily_log with final session data
    let date_str = today.format("%Y-%m-%d").to_string();
    let reward_bin = if reward_achieved { 1 } else { 0 };
    let reward_scaled = (points_earned as f64 / DAILY_GOAL_POINTS as f64).min(1.0);

    use crate::storage::db::progress_path;
    use rusqlite::Connection;

    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;

    // Upsert daily_log with session completion data
    conn.execute(
        "INSERT INTO daily_log (date, cards_studied, points, reward_scaled, reward_bin) 
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(date) DO UPDATE SET 
           cards_studied = excluded.cards_studied,
           points = excluded.points,
           reward_scaled = excluded.reward_scaled,
           reward_bin = excluded.reward_bin",
        rusqlite::params![
            &date_str,
            cards_studied as i64,
            points_earned as i64,
            reward_scaled,
            reward_bin as i64
        ],
    )?;

    // Apply bandit rewards for today's chosen parameters
    bandit::apply_reward(today, reward_achieved)?;

    // Force database flush for persistence
    flush_database()?;

    println!("💾 Session finalized: daily_log updated, bandit rewards applied");

    Ok(())
}
