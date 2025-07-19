# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

### Local Development
```bash
# Build for local development (x86_64)
cargo build --release

# Run locally with a deck
cargo run -- /path/to/your/deck.apkg

# Run tests
cargo test
```

### Cross-compilation for Handheld Devices
```bash
# Cross-compile for handheld devices (TrimUI Brick, RG35XX Plus - both ARM64)
cargo build --release --target=aarch64-unknown-linux-gnu

# Build for desktop testing (X86-64)
cargo build --release --target=x86_64-unknown-linux-gnu

# Full build and deploy pipeline
./build.sh
```

### Build Scripts
- `build.sh`: Complete build pipeline using Docker for ARM cross-compilation, copies assets, and deploys via rsync/scp to target device
- `launch.sh`: Launch script for the TrimUI Brick device
- `cardbrick.sh`: Entry point script for the handheld device

## Architecture Overview

CardBrick is a native Rust SDL2 application designed for the TrimUI Brick handheld device. The architecture follows these core principles:

### State-Driven Design
- Central `AppState` struct contains all application state
- Scene-based state machine with `GameState` enum:
  - `MainMenu`: Initial application screen
  - `DeckSelection`: Choose from available .apkg files
  - `Loading`: Background deck loading with progress bar
  - `Studying`: Core flashcard review loop
  - `Error`: Error display state

### Key Modules

#### `main.rs`
- Application entry point and main event loop
- Initializes SDL2, audio, controllers, and fonts
- Delegates input handling and rendering to scene-specific modules

#### `state.rs`
- Defines core state structures (`AppState`, `GameState`, `DeckMetadata`)
- Input mapping for TrimUI Brick controller (`BrickButton`, `BrickInput`)
- Sound effect management

#### `deck/` module
- `loader.rs`: Background thread deck loading from .apkg files
- `html_parser.rs`: Lightweight HTML processor for Anki card content, with special ruby/furigana support
- Core data structures: `Card`, `Note`, `Deck`

#### `scheduler.rs`
- SM-2 spaced repetition algorithm implementation
- `Scheduler` trait with `Sm2Scheduler` implementation
- Card review queue management and session tracking

#### `ui/` module
- `canvas.rs`: 512x364 logical canvas with pixel-doubling for crisp rendering
- `font.rs`: Text layout and rendering engine with multi-font fallback support
- `sprite.rs`: Animated sprite system (currently "mother" sprite)

#### `scenes/` module
Scene-specific logic split into input handling, rendering, and game logic:
- `main_menu/`: Application startup screen
- `deck_selection/`: .apkg file browser
- `studying/`: Core flashcard review interface

#### `storage/` module
- `db.rs`: Local database for progress tracking
- `replay_log.rs`: Session replay logging

### Threading Model
- Main thread: UI, input, and rendering
- Background thread: Deck loading only (with progress updates via channels)
- All other operations are single-threaded for simplicity

### Performance Considerations
- HTML parsing is pre-processed into clean strings at load time
- Text layout is separated from rendering for performance
- Furigana toggling uses pre-calculated layouts
- 60 FPS target with fixed timestep

### Target Device Specifics
- TrimUI Brick: 640x480 display, ARM7 processor
- SDL2 dynamic linking against system libraries
- Controller input via Gilrs library
- Audio support for sound effects

## Development Notes

### Font System
The application uses a three-tier font system:
- Large font: Main content display
- Medium font: UI elements and navigation
- Small font: Hints and command help (with emoji fallback support)

### HTML Processing
The HTML parser is intentionally minimal and converts Anki HTML to clean text. Special handling for:
- Ruby tags for Japanese furigana
- Basic formatting (bold, italic, paragraphs)
- No complex CSS or web rendering

### Testing
Unit tests are embedded in modules using `#[cfg(test)]`, particularly in `scheduler.rs` for SM-2 algorithm validation.