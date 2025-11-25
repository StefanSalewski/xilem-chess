// Xilem GUI for the tiny Salewski chess engine
// v0.5 -- 25-NOV-2025
// (C) 2015 - 2032 Dr. Stefan Salweski

use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use masonry::properties::types::AsUnit;
use masonry::properties::types::Length;
use masonry::{dpi::LogicalSize, parley::FontStack};
use masonry_winit::app::{EventLoop, EventLoopBuilder};
use tokio::time;
use winit::error::EventLoopError;
use xilem::view::CrossAxisAlignment;
use xilem::{
    core::fork,
    view::{
        button, checkbox, flex_col, flex_row, grid, label, prose, sized_box, slider, task,
        text_button, FlexExt, FlexSpacer, GridExt,
    },
    Blob, Color, WidgetView, WindowOptions, Xilem,
};
use xilem_core::Edit;
use xilem::style::Style;

mod engine;

const TIMER_TICK_MS: u64 = 100;
const TIMER_TICK_SECS: f64 = TIMER_TICK_MS as f64 / 1000.0;
const BOARD_SIZE: usize = 8;
const GAP: Length = Length::const_px(12.0);
const TINY_GAP: Length = Length::const_px(4.0);

#[derive(Clone, Copy, Debug)]
enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    White,
    Black,
}

#[derive(Clone, Copy, Debug)]
struct ColoredPiece {
    piece: Piece,
    side: Side,
}

type BoardView = [[Option<ColoredPiece>; BOARD_SIZE]; BOARD_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlayerKind {
    Human,
    Engine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Waiting to decide whose turn it is next.
    Uninitialized,
    /// Game is over; input is effectively disabled.
    Inactive,
    /// Human can select a piece to move.
    Ready,
    /// Human clicked a destination square; apply the move.
    MoveAttempt,
    /// Engine is thinking in the background.
    EngineThinking,
    /// Engine move has been produced; apply it.
    EnginePlaying,
}

/// Map a "engine plays this side" boolean to a PlayerKind.
const PLAYER_FOR_ENGINE_FLAG: [PlayerKind; 2] = [PlayerKind::Human, PlayerKind::Engine];

fn engine_to_board(engine_board: engine::Board) -> BoardView {
    use Piece::*;
    use Side::{Black, White};

    let mut board = [[None; BOARD_SIZE]; BOARD_SIZE];

    for (i, &val) in engine_board.iter().enumerate() {
        let piece_side = match val {
            1 => Some((Pawn, White)),
            2 => Some((Knight, White)),
            3 => Some((Bishop, White)),
            4 => Some((Rook, White)),
            5 => Some((Queen, White)),
            6 => Some((King, White)),
            -1 => Some((Pawn, Black)),
            -2 => Some((Knight, Black)),
            -3 => Some((Bishop, Black)),
            -4 => Some((Rook, Black)),
            -5 => Some((Queen, Black)),
            -6 => Some((King, Black)),
            _ => None,
        };
        if let Some((piece, side)) = piece_side {
            board[i / BOARD_SIZE][i % BOARD_SIZE] = Some(ColoredPiece { piece, side });
        }
    }
    board
}

fn piece_unicode(piece: ColoredPiece, solid: bool) -> &'static str {
    use Piece::*;
    use Side::{Black, White};

    // When `solid` is true, always draw the black glyph.
    let effective_side = if solid { Black } else { piece.side };

    match (piece.piece, effective_side) {
        (King, White) => "♔",
        (Queen, White) => "♕",
        (Rook, White) => "♖",
        (Bishop, White) => "♗",
        (Knight, White) => "♘",
        (Pawn, White) => "♙",
        (King, Black) => "♚",
        (Queen, Black) => "♛",
        (Rook, Black) => "♜",
        (Bishop, Black) => "♝",
        (Knight, Black) => "♞",
        (Pawn, Black) => "♟︎",
    }
}

