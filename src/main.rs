// CardBrick - main.rs
// Phase 0: "Hello, Brick!" - Window and basic event loop.

// Import the SDL2 crate
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use std::time::Duration;

pub fn main() -> Result<(), String> {
    // Initialize SDL2
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    // Create the window.
    // The TrimUI Brick has a 1024x768 screen, but our logical canvas is 512x364.
    // For this initial test, we'll just create a window.
    // The final version will have letterboxing.
    let window = video_subsystem
        .window("CardBrick v0.1", 1024, 768)
        .position_centered()
        .opengl() // Use the OpenGL renderer
        .build()
        .map_err(|e| e.to_string())?;

    // Create a canvas to draw on
    let mut canvas = window
        .into_canvas()
        .build()
        .map_err(|e| e.to_string())?;

    // Set the draw color to a dark grey
    canvas.set_draw_color(Color::RGB(40, 40, 40));
    canvas.clear();
    canvas.present();

    // Create an event pump to handle events (keyboard, mouse, etc.)
    let mut event_pump = sdl_context.event_pump()?;

    // Main application loop
    'running: loop {
        canvas.set_draw_color(Color::RGB(40, 40, 40));
        canvas.clear();

        // Process all pending events
        for event in event_pump.poll_iter() {
            match event {
                // Quit event (e.g., closing the window)
                Event::Quit { .. }
                // Keydown event for the Escape key
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => {}
            }
        }

        // --- DRAWING HAPPENS HERE ---

        // Present the canvas to the screen
        canvas.present();

        // Sleep for a short duration to avoid pegging the CPU
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
