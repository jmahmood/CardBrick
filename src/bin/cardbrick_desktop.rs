use cardbrick::desktop::ui::CardBrickApp;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "CardBrick Desktop")]
#[command(author = "CardBrick Team")]
#[command(version = "0.1.0")]
#[command(about = "Desktop flashcard study application for KARTA decks", long_about = None)]
struct Args {
    /// Path to the deck JSON file (optional - will show deck browser if not provided)
    #[arg(short, long, value_name = "FILE")]
    deck: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    // Set up egui app
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KARTA CardBrick")
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    // Run the app
    eframe::run_native(
        "CardBrick",
        native_options,
        Box::new(move |_cc| {
            // Create app with optional deck path
            Ok(Box::new(CardBrickApp::new_with_deck_path(args.deck)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}
