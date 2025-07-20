// src/ui/canvas.rs
// Manages the main rendering canvas and logical scaling for macroquad.

use macroquad::prelude::*;

const LOGICAL_WIDTH: u32 = 512;
const LOGICAL_HEIGHT: u32 = 384;

pub struct CanvasManager {
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
    offset_x: f32,
    offset_y: f32,
}

impl CanvasManager {
    pub fn new() -> Result<Self, String> {
        let logical_width = LOGICAL_WIDTH as f32;
        let logical_height = LOGICAL_HEIGHT as f32;
        
        // Calculate scaling and centering
        let screen_w = screen_width();
        let screen_h = screen_height();
        
        let scale_x = screen_w / logical_width;
        let scale_y = screen_h / logical_height;
        let scale_factor = scale_x.min(scale_y);
        
        let offset_x = (screen_w - logical_width * scale_factor) / 2.0;
        let offset_y = (screen_h - logical_height * scale_factor) / 2.0;

        Ok(CanvasManager {
            logical_width,
            logical_height,
            scale_factor,
            offset_x,
            offset_y,
        })
    }

    /// Prepares for a new frame by clearing the background.
    pub fn start_frame(&mut self) -> Result<(), String> {
        // Clear the entire screen with black for letterboxing
        clear_background(Color::from_rgba(0, 0, 0, 255));
        Ok(())
    }

    /// Ends the frame (no-op for macroquad as presentation is handled automatically).
    pub fn end_frame(&mut self) {
        // In macroquad, frame presentation is handled automatically
    }
    
    /// Transform logical coordinates to screen coordinates.
    pub fn logical_to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.offset_x + x * self.scale_factor,
            self.offset_y + y * self.scale_factor
        )
    }
    
    /// Get the scale factor for drawing operations.
    pub fn get_scale_factor(&self) -> f32 {
        self.scale_factor
    }
    
    /// Get logical dimensions.
    pub fn logical_size(&self) -> (f32, f32) {
        (self.logical_width, self.logical_height)
    }
    
    /// Set a clipping rectangle in logical coordinates.
    pub fn set_clip_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        let (screen_x, screen_y) = self.logical_to_screen(x as f32, y as f32);
        let screen_w = w as f32 * self.scale_factor;
        let screen_h = h as f32 * self.scale_factor;
        
        // Note: macroquad doesn't have built-in clipping, we'll need to handle this in drawing calls
    }
    
    /// Clear the clipping rectangle.
    pub fn clear_clip_rect(&self) {
        // Note: macroquad doesn't have built-in clipping
    }
}
