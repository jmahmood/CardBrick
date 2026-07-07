#![allow(dead_code)]

// CardBrick - Macroquad Version with Device Compatibility
use crate::scenes::deck_selection::draw_deck_selection_scene;
use std::io::Write;

use config::{assets, UiAssets};

use evdev::Device as EvdevDevice;
use log::{debug, error, info, warn};
use macroquad::miniquad;
use macroquad::prelude::*;
use std::os::fd::AsRawFd;

mod config;
mod debug;
mod deck;
mod perf;
mod scenes;
mod scheduler;
mod state;
mod storage;
mod ui;

#[cfg(test)]
mod testing;

use config::Config;
use deck::html_parser;
use deck::scanner;
use scenes::deck_selection::DeckSelectionState;
use scenes::main_menu::MainMenuState;
use scheduler::{Scheduler, Sm2Scheduler};
use state::{
    map_evdev_to_brick_input, AppState, AudioManager, BackgroundFontMessage, BrickButton,
    BrickInput, DeckMetadata, GameState, LoaderMessage,
};
use storage::{DatabaseManager, ReplayLogger};
use ui::{CanvasManager, FontManager, Sprite};

#[derive(Debug, Clone)]
enum DeviceType {
    Desktop,
    TrimUIBrick,
    RG35XXPlus,
}

#[derive(Clone)]
struct DeviceConfig {
    _device_type: DeviceType,
    display_width: u32,
    display_height: u32,
    fullscreen: bool,
}

impl DeviceConfig {
    fn detect_device() -> Self {
        info!("Detecting device type...");

        // Check for TrimUI Brick
        let trimui_path = "/mnt/SDCARD/Tools";
        debug!("Checking for TrimUI Brick at: {}", trimui_path);
        if std::path::Path::new(trimui_path).exists() {
            info!("Detected TrimUI Brick device");
            return Self {
                _device_type: DeviceType::TrimUIBrick,
                display_width: 640,
                display_height: 480,
                fullscreen: true,
            };
        }

        // Check for RG35XX Plus
        let rg35xx_path = "/storage/.config/distribution";
        debug!("Checking for RG35XX Plus at: {}", rg35xx_path);
        if std::path::Path::new(rg35xx_path).exists() {
            info!("Detected RG35XX Plus device");
            return Self {
                _device_type: DeviceType::RG35XXPlus,
                display_width: 640,
                display_height: 480,
                fullscreen: true,
            };
        }

        // Default to desktop
        info!("Defaulting to Desktop device");
        Self {
            _device_type: DeviceType::Desktop,
            display_width: 1024,
            display_height: 768,
            fullscreen: false,
        }
    }
}

struct CardBrickApp {
    app_state: AppState,
    last_input_time: f64,
    needs_redraw: bool,
}

