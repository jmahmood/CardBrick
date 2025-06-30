// CardBrick - main.rs (Refactor Step 6: Deck Selection Scene)

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
// --- FIX: Removed unused `self` and `thread` imports ---
use std::sync::mpsc::{Receiver};

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
use scenes::studying::StudyingState;
use scenes::deck_selection::DeckSelectionState;

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
    DeckSelection(DeckSelectionState),
    Loading {
        rx: Receiver<LoaderMessage>,
        loading_layout: TextLayout,
        progress: f32,
        deck_id_to_load: String,
    },
    Studying(StudyingState<'a>),
    Error(String),
}

pub struct AppState<'a> {
    pub game_state: GameState<'a>,
    pub available_decks: Vec<DeckMetadata>,
    pub canvas_manager: CanvasManager<'a>,
    pub font_manager: FontManager<'a, 'a>,
    pub small_font_manager: FontManager<'a, 'a>,
    pub hint_font_manager: FontManager<'a, 'a>,
    pub sprite: Sprite,
    pub config: Config,
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
        GameState::DeckSelection(_) => scenes::deck_selection::input::handle_deck_selection_input(state, event),
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
            GameState::DeckSelection(deck_selection_state) => {
                scenes::deck_selection::draw_deck_selection_scene(canvas, &mut state.font_manager, &mut state.small_font_manager, deck_selection_state, &state.config)
            },
            GameState::Loading { loading_layout, progress, .. } => {
                draw_loading_scene(canvas, &mut state.font_manager, loading_layout, *progress)
            },
            GameState::Studying(studying_state) => {
                scenes::studying::draw_studying_scene(canvas, studying_state, &mut state.font_manager, &mut state.small_font_manager, &mut state.hint_font_manager, &mut state.sprite)
            },
            GameState::Error(e) => draw_error_scene(canvas, &mut state.font_manager, e),
        }
    })?;
    state.canvas_manager.end_frame();
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
