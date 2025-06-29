// CardBrick - main.rs

use std::env;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

mod deck;
mod scheduler;
mod ui;
mod storage;
mod debug;

use deck::{Card, Deck};
use scheduler::{Rating, Scheduler, Sm2Scheduler};
use ui::{CanvasManager, FontManager, font::TextLayout, sprite::Sprite};
use deck::html_parser;
use storage::{DatabaseManager, ReplayLogger};
use debug::Tracer;

pub enum LoaderMessage { Progress(f32), Complete(Result<Deck, String>), }
enum GameState { Loading, Reviewing, Done, Error(String), }

struct AppState<'a> {
    game_state: GameState,
    scheduler: Option<Box<dyn Scheduler + 'a>>,
    canvas_manager: CanvasManager<'a>,
    font_manager: FontManager<'a, 'a>,
    small_font_manager: FontManager<'a, 'a>,
    hint_font_manager: FontManager<'a, 'a>,
    sprite: Sprite,
    current_card: Option<Card>,
    is_answer_revealed: bool,
    hint_layout: Option<TextLayout>,
    loading_layout: Option<TextLayout>,
    scroll_offset: i32,
    loading_progress: f32,
    db_manager: Option<DatabaseManager>,
    replay_logger: Option<ReplayLogger>,
    show_ruby_text: bool,

    front_layout_default: Option<TextLayout>,
    front_layout_ruby: Option<TextLayout>,

    back_layout_default: Option<TextLayout>,
    back_layout_ruby: Option<TextLayout>,

    small_front_layout_default: Option<TextLayout>,
    small_front_layout_ruby: Option<TextLayout>,

}

pub fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { return Err(format!("Usage: {} <path/to/deck.apkg>", args.get(0).unwrap_or(&"cardbrick".to_string()))); }
    let deck_path = PathBuf::from(&args[1]);     // Use PathBuf for an owned path
    let deck_id = deck_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_string(); // Create an owned String for the ID

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1"); // Enable anti-aliasing et al
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    let window = video_subsystem.window("CardBrick v0.1", 1024, 768).position_centered().build().map_err(|e| e.to_string())?;
    let sdl_canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = sdl_canvas.texture_creator();
    
    let (tx, rx) = mpsc::channel::<LoaderMessage>();
    thread::spawn(move || { deck::loader::load_apkg(&deck_path, tx); });

    // TODO: Move the font into a local directory.
    let font_path = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc";

    // Initialize FontManager instances first, as they are needed for loading_layout
    let mut font_manager = FontManager::new(&ttf_context, font_path, 32)?;

    // This needs to be mutable because we want the user to be able to change the size of the font among other things.
    let mut small_font_manager = FontManager::new(&ttf_context, font_path, 24)?;
    let mut hint_font_manager = FontManager::new(&ttf_context, font_path, 20)?;

    // Calculate loading_layout ONCE
    let loading_spans = html_parser::parse_html_to_spans("Loading Deck...");
    let loading_layout = Some(font_manager.layout_text_binary(&loading_spans, 400_u32, false)?);

    let mut app_state = AppState {
        game_state: GameState::Loading,
        scheduler: None,
        canvas_manager: CanvasManager::new(sdl_canvas, &texture_creator)?,
        font_manager, // Use the initialized manager
        small_font_manager, // Use the initialized manager
        hint_font_manager, // Use the initialized manager
        sprite: Sprite::new(),
        current_card: None,
        is_answer_revealed: false,
        front_layout_default: None, front_layout_ruby: None,
        back_layout_default: None, back_layout_ruby: None, 
        small_front_layout_default: None, small_front_layout_ruby: None,
        hint_layout: None,
        loading_layout, // NEW: Pass the pre-calculated layout
        scroll_offset: 0, loading_progress: 0.0,
        db_manager: Some(DatabaseManager::new(&deck_id).map_err(|e| e.to_string())?),
        replay_logger: Some(ReplayLogger::new(&deck_id).map_err(|e| e.to_string())?),
        show_ruby_text: false,
    };
    
    run(&mut app_state, &mut sdl_context.event_pump()?, rx)
}

