# xilem-chess

Xilem-based GUI for the Tiny Salewski Chess Engine
![Chess UI](http://ssalewski.de/tmp/xilem-chess.png)

## ✨ Overview

`xilem-chess` is a graphical user interface for the compact [Salewski chess engine](https://ssalewski.de/), built with [Xilem](https://github.com/linebender/xilem) — a declarative Rust GUI framework. It demonstrates a clean and interactive chess UI while integrating real-time engine responses using multi-threading and channels.

This project showcases:

* A full chess GUI with Unicode piece rendering
* Player vs Engine and Engine vs Engine play modes
* Responsive board layout with adjustable engine move time
* Highlighted move suggestions and last-move tracking
* Threaded engine communication using Rust’s `mpsc` channels

## 🚀 Features

* ✅ Fully playable chess interface
* ✅ Configurable time-per-move for engine
* ✅ Rotate board view
* ✅ Print move list to console
* ✅ Toggle engine control for each color
* ✅ Optional solid-colored Unicode pieces
* ✅ Responsive layout built with Xilem flex/grid
* ⚠️ No drag-and-drop (yet), only click-based input
* ⚠️ No game state persistence or PGN export
* ❌ Dynamic window title and scaling not yet supported by Xilem

---

## 📦 Dependencies

* Rust 1.78+ (2024 edition)
* [Xilem](https://github.com/linebender/xilem) (latest Git version)
* [masonry](https://github.com/linebender/xilem/tree/main/masonry) for layout and widgets
* `tokio`, `num-traits`, and `winit` for concurrency and platform support

---

## 🔧 Build & Run

Clone and run with Cargo:

```bash
git clone https://github.com/stefansalewski/xilem-chess.git
cd xilem-chess
RUST_LOG=off cargo run
```

You’ll see a playable chessboard with control options on the left.

---

## 🕹️ Controls

| Control                | Description                            |
| ---------------------- | -------------------------------------- |
| **Engine plays White** | Toggle engine control for white pieces |
| **Engine plays Black** | Toggle engine control for black pieces |
| **Rotate**             | Flip board orientation                 |
| **New game**           | Restart from initial position          |
| **Print movelist**     | Log move history to terminal           |
| **Sec/move**           | Adjust engine thinking time            |

Moves are made by clicking one square, then the destination square.

---

## 🧠 Architecture

The core structure uses:

* `AppState`: holds game state, settings, and board UI data
* `engine::Game`: encapsulates the chess logic and rules
* A message loop via `task(...)` + `mpsc::Receiver<Move>` for threaded engine replies
* `engine_to_board(...)`: converts internal engine board to a UI-friendly 2D array
* Xilem widgets (`grid`, `button`, `checkbox`, `label`, etc.) for layout

---

## 📱 Platform Support

Tested on:

* ✅ Linux (X11 and Wayland)
* ✅ Windows (expected to work)
* ⚠️ macOS (untested, but should work)
* ⚠️ Android (via `android_main`, experimental)

---

## ❗ Limitations

* Xilem does not yet support dynamic scaling of widgets or dynamic window titles
* Promotion, PGN import/export, and drag-drop interactions are not yet implemented

---

## 🧪 Development Notes

This project was inspired by the Xilem examples `stopwatch.rs` and `calc.rs`, and the earlier `tiny-chess` version using egui. This version focuses on clean separation of UI state and engine logic.

Use the following to experiment or debug:

```bash
RUST_LOG=debug cargo run
```

---

## 📄 License

(C) 2015–2032 Dr. Stefan Salewski.
MIT or Apache 2.0 (same as Rust ecosystem).

---


