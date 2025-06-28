// src/ui/sprite.rs
// Manages the animated "mother" sprite.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;
use std::time::Instant;

// Represents the different emotional states of the sprite.
pub enum SpriteState {
    Idle,
    // We'll add more later, like Correct, Incorrect, etc.
}

pub struct Sprite {
    state: SpriteState,
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

    /// Updates the sprite's animation state. Should be called once per frame.
    pub fn update(&mut self) {
        // For idle, we'll make the sprite blink every so often.
        if self.last_frame_time.elapsed().as_millis() > 500 {
            self.is_blinking = !self.is_blinking;
            self.last_frame_time = Instant::now();
        }
    }

    /// Draws the sprite to the canvas.
    pub fn draw(&self, canvas: &mut Canvas<Window>) -> Result<(), String> {
        // Placeholder sprite drawing logic.
        // **FIXED**: Positioned the sprite in the bottom right corner, aligned with the control hints.
        let base_rect = Rect::new(470, 330, 32, 32);
        
        // Draw body
        canvas.set_draw_color(Color::RGB(200, 200, 255));
        canvas.fill_rect(base_rect)?;

        // Draw eyes
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        if !self.is_blinking {
            let eye1 = Rect::new(476, 340, 5, 5); // Adjusted y-coordinate
            let eye2 = Rect::new(488, 340, 5, 5); // Adjusted y-coordinate
            canvas.fill_rects(&[eye1, eye2])?;
        }

        Ok(())
    }
}
