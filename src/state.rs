// src/state.rs

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::config::Config;
use crate::deck::Deck;
use crate::scenes::deck_selection::DeckSelectionState;
use crate::scenes::main_menu::MainMenuState;
use crate::scenes::studying::StudyingState;
use crate::ui::font::TextLayout;
use crate::ui::{CanvasManager, FontManager, sprite::Sprite};
use sdl2::controller::{GameController};
use sdl2::controller::Button as CtrlBtn;
use sdl2::keyboard::Keycode;
use sdl2::event::Event;
use sdl2::mixer::{self, Chunk};


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
pub enum GameState<'a> {
    MainMenu(MainMenuState),
    GoToDeckSelection,
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

pub struct Sfx {
    pub up_down_sound: Chunk,
    pub open_sound: Chunk,
    pub mixer_ctx: mixer::Sdl2MixerContext
}

/// The top-level state for the entire application.
pub struct AppState<'a> {
    pub game_state: GameState<'a>,
    pub available_decks: Vec<DeckMetadata>,
    pub canvas_manager: CanvasManager<'a>,
    pub font_manager: FontManager<'a, 'a>,
    pub small_font_manager: FontManager<'a, 'a>,
    pub hint_font_manager: FontManager<'a, 'a>,
    pub sprite: Sprite,
    pub config: Config,
    pub controllers: Vec<GameController>,
    pub sfx: Sfx
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

pub fn map_to_brick_input(ev: &Event) -> Option<BrickInput> {
    match ev {
        // 1) Controller D‑pad & face buttons
        Event::ControllerButtonDown { button, .. } => {
            let b = match button {
                CtrlBtn::B        => BrickButton::A,
                // …but you know it’s really the A button on the Brick.
                CtrlBtn::A        => BrickButton::B,
                CtrlBtn::Y        => BrickButton::X,
                CtrlBtn::X        => BrickButton::Y,
                CtrlBtn::DPadUp   => BrickButton::DPadUp,
                CtrlBtn::DPadDown => BrickButton::DPadDown,
                CtrlBtn::DPadLeft => BrickButton::DPadLeft,
                CtrlBtn::DPadRight=> BrickButton::DPadRight,
                CtrlBtn::Start    => BrickButton::Start,
                CtrlBtn::Back     => BrickButton::Back,
                CtrlBtn::Guide    => BrickButton::Guide,
                CtrlBtn::LeftShoulder    => BrickButton::LeftShoulder,
                CtrlBtn::RightShoulder    => BrickButton::RightShoulder,
                CtrlBtn::RightStick    => BrickButton::RightStick,
                CtrlBtn::LeftStick    => BrickButton::LeftStick,
                _                 => return None,
            };
            Some(BrickInput::ButtonDown(b))
        }
        Event::ControllerButtonUp { button, .. } => {
            let b = match button {
                CtrlBtn::B        => BrickButton::A,
                // …but you know it’s really the A button on the Brick.
                CtrlBtn::A        => BrickButton::B,
                CtrlBtn::Y        => BrickButton::X,
                CtrlBtn::X        => BrickButton::Y,
                CtrlBtn::DPadUp   => BrickButton::DPadUp,
                CtrlBtn::DPadDown => BrickButton::DPadDown,
                CtrlBtn::DPadLeft => BrickButton::DPadLeft,
                CtrlBtn::DPadRight=> BrickButton::DPadRight,
                CtrlBtn::Start    => BrickButton::Start,
                CtrlBtn::Back     => BrickButton::Back,
                CtrlBtn::Guide    => BrickButton::Guide,
                CtrlBtn::LeftShoulder    => BrickButton::LeftShoulder,
                CtrlBtn::RightShoulder    => BrickButton::RightShoulder,
                CtrlBtn::RightStick    => BrickButton::RightStick,
                CtrlBtn::LeftStick    => BrickButton::LeftStick,
                _                 => return None,
            };
            Some(BrickInput::ButtonUp(b))
        }

        // 2) The Power key comes through as a regular KeyDown/Up
        Event::KeyDown { keycode: Some(Keycode::Power), .. } => {
            Some(BrickInput::ButtonDown(BrickButton::Power))
        }
        Event::KeyUp   { keycode: Some(Keycode::Power), .. } => {
            Some(BrickInput::ButtonUp(  BrickButton::Power))
        }

        // 3) The “volume” buttons arrive as joystick buttons
        Event::JoyButtonDown { button_idx: 14, .. } => {
            Some(BrickInput::ButtonDown(BrickButton::VolumeUp))
        }
        Event::JoyButtonUp   { button_idx: 14, .. } => {
            Some(BrickInput::ButtonUp(  BrickButton::VolumeUp))
        }
        Event::JoyButtonDown { button_idx: 13, .. } => {
            Some(BrickInput::ButtonDown(BrickButton::VolumeDown))
        }
        Event::JoyButtonUp   { button_idx: 13, .. } => {
            Some(BrickInput::ButtonUp(  BrickButton::VolumeDown))
        }

        // 4) Triggers as analog axes (SDL reports both ControllerAxisMotion and JoyAxisMotion)
        Event::ControllerAxisMotion { axis, value, .. } => {
            let axis = match axis {
                sdl2::controller::Axis::TriggerLeft  => BrickAxis::TriggerLeft,
                sdl2::controller::Axis::TriggerRight => BrickAxis::TriggerRight,
                _                                    => return None,
            };
            // Normalize [-32768..32767] → [-1.0..1.0]
            let v = *value as f32 / 32767.0;
            Some(BrickInput::AxisMotion { axis, value: v })
        }
        Event::JoyAxisMotion { axis_idx: 2, value, .. } => {
            let v = *value as f32 / 32767.0;
            Some(BrickInput::AxisMotion { axis: BrickAxis::TriggerLeft,  value: v })
        }
        Event::JoyAxisMotion { axis_idx: 5, value, .. } => {
            let v = *value as f32 / 32767.0;
            Some(BrickInput::AxisMotion { axis: BrickAxis::TriggerRight, value: v })
        }

        _ => None,
    }

}