impl CardBrickApp {
    async fn new_with_progress(
        loading_canvas: &mut CanvasManager,
        loading_steps: &[&str],
    ) -> Result<Self, String> {
        info!("Initializing CardBrick application...");

        // Step 0: Initialize configuration
        Self::show_loading_step(loading_canvas, loading_steps, 0).await;
        let config = Config::new();

        if let Err(e) = test_file_creation() {
            return Err(format!("[File Creation Test] FAILED with error: {}", e));
        }

        // Initialize audio
        let audio = AudioManager::new().map_err(|e| {
            error!("Audio init failed: {}", e);
            e
        })?;

        // Initialize gamepad
        let gamepad = find_gamepad();

        // Step 1: Load small menu font for immediate startup
        info!(
            "Loading small menu font from embedded bytes (size: {} bytes)",
            assets::MENU_FONT.len()
        );
        let _menu_font =
            Self::load_menu_font_with_progress(loading_canvas, loading_steps, 1).await?;

        // Step 2: Create canvas manager
        Self::show_loading_step(loading_canvas, loading_steps, 2).await;
        let canvas_manager = CanvasManager::new()?;

        // Step 3: Create basic font managers using menu font for now
        Self::show_loading_step(loading_canvas, loading_steps, 3).await;
        let menu_font = load_ttf_font_from_bytes(assets::MENU_FONT)
            .map_err(|e| format!("Failed to load menu font: {:?}", e))?;

        // Create temporary font managers using the small meFnu font
        let font_manager = FontManager::from_loaded_font(
            Some(menu_font.clone()),
            None,
            config.font_size_large.try_into().unwrap(),
        );
        let small_font_manager = FontManager::from_loaded_font(
            Some(menu_font.clone()),
            None,
            config.font_size_medium.try_into().unwrap(),
        );
        let hint_font_manager = FontManager::from_loaded_font(
            Some(menu_font.clone()),
            None,
            config.font_size_small.try_into().unwrap(),
        );

        // Step 4: Load available decks from cache
        Self::show_loading_step(loading_canvas, loading_steps, 4).await;
        let available_decks = scanner::load_cached_decks()?;
        if available_decks.is_empty() {
            return Err(
                "No cached decks found. Run precache_decks.py to cache .apkg files.".to_string(),
            );
        }

        // Step 5: Skip UI assets loading (load lazily when needed)
        Self::show_loading_step(loading_canvas, loading_steps, 5).await;

        // Step 6: Start background font loading (non-blocking)
        Self::show_loading_step(loading_canvas, loading_steps, 6).await;
        let background_font_receiver = Self::start_background_font_loading().await?;

        // Step 7: Finalize setup
        Self::show_loading_step(loading_canvas, loading_steps, 7).await;

        // Set initial performance monitoring scene
        perf::set_perf_scene("main_menu");

        let app_state = AppState {
            game_state: GameState::MainMenu(MainMenuState::new()),
            available_decks,
            canvas_manager,
            font_manager,
            small_font_manager,
            hint_font_manager,
            sprite: Sprite::new(),
            config,
            gamepad,
            audio,
            background_font_receiver: Some(background_font_receiver),
            japanese_font_ready: false,
            ui_assets: None,
        };

        info!("CardBrick application initialized successfully");
        Ok(Self {
            app_state,
            last_input_time: 0.0,
            needs_redraw: true,
        })
    }

    async fn show_loading_step(
        loading_canvas: &mut CanvasManager,
        loading_steps: &[&str],
        step: usize,
    ) {
        loading_canvas.start_frame().unwrap();

        // Draw loading background
        let (logical_width, logical_height) = loading_canvas.logical_size();
        let (screen_x, screen_y) = loading_canvas.logical_to_screen(0.0, 0.0);
        let screen_w = logical_width * loading_canvas.get_scale_factor();
        let screen_h = logical_height * loading_canvas.get_scale_factor();

        draw_rectangle(
            screen_x,
            screen_y,
            screen_w,
            screen_h,
            Color::from_rgba(40, 40, 45, 255),
        );

        // Draw current step
        let progress = (step as f32 + 1.0) / loading_steps.len() as f32;
        draw_text(
            "Loading CardBrick...",
            screen_x + 100.0,
            screen_y + 180.0,
            32.0,
            WHITE,
        );
        if step < loading_steps.len() {
            draw_text(
                loading_steps[step],
                screen_x + 100.0,
                screen_y + 220.0,
                20.0,
                Color::from_rgba(200, 200, 200, 255),
            );
        }

        // Draw progress bar
        let (bg_x, bg_y) = loading_canvas.logical_to_screen(100.0, 260.0);
        let bg_w = 312.0 * loading_canvas.get_scale_factor();
        let bg_h = 30.0 * loading_canvas.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(80, 80, 80, 255));

