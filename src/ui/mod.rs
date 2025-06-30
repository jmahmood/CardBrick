// src/ui/mod.rs
// This module contains all components related to the User Interface.

pub mod canvas; // For upscaling our view
pub mod font;   // For text 
pub mod sprite; // For cute sprites (not yet implemented)

pub use self::canvas::CanvasManager;
pub use self::font::FontManager;
pub use self::sprite::Sprite;
