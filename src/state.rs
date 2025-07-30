// src/state.rs

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::config::{Config, UiAssets};
use crate::deck::Deck;
use crate::scenes::deck_selection::DeckSelectionState;
use crate::scenes::main_menu::MainMenuState;
use crate::scenes::studying::StudyingState;
use crate::scenes::progress::ProgressState;
use crate::scenes::restore_wizard::RestoreWizardState;
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
    DeckSelection(DeckSelectionState),
    Loading {
        rx: Receiver<LoaderMessage>,
        loading_layout: TextLayout,
        progress: f32,
        deck_id_to_load: String,
    },
    Studying(StudyingState<'static>),
    Progress(ProgressState),
    RestoreWizard(RestoreWizardState),
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
    pub ui_assets: Option<UiAssets>,
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
    #[allow(dead_code)]
    Power,
    #[allow(dead_code)]
    VolumeUp,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    TriggerLeft,
    #[allow(dead_code)]
    TriggerRight,
}

/// A unified, high‑level event that your app actually handles.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BrickInput {
    ButtonDown(BrickButton),
    ButtonUp(BrickButton),
    #[allow(dead_code)]
    AxisMotion { axis: BrickAxis, value: f32 },
}

pub fn map_evdev_to_brick_input(event: &evdev::InputEvent) -> Option<BrickInput> {
    use evdev::{EventType};
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::{InputEvent, EventType};

    #[test]
    fn test_button_down_mapping() {
        // Test A button (code 305)
        let event = InputEvent::new(EventType::KEY, 305, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::A)));

        // Test B button (code 304)
        let event = InputEvent::new(EventType::KEY, 304, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::B)));

        // Test Y button (code 308)
        let event = InputEvent::new(EventType::KEY, 308, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::Y)));

        // Test X button (code 307)
        let event = InputEvent::new(EventType::KEY, 307, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::X)));
    }

    #[test]
    fn test_dpad_mapping() {
        // Test D-pad Up (code 544)
        let event = InputEvent::new(EventType::KEY, 544, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::DPadUp)));

        // Test D-pad Down (code 545)
        let event = InputEvent::new(EventType::KEY, 545, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::DPadDown)));

        // Test D-pad Left (code 546)
        let event = InputEvent::new(EventType::KEY, 546, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::DPadLeft)));

        // Test D-pad Right (code 547)
        let event = InputEvent::new(EventType::KEY, 547, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::DPadRight)));
    }

    #[test]
    fn test_shoulder_buttons_mapping() {
        // Test Left Shoulder (code 310)
        let event = InputEvent::new(EventType::KEY, 310, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::LeftShoulder)));

        // Test Right Shoulder (code 311)
        let event = InputEvent::new(EventType::KEY, 311, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::RightShoulder)));

        // Test Left Stick mapped from L2 (code 312)
        let event = InputEvent::new(EventType::KEY, 312, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::LeftStick)));

        // Test Right Stick mapped from R2 (code 313)
        let event = InputEvent::new(EventType::KEY, 313, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::RightStick)));
    }

    #[test]
    fn test_control_buttons_mapping() {
        // Test Back button (Select, code 314)
        let event = InputEvent::new(EventType::KEY, 314, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::Back)));

        // Test Start button (code 315)
        let event = InputEvent::new(EventType::KEY, 315, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::Start)));

        // Test Guide button (Menu, code 316)
        let event = InputEvent::new(EventType::KEY, 316, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonDown(BrickButton::Guide)));
    }

    #[test]
    fn test_button_up_mapping() {
        // Test A button release (code 305, value 0)
        let event = InputEvent::new(EventType::KEY, 305, 0);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonUp(BrickButton::A)));

        // Test B button release (code 304, value 0)
        let event = InputEvent::new(EventType::KEY, 304, 0);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, Some(BrickInput::ButtonUp(BrickButton::B)));
    }

    #[test]
    fn test_unknown_key_code() {
        // Test unknown key code (999)
        let event = InputEvent::new(EventType::KEY, 999, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, None);
    }

    #[test]
    fn test_non_key_event() {
        // Test non-KEY event type (should return None)
        let event = InputEvent::new(EventType::RELATIVE, 305, 1);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, None);
    }

    #[test]
    fn test_key_repeat_value() {
        // Test key repeat value (value 2) - should return None
        let event = InputEvent::new(EventType::KEY, 305, 2);
        let result = map_evdev_to_brick_input(&event);
        assert_eq!(result, None);
    }
}

