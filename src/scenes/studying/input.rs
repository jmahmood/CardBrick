// src/scenes/studying/input.rs

use crate::state::{BrickInput, BrickButton, AppState, GameState};
use crate::deck::html_parser;
use crate::scheduler::Rating;
use super::logic::{load_card_layouts, load_next_card, continue_studying};
use super::StudyingState;

/// Handles input events for the studying scene - ported from SDL2 to pure evdev
pub fn handle_studying_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::Studying(studying_state) = &mut state.game_state {
        match input {
            // DPad Down: Reveal answer or scroll down when answer is revealed
            BrickInput::ButtonDown(BrickButton::DPadDown) => {
                if studying_state.is_answer_revealed {
                    // Handle scrolling when answer is revealed
                    let scroll_speed = 30;
                    let viewport_height = 290;
                    let total_height = if let (Some(front), Some(back)) = (&studying_state.small_front_layout_default, &studying_state.back_layout_default) {
                        front.total_height + back.total_height + 20
                    } else { 0 };
                    let max_scroll = (total_height - viewport_height).max(0);
                    studying_state.scroll_offset = (studying_state.scroll_offset + scroll_speed).min(max_scroll);
                } else {
                    // Reveal the answer
                    studying_state.is_answer_revealed = true;
                    let margin: u32 = 30;
                    let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard [LB:Rewind] [RB:Ruby]");
                    studying_state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, state.config.window_width / 2 - margin * 2, studying_state.show_ruby_text)?);
                }
            },
            
            // DPad Up: Scroll up when answer is revealed
            BrickInput::ButtonDown(BrickButton::DPadUp) => {
                if studying_state.is_answer_revealed {
                    let scroll_speed = 30;
                    studying_state.scroll_offset = (studying_state.scroll_offset - scroll_speed).max(0);
                }
            },
            
            // A Button: Rate card as "Good" or Continue studying if done
            BrickInput::ButtonDown(BrickButton::A) => {
                if studying_state.is_done {
                    // User wants to continue studying beyond daily goal
                    continue_studying(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                } else if studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Good, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // B Button: Rate card as "Again" or Return to menu if done
            BrickInput::ButtonDown(BrickButton::B) => {
                if studying_state.is_done {
                    // User wants to return to deck selection
                    state.game_state = GameState::GoToDeckSelection;
                } else if studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Again, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // X Button: Rate card as "Easy"
            BrickInput::ButtonDown(BrickButton::X) => {
                if studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Easy, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // Y Button: Rate card as "Hard"
            BrickInput::ButtonDown(BrickButton::Y) => {
                if studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Hard, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // Left Shoulder: Rewind last answer (important for spaced repetition)
            BrickInput::ButtonDown(BrickButton::LeftShoulder) => {
                if let Some(card) = &studying_state.current_card {
                    studying_state.scheduler.add_card_to_front(card.id);
                }
                if let Some(rewound_card) = studying_state.scheduler.rewind_last_answer() {
                    studying_state.current_card = Some(rewound_card.clone());
                    load_card_layouts(studying_state, &rewound_card, &mut state.font_manager, &mut state.small_font_manager);
                } else {
                    load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                }
            },
            
            // Right Shoulder: Show ruby text (furigana) - CRITICAL for Japanese learning
            BrickInput::ButtonDown(BrickButton::RightShoulder) => {
                studying_state.show_ruby_text = true;
            },
            
            BrickInput::ButtonUp(BrickButton::RightShoulder) => {
                studying_state.show_ruby_text = false;
            },
            
            // Back button: Return to deck selection
            BrickInput::ButtonDown(BrickButton::Back) => {
                state.game_state = GameState::GoToDeckSelection;
            },
            
            // Start button: Could be used for options menu in future
            BrickInput::ButtonDown(BrickButton::Start) => {
                // Currently unused - could show options screen
            },
            
            _ => {}
        }
    }
    Ok(())
}

/// Helper function to rate a card and proceed to the next one
fn rate_card_and_continue(
    studying_state: &mut StudyingState,
    rating: Rating,
    font_manager: &mut crate::ui::FontManager,
    small_font_manager: &mut crate::ui::FontManager,
) -> Result<(), String> {
    if let Some(card) = &studying_state.current_card {
        if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, rating) {
            studying_state.replay_logger.log_action(&updated_card, rating).map_err(|e| e.to_string())?;
            studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
        }
    }
    load_next_card(studying_state, font_manager, small_font_manager);
    Ok(())
}