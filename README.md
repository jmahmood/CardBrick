# CardBrick

**CardBrick** is a minimalist, performant, and Anki-compatible flashcard application specifically designed for the TrimUI Brick handheld device running MinUI. The primary goal is to provide a "snappy" and distraction-free environment for language learning, with an initial focus on Japanese (JLPT N3-N1).

This project is being developed in Rust (2021 Edition) and uses the SDL2 library for native rendering and input handling.

## Core Features (MVP)

* **Deck Loading:** Reads Anki `.apkg` files containing the "Basic" card type.
* **Spaced Repetition:** Implements a simplified SM-2 scheduling algorithm.
* **Interactive Review Loop:** A clean UI for reviewing cards, rating them (Again, Hard, Good, Easy), and seeing session progress.
* **Rich Text & Furigana:** Supports basic HTML formatting (`<b>`, `<i>`, `<p>`, etc.) and includes a first-class implementation for toggling Japanese Furigana readings.
* **Performance First:** The architecture prioritizes speed and low memory usage, using a native rendering pipeline and avoiding bloated web technologies. The UI is rendered to a 512x364 logical canvas and pixel-doubled for a sharp, retro look.

## Getting Started

### Prerequisites

* Rust toolchain (`rustup`)
* SDL2 development libraries (`libsdl2-dev`, `libsdl2-image-dev`, `libsdl2-ttf-dev`)
* A C/C++ toolchain (`build-essential` or similar)

### Building and Running

1.  **Clone the repository:**
    ```bash
    git clone <your-repo-url>
    cd CardBrick
    ```

2.  **Build for your local machine (x86_64):**
    ```bash
    cargo build --release
    ```

3.  **Run locally:**
    ```bash
    cargo run -- /path/to/your/deck.apkg
    ```

4.  **Cross-compile for the TrimUI Brick (ARMv7):**
    *Ensure you have the ARM cross-compilation toolchain and libraries installed as configured in `.github/workflows/ci.yml`.*
    ```bash
    cargo build --release --target=armv7-unknown-linux-gnueabihf
    ```

## Project Structure Overview

* `src/main.rs`: The main application entry point, event loop, and state management.
* `src/deck/`: Handles loading and parsing of Anki `.apkg` files.
    * `loader.rs`: Extracts the SQLite database from the zip archive.
    * `html_parser.rs`: A simple, robust HTML processor to clean card content and handle special tags like `<ruby>`.
* `src/scheduler.rs`: Contains the core spaced repetition logic (SM-2) and unit tests.
* `src/ui/`: Contains all rendering and UI management code.
    * `canvas.rs`: Manages the main window and the logical, scalable canvas.
    * `font.rs`: Handles font loading, text layout, and rendering.
    * `sprite.rs`: Manages the "mother" sprite's state and animation.
* `tests/`: Integration tests (to be added).
* `.github/workflows/ci.yml`: The continuous integration pipeline for automated builds and tests.