struct AppState {
    /// Current engine game state.
    game: Arc<Mutex<engine::Game>>,
    /// Receiver for the background engine thread replying with a move.
    rx: Option<mpsc::Receiver<engine::Move>>,
    /// View of the board as Unicode pieces; derived from `game`.
    board: BoardView,
    /// Currently selected square (for human moves).
    selected: Option<(usize, usize)>,
    /// Per-square tags for highlighting last move, possible moves, etc.
    square_tags: engine::Board,
    /// High-level application phase (whose turn, what we're waiting for).
    phase: Phase,
    /// Status line below controls.
    status: String,
    /// Player on each side (0 = white, 1 = black).
    players: [PlayerKind; 2],
    /// UI flags for checkboxes.
    engine_plays_white: bool,
    engine_plays_black: bool,
    /// If true, use "solid" Unicode pieces (always black glyphs).
    use_solid_unicode: bool,
    /// If true, draw board from white's perspective; otherwise black's.
    rotated: bool,
    /// If false, the periodic task isn't scheduled.
    active: bool,
    /// Time per engine move (seconds).
    time_per_move: f64,
    /// Accumulated clock time in seconds for [white, black].
    time_elapsed: [f64; 2],
    /// Current side to move (0 = white, 1 = black).
    turn: usize,
    /// Pending human move as linear indices (from, to), if any.
    pending_move: Option<(usize, usize)>,
    /// Move list in text form.
    movelist: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        let game = engine::new_game();
        let board = engine_to_board(engine::get_board(&game));

        Self {
            game: Arc::new(Mutex::new(game)),
            rx: None,
            board,
            selected: None,
            square_tags: [0; 64],
            phase: Phase::Uninitialized,
            status: "Tiny chess".into(),
            players: [PlayerKind::Human, PlayerKind::Engine],
            engine_plays_white: false,
            engine_plays_black: true,
            use_solid_unicode: false,
            rotated: false,
            active: true,
            time_per_move: 1.5,
            time_elapsed: [0.0, 0.0],
            turn: 0,
            pending_move: None,
            movelist: Vec::new(),
        }
    }
}

impl AppState {
    fn formatted_clock(secs: f64) -> String {
        // Simple "MM:SS" display
        let total = secs.round() as u64;
        let minutes = total / 60;
        let seconds = total % 60;
        format!("{minutes:02}:{seconds:02}")
    }

