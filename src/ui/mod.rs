// src/ui/mod.rs
// This module contains all components related to the User Interface.

// We declare the canvas, font, and sprite modules.
pub mod canvas;
pub mod font;
pub mod sprite; // Make the sprite module public

// We can re-export the main structs here for easier access from other parts of the app.
pub use self::canvas::CanvasManager;
pub use self::font::FontManager;
pub use self::sprite::Sprite; // Re-export the Sprite struct
