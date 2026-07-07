// src/ui/sprite.rs
// Manages the animated "mother" sprite.

use crate::ui::CanvasManager;
use macroquad::prelude::*;
use std::time::Instant;

// Represents the different emotional states of the sprite.
pub enum SpriteState {
    Idle,
    // We'll add more later, like Correct, Incorrect, etc.
}

pub struct Sprite {
    #[allow(dead_code)]
    state: SpriteState, // We plan on using this in tehfuture.
    last_frame_time: Instant,
    is_blinking: bool,
}

impl Sprite {
    pub fn new() -> Self {
        Sprite {
            state: SpriteState::Idle,
            last_frame_time: Instant::now(),
            is_blinking: false,
        }
    }
}

impl Default for Sprite {
    fn default() -> Self {
        Self::new()
    }
}

impl Sprite {
    /// Updates the sprite's animation state. Should be called once per frame.
    /// Returns true if the visual state changed this frame (requires redraw).
    pub fn update(&mut self) -> bool {
        // For idle, we'll make the sprite blink every so often.
        if self.last_frame_time.elapsed().as_millis() > 500 {
            self.is_blinking = !self.is_blinking;
            self.last_frame_time = Instant::now();
            return true;
        }
        false
    }

    /// Draws the sprite to the canvas.
    pub fn draw(&self, canvas_manager: &CanvasManager) -> Result<(), String> {
        // Placeholder sprite drawing logic.
        // **FIXED**: Positioned the sprite in the bottom right corner, aligned with the control hints.
        let (sprite_x, sprite_y) = canvas_manager.logical_to_screen(470.0, 330.0);
        let sprite_size = 32.0 * canvas_manager.get_scale_factor();

        // Draw body
        draw_rectangle(
            sprite_x,
            sprite_y,
            sprite_size,
            sprite_size,
            Color::from_rgba(200, 200, 255, 255),
        );

        // Draw eyes
        if !self.is_blinking {
            let (eye1_x, eye1_y) = canvas_manager.logical_to_screen(476.0, 340.0);
            let (eye2_x, eye2_y) = canvas_manager.logical_to_screen(488.0, 340.0);
            let eye_size = 5.0 * canvas_manager.get_scale_factor();

            draw_rectangle(eye1_x, eye1_y, eye_size, eye_size, BLACK);
            draw_rectangle(eye2_x, eye2_y, eye_size, eye_size, BLACK);
        }

        Ok(())
    }
}
