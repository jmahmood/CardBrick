# CardBrick – TrimUI Brick Flash‑Card App

> **Purpose**  A complete specification and step‑by‑step build guide you can feed to GitHub Copilot / Codex to generate the entire MVP application—from repo scaffolding to final `.opk` package—targeting the TrimUI Brick (MinUI firmware).

---

## 1. High‑Level Overview

| Item             | Spec                                                                                                                         |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| **Device**       | TrimUI Brick (Allwinner A7 @ ≈1 GHz, 512–1024 MB RAM, 3.5″ 1024×768 screen, physical D‑pad + A/B/X/Y + LB/RB + Start/Select) |
| **OS / SDK**     | MinUI (Busybox‑style Linux), SDL2 home‑brew stack                                                                            |
| **App Name**     | **CardBrick** (working title)                                                                                                |
| **License**      | GPL‑3.0‑or‑later; paid pixel font under separate license shipped as binary asset                                             |
| **Primary Goal** | An Anki‑compatible, read‑only flash‑card reviewer optimised for language learning (Japanese JLPT N3‑to‑N1 focus)             |

### Core MVP Features

1. **Deck Loading** – `.apkg` only (Basic card type). Images + limited HTML supported.
2. **SM‑2 Scheduler** – identical to desktop Anki, exposed behind pluggable interface.
3. **Review Loop** – arrow to reveal; **A Good**, **B Again**, **X Easy**, **Y Hard**; rewind last ≤2 cards.
4. **Pixel‑Perfect UI** – logical 512×364 canvas, pixel‑doubled, 8‑bit Unicode font, progress bar + “mother” sprite.
5. **Mic Notes** – record ≤60 s Ogg/Opus per card.
6. **Persistence** – SQLite + replay log; auto‑rebuild on corruption.
7. **Sync** – HTTP server on port 8787, discovered via mDNS `_cardbrick._tcp`.
8. **Power** – instant resume, auto‑save after each rating.

---

## 2. Detailed Functional Requirements

### 2.1 Deck Support

* **Accepted file**: `.apkg` (zip‑container).
* **Card type**: *Basic* only (front/back).
* **Media**: PNG/JPEG images allowed.

  * Down‑scale ≥512×364; max 300 kB RAM.
* **HTML subset** (sanitised):

  ```html
  <p>, <br>, <div>, <ul>/<li>, <img>, inline style="font-size|color|background"
  ```
* **Unsupported content** silently stripped; log warning.

### 2.2 Spaced‑Repetition Algorithm

* SM‑2 with ease‑factor, interval, lapses identical to Anki 2.1.
* Algorithm exposed via `trait Scheduler` for future plug‑ins.

### 2.3 UI / UX

| Element                                              | Details                                                                                                                |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Canvas**                                           | 512 × 364 virtual, scaled ×2 to fullscreen 1024 × 728 (letterboxed top/bottom).                                        |
| **Font**                                             | Paid 8‑bit Unicode pixel font covering full JIS‑X‑0208; fallback atlas generated from Noto Sans CJK subset at install. |
| **Controls**                                         | ◀▶▲▼ reveal answer (any direction).                                                                                    |
| A = Good, B = Again, X = Easy, Y = Hard              |                                                                                                                        |
| Start → Options (menu: rewind ≤2, record note, exit) |                                                                                                                        |
| **Progress**                                         | HP‑style bar + numeric “n / total”; colour shifts red→green.                                                           |
| **Sprite**                                           | Tiny 32×32 “mother” pixel character; idle blink 5 fps; shake/frown on many wrong answers.                              |
| **Image viewer**                                     | When `<img>` tapped (by LB/RB or Select), show full‑screen; LB prev, RB next if multiple.                              |

### 2.4 Audio Notes

* Capture with SDL Audio, 44 kHz mono PCM; save directly as uncompressed WAV (`.wav`) files.
* Stored: `/anki/audio/{deck_id}/{card_id}_{timestamp}.wav`.

### 2.5 Persistence & Recovery

* **SQLite** per deck in `/anki/history/{deck_id}.db`.
* **Replay log** plain‑text (`YYYYMMDD‑HHMMSS‑seq.txt`) in `/anki/history/txn/`.

  * Each action: `card_id,event,ease,old_ivl,new_ivl,ts`.
* On DB open failure: rename to `.corrupt`, rebuild via replay with progress bar.
* Auto‑commit after every rating; beats power loss.

### 2.6 Sync Service

* Embedded HTTP/1.1 file server (crate `tiny_http`).
* Endpoint `/` lists decks, history, audio; GET‑only.
* Service advertises via mDNS (`cardbrick.local`, TXT `ver=0.1`).

### 2.7 Storage & Limits

* Global cap 1 GB for `/anki/` (history + audio).
* Oldest mic files purged first when >1024 MB.

### 2.8 Power & Performance

* Idle loop throttled to 5 fps; screen blanking per MinUI default.
* Resume returns to last question state; undo queue cleared on power‑cycle.

---

## 3. Technology Stack

| Layer                    | Choice              | Crate / Library                             |
| ------------------------ | ------------------- | ------------------------------------------- |
| **Language**             | Rust 2021           | —                                           |
| **Windowing / Input**    | SDL2 bundled        | `sdl2` crate (`features = ["bundled"]`)     |
| **Rendering**            | SDL2 2D API         | `sdl2::render`, custom bitmap‑font blitter  |
| **HTML Parser**          | Lightweight subset  | `tl` or custom regex (performance OK)       |
| **Image Decode**         | PNG/JPEG            | `image` crate (`jpeg`, `png` features)      |
| **SQLite**               | Deck import & state | `rusqlite` (bundles `libsqlite3‑sys`)       |
| **Audio Capture / Opus** | Mic recording       | `miniaudio` + `opus-sys` (`libopus` static) |
| **HTTP Server**          | Sync endpoint       | `tiny_http` (simple, 0 deps)                |
| **mDNS**                 | Discovery           | `libmdns` crate                             |
| **Build**                | Cargo (cross)       | `cross` helper or manual target             |



## Planned Phases

1. **Environment Setup** – configure the cross‑compile toolchain and basic build system.
2. **Core Deck Logic** – implement flashcard structures and persistence.
3. **UI Integration** – display cards on Trim UI Brick and handle input.
4. **Polish and Deployment** – refine interactions and package the app.

## Building the Project

CardBrick uses a standard CMake workflow. To build on a development machine:

```bash
cmake -S . -B build
cmake --build build
```