/// Helper to load the next card from the scheduler and prepare its layouts.
fn load_next_card(state: &mut AppState) {
    if let Some(scheduler) = state.scheduler.as_mut() {
        state.current_card = scheduler.next_card();
        if let Some(card) = state.current_card.clone() {
            load_card_layouts(state, &card);
        } else {
            state.game_state = GameState::Done;
            state.front_layout_default = None;
            state.front_layout_ruby = None;
            state.back_layout_default = None;
            state.back_layout_ruby = None;
            state.small_front_layout_default = None;
            state.small_front_layout_ruby = None;
            state.hint_layout = None;
        }
    }
}

/// Helper to calculate all necessary layouts for a given card.
fn load_card_layouts(state: &mut AppState, card: &Card) {
    #[cfg(debug_assertions)]
    let _layout_tracer = Tracer::new("Load Card Layout");

    state.is_answer_revealed = false;
    state.scroll_offset = 0;
    state.hint_layout = None;

    if let Some(scheduler) = &state.scheduler {
        if let Some(note) = scheduler.get_note(card.note_id) {
            let content_width = 512 - 60;
            let front_html = note.fields.get(0).map_or("", |s| s.as_str());
            let back_html = note.fields.get(1).map_or("", |s| s.as_str());

            println!("\n--- Raw Front HTML ---");
            println!("{}", front_html);
            println!("----------------------\n");

            println!("\n--- Raw Back HTML ---");
            println!("{}", back_html);
            println!("---------------------\n");

            state.front_layout_default = Some(state.font_manager.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, false).unwrap());
            state.small_front_layout_default = Some(state.small_font_manager.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, false).unwrap());
            state.back_layout_default = Some(state.font_manager.layout_text_binary(&html_parser::parse_html_to_spans(back_html), content_width, false).unwrap());

            state.front_layout_ruby = Some(state.font_manager.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, true).unwrap());
            state.small_front_layout_ruby = Some(state.small_font_manager.layout_text_binary(&html_parser::parse_html_to_spans(front_html), content_width, true).unwrap());
            state.back_layout_ruby = Some(state.font_manager.layout_text_binary(&html_parser::parse_html_to_spans(back_html), content_width, true).unwrap());

        }
    }
}

fn run(state: &mut AppState, event_pump: &mut sdl2::EventPump, rx: Receiver<LoaderMessage>) -> Result<(), String> {
    'running: loop {
        if let GameState::Loading = state.game_state {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    LoaderMessage::Complete(Ok(deck)) => {
                        state.scheduler = Some(Box::new(Sm2Scheduler::new(deck)));
                        state.game_state = GameState::Reviewing;
                        load_next_card(state);
                    }
                    LoaderMessage::Complete(Err(e)) => { state.game_state = GameState::Error(e); }
                    LoaderMessage::Progress(p) => { state.loading_progress = p; }
                }
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                Event::KeyDown { keycode: Some(keycode), repeat: false, .. } => {
                    // --- ADD THIS LOGIC ---
                    // Check for the "Show Ruby" button being pressed
                    if keycode == Keycode::LShift { // <-- REPLACE WITH YOUR LT BUTTON KEYCODE
                        state.show_ruby_text = true;
                    } else {
                        // Pass other keypresses to your regular handler
                        handle_keypress(state, keycode)?;
                    }
                }
                Event::KeyUp { keycode: Some(keycode), repeat: false, .. } => {
                    // --- ADD THIS LOGIC ---
                    // Check for the "Show Ruby" button being released
                    if keycode == Keycode::LShift { // <-- REPLACE WITH YOUR LT BUTTON KEYCODE
                        state.show_ruby_text = false;
                    }
                }
                _ => {}

            }
        }
        
        state.sprite.update();
        draw_scene(state)?;
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}

