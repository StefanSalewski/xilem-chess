# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
# Run in release mode (recommended — engine is slow without optimization)
cargo run --release

# Run with debug logging
RUST_LOG=debug cargo run

# Use system font instead of bundled Noto Sans Symbols 2
cargo run --release --features useSystemFont

# Enable engine debug output
cargo run --release --features salewskiChessDebug

# Install as a binary
cargo install --path .
```

There are no automated tests. The Xilem dependency tracks the latest Git HEAD of `https://github.com/linebender/xilem`, so compilation may occasionally break when Xilem's API changes.

## Clippy

The engine has specific lint suppressions; use this to avoid false positives:

```bash
cargo clippy -- -A clippy::collapsible_if -A unreachable_code -A dead_code -A clippy::upper_case_acronyms -A clippy::out_of_bounds_indexing -A clippy::overly_complex_bool_expr -A clippy::too_many_arguments -A clippy::assertions_on_constants
```

## Architecture

Two source files:

- **`src/main.rs`** — all UI code. Owns `AppState`, builds Xilem views, manages game lifecycle.
- **`src/engine.rs`** — self-contained chess engine ported from Nim. Exposes a C-style API (`new_game`, `reset_game`, `get_board`, `do_move`, `reply`, `tag`, `move_is_valid2`, `move_to_str`, `print_move_list`).

### State machine (`Phase` enum)

`AppState` drives the game through a state machine:

```
Uninitialized → Ready (human turn) or EngineThinking (engine turn)
Ready → MoveAttempt (human clicks destination)
MoveAttempt → Uninitialized (after applying/rejecting move)
EngineThinking → EnginePlaying (spawns background thread)
EnginePlaying → Uninitialized (after receiving move via mpsc channel)
Any → Inactive (checkmate/game over)
```

### Engine threading

When it's the engine's turn, `main.rs` spawns a `std::thread` that calls `engine::reply()` while holding the `Arc<Mutex<engine::Game>>`. The result is sent back via `mpsc::channel` and consumed in `AppState::tick()`, which fires every 100 ms via a Xilem `task` future.

### Board representation

- Engine uses a flat `[i8; 64]` array (`engine::Board`) where positive values = white pieces, negative = black (1=Pawn … 6=King).
- UI uses `[[Option<ColoredPiece>; 8]; 8]` (`BoardView`). Conversion happens in `engine_to_board()`.
- `square_tags: engine::Board` drives highlight colors (1 = legal move hint, 2 = last move, -1 = selected square).

### Font

By default, `NotoSansSymbols2-Regular.ttf` is embedded at compile time via `include_bytes!` and registered with Xilem. The `useSystemFont` feature skips font registration and lets the OS provide a Unicode chess-capable font.
