// CardBrick - main.rs (Refactor Step 5: Studying Scene)

mod config;
mod deck;
mod scheduler;
mod ui;
mod storage;
mod debug;
mod scenes;

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
use deck::{Deck};
use scheduler::{Scheduler, Sm2Scheduler};
use ui::{CanvasManager, FontManager, font::TextLayout, sprite::Sprite};
use deck::html_parser;
use storage::{DatabaseManager, ReplayLogger};
use scenes::main_menu::MainMenuState;
use scenes::studying::StudyingState; // <-- Import the new state

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

pub enum GameState<'a> {
    MainMenu(MainMenuState),
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
    Studying(StudyingState<'a>), // <-- Now uses the imported StudyingState
    Error(String),
}

// --- StudyingState struct is now REMOVED from main.rs ---

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
        game_state: GameState::MainMenu(MainMenuState::new()),
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

fn handle_input(state: &mut AppState, event: Event) -> Result<(), String> {
    match &mut state.game_state {
        GameState::MainMenu(_) => scenes::main_menu::input::handle_main_menu_input(state, event),
        GameState::DeckSelection { .. } => handle_deck_selection_input(state, event),
        // --- Delegate to the new scene-specific handler ---
        GameState::Studying(_) => scenes::studying::input::handle_studying_input(state, event),
        _ => Ok(()),
    }
}

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
            // --- Delegate to the new scene-specific drawing function ---
            GameState::Studying(studying_state) => {
                scenes::studying::draw_studying_scene(canvas, studying_state, &mut state.font_manager, &mut state.small_font_manager, &mut state.hint_font_manager, &mut state.sprite)
            },
            GameState::Error(e) => draw_error_scene(canvas, &mut state.font_manager, e),
        }
    })?;
    state.canvas_manager.end_frame();
    Ok(())
}

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
                        // --- Use the new paths for state creation and logic ---
                        let mut studying_state = scenes::studying::StudyingState::new(scheduler, db_manager, replay_logger);
                        scenes::studying::logic::load_next_card(&mut studying_state, &mut state.font_manager, &mut state.small_font_manager);
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

// --- All studying-related functions are now REMOVED from main.rs ---
// - handle_studying_input
// - draw_studying_scene
// - impl<'a> StudyingState<'a>
// - load_next_card
// - load_card_layouts

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