        let fg_w = bg_w * progress;
        draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(100, 180, 255, 255));

        loading_canvas.end_frame();
        next_frame().await;
    }

    async fn load_menu_font_with_progress(
        loading_canvas: &mut CanvasManager,
        loading_steps: &[&str],
        step: usize,
    ) -> Result<Font, String> {
        let size_kb = assets::MENU_FONT.len() as f32 / 1024.0;

        Self::show_loading_progress(
            loading_canvas,
            loading_steps,
            step,
            &format!("Loading menu font ({:.1}KB)", size_kb),
            0.5,
        )
        .await;

        // Load the small menu font (should be very fast)
        let font = load_ttf_font_from_bytes(assets::MENU_FONT)
            .map_err(|e| format!("Failed to load menu font: {:?}", e))?;

        Self::show_loading_progress(loading_canvas, loading_steps, step, "Menu font ready", 1.0)
            .await;

        Ok(font)
    }

    async fn show_loading_progress(
        loading_canvas: &mut CanvasManager,
        loading_steps: &[&str],
        step: usize,
        detailed_msg: &str,
        progress: f32,
    ) {
        loading_canvas.start_frame().unwrap();

        // Draw loading background
        let (logical_width, logical_height) = loading_canvas.logical_size();
        let (screen_x, screen_y) = loading_canvas.logical_to_screen(0.0, 0.0);
        let screen_w = logical_width * loading_canvas.get_scale_factor();
        let screen_h = logical_height * loading_canvas.get_scale_factor();

        draw_rectangle(
            screen_x,
            screen_y,
            screen_w,
            screen_h,
            Color::from_rgba(40, 40, 45, 255),
        );

        // Draw current step with detailed progress
        let overall_progress = (step as f32 + progress) / loading_steps.len() as f32;
        draw_text(
            "Loading CardBrick...",
            screen_x + 100.0,
            screen_y + 160.0,
            32.0,
            WHITE,
        );
        if step < loading_steps.len() {
            draw_text(
                loading_steps[step],
                screen_x + 100.0,
                screen_y + 190.0,
                20.0,
                Color::from_rgba(200, 200, 200, 255),
            );
        }
        draw_text(
            detailed_msg,
            screen_x + 100.0,
            screen_y + 220.0,
            16.0,
            Color::from_rgba(150, 150, 150, 255),
        );

        // Draw overall progress bar
        let (bg_x, bg_y) = loading_canvas.logical_to_screen(100.0, 250.0);
        let bg_w = 312.0 * loading_canvas.get_scale_factor();
        let bg_h = 20.0 * loading_canvas.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(80, 80, 80, 255));

        let fg_w = bg_w * overall_progress;
        draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(100, 180, 255, 255));

        // Draw step-specific progress bar
        let (step_bg_x, step_bg_y) = loading_canvas.logical_to_screen(100.0, 280.0);
        let step_bg_h = 15.0 * loading_canvas.get_scale_factor();
        draw_rectangle(
            step_bg_x,
            step_bg_y,
            bg_w,
            step_bg_h,
            Color::from_rgba(60, 60, 60, 255),
        );

        let step_fg_w = bg_w * progress;
        draw_rectangle(
            step_bg_x,
            step_bg_y,
            step_fg_w,
            step_bg_h,
            Color::from_rgba(80, 160, 255, 255),
        );

        loading_canvas.end_frame();
        next_frame().await;
    }

    async fn start_background_font_loading(
    ) -> Result<std::sync::mpsc::Receiver<BackgroundFontMessage>, String> {
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();

        // Start background thread for simulating font loading progress
        thread::spawn(move || {
            info!("Starting background font loading progress simulation...");

            let total_size = assets::JAPANESE_FONT.len();
            let progress_points = [0.0, 0.25, 0.5, 0.75, 0.85];

            for (i, &progress) in progress_points.iter().enumerate() {
                let message = match i {
                    0 => "Starting Japanese font parsing...",
                    1 => "Parsing font tables...",
                    2 => "Loading character mappings...",
                    3 => "Processing CJK glyphs...",
                    4 => "Finalizing font data...",
                    _ => "Loading Japanese font...",
                };

                if let Err(e) = tx.send(BackgroundFontMessage::Progress {
                    font_type: "Japanese".to_string(),
                    progress,
                    message: message.to_string(),
                    size_info: format!("{:.1}KB", total_size as f32 / 1024.0),
                }) {
                    error!("Failed to send progress update: {}", e);
                }

                // Simulate parsing work
                std::thread::sleep(std::time::Duration::from_millis(200));
            }

            // Signal that we're ready to load the font on the main thread
            if let Err(e) = tx.send(BackgroundFontMessage::ReadyToLoadFont) {
                error!("Failed to send font loading signal: {}", e);
            }

            info!("Background font loading progress simulation completed");
        });

        Ok(rx)
    }

    fn handle_background_font_messages(&mut self) {
        let mut messages_to_process = Vec::new();
        let mut should_remove_receiver = false;

        if let Some(ref receiver) = self.app_state.background_font_receiver {
            // Collect all available background font messages
            while let Ok(message) = receiver.try_recv() {
                messages_to_process.push(message);
            }
        }

        // Process the collected messages
        for message in messages_to_process {
            match message {
                BackgroundFontMessage::Progress {
                    font_type,
                    progress,
                    message,
                    size_info,
                } => {
                    debug!(
                        "Background font loading progress: {} - {:.1}% - {} ({})",
                        font_type,
                        progress * 100.0,
                        message,
                        size_info
                    );
                }
                BackgroundFontMessage::ReadyToLoadFont => {
                    info!("Loading Japanese font on main thread...");
                    // Load the font on the main thread (macroquad requirement)
                    match load_ttf_font_from_bytes(assets::JAPANESE_FONT) {
                        Ok(font) => {
                            info!("Japanese font loaded successfully on main thread");
                            // Update font managers with the new Japanese font
                            self.app_state.font_manager = FontManager::from_loaded_font(
                                Some(font.clone()),
                                None, // No fallback needed since Japanese font is comprehensive
                                32,
                            );
                            // self.app_state.small_font_manager = FontManager::from_loaded_font(
                            //     Some(font.clone()),
                            //     None, // No fallback needed since Japanese font is comprehensive
                            //     20
                            // );
                            // self.app_state.hint_font_manager = FontManager::from_loaded_font(
                            //     Some(font),
                            //     None, // No fallback needed since Japanese font is comprehensive
                            //     16
                            // );
                            self.app_state.japanese_font_ready = true;
                            // Ensure we redraw to reflect the new font
                            self.needs_redraw = true;
                            should_remove_receiver = true;
                        }
                        Err(e) => {
                            error!("Failed to load Japanese font on main thread: {:?}", e);
                            // Keep using the menu font as fallback
                            should_remove_receiver = true;
                        }
                    }
                }
            }
        }

        // Remove the receiver if we're done with font loading
        if should_remove_receiver {
            self.app_state.background_font_receiver = None;
        }
    }

    fn update(&mut self) -> Result<(), String> {
        let current_time = get_time();
        let mut input_handled = false;

        // Handle background font loading messages
        self.handle_background_font_messages();

        // Handle gamepad input
        let events = if let Some(gamepad) = &mut self.app_state.gamepad {
            match gamepad.fetch_events() {
                Ok(events) => Some(events.collect::<Vec<_>>()),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No events available, which is normal
                    None
                }
                Err(e) => {
                    warn!("Error reading gamepad events: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Process events after releasing the gamepad borrow
        if let Some(events) = events {
            for event in events {
                if let Some(input) = map_evdev_to_brick_input(&event) {
                    self.handle_input(input)?;
                    input_handled = true;
                }
            }
        }

        // Handle keyboard input (for testing)
        if is_key_pressed(KeyCode::Escape) {
            return Err("User quit".into());
        }

        if is_key_pressed(KeyCode::R) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::RightShoulder))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::L) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::LeftShoulder))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::A) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::A))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::B) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::B))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::X) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::X))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Y) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::Y))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Space) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::A))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Enter) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::DPadRight))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Left) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::DPadLeft))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Up) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::DPadUp))?;
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Down) {
            self.handle_input(BrickInput::ButtonDown(BrickButton::DPadDown))?;
            input_handled = true;
        }

        if input_handled {
            self.last_input_time = current_time;
            self.needs_redraw = true;
        }

        // Update game state
        self.update_state()?;
        if self.app_state.sprite.update() {
            self.needs_redraw = true;
        }

        // Minimal redraw cadence for animated studying screens
        if let GameState::Studying(studying_state) = &mut self.app_state.game_state {
            use crate::scenes::studying::StudyingScreenMode;
            match studying_state.mode {
                StudyingScreenMode::GoalSplash => {
                    // Fade-in and timer-driven transition need continuous redraw
                    self.needs_redraw = true;
                }
                // Flash prompt ~2Hz; redraw on boundary to save power.
                StudyingScreenMode::SessionComplete
                    if studying_state.animation_frame_counter % 30 == 0 =>
                {
                    self.needs_redraw = true;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, input: BrickInput) -> Result<(), String> {
        // Any input implies a visible change soon; mark for redraw.
        self.needs_redraw = true;
        if let BrickInput::ButtonDown(BrickButton::Guide) = input {
            return Err("User quit".into());
        }

        match &mut self.app_state.game_state {
            GameState::MainMenu(_) => {
                scenes::main_menu::input::handle_main_menu_input(&mut self.app_state, input)
            }
            GameState::DeckSelection(_) => {
                scenes::deck_selection::input::handle_deck_selection_input(
                    &mut self.app_state,
                    input,
                )
            }
            GameState::Studying(_) => {
                scenes::studying::input::handle_studying_input(&mut self.app_state, input)
            }
            GameState::Progress(_) => {
                scenes::progress::input::handle_progress_input(&mut self.app_state, input)
            }
            GameState::RestoreWizard(_) => {
                scenes::restore_wizard::input::handle_restore_wizard_input(
                    &mut self.app_state,
                    input,
                )
            }
            _ => Ok(()),
        }
    }

    fn update_state(&mut self) -> Result<(), String> {
        let old_state = std::mem::replace(
            &mut self.app_state.game_state,
            GameState::Error("Temporary state".to_string()),
        );
        self.app_state.game_state = match old_state {
            GameState::Loading {
                rx,
                loading_layout,
                progress,
                deck_id_to_load,
            } => {
                if let Ok(msg) = rx.try_recv() {
                    match msg {
                        LoaderMessage::Complete(Ok(deck)) => {
                            let start_time = std::time::Instant::now();
                            println!("🎯 Starting post-load initialization...");

                            // Initialize progress database with all required tables BEFORE using queue system
                            crate::storage::db::init_progress_database()
                                .map_err(|e| e.to_string())?;
                            println!(
                                "🎯 [{}ms] Progress database initialized",
                                start_time.elapsed().as_millis()
                            );

                            let scheduler = Box::new(Sm2Scheduler::new(deck));
                            println!(
                                "🎯 [{}ms] Scheduler created",
                                start_time.elapsed().as_millis()
                            );

                            let db_manager = DatabaseManager::new(&deck_id_to_load)
                                .map_err(|e| e.to_string())?;
                            println!(
                                "🎯 [{}ms] DatabaseManager created",
                                start_time.elapsed().as_millis()
                            );

                            let replay_logger =
                                ReplayLogger::new(&deck_id_to_load).map_err(|e| e.to_string())?;
                            println!(
                                "🎯 [{}ms] ReplayLogger created",
                                start_time.elapsed().as_millis()
                            );

                            let mut studying_state = scenes::studying::StudyingState::new(
                                scheduler,
                                db_manager,
                                replay_logger,
                            );
                            println!(
                                "🎯 [{}ms] StudyingState created",
                                start_time.elapsed().as_millis()
                            );

                            scenes::studying::logic::load_next_card(
                                &mut studying_state,
                                &mut self.app_state.font_manager,
                                &mut self.app_state.small_font_manager,
                            );
                            println!(
                                "🎯 [{}ms] TOTAL POST-LOAD TIME - Study mode ready",
                                start_time.elapsed().as_millis()
                            );

                            // Set performance monitoring scene
                            perf::set_perf_scene("studying");

                            GameState::Studying(Box::new(studying_state))
                        }
                        LoaderMessage::Complete(Err(e)) => GameState::Error(e),
                        LoaderMessage::Progress(p) => GameState::Loading {
                            rx,
                            loading_layout,
                            progress: p,
                            deck_id_to_load,
                        },
                    }
                } else {
                    GameState::Loading {
                        rx,
                        loading_layout,
                        progress,
                        deck_id_to_load,
                    }
                }
            }
            GameState::GoToDeckSelection => {
                // Set performance monitoring scene
                perf::set_perf_scene("deck_selection");

                let new_state = DeckSelectionState::new(self.app_state.available_decks.clone())?;

                // Load UI assets once when entering deck selection
                if self.app_state.ui_assets.is_none() {
                    // Since we can't await in a sync function, we'll use futures::executor::block_on
                    // This is acceptable for asset loading during state transitions
                    let assets_dir = &self.app_state.config.assets_dir;
                    if let Ok(assets) = futures::executor::block_on(UiAssets::load(assets_dir)) {
                        self.app_state.ui_assets = Some(assets);
                    }
                }

                GameState::DeckSelection(new_state)
            }
            other_state => other_state,
        };
        // State changes typically require a redraw
        self.needs_redraw = true;
        Ok(())
    }

    fn draw(&mut self) -> Result<(), String> {
        self.app_state.canvas_manager.start_frame()?;

        match &mut self.app_state.game_state {
            GameState::MainMenu(main_menu_state) => scenes::main_menu::draw_main_menu_scene(
                &self.app_state.font_manager,
                &self.app_state.small_font_manager,
                &self.app_state.canvas_manager,
                main_menu_state,
            ),
            GameState::DeckSelection(deck_selection_state) => draw_deck_selection_scene(
                &self.app_state.font_manager,
                &self.app_state.small_font_manager,
                &self.app_state.canvas_manager,
                deck_selection_state,
                &self.app_state.ui_assets,
            ),
            GameState::Loading {
                loading_layout,
                progress,
                ..
            } => draw_loading_scene(
                &self.app_state.font_manager,
                &self.app_state.canvas_manager,
                loading_layout,
                *progress,
            ),
            GameState::Studying(studying_state) => scenes::studying::draw_studying_scene(
                studying_state,
                &self.app_state.font_manager,
                &self.app_state.small_font_manager,
                &self.app_state.hint_font_manager,
                &mut self.app_state.sprite,
                &self.app_state.canvas_manager,
            ),
            GameState::Progress(progress_state) => scenes::progress::draw_progress_scene(
                progress_state,
                &self.app_state.font_manager,
                &self.app_state.small_font_manager,
                &self.app_state.canvas_manager,
            ),
            GameState::RestoreWizard(restore_wizard_state) => {
                crate::scenes::restore_wizard::draw_restore_wizard_scene(
                    &self.app_state.font_manager,
                    &self.app_state.canvas_manager,
                    restore_wizard_state,
                )
            }
            GameState::Error(e) => draw_error_scene(
                &self.app_state.font_manager,
                &self.app_state.canvas_manager,
                e,
            ),
            GameState::GoToDeckSelection => {
                // This should be handled in update_state
                Ok(())
            }
        }?;

        self.app_state.canvas_manager.end_frame();
        Ok(())
    }
}

fn find_gamepad() -> Option<EvdevDevice> {
    info!("Searching for gamepad...");
    for i in 0..32 {
        let path = format!("/dev/input/event{}", i);
        if let Ok(dev) = EvdevDevice::open(&path) {
            let name = dev.name().unwrap_or_default().to_lowercase();
            if name.contains("h700 gamepad") {
                info!("Success! Found device: {}", path);

                // Set non-blocking mode
                let raw_fd = dev.as_raw_fd();
                match nix::fcntl::fcntl(
                    raw_fd,
                    nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
                ) {
                    Ok(_) => {
                        info!("Gamepad set to non-blocking mode");
                        return Some(dev);
                    }
                    Err(e) => {
                        error!("Failed to set gamepad to non-blocking: {}", e);
                        return None;
                    }
                }
            }
        }
    }
    warn!("Could not find 'h700 gamepad' device.");
    None
}

fn draw_loading_scene(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    layout: &ui::font::TextLayout,
    progress: f32,
) -> Result<(), String> {
    font_manager.draw_layout(layout, 150, 150, false, canvas_manager)?;

    // Draw progress bar background
    let (bg_x, bg_y) = canvas_manager.logical_to_screen(100.0, 200.0);
    let bg_w = 312.0 * canvas_manager.get_scale_factor();
    let bg_h = 30.0 * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(80, 80, 80, 255));

    // Draw progress bar foreground
    let bar_width = 312.0 * progress;
    let fg_w = bar_width * canvas_manager.get_scale_factor();
    draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(100, 180, 255, 255));

    Ok(())
}

