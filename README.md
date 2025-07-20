# CardBrick

**CardBrick** is a minimalist, performant, and Anki-compatible flashcard application specifically designed for small devices that run the ROCKNIX operating system.

It is being tested on the TrimUI Brick running MinUI and on the RG35XX Plus / RG35XX SP (Both running Rocknix).  

The primary goal is to provide a "snappy" and distraction-free environment for language learning, with an initial focus on supporting Japanese learning for my children (JLPT N3-N1).

This project is being developed in Rust (2021 Edition).  We use `evdev` for input handling, `macroquad` for the display and `rodio` for sound.

## Core Features (MVP)

* **Deck Loading:** Reads Anki `.apkg` files containing the "Basic" card type.
* **Spaced Repetition:** Implements a simplified SM-2 scheduling algorithm.
* **Interactive Review Loop:** A clean UI for reviewing cards, rating them (Again, Hard, Good, Easy), and seeing session progress.
* **Rich Text & Furigana:** Supports basic HTML formatting (`<b>`, `<i>`, `<p>`, etc.) and includes a first-class implementation for toggling Japanese Furigana readings.
* **Performance First:** The architecture prioritizes speed and low memory usage, using a native rendering pipeline and avoiding bloated web technologies. The UI is rendered to a 512x364 logical canvas and pixel-doubled for a sharp, retro look.

## Getting Started

### Prerequisites

* Rust toolchain (`rustup`, `cargo`, `cross`)

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
    cargo run
    ```

    You will be expected to have the deck files in the subdirectory underneath the application.

4.  **Cross-compile for the TrimUI Brick / RG35XX Plus (ARM64):**
    ```bash
cross build --release --target aarch64-unknown-linux-gnu
```
    Cross will import all of the annoying dependecies without having to litter your Linux build with a different application.  You will get one binary with everything (other than decks and some metadata) being built into the executable.

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

## Other scripts

- If you need DTC for working your device, you can build it using `toolchain/Dockerfile.dtc`
- If you need scp / rsync for your device, you can build them using `toolchain/Dockerfile.scp_and_rsync`
- I have a script that fixes ruby tags so they are properly read by the script I am using.  You can find it under `scripts/ruby_fixer.py`

## License

Licensed under GPL‑3.0 or later. See LICENSE for details.
