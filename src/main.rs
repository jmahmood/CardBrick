// CardBrick - main.rs (Refactor Step 4: Main Menu Scene)

mod config;
mod deck;
mod scheduler;
mod ui;
mod storage;
mod debug;
mod scenes; // <-- Add scenes module

use std::env;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

use config::Config;
use deck::{Card, Deck};
use scheduler::{Rating, Scheduler, Sm2Scheduler};
use ui::{CanvasManager, FontManager, font::TextLayout, sprite::Sprite};
use deck::html_parser;
use storage::{DatabaseManager, ReplayLogger};
use debug::Tracer;
use scenes::main_menu::MainMenuState; // <-- Import the new state

// --- Data Structures for the State Machine ---

#[derive(Clone)]
pub struct DeckMetadata {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

pub enum LoaderMessage {
    Progress(f32),
    Complete(Result<Deck, String>),
}

// --- GameState Enum is now cleaner ---
pub enum GameState<'a> {
    MainMenu(MainMenuState), // <-- Use the new MainMenuState struct
    DeckSelection {
        decks: Vec<DeckMetadata>,
        deck_layouts: Vec<TextLayout>,
        selected_index: usize,
    },
    Loading {
        rx: Receiver<LoaderMessage>,
        loading_layout: TextLayout,
        progress: f32,
        deck_id_to_load: String,
    },
    Studying(StudyingState<'a>),
    Error(String),
}

pub struct StudyingState<'a> {
    is_done: bool,
    scheduler: Box<dyn Scheduler + 'a>,
    db_manager: DatabaseManager,
    replay_logger: ReplayLogger,
    current_card: Option<Card>,
    is_answer_revealed: bool,
    scroll_offset: i32,
    show_ruby_text: bool,
    front_layout_default: Option<TextLayout>,
    front_layout_ruby: Option<TextLayout>,
    back_layout_default: Option<TextLayout>,
    back_layout_ruby: Option<TextLayout>,
    small_front_layout_default: Option<TextLayout>,
    small_front_layout_ruby: Option<TextLayout>,
    hint_layout: Option<TextLayout>,
    done_layout: Option<TextLayout>,
}

pub struct AppState<'a> {
    game_state: GameState<'a>,
    available_decks: Vec<DeckMetadata>,
    canvas_manager: CanvasManager<'a>,
    font_manager: FontManager<'a, 'a>,
    small_font_manager: FontManager<'a, 'a>,
    hint_font_manager: FontManager<'a, 'a>,
    sprite: Sprite,
    config: Config,
}

pub fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(format!("Usage: {} <path/to/deck.apkg>", args.get(0).unwrap_or(&"cardbrick".to_string())));
    }

    let config = Config::new();
    
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    let window = video_subsystem.window(config.window_title, config.window_width, config.window_height).position_centered().build().map_err(|e| e.to_string())?;
    let sdl_canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = sdl_canvas.texture_creator();
    
    let initial_deck_path = PathBuf::from(&args[1]);
    let deck_id = initial_deck_path.file_stem().and_then(|s| s.to_str()).unwrap_or("default").to_string();
    let deck_name = deck_id.clone();
    
    let available_decks = vec![
        DeckMetadata { id: deck_id, name: deck_name, path: initial_deck_path }
    ];

    let mut app_state = AppState {
        game_state: GameState::MainMenu(MainMenuState::new()), // <-- Start with the new state
        available_decks,
        canvas_manager: CanvasManager::new(sdl_canvas, &texture_creator)?,
        font_manager: FontManager::new(&ttf_context, config.font_path, config.font_size_large.try_into().unwrap())?,
        small_font_manager: FontManager::new(&ttf_context, config.font_path, config.font_size_medium.try_into().unwrap())?,
        hint_font_manager: FontManager::new(&ttf_context, config.font_path, config.font_size_small.try_into().unwrap())?,
        sprite: Sprite::new(),
        config,
    };
    
    run(&mut app_state, &mut sdl_context.event_pump()?)
}

