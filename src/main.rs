// CardBrick - main.rs
// Phase 1: Test the .apkg loader.

use std::env;
use std::path::Path;

// This tells Rust that we have a module named `deck`
mod deck;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Collect command-line arguments.
    let args: Vec<String> = env::args().collect();

    // Check if a file path was provided.
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_apkg_file>", args[0]);
        // Return a simple error.
        return Err("No file path provided.".into());
    }

    // The first argument (args[0]) is the program name, the second (args[1]) is our path.
    let file_path = Path::new(&args[1]);

    // Call our new loader function.
    let deck = deck::loader::load_apkg(file_path)?;

    println!("\n--- Deck Loaded Successfully! ---");
    println!("Total cards: {}", deck.cards.len());
    println!("Total notes: {}", deck.notes.len());

    // Let's inspect the first 5 cards to verify.
    println!("\n--- First 5 Cards ---");
    for card in deck.cards.iter().take(5) {
        // Find the note associated with this card.
        if let Some(note) = deck.notes.get(&card.note_id) {
            let front = note.fields.get(0).map(String::as_str).unwrap_or(" (no front)");
            let back = note.fields.get(1).map(String::as_str).unwrap_or(" (no back)");
            println!("Card ID: {}, Front: '{}', Back: '{}'", card.id, front, back);
        }
    }
    
    // Return Ok with the unit type `()` to indicate success.
    Ok(())
}
