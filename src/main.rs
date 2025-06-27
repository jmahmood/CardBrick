// CardBrick - main.rs
// Phase 2: UI Canvas and Font Rendering

use std::env; // Import the 'env' module
use std::path::Path;
use std::time::Duration;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
// Removed the problematic HintPriority import

// This tells Rust that we have a module named `deck` and one named `ui`.
mod deck;
mod ui;

// We can use the `use` keyword to bring our new managers into scope.
use ui::{CanvasManager, FontManager};

pub fn main() -> Result<(), String> {
    // --- Get file path from command-line arguments ---
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let error_msg = format!("Usage: {} <path/to/deck.apkg>", args.get(0).unwrap_or(&"cardbrick".to_string()));
        eprintln!("{}", error_msg);
        return Err(error_msg);
    }
    let deck_path = Path::new(&args[1]);

    // --- Boilerplate SDL2 Initialization ---
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    
    // **FIXED**: Set the render scale quality hint using the simpler `set` function.
    // This tells SDL to use a high-quality scaling algorithm (linear filtering).
    // "0" is nearest pixel, "1" is linear, "2" is anisotropic.
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");

    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    let window = video_subsystem
        .window("CardBrick v0.1", 1024, 768) // Full window size
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let sdl_canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = sdl_canvas.texture_creator();

    // --- Our Custom Managers ---
    let mut canvas_manager = CanvasManager::new(sdl_canvas, &texture_creator)?;
    
    // NOTE: Changed to a font that supports CJK characters.
    // On Ubuntu, you can install this with: `sudo apt install fonts-noto-cjk`
    // The exact path might vary slightly.
    let font_manager = FontManager::new(&ttf_context, "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 28)?;

    // --- Load the Deck from the provided path ---
    println!("Attempting to load deck from: {:?}", deck_path);
    let deck = deck::loader::load_apkg(deck_path)
        .map_err(|e| format!("Failed to load deck: {}", e))?;
    
    // Get the first card to display.
    let first_card = deck.cards.get(0).ok_or("Deck has no cards!")?;
    let first_note = deck.notes.get(&first_card.note_id).ok_or("Card has no note!")?;
    let front_text = first_note.fields.get(0).map(String::as_str).unwrap_or("");
    let back_text = first_note.fields.get(1).map(String::as_str).unwrap_or("");

    let mut event_pump = sdl_context.event_pump()?;

    // --- Main Application Loop ---
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                _ => {}
            }
        }

        // Start a new frame on our logical canvas.
        canvas_manager.start_frame()?;

        // Use our managers to draw things.
        canvas_manager.with_canvas(|canvas| {
            // Draw the front text
            font_manager.draw_text(canvas, "Front:", 50, 50)?;
            font_manager.draw_text(canvas, front_text, 50, 90)?;
            
            // Draw the back text
            font_manager.draw_text(canvas, "Back:", 50, 200)?;
            font_manager.draw_text(canvas, back_text, 50, 240)?;

            Ok(())
        })?;

        // Render the completed logical canvas to the screen (with scaling).
        canvas_manager.end_frame();

        // Sleep to cap the frame rate.
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
