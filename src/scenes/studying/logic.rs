// src/scenes/studying/logic.rs

use crate::deck::{html_parser, Card};
use crate::debug::Tracer;
use crate::ui::FontManager;
use crate::scheduler::queue::{self, PACK_SIZE_DEFAULT};
use super::{StudyingState, StudyingScreenMode};
use chrono::{Utc};
use macroquad::prelude::*;
// use rand::seq::SliceRandom; // We moved this logic into the db.
// use rand::thread_rng;

/// Loads the next card from the scheduler into the state.
/// Now integrates with the daily queue system from Core Learning Loop.
pub fn load_next_card(state: &mut StudyingState, font: &mut FontManager, small_font: &mut FontManager) {
    // For Sprint 0: Try to ensure today's queue exists first
    let today = Utc::now().date_naive();
    if let Err(e) = queue::ensure_today(today) {
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
        let spans = html_parser::parse_html_to_spans("Daily goal complete! 🎯\nA: Continue  B: Menu");
        state.banner_layout = font.layout_text_binary(&spans, 380, false).ok();

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