fn run(state: &mut AppState, event_pump: &mut sdl2::EventPump) -> Result<(), String> {
    'running: loop {
        for event in event_pump.poll_iter() {
            if let Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } = event {
                break 'running;
            }
            handle_input(state, event)?;
        }
        
        update_state(state)?;
        state.sprite.update();
        draw_scene(state)?;
        
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}

// --- handle_input now delegates to the scene-specific handler ---
fn handle_input(state: &mut AppState, event: Event) -> Result<(), String> {
    match &mut state.game_state {
        GameState::MainMenu(_) => scenes::main_menu::input::handle_main_menu_input(state, event),
        GameState::DeckSelection { .. } => handle_deck_selection_input(state, event),
        GameState::Studying(_) => handle_studying_input(state, event),
        _ => Ok(()),
    }
}

// --- draw_scene now delegates to the scene-specific drawing function ---
fn draw_scene(state: &mut AppState) -> Result<(), String> {
    state.canvas_manager.start_frame()?;
    state.canvas_manager.with_canvas(|canvas| {
        match &mut state.game_state {
            GameState::MainMenu(main_menu_state) => {
                scenes::main_menu::draw_main_menu_scene(canvas, &mut state.font_manager, main_menu_state)
            },
            GameState::DeckSelection { deck_layouts, selected_index, .. } => {
                draw_deck_selection_scene(canvas, &mut state.font_manager, &mut state.small_font_manager, deck_layouts, *selected_index, &state.config)
            },
            GameState::Loading { loading_layout, progress, .. } => {
                draw_loading_scene(canvas, &mut state.font_manager, loading_layout, *progress)
            },
            GameState::Studying(studying_state) => {
                draw_studying_scene(canvas, studying_state, &mut state.font_manager, &mut state.small_font_manager, &mut state.hint_font_manager, &mut state.sprite)
            },
            GameState::Error(e) => draw_error_scene(canvas, &mut state.font_manager, e),
        }
    })?;
    state.canvas_manager.end_frame();
    Ok(())
}

// --- handle_main_menu_input function is now REMOVED from main.rs ---

// --- draw_main_menu_scene function is now REMOVED from main.rs ---