fn handle_keypress(state: &mut AppState, keycode: Keycode) -> Result<(), String> {
    if let GameState::Reviewing = &state.game_state {
        // --- Handle Rewind ---
        if keycode == Keycode::Return {
            if let Some(scheduler) = state.scheduler.as_mut() {
                // First, hold the card that's currently on screen.
                if let Some(card) = &state.current_card {
                    scheduler.add_card_to_front(card.id);
                }
                
                // Then, get the previously answered card from the scheduler.
                if let Some(rewound_card) = scheduler.rewind_last_answer() {
                    load_card_layouts(state, &rewound_card);
                    state.current_card = Some(rewound_card);
                } else {
                    // If there's nothing to rewind, get the held card back.
                    load_next_card(state);
                }
            }
            return Ok(());
        }

        if let Some(card) = state.current_card.clone() {
            if state.is_answer_revealed {
                let rating = match keycode {
                    Keycode::B => Some(Rating::Again), Keycode::Y => Some(Rating::Hard),
                    Keycode::A => Some(Rating::Good), Keycode::X => Some(Rating::Easy),
                    _ => None,
                };
        
                if let Some(r) = rating {
                    let updated_card = if let Some(scheduler) = state.scheduler.as_mut() {
                        scheduler.answer_card(card.id, r)
                    } else {
                        None
                    };

                    if let Some(card_to_save) = updated_card {
                        if let Some(logger) = &state.replay_logger {
                            logger.log_action(&card_to_save, r).unwrap_or_else(|e| eprintln!("Failed to log action: {}", e));
                        }
                        if let Some(db) = &state.db_manager {
                            db.update_card_state(&card_to_save).unwrap_or_else(|e| eprintln!("Failed to update db: {}", e));
                        }
                    }

                    load_next_card(state);
                } else {
                    // Scrolling logic
                    let scroll_speed = 30;
                    let viewport_height = 290;
                    let total_height = if let (Some(front), Some(back)) = (&state.small_front_layout_default, &state.back_layout_default) {
                        front.total_height + back.total_height + 20
                    } else { 0 };

                    match keycode {
                        Keycode::Up => { state.scroll_offset = (state.scroll_offset - scroll_speed).max(0); }
                        Keycode::Down => {
                            let max_scroll = (total_height - viewport_height).max(0);
                            state.scroll_offset = (state.scroll_offset + scroll_speed).min(max_scroll);
                        }
                        _ => {}
                    }
                }
            } else if let Keycode::Up | Keycode::Down | Keycode::Left | Keycode::Right = keycode {
                state.is_answer_revealed = true;
                let margin: u32 = 30;
                let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard (Up/Down) [Enter:Rewind]");
                state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, 512 - margin * 2, state.show_ruby_text).unwrap());
            }
        }
    }
    Ok(())
}

