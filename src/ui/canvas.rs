// src/ui/canvas.rs
// Manages the main rendering canvas and the logical, scalable texture.

use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use sdl2::pixels::Color;
use sdl2::rect::Rect; // Removed unused `Point`

const LOGICAL_WIDTH: u32 = 512;
const LOGICAL_HEIGHT: u32 = 364;

pub struct CanvasManager<'a> {
    // The main SDL canvas that draws to the window.
    sdl_canvas: Canvas<Window>,
    // The texture creator, needed to create new textures.
    texture_creator: &'a TextureCreator<WindowContext>,
    // Our logical canvas, a texture we render to instead of the main window.
    logical_canvas: Texture<'a>,
}

impl<'a> CanvasManager<'a> {
    pub fn new(mut sdl_canvas: Canvas<Window>, texture_creator: &'a TextureCreator<WindowContext>) -> Result<Self, String> {
        // Create the texture that will act as our logical screen.
        // It's a "target" texture, which means we can draw onto it.
        let logical_canvas = texture_creator
            .create_texture_target(None, LOGICAL_WIDTH, LOGICAL_HEIGHT)
            .map_err(|e| e.to_string())?;

        // Set the blend mode for the main canvas to allow for transparency.
        sdl_canvas.set_blend_mode(sdl2::render::BlendMode::Blend);

        Ok(CanvasManager {
            sdl_canvas,
            texture_creator,
            logical_canvas,
        })
    }

    /// Prepares for a new frame by setting the render target to our logical canvas
    /// and clearing it with a background color.
    pub fn start_frame(&mut self) -> Result<(), String> {
        self.sdl_canvas.with_texture_canvas(&mut self.logical_canvas, |texture_canvas| {
            texture_canvas.set_draw_color(Color::RGB(40, 40, 45));
            texture_canvas.clear();
        }).map_err(|e| e.to_string()) // Map the SDL error to a String error
    }

    /// This is where the magic happens. We take what was drawn on the logical canvas
    /// and render it, scaled up, to the main window.
    pub fn end_frame(&mut self) {
        // Clear the main window (this will create the letterbox effect).
        self.sdl_canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.sdl_canvas.clear();

        // Calculate the destination rectangle for our scaled-up canvas.
        // This will be 1024x728, centered in the window.
        let (window_w, window_h) = self.sdl_canvas.window().size();
        let scale_factor = 2.0;
        let dest_w = (LOGICAL_WIDTH as f32 * scale_factor) as u32;
        let dest_h = (LOGICAL_HEIGHT as f32 * scale_factor) as u32;
        let dest_rect = Rect::new(
            ((window_w - dest_w) / 2) as i32,
            ((window_h - dest_h) / 2) as i32,
            dest_w,
            dest_h
        );

        // Copy the logical canvas texture to the main canvas.
        self.sdl_canvas.copy(&self.logical_canvas, None, Some(dest_rect)).unwrap();
        
        // Present the final rendered image to the screen.
        self.sdl_canvas.present();
    }
    
    // A helper to allow other modules to draw on our logical canvas.
    // This now correctly handles closures that can return their own errors.
    pub fn with_canvas<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut Canvas<Window>) -> Result<(), String>,
    {
        // This is a bit complex, but it allows the calling code (in main.rs) to use the `?` operator
        // inside the closure, which is much cleaner.
        let mut closure_result: Result<(), String> = Ok(());

        let render_result = self.sdl_canvas.with_texture_canvas(&mut self.logical_canvas, |texture_canvas| {
            closure_result = f(texture_canvas);
        });

        // First, check for errors from the closure itself.
        closure_result?;
        // Then, check for errors from the SDL rendering process.
        render_result.map_err(|e| e.to_string())?;

        Ok(())
    }
}