fn handle_deck_selection_input(state: &mut AppState, event: Event) -> Result<(), String> {
    if let Event::KeyDown { keycode: Some(keycode), repeat: false, .. } = event {
        if let GameState::DeckSelection { decks, selected_index, .. } = &mut state.game_state {
            match keycode {
                Keycode::Up => *selected_index = selected_index.saturating_sub(1),
                Keycode::Down => *selected_index = (*selected_index + 1).min(decks.len().saturating_sub(1)),
                Keycode::Backspace => {
                    state.game_state = GameState::MainMenu(MainMenuState::new());
                }
                Keycode::Return => {
                    let selected_deck = &decks[*selected_index];
                    let deck_path = selected_deck.path.clone();
                    let deck_id = selected_deck.id.clone();
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || { deck::loader::load_apkg(&deck_path, tx); });
                    let loading_spans = html_parser::parse_html_to_spans("Loading Deck...");
                    let loading_layout = state.font_manager.layout_text_binary(&loading_spans, 400, false)?;
                    state.game_state = GameState::Loading { rx, loading_layout, progress: 0.0, deck_id_to_load: deck_id };
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw_deck_selection_scene(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, font_manager: &mut FontManager, small_font_manager: &mut FontManager, layouts: &[TextLayout], selected_index: usize, config: &Config) -> Result<(), String> {
    font_manager.draw_single_line(canvas, "Select a Deck", 20, 20)?;
    small_font_manager.draw_single_line(canvas, "Press Backspace to return to Main Menu", 20, 70)?;

    let mut y_pos = 150;
    let max_width = config.window_width - 40;

    for (i, layout) in layouts.iter().enumerate() {
        if i == selected_index {
            let highlight_rect = Rect::new(18, y_pos, max_width, layout.total_height as u32);
            canvas.set_draw_color(Color::RGB(80, 80, 80));
            canvas.fill_rect(highlight_rect)?;
        }
        small_font_manager.draw_layout(canvas, layout, 20, y_pos, false)?;
        y_pos += layout.total_height + 10;
    }
    Ok(())
}

fn update_state(state: &mut AppState) -> Result<(), String> {
    let old_state = std::mem::replace(&mut state.game_state, GameState::Error("Temporary state".to_string()));
    state.game_state = match old_state {
        GameState::Loading { rx, loading_layout, progress, deck_id_to_load } => {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    LoaderMessage::Complete(Ok(deck)) => {
                        let scheduler = Box::new(Sm2Scheduler::new(deck));
                        let db_manager = DatabaseManager::new(&deck_id_to_load).map_err(|e| e.to_string())?;
                        let replay_logger = ReplayLogger::new(&deck_id_to_load).map_err(|e| e.to_string())?;
                        let mut studying_state = StudyingState::new(scheduler, db_manager, replay_logger);
                        load_next_card(&mut studying_state, &mut state.font_manager, &mut state.small_font_manager);
                        GameState::Studying(studying_state)
                    }
                    LoaderMessage::Complete(Err(e)) => GameState::Error(e),
                    LoaderMessage::Progress(p) => GameState::Loading { rx, loading_layout, progress: p, deck_id_to_load },
                }
            } else {
                GameState::Loading { rx, loading_layout, progress, deck_id_to_load }
            }
        }
        other_state => other_state,
    };
    Ok(())
}

fn handle_studying_input(state: &mut AppState, event: Event) -> Result<(), String> {
    if let GameState::Studying(studying_state) = &mut state.game_state {
        if let Event::KeyDown { keycode: Some(keycode), repeat: false, .. } = event {
            if keycode == Keycode::Backspace {
                state.game_state = GameState::DeckSelection {
                    decks: state.available_decks.clone(),
                    deck_layouts: vec![], // This will be recalculated next time
                    selected_index: 0,
                };
                // Pre-calculate layouts for the DeckSelection screen again
                if let GameState::DeckSelection { deck_layouts, .. } = &mut state.game_state {
                    let max_width = state.config.window_width - 40;
                     *deck_layouts = state.available_decks.iter().map(|deck| {
                        let spans = html_parser::parse_html_to_spans(&deck.name);
                        state.small_font_manager.layout_text_binary(&spans, max_width, false)
                    }).collect::<Result<Vec<_>,_>>()?;
                }
                return Ok(());
            }

            if keycode == Keycode::LShift {
                studying_state.show_ruby_text = true;
                return Ok(());
            }

            if keycode == Keycode::Return {
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
                studying_state.is_answer_revealed = true;
                let margin: u32 = 30;
                let hint_spans = html_parser::parse_html_to_spans("A:Good B:Again X:Easy Y:Hard (Up/Down) [Enter:Rewind]");
                studying_state.hint_layout = Some(state.hint_font_manager.layout_text_binary(&hint_spans, state.config.window_width / 2 - margin * 2, studying_state.show_ruby_text)?);
            }
        }
        if let Event::KeyUp { keycode: Some(Keycode::LShift), .. } = event {
            studying_state.show_ruby_text = false;
        }
    }
    Ok(())
}

fn draw_studying_scene(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, studying_state: &mut StudyingState, font_manager: &mut FontManager, small_font_manager: &mut FontManager, hint_font_manager: &mut FontManager, sprite: &mut Sprite) -> Result<(), String> {
    let margin: u32 = 30;
    let total = studying_state.scheduler.total_session_cards();
    if total > 0 {
        let completed = studying_state.scheduler.reviews_complete();
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
        let (text_w, text_h) = hint_font_manager.size_of_text(&progress_text)?;
        let text_x = (512 as i32 - text_w as i32 - 10).max(0);
        let text_y = (bar_height as i32 - text_h as i32) / 2;
        hint_font_manager.draw_single_line(canvas, &progress_text, text_x, text_y)?;
    }
    
    sprite.draw(canvas)?;
    let content_viewport = Rect::new(0, 25, 512, 305);
    canvas.set_clip_rect(Some(content_viewport));

    if !studying_state.is_answer_revealed {
        let layout_to_draw = if studying_state.show_ruby_text { &studying_state.front_layout_ruby } else { &studying_state.front_layout_default };
        if let Some(layout) = layout_to_draw {
            font_manager.draw_layout(canvas, layout, margin as i32, 40, studying_state.show_ruby_text)?;
        }
    } else {
        let mut y_pos = 40 - studying_state.scroll_offset;
        let small_front_layout_to_draw = if studying_state.show_ruby_text { &studying_state.small_front_layout_ruby } else { &studying_state.small_front_layout_default };
        let back_layout_to_draw = if studying_state.show_ruby_text { &studying_state.back_layout_ruby } else { &studying_state.back_layout_default };
        if let Some(layout) = small_front_layout_to_draw {
            small_font_manager.draw_layout(canvas, layout, margin as i32, y_pos, studying_state.show_ruby_text)?;
            y_pos += layout.total_height + 20;
        }
        if let Some(layout) = back_layout_to_draw {
            font_manager.draw_layout(canvas, layout, margin as i32, y_pos, studying_state.show_ruby_text)?;
        }
    }

    if studying_state.is_done {
        if let Some(layout) = &studying_state.done_layout {
            font_manager.draw_layout(canvas, layout, 150, 150, studying_state.show_ruby_text)?;
        }
    }
    
    canvas.set_clip_rect(None);
    if studying_state.is_answer_revealed {
        if let Some(hint_layout) = &studying_state.hint_layout {
            hint_font_manager.draw_layout(canvas, hint_layout, margin as i32, 335, studying_state.show_ruby_text)?;
        }
    }
    Ok(())
}

fn draw_loading_scene(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, font_manager: &mut FontManager, layout: &TextLayout, progress: f32) -> Result<(), String> {
    font_manager.draw_layout(canvas, layout, 150, 150, false)?;
    let bar_bg_rect = Rect::new(100, 200, 312, 30);
    canvas.set_draw_color(Color::RGB(80, 80, 80));
    canvas.fill_rect(bar_bg_rect)?;
    let bar_width = (312.0 * progress) as u32;
    let bar_fg_rect = Rect::new(100, 200, bar_width.min(312), 30);
    canvas.set_draw_color(Color::RGB(100, 180, 255));
    canvas.fill_rect(bar_fg_rect)?;
    Ok(())
}

fn draw_error_scene(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, font_manager: &mut FontManager, msg: &str) -> Result<(), String> {
    let margin: u32 = 30;
    let error_spans = html_parser::parse_html_to_spans(&format!("Error: {}", msg));
    let layout = font_manager.layout_text_binary(&error_spans, 512 - margin * 2, false)?;
    font_manager.draw_layout(canvas, &layout, margin as i32, 40, false)?;
    Ok(())
}

impl<'a> StudyingState<'a> {
    fn new(scheduler: Box<dyn Scheduler + 'a>, db_manager: DatabaseManager, replay_logger: ReplayLogger) -> Self {
        Self { is_done: false, scheduler, db_manager, replay_logger, current_card: None, is_answer_revealed: false, scroll_offset: 0, show_ruby_text: false, front_layout_default: None, front_layout_ruby: None, back_layout_default: None, back_layout_ruby: None, small_front_layout_default: None, small_front_layout_ruby: None, hint_layout: None, done_layout: None }
    }
}

fn load_next_card(state: &mut StudyingState, font: &mut FontManager, small_font: &mut FontManager) {
    state.current_card = state.scheduler.next_card();
    if let Some(card) = state.current_card.clone() {
        load_card_layouts(state, &card, font, small_font);
    } else {
        state.is_done = true;
        let done_spans = html_parser::parse_html_to_spans("Deck Complete!");
        state.done_layout = font.layout_text_binary(&done_spans, 400_u32, false).ok();
    }
}

fn load_card_layouts(state: &mut StudyingState, card: &Card, font: &mut FontManager, small_font: &mut FontManager) {
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
