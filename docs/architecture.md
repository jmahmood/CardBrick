# **CardBrick Architecture**

This document outlines the high-level architecture and key design decisions for the CardBrick application.

## **Core Philosophy**

1. **Native & Performant:** The application is built as a pure native Rust binary using SDL2. There are no web views or heavy frameworks. Performance and low resource usage are the highest priorities to ensure a "snappy" experience and long battery life on the target device (TrimUI Brick).  
2. **State-Driven UI:** The application logic is contained within a main run loop that operates on a central AppState struct. All drawing and input handling is a function of the data within this struct.  
3. **Data Flow:** The flow of data is strictly one-way during the review loop: Scheduler \-\> AppState \-\> UI Rendering. This keeps the logic clean and easy to reason about.

## **Key Modules & Responsibilities**

### **main.rs**

* **Entry Point:** Initializes SDL, the window, and all subsystems.  
* **State Management:** Owns the primary AppState struct.  
* **Main Loop (run):** Contains the core game loop that processes events, updates state, and triggers draws.  
* **Event Handling (handle\_keypress):** Translates raw SDL2 keypresses into application-specific actions (e.g., rating a card, toggling furigana).

### **deck Module**

* **loader.rs:** Responsible for the I/O-heavy task of reading the .apkg file. It runs on a background thread to avoid blocking the UI, sending progress updates and the final Deck object back to the main thread via a channel.  
* **html\_parser.rs:** This is **not** a full HTML renderer. It is a lightweight data processor. Its sole purpose is to take the raw HTML string from an Anki card field and convert it into a clean, plain String for rendering. It has special logic to handle \<ruby\> tags for the furigana toggle feature.

### **scheduler Module**

* **The Brain:** This module contains the core spaced repetition logic (SM-2).  
* **Data Ownership:** It owns the "master copy" of all card and note data in HashMaps for fast lookups.  
* **Idempotent & Testable:** The scheduler's logic is self-contained and is validated by a suite of unit tests located at the bottom of the file (\#\[cfg(test)\]).

### **ui Module**

* **canvas.rs (CanvasManager):** Manages the 512x364 logical canvas. All drawing happens on this off-screen texture. The end\_frame method handles scaling this texture up to the window with anti-aliasing (linear filtering).  
* **font.rs (FontManager):** A simplified and robust text engine.  
  * layout\_text: Takes a plain String and calculates line breaks based on character widths, producing a TextLayout object.  
  * draw\_layout: Takes a pre-calculated TextLayout and renders it to the screen. This separation of layout and drawing is a key performance optimization.  
* **sprite.rs (Sprite):** A simple state machine for the "mother" sprite, managing its animation and position.

## **Important Design Decisions**

* **HTML Parsing:** We deliberately avoid a complex rich text engine. Instead, we pre-process HTML into two clean string versions (Kanji and Furigana) upon card load. The UI then simply toggles between rendering these two pre-calculated layouts. This is extremely fast and avoids complex state management during the render loop.  
* **Dynamic Linking:** The application is dynamically linked against system libraries like SDL2. This keeps the executable size small and leverages the shared libraries provided by the MinUI operating system, which is standard practice for this platform.  
* **Threading Model:** The initial deck loading is the only multi-threaded part of the application. This is done to provide an immediate "Loading..." screen to the user. All other operations (reviewing, drawing) are single-threaded for simplicity.
