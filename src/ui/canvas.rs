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
        
        Self::new_with_screen_size(screen_w, screen_h)
    }
    
    /// Create a CanvasManager with specific screen dimensions (for testing)
    pub fn new_with_screen_size(screen_w: f32, screen_h: f32) -> Result<Self, String> {
        let logical_width = LOGICAL_WIDTH as f32;
        let logical_height = LOGICAL_HEIGHT as f32;
        
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_perfect_aspect_ratio() {
        // Test with perfect 4:3 aspect ratio matching logical size
        let canvas = CanvasManager::new_with_screen_size(1024.0, 768.0).unwrap();
        
        // Should scale 2x exactly
        assert_eq!(canvas.get_scale_factor(), 2.0);
        
        // Should be perfectly centered
        assert_eq!(canvas.offset_x, 0.0);
        assert_eq!(canvas.offset_y, 0.0);
        
        // Test coordinate transformation
        let (screen_x, screen_y) = canvas.logical_to_screen(0.0, 0.0);
        assert_eq!(screen_x, 0.0);
        assert_eq!(screen_y, 0.0);
        
        let (screen_x, screen_y) = canvas.logical_to_screen(256.0, 192.0);
        assert_eq!(screen_x, 512.0);
        assert_eq!(screen_y, 384.0);
    }

    #[test]
    fn test_canvas_wider_screen() {
        // Test with wider screen (letterboxing on sides)
        let canvas = CanvasManager::new_with_screen_size(1600.0, 768.0).unwrap();
        
        // Scale factor should be limited by height
        let expected_scale = 768.0 / 384.0; // 2.0
        assert_eq!(canvas.get_scale_factor(), expected_scale);
        
        // Should have horizontal offset for centering
        let expected_logical_screen_width = 512.0 * expected_scale;
        let expected_offset_x = (1600.0 - expected_logical_screen_width) / 2.0;
        assert_eq!(canvas.offset_x, expected_offset_x);
        assert_eq!(canvas.offset_y, 0.0);
    }

    #[test]
    fn test_canvas_taller_screen() {
        // Test with taller screen (letterboxing on top/bottom)
        let canvas = CanvasManager::new_with_screen_size(1024.0, 1200.0).unwrap();
        
        // Scale factor should be limited by width
        let expected_scale = 1024.0 / 512.0; // 2.0
        assert_eq!(canvas.get_scale_factor(), expected_scale);
        
        // Should have vertical offset for centering
        let expected_logical_screen_height = 384.0 * expected_scale;
        let expected_offset_y = (1200.0 - expected_logical_screen_height) / 2.0;
        assert_eq!(canvas.offset_x, 0.0);
        assert_eq!(canvas.offset_y, expected_offset_y);
    }

    #[test]
    fn test_canvas_small_screen() {
        // Test with smaller screen
        let canvas = CanvasManager::new_with_screen_size(640.0, 480.0).unwrap();
        
        // Scale factor should be less than 1
        let expected_scale = (640.0_f32 / 512.0).min(480.0 / 384.0); // 1.25
        assert_eq!(canvas.get_scale_factor(), expected_scale);
        
        // Test coordinate transformation maintains proportion
        let (screen_x, screen_y) = canvas.logical_to_screen(512.0, 384.0);
        assert!((screen_x - 640.0).abs() < 0.001);
        assert!((screen_y - 480.0).abs() < 0.001);
    }

    #[test]
    fn test_canvas_coordinate_transformation() {
        let canvas = CanvasManager::new_with_screen_size(1024.0, 768.0).unwrap();
        
        // Test corner coordinates
        let (x, y) = canvas.logical_to_screen(0.0, 0.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        
        let (x, y) = canvas.logical_to_screen(512.0, 384.0);
        assert_eq!(x, 1024.0);
        assert_eq!(y, 768.0);
        
        // Test center coordinate
        let (x, y) = canvas.logical_to_screen(256.0, 192.0);
        assert_eq!(x, 512.0);
        assert_eq!(y, 384.0);
    }

    #[test]
    fn test_canvas_logical_size() {
        let canvas = CanvasManager::new_with_screen_size(1920.0, 1080.0).unwrap();
        
        let (width, height) = canvas.logical_size();
        assert_eq!(width, 512.0);
        assert_eq!(height, 384.0);
    }

    #[test]
    fn test_canvas_coordinate_transformation_with_offset() {
        // Wide screen that requires horizontal offset
        let canvas = CanvasManager::new_with_screen_size(1536.0, 768.0).unwrap();
        
        let scale = canvas.get_scale_factor();
        let expected_offset_x = (1536.0 - 512.0 * scale) / 2.0;
        
        // Test that coordinates include the offset
        let (screen_x, screen_y) = canvas.logical_to_screen(0.0, 0.0);
        assert_eq!(screen_x, expected_offset_x);
        assert_eq!(screen_y, 0.0);
        
        let (screen_x, screen_y) = canvas.logical_to_screen(256.0, 192.0);
        assert_eq!(screen_x, expected_offset_x + 256.0 * scale);
        assert_eq!(screen_y, 192.0 * scale);
    }

    #[test]
    fn test_canvas_extreme_aspect_ratios() {
        // Very wide screen
        let canvas = CanvasManager::new_with_screen_size(2560.0, 600.0).unwrap();
        let scale = canvas.get_scale_factor();
        
        // Scale should be limited by height
        assert_eq!(scale, 600.0 / 384.0);
        assert!(canvas.offset_x > 0.0);
        assert_eq!(canvas.offset_y, 0.0);
        
        // Very tall screen
        let canvas = CanvasManager::new_with_screen_size(400.0, 1200.0).unwrap();
        let scale = canvas.get_scale_factor();
        
        // Scale should be limited by width
        assert_eq!(scale, 400.0 / 512.0);
        assert_eq!(canvas.offset_x, 0.0);
        assert!(canvas.offset_y > 0.0);
    }

    #[test]
    fn test_canvas_minimum_size() {
        // Test with very small screen
        let canvas = CanvasManager::new_with_screen_size(256.0, 192.0).unwrap();
        
        let scale = canvas.get_scale_factor();
        assert!(scale < 1.0);
        assert_eq!(scale, 0.5); // 256/512 = 0.5, 192/384 = 0.5
        
        // Coordinates should still transform correctly
        let (x, y) = canvas.logical_to_screen(512.0, 384.0);
        assert_eq!(x, 256.0);
        assert_eq!(y, 192.0);
    }

    #[test]
    fn test_canvas_scaling_preserves_aspect_ratio() {
        let test_cases = vec![
            (800.0, 600.0),
            (1920.0, 1080.0),
            (1366.0, 768.0),
            (640.0, 480.0),
        ];
        
        for (screen_w, screen_h) in test_cases {
            let canvas = CanvasManager::new_with_screen_size(screen_w, screen_h).unwrap();
            
            // The scaled logical size should fit within the screen
            let scale = canvas.get_scale_factor();
            let scaled_logical_w = 512.0 * scale;
            let scaled_logical_h = 384.0 * scale;
            
            assert!(scaled_logical_w <= screen_w + 0.001); // Allow for floating point precision
            assert!(scaled_logical_h <= screen_h + 0.001);
            
            // At least one dimension should exactly match (the limiting dimension)
            let width_matches = (scaled_logical_w - screen_w).abs() < 0.001;
            let height_matches = (scaled_logical_h - screen_h).abs() < 0.001;
            assert!(width_matches || height_matches);
        }
    }
}
