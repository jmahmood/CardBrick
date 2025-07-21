// src/scenes/studying/logic.rs

use crate::deck::{html_parser, Card};
use crate::debug::Tracer;
use crate::ui::FontManager;
use crate::scheduler::queue;
use super::StudyingState;
use chrono::{Utc, TimeZone};
use rand::seq::SliceRandom;
use rand::thread_rng;

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
        load_card_layouts(state, &card, font, small_font);
    } else {
        // Daily queue is complete - offer to continue with more cards
        state.is_done = true;
        let done_spans = html_parser::parse_html_to_spans("Daily goal complete! 🎯\nA: Continue studying  B: Return to menu");
        state.done_layout = font.layout_text_binary(&done_spans, 400_u32, false).ok();
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
    if let Ok(new_cards) = state.scheduler.load_more_cards(12) {
        if new_cards.len() > 0 {
            // Reset the done state and load next card
            state.is_done = false;
            load_next_card(state, font, small_font);
            return;
        }
    }
    
    // If no new cards available, try to reorder existing cards
    let additional_count = state.scheduler.introduce_new_cards(12);
    
    if additional_count > 0 {
        // Reset the done state and load next card
        state.is_done = false;
        load_next_card(state, font, small_font);
    } else {
        // No more cards available
        let done_spans = html_parser::parse_html_to_spans("No more cards available in this deck! 🎯\nB: Return to menu");
        state.done_layout = font.layout_text_binary(&done_spans, 400_u32, false).ok();
    }
}
