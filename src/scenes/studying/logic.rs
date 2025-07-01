// src/scenes/studying/logic.rs

use crate::deck::{html_parser, Card};
use crate::debug::Tracer;
use crate::ui::FontManager;
use super::StudyingState;

/// Loads the next card from the scheduler into the state.
pub fn load_next_card(state: &mut StudyingState, font: &mut FontManager, small_font: &mut FontManager) {
    state.current_card = state.scheduler.next_card();
    if let Some(card) = state.current_card.clone() {
        load_card_layouts(state, &card, font, small_font);
    } else {
        state.is_done = true;
        let done_spans = html_parser::parse_html_to_spans("Deck Complete!");
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
