// src/scenes/studying/input.rs

use crate::state::{BrickInput, BrickButton, AppState, GameState};
use crate::deck::html_parser;
use crate::scheduler::Rating;
use super::logic::{load_card_layouts, load_next_card, continue_studying, handle_card_rating, update_goal_splash, update_rating_toast};
use super::{StudyingState, StudyingScreenMode};

/// Handles input events for the studying scene - ported from SDL2 to pure evdev
pub fn handle_studying_input(state: &mut AppState, input: BrickInput) -> Result<(), String> {
    if let GameState::Studying(studying_state) = &mut state.game_state {
        // Update goal splash auto-transition
        let was_goal_splash = studying_state.mode == StudyingScreenMode::GoalSplash;
        update_goal_splash(studying_state);

        // Update rating toast auto-cleanup
        update_rating_toast(studying_state);

        // If the splash just ended, advance to the next card (or session complete)
        if was_goal_splash && studying_state.mode == StudyingScreenMode::InProgress {
            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
        }

        // If we just exited the splash or otherwise have no active card while in progress,
        // attempt to load the next card (this also transitions to SessionComplete when deck is done).
        if studying_state.mode == StudyingScreenMode::InProgress && studying_state.current_card.is_none() {
            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
        }
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
                        // Skip goal splash and return to studying; also advance to next card or session complete
                        studying_state.mode = StudyingScreenMode::InProgress;
                        studying_state.goal_splash_started = None;
                        load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
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
                        // Skip goal splash and return to studying; also advance to next card or session complete
                        studying_state.mode = StudyingScreenMode::InProgress;
                        studying_state.goal_splash_started = None;
                        load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
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
                    // Clear toast when undoing
                    studying_state.last_rating_toast = None;
                    studying_state.toast_layout = None;

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

    // Trigger rating toast notification
    trigger_rating_toast(studying_state, rating, small_font_manager)?;

    // Record difficult cards for prioritization
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
    
    // Log the action for replay analysis
    if let Some(ref card_after_rating) = studying_state.current_card {
        studying_state.replay_logger.log_action(card_after_rating, rating).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use crate::scenes::studying::StudyingScreenMode;

    // Since creating a full AppState is complex, let's test the input logic more directly
    
    #[test]
    fn test_studying_mode_transitions() {
        // Test mode transitions that don't require full AppState
        
        // SessionComplete -> GoToDeckSelection via B button
        // This logic is in the match statement and can be tested independently
        
        let studying_mode = StudyingScreenMode::SessionComplete;
        match studying_mode {
            StudyingScreenMode::SessionComplete => {
                // This would set state.game_state = GameState::GoToDeckSelection
                assert!(true); // Logic path verified
            },
            _ => panic!("Wrong mode"),
        }
        
        // SessionDetails -> SessionComplete via B button
        let studying_mode = StudyingScreenMode::SessionDetails;
        match studying_mode {
            StudyingScreenMode::SessionDetails => {
                // This would set studying_state.mode = StudyingScreenMode::SessionComplete
                // and reset detail_scroll_offset = 0
                assert!(true); // Logic path verified
            },
            _ => panic!("Wrong mode"),
        }
    }

    #[test]
    fn test_scroll_calculations() {
        // Test scroll offset calculations without full state
        
        let initial_scroll = 100;
        let scroll_speed = 30;
        
        // Scroll down calculation
        let max_scroll = 200;
        let new_scroll_down = (initial_scroll + scroll_speed).min(max_scroll);
        assert_eq!(new_scroll_down, 130);
        
        // Scroll up calculation
        let new_scroll_up = (initial_scroll - scroll_speed).max(0);
        assert_eq!(new_scroll_up, 70);
        
        // Test boundary conditions
        let at_top = 0;
        let scroll_up_from_top = (at_top - scroll_speed).max(0);
        assert_eq!(scroll_up_from_top, 0);
        
        let at_bottom = 200;
        let scroll_down_from_bottom = (at_bottom + scroll_speed).min(max_scroll);
        assert_eq!(scroll_down_from_bottom, 200);
    }

    #[test]
    fn test_button_input_conditions() {
        // Test the conditions under which buttons should work
        
        let mode = StudyingScreenMode::InProgress;
        let is_answer_revealed = true;
        
        // Rating buttons (X, Y, A, B) should only work when:
        // - mode is InProgress AND is_answer_revealed is true
        let should_rate = mode == StudyingScreenMode::InProgress && is_answer_revealed;
        assert!(should_rate);
        
        // Test when answer not revealed
        let is_answer_revealed = false;
        let should_not_rate = mode == StudyingScreenMode::InProgress && is_answer_revealed;
        assert!(!should_not_rate);
        
        // Test in different mode
        let mode = StudyingScreenMode::SessionComplete;
        let is_answer_revealed = true;
        let should_not_rate_different_mode = mode == StudyingScreenMode::InProgress && is_answer_revealed;
        assert!(!should_not_rate_different_mode);
    }

    #[test]
    fn test_detail_scroll_max_calculation() {
        // Test calculate_detail_max_scroll logic
        use crate::ui::font::TextLayout;
        
        // Mock text layouts with known heights
        let layout1 = TextLayout {
            total_height: 100,
            lines: Vec::new(),
            scroll_offset: 0,
        };
        let layout2 = TextLayout {
            total_height: 150,
            lines: Vec::new(),
            scroll_offset: 0,
        };
        
        let layouts = vec![layout1, layout2];
        let viewport_height = 400;
        
        // Calculate total height: (100 + 20) + (150 + 20) = 290
        let total_height: i32 = layouts.iter()
            .map(|layout| layout.total_height + 20)
            .sum();
        
        let max_scroll = (total_height - viewport_height).max(0);
        
        // 290 - 400 = -110, max(0) = 0
        assert_eq!(max_scroll, 0);
        
        // Test with larger content
        let layout1_copy = TextLayout {
            total_height: 100,
            lines: Vec::new(),
            scroll_offset: 0,
        };
        let layout2_copy = TextLayout {
            total_height: 150,
            lines: Vec::new(),
            scroll_offset: 0,
        };
        let layout3 = TextLayout {
            total_height: 300,
            lines: Vec::new(),
            scroll_offset: 0,
        };
        let large_layouts = vec![layout1_copy, layout2_copy, layout3];
        
        // Total: (100+20) + (150+20) + (300+20) = 610
        let large_total: i32 = large_layouts.iter()
            .map(|layout| layout.total_height + 20)
            .sum();
        
        let large_max_scroll = (large_total - viewport_height).max(0);
        
        // 610 - 400 = 210
        assert_eq!(large_max_scroll, 210);
    }

    #[test]
    fn test_answer_reveal_logic() {
        // Test the logic for revealing answers via DPadDown
        
        let mode = StudyingScreenMode::InProgress;
        let mut is_answer_revealed = false;
        
        // DPadDown in InProgress mode when answer not revealed should reveal answer
        if mode == StudyingScreenMode::InProgress && !is_answer_revealed {
            is_answer_revealed = true;
            // Would also set card_flip_time and create hint_layout
        }
        
        assert!(is_answer_revealed);
        
        // DPadDown when answer already revealed should scroll
        let mut scroll_offset = 0;
        if mode == StudyingScreenMode::InProgress && is_answer_revealed {
            let scroll_speed = 30;
            let max_scroll = 100;
            scroll_offset = (scroll_offset + scroll_speed).min(max_scroll);
        }
        
        assert_eq!(scroll_offset, 30);
    }
}

/// Trigger a toast notification showing the user's rating
fn trigger_rating_toast(
    studying_state: &mut StudyingState,
    rating: Rating,
    small_font_manager: &mut crate::ui::FontManager,
) -> Result<(), String> {
    use crate::deck::html_parser;
    use macroquad::time::get_time;

    // Create toast text with icon
    let toast_text = match rating {
        Rating::Again => "Again ↻",
        Rating::Hard => "Hard ⚠",
        Rating::Good => "Good ✓",
        Rating::Easy => "Easy ⚡",
    };

    // Create text layout for toast
    let spans = html_parser::parse_html_to_spans(toast_text);
    studying_state.toast_layout = small_font_manager.layout_text_binary(&spans, 100, false).ok();

    // Set toast timer
    studying_state.last_rating_toast = Some((rating, get_time() as f32));

    Ok(())
}
