// src/scenes/studying/input.rs

use crate::BrickInput;
use crate::BrickButton;
use crate::map_to_brick_input;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::{AppState, GameState};
use crate::deck::html_parser;
use crate::scheduler::Rating;
use super::logic::{load_card_layouts, load_next_card};

/// Handles input events for the studying scene.
pub fn handle_studying_input(state: &mut AppState, event: Event) -> Result<(), String> {
    if let GameState::Studying(studying_state) = &mut state.game_state {
        if let Some(input) = map_to_brick_input(&event) {
            match input {
                BrickInput::ButtonDown(BrickButton::DPadDown) => {
                    match studying_state.is_answer_revealed {
                        true => {

                        },
                        _ => {
                            studying_state.is_answer_revealed = true;
                            let margin: u32 = 30;
                            let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard [LB:Rewind] [RB:Ruby]");
                            studying_state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, state.config.window_width / 2 - margin * 2, studying_state.show_ruby_text)?);
                        }
                    }
                }
                BrickInput::ButtonDown(BrickButton::DPadUp) => {
                },
                BrickInput::ButtonDown(BrickButton::A) => {
                    if studying_state.is_answer_revealed {
                        let rating = Some(Rating::Good);
                        if let Some(r) = rating {
                            if let Some(card) = &studying_state.current_card {
                                if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, r) {
                                    studying_state.replay_logger.log_action(&updated_card, r).map_err(|e| e.to_string())?;
                                    studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
                                }
                            }
                            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                        }

                    }

                },
                BrickInput::ButtonDown(BrickButton::B) => {
                    if studying_state.is_answer_revealed {
                        let rating = Some(Rating::Good);
                        if let Some(r) = rating {
                            if let Some(card) = &studying_state.current_card {
                                if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, r) {
                                    studying_state.replay_logger.log_action(&updated_card, r).map_err(|e| e.to_string())?;
                                    studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
                                }
                            }
                            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                        }
                    }
                },
                BrickInput::ButtonDown(BrickButton::X) => {
                    if studying_state.is_answer_revealed {
                        let rating = Some(Rating::Good);
                        if let Some(r) = rating {
                            if let Some(card) = &studying_state.current_card {
                                if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, r) {
                                    studying_state.replay_logger.log_action(&updated_card, r).map_err(|e| e.to_string())?;
                                    studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
                                }
                            }
                            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                        }
                    }
                },
                BrickInput::ButtonDown(BrickButton::Y) => {
                    if studying_state.is_answer_revealed {
                        let rating = Some(Rating::Good);
                        if let Some(r) = rating {
                            if let Some(card) = &studying_state.current_card {
                                if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, r) {
                                    studying_state.replay_logger.log_action(&updated_card, r).map_err(|e| e.to_string())?;
                                    studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
                                }
                            }
                            load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                        }
                    }
                },

                BrickInput::ButtonDown(BrickButton::Start) => {
                    // We probably want to show an options screen
                },
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
                    return Ok(());
                },
                BrickInput::ButtonDown(BrickButton::RightShoulder) => {
                    studying_state.show_ruby_text = true;
                    return Ok(());
                },
                BrickInput::ButtonUp(BrickButton::RightShoulder) => {
                    studying_state.show_ruby_text = false;
                    return Ok(());
                },

                _ => {}
            }
        } 
        if let Event::KeyDown { keycode: Some(keycode), repeat: false, .. } = event {
            if keycode == Keycode::Backspace {
                state.game_state = GameState::GoToDeckSelection;
                return Ok(());
            }

            if keycode == Keycode::LShift {
                studying_state.show_ruby_text = true;
                return Ok(());
            }

            if keycode == Keycode::Return {
            }

            if studying_state.is_answer_revealed {
                let rating = match keycode {
                    Keycode::B => Some(Rating::Again),
                    Keycode::Y => Some(Rating::Hard),
                    Keycode::A => Some(Rating::Good),
                    Keycode::X => Some(Rating::Easy),
                    _ => None,
                };
                if let Some(r) = rating {
                    if let Some(card) = &studying_state.current_card {
                        if let Some(updated_card) = studying_state.scheduler.answer_card(card.id, r) {
                            studying_state.replay_logger.log_action(&updated_card, r).map_err(|e| e.to_string())?;
                            studying_state.db_manager.update_card_state(&updated_card).map_err(|e| e.to_string())?;
                        }
                    }
                    load_next_card(studying_state, &mut state.font_manager, &mut state.small_font_manager);
                } else {
                    // Handle scrolling
                    let scroll_speed = 30;
                    let viewport_height = 290;
                    let total_height = if let (Some(front), Some(back)) = (&studying_state.small_front_layout_default, &studying_state.back_layout_default) {
                        front.total_height + back.total_height + 20
                    } else { 0 };
                    match keycode {
                        Keycode::Up => { studying_state.scroll_offset = (studying_state.scroll_offset - scroll_speed).max(0); }
                        Keycode::Down => {
                            let max_scroll = (total_height - viewport_height).max(0);
                            studying_state.scroll_offset = (studying_state.scroll_offset + scroll_speed).min(max_scroll);
                        }
                        _ => {}
                    }
                }
            } else if let Keycode::Up | Keycode::Down | Keycode::Left | Keycode::Right = keycode {
                // Reveal answer
                studying_state.is_answer_revealed = true;
                let margin: u32 = 30;
                let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard [Enter:Rewind]");
                studying_state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, state.config.window_width / 2 - margin * 2, studying_state.show_ruby_text)?);
            }
        }
        if let Event::KeyUp { keycode: Some(Keycode::LShift), .. } = event {
            studying_state.show_ruby_text = false;
        }
    }
    Ok(())
}
