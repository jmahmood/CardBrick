// src/scenes/studying/input.rs

use crate::state::{BrickInput, BrickButton, AppState, GameState};
use crate::deck::html_parser;
use crate::scheduler::Rating;
use super::logic::{load_card_layouts, load_next_card, continue_studying, handle_card_rating, update_goal_splash};
use super::{StudyingState, StudyingScreenMode};

/// Handles input events for the studying scene - ported from SDL2 to pure evdev
pub fn handle_studying_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::Studying(studying_state) = &mut state.game_state {
        // Update goal splash auto-transition
        update_goal_splash(studying_state);
        match input {
            // DPad Down: Reveal answer, scroll down, or scroll detail view
            BrickInput::ButtonDown(BrickButton::DPadDown) => {
                match studying_state.mode {
                    StudyingScreenMode::InProgress => {
                        if studying_state.is_answer_revealed {
                            // Handle scrolling when answer is revealed
                            let scroll_speed = 30;
                            let viewport_height = 350;
                            let total_height = if let (Some(front), Some(back)) = (&studying_state.small_front_layout_default, &studying_state.back_layout_default) {
                                front.total_height + back.total_height + 20
                            } else { 0 };
                            let max_scroll = (total_height - viewport_height + 50).max(0);
                            studying_state.scroll_offset = (studying_state.scroll_offset + scroll_speed).min(max_scroll);
                            println!("scroll_offset={}", studying_state.scroll_offset);
                        } else {
                            // Reveal the answer
                            studying_state.is_answer_revealed = true;
                            
                            // Record flip time for speed bonus calculation
                            studying_state.card_flip_time = Some(macroquad::time::get_time() as f32);
                            
                            let margin: u32 = 30;
                            let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard [LB:Rewind] [RB:Ruby]");
                            studying_state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, state.config.window_width / 2 - margin * 2, studying_state.show_ruby_text)?);
                        }
                    },
                    StudyingScreenMode::SessionDetails => {
                        // Scroll down in detail view
                        let max_scroll = calculate_detail_max_scroll(studying_state);
                        studying_state.detail_scroll_offset = (studying_state.detail_scroll_offset + 30).min(max_scroll);
                    },
                    _ => {},
                }
            },
            
            // DPad Up: Scroll up when answer is revealed or in detail view
            BrickInput::ButtonDown(BrickButton::DPadUp) => {
                match studying_state.mode {
                    StudyingScreenMode::InProgress => {
                        if studying_state.is_answer_revealed {
                            let scroll_speed = 30;
                            println!("scroll_offset={}", studying_state.scroll_offset);
                            studying_state.scroll_offset = (studying_state.scroll_offset - scroll_speed).max(0);
                        }
                    },
                    StudyingScreenMode::SessionDetails => {
                        // Scroll up in detail view
                        studying_state.detail_scroll_offset = (studying_state.detail_scroll_offset - 30).max(0);
                    },
                    _ => {},
                }
            },
            
            // A Button: Rate card as "Good" or Continue studying if done
            BrickInput::ButtonDown(BrickButton::A) => {
                match studying_state.mode {
                    StudyingScreenMode::InProgress => {
                        if studying_state.is_answer_revealed {
                            rate_card_and_continue(studying_state, Rating::Good, &mut state.font_manager, &mut state.small_font_manager)?;
                        }
                    },
                    StudyingScreenMode::GoalSplash => {
                        // Skip goal splash and return to studying
                        studying_state.mode = StudyingScreenMode::InProgress;
                        studying_state.goal_splash_started = None;
                    },
                    StudyingScreenMode::SessionComplete => {
                        // User wants to continue studying beyond daily goal
                        continue_studying(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                    },
                    StudyingScreenMode::ExhaustedDeck => {
                        // A button does nothing in exhausted deck mode
                    },
                    StudyingScreenMode::SessionDetails => {
                        // A button does nothing in session details mode
                    },
                }
            },
            
            // B Button: Rate card as "Again" or Return to menu if done
            BrickInput::ButtonDown(BrickButton::B) => {
                match studying_state.mode {
                    StudyingScreenMode::InProgress => {
                        if studying_state.is_answer_revealed {
                            rate_card_and_continue(studying_state, Rating::Again, &mut state.font_manager, &mut state.small_font_manager)?;
                        }
                    },
                    StudyingScreenMode::GoalSplash => {
                        // Skip goal splash and return to studying  
                        studying_state.mode = StudyingScreenMode::InProgress;
                        studying_state.goal_splash_started = None;
                    },
                    StudyingScreenMode::SessionComplete | StudyingScreenMode::ExhaustedDeck => {
                        // User wants to return to deck selection
                        state.game_state = GameState::GoToDeckSelection;
                    },
                    StudyingScreenMode::SessionDetails => {
                        // Return to session complete screen
                        studying_state.mode = StudyingScreenMode::SessionComplete;
                        studying_state.detail_scroll_offset = 0; // Reset scroll
                    },
                }
            },
            
            // X Button: Rate card as "Easy"
            BrickInput::ButtonDown(BrickButton::X) => {
                if studying_state.mode == StudyingScreenMode::InProgress && studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Easy, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // Y Button: Rate card as "Hard"
            BrickInput::ButtonDown(BrickButton::Y) => {
                if studying_state.mode == StudyingScreenMode::InProgress && studying_state.is_answer_revealed {
                    rate_card_and_continue(studying_state, Rating::Hard, &mut state.font_manager, &mut state.small_font_manager)?;
                }
            },
            
            // Left Shoulder: Rewind last answer (important for spaced repetition)
            BrickInput::ButtonDown(BrickButton::LeftShoulder) => {
                if studying_state.mode == StudyingScreenMode::InProgress {
                    if let Some(card) = &studying_state.current_card {
                        studying_state.scheduler.add_card_to_front(card.id);
                    }
                    if let Some(rewound_card) = studying_state.scheduler.rewind_last_answer() {
                        studying_state.current_card = Some(rewound_card.clone());
                        load_card_layouts(studying_state, &rewound_card, &mut state.font_manager, &mut state.small_font_manager);
                    } else {
                        load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                    }
                }
            },
            
            // Right Shoulder: Show ruby text (furigana) in progress mode, switch to details in session complete
            BrickInput::ButtonDown(BrickButton::RightShoulder) => {
                match studying_state.mode {
                    StudyingScreenMode::InProgress => {
                        studying_state.show_ruby_text = true;
                    },
                    StudyingScreenMode::SessionComplete => {
                        // Switch to detail view of challenging cards
                        studying_state.mode = StudyingScreenMode::SessionDetails;
                        // Build detail layouts for cards marked as Hard or Again
                        build_session_detail_layouts(studying_state, &mut state.small_font_manager)?;
                    },
                    _ => {},
                }
            },
            
            BrickInput::ButtonUp(BrickButton::RightShoulder) => {
                if studying_state.mode == StudyingScreenMode::InProgress {
                    studying_state.show_ruby_text = false;
                }
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

/// Helper function to rate a card and proceed to the next one - Sprint 3 version with points tracking
fn rate_card_and_continue(
    studying_state: &mut StudyingState,
    rating: Rating,
    font_manager: &mut crate::ui::FontManager,
    small_font_manager: &mut crate::ui::FontManager,
) -> Result<(), String> {
    let card_id = if let Some(card) = &studying_state.current_card {
        card.id
    } else {
        return Ok(());
    };
    
    // Use new Sprint 3 logic for handling card rating with points and goal detection
    if let Err(e) = handle_card_rating(studying_state, card_id, rating, font_manager) {
        eprintln!("Warning: Failed to handle card rating: {}", e);
    }
    
    // Legacy integrations for existing systems - record difficult cards for prioritization
    match rating {
        crate::scheduler::Rating::Again => {
            if let Err(e) = studying_state.db_manager.record_difficult_card(card_id, "failed") {
                eprintln!("Warning: Failed to record difficult card: {}", e);
            }
        }
        crate::scheduler::Rating::Hard => {
            if let Err(e) = studying_state.db_manager.record_difficult_card(card_id, "hard") {
                eprintln!("Warning: Failed to record difficult card: {}", e);
            }
        }
        _ => {}
    }
    
    // Get updated card state from current_card for legacy systems logging
    if let Some(ref card_after_rating) = studying_state.current_card {
        // Legacy systems
        studying_state.replay_logger.log_action(card_after_rating, rating).map_err(|e| e.to_string())?;
        studying_state.db_manager.update_card_state(card_after_rating).map_err(|e| e.to_string())?;
    }
    
    // Only load next card if we're not in goal splash mode
    if studying_state.mode != StudyingScreenMode::GoalSplash {
        load_next_card(studying_state, font_manager, small_font_manager);
    }
    
    Ok(())
}

/// Build text layouts for the session detail view showing challenging cards from the entire day
pub fn build_session_detail_layouts(studying_state: &mut StudyingState, small_font_manager: &mut crate::ui::FontManager) -> Result<(), String> {
    use crate::scheduler::Rating;
    use crate::deck::html_parser;
    
    studying_state.detail_layouts.clear();
    
    // Get today's challenging cards from the database (persistent across sessions)
    let (failed_cards, hard_cards) = studying_state.db_manager.get_todays_difficult_cards()
        .map_err(|e| format!("Failed to get today's difficult cards: {}", e))?;
    
    // Combine into a single list with ratings - failed cards get Rating::Again, hard cards get Rating::Hard
    let mut challenging_cards: Vec<(i64, Rating)> = Vec::new();
    for card_id in failed_cards {
        challenging_cards.push((card_id, Rating::Again));
    }
    for card_id in hard_cards {
        challenging_cards.push((card_id, Rating::Hard));
    }
    
    if challenging_cards.is_empty() {
        // Show a message that there were no challenging cards today
        let spans = html_parser::parse_html_to_spans("Great job! No challenging cards today. 🎯");
        if let Ok(layout) = small_font_manager.layout_text_binary(&spans, 450, false) {
            studying_state.detail_layouts.push(layout);
        }
        return Ok(());
    }
    
    // Add header
    let header_text = format!("Today's Challenging Cards ({} cards)", challenging_cards.len());
    let header_spans = html_parser::parse_html_to_spans(&header_text);
    if let Ok(layout) = small_font_manager.layout_text_binary(&header_spans, 450, false) {
        studying_state.detail_layouts.push(layout);
    }
    
    // Add each challenging card's front and back
    for (card_id, rating) in challenging_cards {
        println!("DEBUG: Processing challenging card_id: {}", card_id);
        // Get the card and note info directly from the database (works for all cards from today)
        match studying_state.scheduler.get_card_note_from_db(card_id) {
            Ok(Some((note_id, note))) => {
                println!("DEBUG: Successfully got note_id: {} and note for card_id: {}", note_id, card_id);
            let rating_text = match rating {
                Rating::Hard => "Hard",
                Rating::Again => "Review Again", 
                _ => "Challenging",
            };
            
                let card_text = format!("--- {} ---\nQ: {}\nA: {}\n", rating_text, 
                    note.fields.get(0).unwrap_or(&"[No front]".to_string()),
                    note.fields.get(1).unwrap_or(&"[No back]".to_string())
                );
                let card_spans = html_parser::parse_html_to_spans(&card_text);
                if let Ok(layout) = small_font_manager.layout_text_binary(&card_spans, 450, false) {
                    studying_state.detail_layouts.push(layout);
                    println!("DEBUG: Added layout for card_id: {}", card_id);
                } else {
                    println!("DEBUG: Failed to create layout for card_id: {}", card_id);
                }
            },
            Ok(None) => {
                println!("DEBUG: Card {} not found in database", card_id);
            },
            Err(e) => {
                println!("DEBUG: Database error for card {}: {}", card_id, e);
            }
        }
    }
    
    Ok(())
}

/// Calculate the maximum scroll offset for the detail view
pub fn calculate_detail_max_scroll(studying_state: &StudyingState) -> i32 {
    let viewport_height = 400; // Available height for scrolling
    let total_height: i32 = studying_state.detail_layouts.iter()
        .map(|layout| layout.total_height + 20) // Add spacing between layouts
        .sum();
    
    (total_height - viewport_height).max(0)
}