fn draw_scene(state: &mut AppState) -> Result<(), String> {
    state.canvas_manager.start_frame()?;
    
    state.canvas_manager.with_canvas(|canvas| {
        match &state.game_state {
            GameState::Loading => {
                // NEW: Use the pre-calculated loading_layout
                if let Some(layout) = &state.loading_layout {
                    state.font_manager.draw_layout(canvas, layout, 150, 150, state.show_ruby_text)?;
                } else {
                    // Fallback, though loading_layout should always be present
                    let loading_spans = html_parser::parse_html_to_spans("Loading Deck...");
                    let layout = state.font_manager.layout_text_binary(&loading_spans, 400_u32, state.show_ruby_text)?;
                    state.font_manager.draw_layout(canvas, &layout, 150, 150, state.show_ruby_text)?;
                }

                let bar_bg_rect = Rect::new(100, 200, 312, 30);
                canvas.set_draw_color(Color::RGB(80, 80, 80));
                canvas.fill_rect(bar_bg_rect)?;

                let bar_width = (312.0 * state.loading_progress) as u32;
                let bar_fg_rect = Rect::new(100, 200, bar_width.min(312), 30);
                canvas.set_draw_color(Color::RGB(100, 180, 255));
                canvas.fill_rect(bar_fg_rect)?;
            }
            GameState::Error(e) => {
                let margin: u32 = 30;
                let error_spans = html_parser::parse_html_to_spans(&format!("Error: {}", e));
                let layout = state.font_manager.layout_text_binary(&error_spans, 512 - margin * 2, state.show_ruby_text)?;
                state.font_manager.draw_layout(canvas, &layout, margin as i32, 40, state.show_ruby_text)?;
            }
            GameState::Reviewing | GameState::Done => {
                let margin: u32 = 30;

                if let Some(scheduler) = &state.scheduler {
                    let total = scheduler.total_session_cards();
                    if total > 0 {
                        let completed = scheduler.reviews_complete();
                        let bar_height = 25_u32;
                        
                        let bar_bg_rect = Rect::new(0, 0, 512, bar_height);
                        canvas.set_draw_color(Color::RGB(60, 60, 60));
                        canvas.fill_rect(bar_bg_rect)?;

                        let progress = completed as f32 / total as f32;
                        let progress_width = (512.0 * progress) as u32;
                        let bar_fg_rect = Rect::new(0, 0, progress_width, bar_height);
                        let r = (255.0 * (1.0 - progress)) as u8;
                        let g = (255.0 * progress) as u8;
                        canvas.set_draw_color(Color::RGB(r, g, 80));
                        canvas.fill_rect(bar_fg_rect)?;

                        let progress_text = format!("{}/{}", completed, total);
                        let (text_w, text_h) = state.hint_font_manager.size_of_text(&progress_text)?;
                        let text_x = (512 as i32 - text_w as i32 - 10).max(0);
                        let text_y = (bar_height as i32 - text_h as i32) / 2;
                        state.hint_font_manager.draw_single_line(canvas, &progress_text, text_x, text_y)?;
                    }
                }
                
                state.sprite.draw(canvas)?;

                let content_viewport = Rect::new(0, 25, 512, 305);
                canvas.set_clip_rect(Some(content_viewport));

                if !state.is_answer_revealed {
                    let layout_to_draw = if state.show_ruby_text {
                        &state.front_layout_ruby
                    } else {
                        &state.front_layout_default
                    };
                    
                    if let Some(layout) = layout_to_draw {
                        state.font_manager.draw_layout(canvas, layout, margin as i32, 40, state.show_ruby_text)?;
                    }
                } else {
                    // When drawing the back of the card (which also contains the front):
                    let mut y_pos = 40 - state.scroll_offset;

                    let small_front_layout_to_draw = if state.show_ruby_text {
                        &state.small_front_layout_ruby
                    } else {
                        &state.small_front_layout_default
                    };
                    
                    let back_layout_to_draw = if state.show_ruby_text {
                        &state.back_layout_ruby
                    } else {
                        &state.back_layout_default
                    };

                    if let Some(layout) = small_front_layout_to_draw {
                        state.small_font_manager.draw_layout(canvas, layout, margin as i32, y_pos, state.show_ruby_text)?;
                        y_pos += layout.total_height + 20; // Now this will work
                    }
                    
                    // --- FIX ---
                    // Then handle the back layout
                    if let Some(layout) = back_layout_to_draw {
                        state.font_manager.draw_layout(canvas, layout, margin as i32, y_pos, state.show_ruby_text)?;
                    }

                }

                if let GameState::Done = state.game_state {
                    let done_spans = html_parser::parse_html_to_spans("Deck Complete!");
                    let layout = state.font_manager.layout_text_binary(&done_spans, 400_u32, state.show_ruby_text)?;
                    state.font_manager.draw_layout(canvas, &layout, 150, 150, state.show_ruby_text)?;
                }
                
                canvas.set_clip_rect(None);

                if state.is_answer_revealed {
                     if let Some(hint_layout) = &state.hint_layout {
                         state.hint_font_manager.draw_layout(canvas, hint_layout, margin as i32, 335, state.show_ruby_text)?;
                     }
                }
            }
        }
        Ok(())
    })?;

    state.canvas_manager.end_frame();
    Ok(())
}