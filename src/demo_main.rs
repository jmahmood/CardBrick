// CardBrick - Macroquad Version with Device Compatibility

use macroquad::prelude::*;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source};
use std::io::Cursor;
use log::{info, warn, error, debug};
use evdev::Device as EvdevDevice;
use std::os::fd::AsRawFd;

mod assets {
    // Use a font that supports Japanese characters
    pub const JAPANESE_FONT: &[u8] = include_bytes!("../assets/font/NotoSansCJK-Regular.ttc");
    // pub const JAPANESE_FONT: &[u8] = include_bytes!("../assets/font/Ac437_Tandy1K-II_200L.ttf");
    pub const CLICK_SOUND: &[u8] = include_bytes!("../assets/sfx/click.wav");
    pub const OPEN_SOUND: &[u8] = include_bytes!("../assets/sfx/open.wav");
}

struct AudioManager {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl AudioManager {
    fn new() -> Result<Self, String> {
        info!("Initializing audio system...");
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| {
                error!("Failed to create audio output: {}", e);
                format!("Failed to create audio output: {}", e)
            })?;
        info!("Audio system initialized successfully");
        Ok(Self { _stream: stream, stream_handle })
    }

    fn play_sound(&self, sound_data: &'static [u8]) -> Result<(), String> {
        debug!("Playing sound (size: {} bytes)", sound_data.len());
        let cursor = Cursor::new(sound_data);
        let source = Decoder::new(cursor)
            .map_err(|e| {
                warn!("Failed to decode audio: {}", e);
                format!("Failed to decode audio: {}", e)
            })?;
        self.stream_handle.play_raw(source.convert_samples())
            .map_err(|e| {
                warn!("Failed to play sound: {}", e);
                format!("Failed to play sound: {}", e)
            })?;
        debug!("Sound played successfully");
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum DeviceType { Desktop, TrimUIBrick, RG35XXPlus }

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

#[derive(Clone)]
struct TestCard {
    front: String,
    back: String,
}

impl TestCard {
    fn sample_cards() -> Vec<Self> {
        vec![
            TestCard {
                front: "Hello / こんにちは 👋".to_string(),
                back: "A greeting in English and Japanese".to_string(),
            },
            TestCard {
                front: "ありがとう".to_string(),
                back: "Thank you".to_string(),
            },
            TestCard {
                front: "日本語".to_string(),
                back: "Japanese language".to_string(),
            },
            TestCard {
                front: "さようなら".to_string(),
                back: "Goodbye".to_string(),
            },
            TestCard {
                front: "Test 🚀".to_string(),
                back: "Testing English rendering.".to_string(),
            },
        ]
    }
}

struct CardBrickApp {
    audio: AudioManager,
    test_cards: Vec<TestCard>,
    current_card: usize,
    show_back: bool,
    font: Option<macroquad::text::Font>,
    last_input_time: f64,
    needs_redraw: bool,
    gamepad: Option<EvdevDevice>,
}

impl CardBrickApp {
    fn new() -> Self {
        info!("Initializing CardBrick application...");
        
        let audio = AudioManager::new().unwrap_or_else(|e| {
            error!("Audio init failed: {}", e);
            // Create a dummy audio manager that does nothing
            warn!("Creating dummy audio manager");
            AudioManager::new().unwrap()
        });

        // Try loading Japanese font with better error handling
        info!("Loading Japanese font from embedded bytes (size: {} bytes)", assets::JAPANESE_FONT.len());
        let font = match load_ttf_font_from_bytes(assets::JAPANESE_FONT) {
            Ok(f) => {
                info!("Japanese font loaded successfully");
                Some(f)
            }
            Err(e) => {
                error!("Failed to load Japanese font: {:?}", e);
                warn!("Falling back to default font");
                None
            }
        };

        let test_cards = TestCard::sample_cards();
        info!("Created {} test cards", test_cards.len());

        // Initialize gamepad
        let gamepad = Self::find_gamepad();

        info!("CardBrick application initialized successfully");
        Self {
            audio,
            test_cards,
            current_card: 0,
            show_back: false,
            font,
            last_input_time: 0.0,
            needs_redraw: true,
            gamepad,
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
                    
                    // Set non-blocking mode exactly like working example
                    let raw_fd = dev.as_raw_fd();
                    match nix::fcntl::fcntl(raw_fd, nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK)) {
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

    fn update(&mut self) {
        use macroquad::prelude::get_time;
        
        let current_time = get_time();
        let mut input_handled = false;

        // Handle gamepad input
        if let Some(gamepad) = &mut self.gamepad {
            match gamepad.fetch_events() {
                Ok(events) => {
                    for event in events {
                        use evdev::EventType;
                        
                        if event.event_type() == EventType::KEY && event.value() == 1 {
                            match event.code() {
                                // A button (BTN_EAST in evdev)
                                305 => {
                                    info!("Gamepad A button pressed, toggling card side");
                                    self.show_back = !self.show_back;
                                    let _ = self.audio.play_sound(assets::CLICK_SOUND);
                                    input_handled = true;
                                }
                                // B button (BTN_SOUTH)
                                304 => {
                                    info!("Gamepad B button pressed, moving to next card");
                                    self.current_card = (self.current_card + 1) % self.test_cards.len();
                                    self.show_back = false;
                                    let _ = self.audio.play_sound(assets::OPEN_SOUND);
                                    input_handled = true;
                                }
                                // Y button (BTN_WEST)
                                308 => {
                                    info!("Gamepad Y button pressed, moving to previous card");
                                    self.current_card = if self.current_card == 0 {
                                        self.test_cards.len() - 1
                                    } else {
                                        self.current_card - 1
                                    };
                                    self.show_back = false;
                                    let _ = self.audio.play_sound(assets::OPEN_SOUND);
                                    input_handled = true;
                                }
                                // D-pad (actual codes from your device)
                                544 => { // Up
                                    info!("Gamepad D-pad Up pressed");
                                }
                                545 => { // Down
                                    info!("Gamepad D-pad Down pressed");
                                }
                                546 => { // Left
                                    info!("Gamepad D-pad Left pressed, moving to previous card");
                                    self.current_card = if self.current_card == 0 {
                                        self.test_cards.len() - 1
                                    } else {
                                        self.current_card - 1
                                    };
                                    self.show_back = false;
                                    let _ = self.audio.play_sound(assets::OPEN_SOUND);
                                    input_handled = true;
                                }
                                547 => { // Right
                                    info!("Gamepad D-pad Right pressed, moving to next card");
                                    self.current_card = (self.current_card + 1) % self.test_cards.len();
                                    self.show_back = false;
                                    let _ = self.audio.play_sound(assets::OPEN_SOUND);
                                    input_handled = true;
                                }
                                // Menu button - quit application
                                316 => {
                                    info!("Menu button pressed, exiting application");
                                    std::process::exit(0);
                                }
                                // Shoulder buttons (for reference)
                                310 => { // L1
                                    info!("L1 button pressed");
                                }
                                311 => { // R1
                                    info!("R1 button pressed");
                                }
                                312 => { // L2
                                    info!("L2 button pressed");
                                }
                                313 => { // R2
                                    info!("R2 button pressed");
                                }
                                // Additional buttons for future reference
                                314 => { // Select
                                    info!("Select button pressed");
                                }
                                315 => { // Start
                                    info!("Start button pressed");
                                }
                                _ => {
                                    info!("Unknown gamepad button {} pressed", event.code());
                                }
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No events available, which is normal
                }
                Err(e) => {
                    warn!("Error reading gamepad events: {}", e);
                }
            }
        }

        // Handle keyboard input (for testing)
        if is_key_pressed(KeyCode::Escape) {
            info!("Escape key pressed, exiting application");
            std::process::exit(0);
        }

        if is_key_pressed(KeyCode::Space) {
            info!("Space key pressed, toggling card side");
            self.show_back = !self.show_back;
            let _ = self.audio.play_sound(assets::CLICK_SOUND);
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Enter) {
            info!("Right/Enter key pressed, moving to next card");
            self.current_card = (self.current_card + 1) % self.test_cards.len();
            self.show_back = false;
            let _ = self.audio.play_sound(assets::OPEN_SOUND);
            input_handled = true;
        }

        if is_key_pressed(KeyCode::Left) {
            info!("Left key pressed, moving to previous card");
            self.current_card = if self.current_card == 0 {
                self.test_cards.len() - 1
            } else {
                self.current_card - 1
            };
            self.show_back = false;
            let _ = self.audio.play_sound(assets::OPEN_SOUND);
            input_handled = true;
        }

        if input_handled {
            self.last_input_time = current_time;
            self.needs_redraw = true;
            debug!("Input handled, marking for redraw");
        }
    }

    fn draw(&mut self) {
        // Always draw every frame like simple_main - no conditional logic
        debug!("Starting draw cycle");
        
        clear_background(Color::from_rgba(26, 38, 51, 255));

        // Draw main card interface
        let screen_width = screen_width();
        let screen_height = screen_height();
        
        debug!("Screen dimensions: {}x{}", screen_width, screen_height);

        // Background card
        draw_rectangle(
            40.0,
            40.0,
            screen_width - 80.0,
            screen_height - 80.0,
            Color::from_rgba(77, 89, 102, 255),
        );
        
        debug!("Background rectangle drawn");

        // Get current card content
        let text_to_display = if self.show_back {
            &self.test_cards[self.current_card].back
        } else {
            &self.test_cards[self.current_card].front
        };

        let counter = format!("Card {}/{}", self.current_card + 1, self.test_cards.len());
        let instruction = if self.show_back {
            "Press A button to show question"
        } else {
            "Press A button to show answer"
        };

        debug!("Drawing card {}/{}, showing {}", 
               self.current_card + 1, 
               self.test_cards.len(), 
               if self.show_back { "back" } else { "front" });
        debug!("Text to display: '{}'", text_to_display);

        // Draw text content
        let text_color = WHITE;
        let _instruction_color = Color::from_rgba(153, 166, 179, 255);

        // Use Japanese font if available
        if let Some(font) = &self.font {
            debug!("Using Japanese font");
            draw_text_ex(&counter, 60.0, 100.0, TextParams {
                font: Some(font),
                font_size: 24,
                color: text_color,
                ..Default::default()
            });
            draw_text_ex(text_to_display, 60.0, 150.0, TextParams {
                font: Some(font),
                font_size: 48,
                color: text_color,
                ..Default::default()
            });
            draw_text_ex(instruction, 60.0, screen_height - 80.0, TextParams {
                font: Some(font),
                font_size: 20,
                color: LIGHTGRAY,
                ..Default::default()
            });
            draw_text_ex("A=Toggle, B=Next, Y=Previous, Left/Right=Navigate, Menu=Quit", 60.0, screen_height - 50.0, TextParams {
                font: Some(font),
                font_size: 18,
                color: LIGHTGRAY,
                ..Default::default()
            });
        } else {
            debug!("Using default font");
            draw_text(&counter, 60.0, 100.0, 24.0, WHITE);
            draw_text(text_to_display, 60.0, 150.0, 48.0, WHITE);
            draw_text(instruction, 60.0, screen_height - 80.0, 20.0, LIGHTGRAY);
            draw_text("A=Toggle, B=Next, Y=Previous, Left/Right=Navigate, Menu=Quit", 60.0, screen_height - 50.0, 18.0, LIGHTGRAY);
        }
        
        debug!("Draw cycle completed");
    }
}

fn window_conf() -> Conf {
    let config = DeviceConfig::detect_device();
    info!("Window config: {}x{}, fullscreen: {}", 
          config.display_width, config.display_height, config.fullscreen);
    
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
        sample_count: 1,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Initialize logging
    env_logger::init();
    const TARGET_FPS: f64 = 60.0;
    const TARGET_DT:  f64 = 1.0 / TARGET_FPS;

    info!("Starting CardBrick application");
        
    let mut app = CardBrickApp::new();
    info!("Application initialized, entering main loop");

    let mut frame_count = 0;
    loop {
        frame_count += 1;
        // if frame_count % 60 == 0 {
        //     info!("Main loop frame {}", frame_count);
        // }
        
        if frame_count % 60 == 0 {
            info!("FPS {}", macroquad::time::get_fps());
        }


        let frame_start = macroquad::time::get_time();
        
        // Add error handling around update and draw
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.update();
            app.draw();
        })) {
            Ok(()) => {
                // Success
            }
            Err(e) => {
                error!("Panic in main loop: {:?}", e);
                // Continue running but log the error
            }
        }
        
        // Limit to 60 FPS
        next_frame().await;

        if cfg!(target_arch = "x86_64") {
            let dt = macroquad::time::get_time() - frame_start;
            if dt < TARGET_DT {
                std::thread::sleep(
                    std::time::Duration::from_secs_f64(TARGET_DT - dt)
                );
            }
        }
    }
}