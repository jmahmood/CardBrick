// src/state.rs

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::config::Config;
use crate::deck::Deck;
// TODO: Re-enable when scenes are fixed
// use crate::scenes::deck_selection::DeckSelectionState;
use crate::scenes::main_menu::MainMenuState;
// use crate::scenes::studying::StudyingState;
use crate::ui::font::TextLayout;
use crate::ui::{CanvasManager, FontManager, sprite::Sprite};
use rodio::{OutputStream, OutputStreamHandle, Decoder, Source};
use evdev::Device as EvdevDevice;
use std::io::Cursor;


/// Holds metadata about a single deck, used for selection screens.
#[derive(Clone)]
pub struct DeckMetadata {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// Messages sent from the deck loading thread to the main thread.
pub enum LoaderMessage {
    Progress(f32),
    Complete(Result<Deck, String>),
}

/// Represents the current screen or state of the application.
pub enum GameState {
    MainMenu(MainMenuState),
    GoToDeckSelection,
    // TODO: Re-enable when scenes are fixed
    // DeckSelection(DeckSelectionState),
    DeckSelection(String), // Temporary placeholder
    Loading {
        rx: Receiver<LoaderMessage>,
        loading_layout: TextLayout,
        progress: f32,
        deck_id_to_load: String,
    },
    // Studying(StudyingState<'a>),
    Studying(String), // Temporary placeholder
    Error(String),
}

pub struct AudioManager {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to create audio output: {}", e))?;
        Ok(Self { _stream: stream, stream_handle })
    }

    pub fn play_sound(&self, sound_data: &'static [u8]) -> Result<(), String> {
        let cursor = Cursor::new(sound_data);
        let source = Decoder::new(cursor)
            .map_err(|e| format!("Failed to decode audio: {}", e))?;
        self.stream_handle.play_raw(source.convert_samples())
            .map_err(|e| format!("Failed to play sound: {}", e))?;
        Ok(())
    }
}

/// The top-level state for the entire application.
pub struct AppState {
    pub game_state: GameState,
    pub available_decks: Vec<DeckMetadata>,
    pub canvas_manager: CanvasManager,
    pub font_manager: FontManager,
    pub small_font_manager: FontManager,
    pub hint_font_manager: FontManager,
    pub sprite: Sprite,
    pub config: Config,
    pub gamepad: Option<EvdevDevice>,
    pub audio: AudioManager,
    pub background_font_receiver: Option<std::sync::mpsc::Receiver<crate::BackgroundFontMessage>>,
    pub japanese_font_ready: bool,
}

/// All the *buttons* as they’re silkscreened (or logically present) on the Brick.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BrickButton {
    A,
    B,
    X,
    Y,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Power,
    VolumeUp,
    VolumeDown,
    LeftShoulder,
    RightShoulder,
    LeftStick,
    RightStick,
    Start,
    Back,
    Guide,
}

/// All the *analog axes* you care about.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BrickAxis {
    TriggerLeft,
    TriggerRight,
}

/// A unified, high‑level event that your app actually handles.
#[derive(Debug, Copy, Clone)]
pub enum BrickInput {
    ButtonDown(BrickButton),
    ButtonUp(BrickButton),
    AxisMotion { axis: BrickAxis, value: f32 },
}

pub fn map_evdev_to_brick_input(event: &evdev::InputEvent) -> Option<BrickInput> {
    use evdev::{EventType, Key};
    
    if event.event_type() == EventType::KEY && event.value() == 1 {
        let button = match event.code() {
            // A button (BTN_EAST in evdev) 
            305 => BrickButton::A,
            // B button (BTN_SOUTH)
            304 => BrickButton::B,
            // Y button (BTN_WEST)
            308 => BrickButton::Y,
            // X button (BTN_NORTH)
            307 => BrickButton::X,
            // D-pad
            544 => BrickButton::DPadUp,
            545 => BrickButton::DPadDown,
            546 => BrickButton::DPadLeft,
            547 => BrickButton::DPadRight,
            // Shoulder buttons
            310 => BrickButton::LeftShoulder,
            311 => BrickButton::RightShoulder,
            312 => BrickButton::LeftStick,  // L2 mapped to LeftStick
            313 => BrickButton::RightStick, // R2 mapped to RightStick
            // Control buttons
            314 => BrickButton::Back,   // Select
            315 => BrickButton::Start,  // Start
            316 => BrickButton::Guide,  // Menu
            _ => return None,
        };
        Some(BrickInput::ButtonDown(button))
    } else if event.event_type() == EventType::KEY && event.value() == 0 {
        let button = match event.code() {
            305 => BrickButton::A,
            304 => BrickButton::B,
            308 => BrickButton::Y,
            307 => BrickButton::X,
            544 => BrickButton::DPadUp,
            545 => BrickButton::DPadDown,
            546 => BrickButton::DPadLeft,
            547 => BrickButton::DPadRight,
            310 => BrickButton::LeftShoulder,
            311 => BrickButton::RightShoulder,
            312 => BrickButton::LeftStick,
            313 => BrickButton::RightStick,
            314 => BrickButton::Back,
            315 => BrickButton::Start,
            316 => BrickButton::Guide,
            _ => return None,
        };
        Some(BrickInput::ButtonUp(button))
    } else {
        None
    }
}