    fn movelist_text(&self) -> String {
        self.movelist
            .chunks(2)
            .enumerate()
            .map(|(idx, chunk)| match chunk {
                [a, b] => format!("{:>3}. {:>7}  {}", idx + 1, a, b),
                [a] => format!("{:>3}. {:>7}", idx + 1, a),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Called periodically by the Xilem `task` to advance the game / UI state.
    fn tick(&mut self) {
        // Only advance clocks during active phases.
        if matches!(
            self.phase,
            Phase::Ready | Phase::MoveAttempt | Phase::EngineThinking | Phase::EnginePlaying
        ) {
            self.time_elapsed[self.turn] += TIMER_TICK_SECS;
        }

        // Periodically sync our board view from the engine state.
        if let Ok(game) = self.game.try_lock() {
            self.board = engine_to_board(engine::get_board(&game));
        }

        match self.phase {
            Phase::Uninitialized => {
                if let Ok(game) = self.game.lock() {
                    let turn = game.move_counter as usize % 2;
                    self.turn = turn;
                    let player = self.players[turn];
                    self.phase = match player {
                        PlayerKind::Human => Phase::Ready,
                        PlayerKind::Engine => Phase::EngineThinking,
                    };
                }
            }
            Phase::MoveAttempt => {
                if let Some((from_idx, to_idx)) = self.pending_move.take() {
                    let from = from_idx as i8;
                    let to = to_idx as i8;

                    let mut game = self.game.lock().unwrap();

                    let valid = engine::move_is_valid2(&mut game, from as i64, to as i64);

                    self.square_tags = [0; 64];

                    if from_idx == to_idx || !valid {
                        self.status = "Invalid move.".into();
                    } else {
                        let flag = engine::do_move(&mut game, from, to, false);
                        let notation = engine::move_to_str(&game, from, to, flag);
                        self.movelist.push(notation.clone());
                        self.status = notation;
                        self.square_tags[from_idx] = 2;
                        self.square_tags[to_idx] = 2;
                    }
                }
                self.phase = Phase::Uninitialized;
            }
            Phase::EngineThinking => {
                // Switch to "playing" and start a background thread to compute a move.
                self.phase = Phase::EnginePlaying;

                if let Ok(mut game) = self.game.try_lock() {
                    game.secs_per_move = self.time_per_move as f32;
                }

                let (tx, rx) = mpsc::channel();
                self.rx = Some(rx);
                let game_clone = Arc::clone(&self.game);

                thread::spawn(move || {
                    let chess_move = engine::reply(&mut game_clone.lock().unwrap());
                    let _ = tx.send(chess_move);
                });
            }
            Phase::EnginePlaying => {
                if let Some(rx) = &self.rx {
                    if let Ok(mv) = rx.try_recv() {
                        let mut game = self.game.lock().unwrap();

                        self.square_tags = [0; 64];
                        self.square_tags[mv.src as usize] = 2;
                        self.square_tags[mv.dst as usize] = 2;

                        let flag =
                            engine::do_move(&mut game, mv.src as i8, mv.dst as i8, false);
                        let notation = engine::move_to_str(
                            &game,
                            mv.src as i8,
                            mv.dst as i8,
                            flag,
                        );

                        self.movelist.push(notation.clone());
                        self.status = format!("{notation} (scr: {})", mv.score);

                        self.rx = None;
                        self.phase = match mv.state {
                            engine::STATE_CHECKMATE => {
                                self.status =
                                    "Checkmate, game terminated!".into();
                                Phase::Inactive
                            }
                            _ if mv.score.abs()
                                > engine::KING_VALUE_DIV_2 as i64 =>
                            {
                                let turns = mv.checkmate_in / 2
                                    + if mv.score > 0 { -1 } else { 1 };
                                self.status
                                    .push_str(&format!(" Checkmate in {}", turns));
                                Phase::Uninitialized
                            }
                            _ => Phase::Uninitialized,
                        };
                    }
                }
            }
            // Ready / Inactive and any other phases: nothing special on tick.
            _ => {}
        }
    }
}

fn board_grid(state: &mut AppState) -> impl WidgetView<Edit<AppState>> + use<> {
    let mut cells = Vec::with_capacity(BOARD_SIZE * BOARD_SIZE);

    for row in 0..BOARD_SIZE {
        for col in 0..BOARD_SIZE {
            let idx = row * BOARD_SIZE + col;

            let (draw_row, draw_col) = if state.rotated {
                (row, col)
            } else {
                (BOARD_SIZE - 1 - row, BOARD_SIZE - 1 - col)
            };

            let shade = match state.square_tags[idx] {
                2 => 25,
                1 => 50,
                _ => 0,
            };

            let color = if (row + col) % 2 == 0 {
                Color::from_rgb8(255, 255, 255 - shade)
            } else {
                Color::from_rgb8(205, 205, 205 - shade)
            };

            let label_text = state.board[row][col]
                .map(|p| piece_unicode(p, state.use_solid_unicode))
                .unwrap_or(" ");

            let label_piece = label(label_text)
                .text_size(96.0)
                .font(FontStack::Source("Noto Sans Symbols 2".into()))
                .color(Color::BLACK);

            let cell = button(label_piece, move |s: &mut AppState| {
                let clicked = (row, col);

                match s.selected {
                    None => {
                        // First click: select a piece and show its legal moves.
                        if s.board[row][col].is_some() {
                            s.selected = Some(clicked);
                            s.pending_move = None;
                            s.square_tags = [0; 64];

                            for m in engine::tag(
                                &mut s.game.lock().unwrap(),
                                idx as i64,
                            ) {
                                s.square_tags[m.di as usize] = 1;
                            }
                            s.square_tags[idx] = -1;
                            s.phase = Phase::Ready;
                        }
                    }
                    Some(prev) if prev != clicked => {
                        // Second click: attempt a move.
                        let from_idx = prev.0 * BOARD_SIZE + prev.1;
                        s.pending_move = Some((from_idx, idx));
                        s.selected = None;
                        s.phase = Phase::MoveAttempt;
                    }
                    Some(_) => {
                        // Second click on same square: deselect.
                        s.selected = None;
                        s.pending_move = None;
                        s.square_tags = [0; 64];
                    }
                }
            })
            .padding(0.0)
            .background_color(color)
            .corner_radius(0.0)
            .grid_pos(draw_col as i32, draw_row as i32);

            cells.push(cell);
        }
    }

    grid(cells, BOARD_SIZE as i32, BOARD_SIZE as i32)
}

fn settings_panel(state: &mut AppState) -> impl WidgetView<Edit<AppState>> + use<> {
    let movelist_text = state.movelist_text();

    flex_col((
        FlexSpacer::Fixed(GAP),
        label(&*state.status),
        FlexSpacer::Fixed(TINY_GAP),
        label(format!(
            "White: {}",
            AppState::formatted_clock(state.time_elapsed[0])
        )),
        label(format!(
            "Black: {}",
            AppState::formatted_clock(state.time_elapsed[1])
        )),
        FlexSpacer::Fixed(TINY_GAP),
        label(format!("{:.2} sec/move", state.time_per_move)),
        slider(
            0.1,
            5.0,
            state.time_per_move,
            |state: &mut AppState, val| {
                state.time_per_move = val;
            },
        ),
        checkbox(
            "Engine plays white",
            state.engine_plays_white,
            |s: &mut AppState, _| {
                s.engine_plays_white = !s.engine_plays_white;
                s.players[0] =
                    PLAYER_FOR_ENGINE_FLAG[s.engine_plays_white as usize];
                s.phase = Phase::Uninitialized;
            },
        ),
        checkbox(
            "Engine plays black",
            state.engine_plays_black,
            |s: &mut AppState, _| {
                s.engine_plays_black = !s.engine_plays_black;
                s.players[1] =
                    PLAYER_FOR_ENGINE_FLAG[s.engine_plays_black as usize];
                s.phase = Phase::Uninitialized;
            },
        ),
        text_button("Rotate", |s: &mut AppState| {
            s.rotated = !s.rotated;
        }),
        text_button("New game", |s: &mut AppState| {
            if let Ok(mut game) = s.game.lock() {
                engine::reset_game(&mut game);
                s.board = engine_to_board(engine::get_board(&game));
                s.square_tags = [0; 64];
                s.selected = None;
                s.pending_move = None;
                s.rx = None;
                s.phase = Phase::Uninitialized;
                s.time_elapsed = [0.0, 0.0];
                s.movelist.clear();
            }
        }),
        text_button("Print movelist", |s: &mut AppState| {
            if let Ok(game) = s.game.lock() {
                engine::print_move_list(&game);
            }
        }),
        sized_box(prose(movelist_text)).width(200.px()),
        FlexSpacer::Fixed(GAP),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(GAP)
}

fn main_layout(state: &mut AppState) -> impl WidgetView<Edit<AppState>> + use<> {
    flex_row((
        FlexSpacer::Fixed(GAP),
        settings_panel(state),
        flex_col((
            FlexSpacer::Fixed(GAP),
            board_grid(state).flex(1.0),
            FlexSpacer::Fixed(GAP),
        ))
        .flex(1.0),
        FlexSpacer::Fixed(GAP),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(GAP)
}

fn app_logic(state: &mut AppState) -> impl WidgetView<Edit<AppState>> + use<> {
    fork(
        main_layout(state),
        state.active.then(|| {
            task(
                |proxy| async move {
                    let mut interval =
                        time::interval(Duration::from_millis(TIMER_TICK_MS));
                    while proxy.message(()).is_ok() {
                        interval.tick().await;
                    }
                },
                |s: &mut AppState, _| {
                    s.tick();
                },
            )
        }),
    )
}

const NOTO_SANS_SYMBOLS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fonts/noto_sans_symbols_2/",
    "NotoSansSymbols2-Regular.ttf"
));

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(
        AppState::default(),
        app_logic,
        WindowOptions::new("Xilem Chess GUI")
            .with_min_inner_size(LogicalSize::new(800.0, 800.0))
            .with_initial_inner_size(LogicalSize::new(1200.0, 1000.0)),
    )
    .with_font(Blob::new(Arc::new(NOTO_SANS_SYMBOLS)));
    app.run_in(event_loop)
}

fn main() -> Result<(), EventLoopError> {
    run(EventLoop::with_user_event())
}