fn draw_error_scene(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    msg: &str,
) -> Result<(), String> {
    let margin: u32 = 30;
    let error_spans = html_parser::parse_html_to_spans(&format!("Error: {}", msg));
    let layout = font_manager.layout_text_binary(&error_spans, 512 - margin * 2, false)?;
    font_manager.draw_layout(&layout, margin as i32, 40, false, canvas_manager)?;
    Ok(())
}

fn test_file_creation() -> std::io::Result<()> {
    println!("[File Test] Starting file creation test...");

    println!("[File Test] Attempting to create a named temp file...");
    let mut temp_file = tempfile::NamedTempFile::new()?;
    println!(
        "[File Test] Successfully created temp file at: {:?}",
        temp_file.path()
    );

    println!("[File Test] Attempting to write a small amount of data...");
    temp_file.write_all(b"hello world")?;
    println!("[File Test] Successfully wrote data.");

    println!("[File Test] The file will now be closed and deleted.");
    temp_file.close()?;

    println!("[File Test] PASSED.");
    Ok(())
}

fn window_conf() -> Conf {
    let config = DeviceConfig::detect_device();
    info!(
        "Window config: {}x{}, fullscreen: {}",
        config.display_width, config.display_height, config.fullscreen
    );

    // Try windowed mode first for debugging
    let use_windowed = std::env::var("CARDBRICK_WINDOWED").is_ok();
    let fullscreen = if use_windowed {
        warn!("Using windowed mode for debugging (CARDBRICK_WINDOWED set)");
        false
    } else {
        config.fullscreen
    };

    Conf {
        window_title: "CardBrick - Pure Rust".to_string(),
        window_width: config.display_width as i32,
        window_height: config.display_height as i32,
        fullscreen,
        platform: miniquad::conf::Platform {
            linux_backend: miniquad::conf::LinuxBackend::WaylandWithX11Fallback, // or WaylandWithX11Fallback
            ..Default::default()
        },
        sample_count: 1,
        window_resizable: false,
        icon: None,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Initialize logging
    env_logger::init();

    info!("Starting CardBrick application");

    // Initialize performance monitoring
    perf::init_perf_monitor();

    // Show loading screen while initializing
    info!("Initializing application...");

    // Create a minimal canvas manager for the loading screen
    let mut loading_canvas = CanvasManager::new().unwrap_or_else(|e| {
        error!("Failed to create canvas: {}", e);
        std::process::exit(1);
    });

    // Show actual loading progress during app initialization
    let loading_steps = vec![
        "Initializing configuration...",
        "Loading menu font...",
        "Creating canvas manager...",
        "Creating basic font managers...",
        "Loading available decks...",
        "Preparing graphics system...",
        "Starting background font loading...",
        "Finalizing setup...",
    ];

    for (i, step_msg) in loading_steps.iter().enumerate() {
        loading_canvas.start_frame().unwrap();

        // Draw loading background
        let (logical_width, logical_height) = loading_canvas.logical_size();
        let (screen_x, screen_y) = loading_canvas.logical_to_screen(0.0, 0.0);
        let screen_w = logical_width * loading_canvas.get_scale_factor();
        let screen_h = logical_height * loading_canvas.get_scale_factor();

        draw_rectangle(
            screen_x,
            screen_y,
            screen_w,
            screen_h,
            Color::from_rgba(40, 40, 45, 255),
        );

        // Draw current step
        let progress = (i as f32 + 1.0) / loading_steps.len() as f32;
        draw_text(
            "Loading CardBrick...",
            screen_x + 100.0,
            screen_y + 180.0,
            32.0,
            WHITE,
        );
        draw_text(
            step_msg,
            screen_x + 100.0,
            screen_y + 220.0,
            20.0,
            Color::from_rgba(200, 200, 200, 255),
        );

        // Draw progress bar
        let (bg_x, bg_y) = loading_canvas.logical_to_screen(100.0, 260.0);
        let bg_w = 312.0 * loading_canvas.get_scale_factor();
        let bg_h = 30.0 * loading_canvas.get_scale_factor();
        draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::from_rgba(80, 80, 80, 255));

        let fg_w = bg_w * progress;
        draw_rectangle(bg_x, bg_y, fg_w, bg_h, Color::from_rgba(100, 180, 255, 255));

        loading_canvas.end_frame();
        next_frame().await;
    }

    let mut app = match CardBrickApp::new_with_progress(&mut loading_canvas, &loading_steps).await {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            return;
        }
    };
    info!("Application initialized, entering main loop");

    // Desktop FPS cap (env CARDBRICK_TARGET_FPS; default 60)
    let target_fps: f64 = std::env::var("CARDBRICK_TARGET_FPS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(60.0);
    let target_dt = 1.0 / target_fps.max(1.0);

    let mut frame_count = 0;
    loop {
        frame_count += 1;
        let frame_start = std::time::Instant::now();

        if frame_count % 60 == 0 {
            if cfg!(debug_assertions) {
                info!("FPS {}", macroquad::time::get_fps());
            }
            // Take performance sample every second (60 frames)
            perf::perf_sample();
        }

        // Save performance data periodically (every 5 minutes = 18000 frames)
        if frame_count % 18000 == 0 {
            let filename = format!("perf_dump_{}.json", frame_count / 18000);
            if let Err(e) = perf::save_perf_results(&filename) {
                warn!("Failed to save performance data: {}", e);
            } else {
                info!("Performance data saved to {}", filename);
            }
        }

        // Add error handling around update and conditional draw
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Err(e) = app.update() {
                error!("Update error: {}", e);
                if e.contains("quit") {
                    // Save performance data before exiting
                    let _ = perf::save_perf_results("perf_final.json");
                    std::process::exit(0);
                }
            }
            if app.needs_redraw {
                if let Err(e) = app.draw() {
                    error!("Draw error: {}", e);
                } else {
                    // A full frame was drawn; we can reset the redraw flag
                    app.needs_redraw = false;
                }
            }
        })) {
            Ok(()) => {
                // Success
            }
            Err(e) => {
                error!("Panic in main loop: {:?}", e);
                // Continue running but log the error
            }
        }

        // Present the frame / pump events
        next_frame().await;

        // When nothing needs redraw, yield extra time to the OS to save power.
        if !app.needs_redraw {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Manual FPS cap on desktop to avoid 100% CPU when vsync is off
        if cfg!(target_arch = "x86_64") || std::env::var("CARDBRICK_FORCE_FPS_CAP").is_ok() {
            let elapsed = frame_start.elapsed().as_secs_f64();
            if elapsed < target_dt {
                std::thread::sleep(std::time::Duration::from_secs_f64(target_dt - elapsed));
            }
        }
    }
}
