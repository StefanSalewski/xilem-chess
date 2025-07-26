// Plain Xilem frontend for the tiny Salewski chess engine
// v 0.1 -- 25-JUL-2025
// (C) 2015 - 2032 Dr. Stefan Salewski
// All rights reserved.

// This version uses threading with spawn and channels.
// GUI is based on the Xilem stopwatch.rs and calc.rs examples and the tiny-chess EGUI version.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use masonry::dpi::LogicalSize;
use masonry_winit::app::{EventLoop, EventLoopBuilder};
use tokio::time;
use winit::error::EventLoopError;
use xilem::core::fork;
use xilem::view::{button, grid, label, sized_box, task, GridExt};
use xilem::{Color, WidgetView, WindowOptions, Xilem};
use xilem::style::Style;

mod engine;

const ENGINE: u8 = 1;
const HUMAN: u8 = 0;

const FIGURES: [&str; 13] = [
    "♚", "♛", "♜", "♝", "♞", "♟", "", "♙", "♘", "♗", "♖", "♕", "♔",
];

const STATE_UZ: i32 = -2;
const STATE_UX: i32 = -1;
const STATE_U0: i32 = 0;
const STATE_U1: i32 = 1;
const STATE_U2: i32 = 2;
const STATE_U3: i32 = 3;

const BOOL_TO_ENGINE: [u8; 2] = [HUMAN, ENGINE];
const BOOL_TO_STATE: [i32; 2] = [STATE_U0, STATE_U2];

fn _print_variable_type<K>(_: &K) {
    println!("{}", std::any::type_name::<K>())
}

#[derive(Clone, Copy, Debug)]
enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ColorSide {
    White,
    Black,
}

#[derive(Clone, Copy, Debug)]
struct ColoredPiece {
    piece: Piece,
    color: ColorSide,
}

fn engine_to_board(b: engine::Board) -> [[Option<ColoredPiece>; 8]; 8] {
    use ColorSide::{Black as B, White as W};
    use Piece::*;

    let mut res = [[None; 8]; 8];
    for (i, el) in b.iter().enumerate() {
        res[i / 8][i % 8] = match el {
            1 => Some(ColoredPiece { piece: Pawn, color: W }),
            2 => Some(ColoredPiece { piece: Knight, color: W }),
            3 => Some(ColoredPiece { piece: Bishop, color: W }),
            4 => Some(ColoredPiece { piece: Rook, color: W }),
            5 => Some(ColoredPiece { piece: Queen, color: W }),
            6 => Some(ColoredPiece { piece: King, color: W }),
            -1 => Some(ColoredPiece { piece: Pawn, color: B }),
            -2 => Some(ColoredPiece { piece: Knight, color: B }),
            -3 => Some(ColoredPiece { piece: Bishop, color: B }),
            -4 => Some(ColoredPiece { piece: Rook, color: B }),
            -5 => Some(ColoredPiece { piece: Queen, color: B }),
            -6 => Some(ColoredPiece { piece: King, color: B }),
            _ => None,
        }
    }
    res
}

fn piece_unicode(piece: ColoredPiece, use_solid: bool) -> &'static str {
    use ColorSide::{Black, White};
    use Piece::*;

    match (piece.piece, if use_solid { Black } else { piece.color }) {
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
        (Pawn, Black) => "♟",
    }
}

struct AppState {
    game: Arc<Mutex<engine::Game>>,
    rx: Option<mpsc::Receiver<engine::Move>>,
    board: [[Option<ColoredPiece>; 8]; 8],
    selected: Option<(usize, usize)>,
    use_solid_unicode: bool,
    active: bool,
    msg: String,
    rotated: bool,
    time_per_move: f32,
    tagged: engine::Board,
    state: engine::State,
    players: [u8; 2],
    engine_plays_white: bool,
    engine_plays_black: bool,
    p0: i32,
    p1: i32,
    new_game: bool,
    bbb: engine::Board,
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
            active: true,
            use_solid_unicode: false,
            msg: "Tiny chess".to_owned(),
            time_per_move: 1.5,
            rotated: true,
            tagged: [0; 64],
            players: [HUMAN, ENGINE],
            p0: -1,
            p1: -1,
            state: STATE_UZ,
            bbb: [0; 64],
            new_game: true,
            engine_plays_white: false,
            engine_plays_black: true,
        }
    }
}

// GUI logic
fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    let board_copy = state.board;
    let mut tagged_copy = state.tagged;
    tagged_copy.reverse();
    let use_solid = state.use_solid_unicode;

    let squares: Vec<_> = (0..8)
        .flat_map(|row| (0..8).map(move |col| {
            let piece = board_copy[row][col];
            let text = piece.map(|p| piece_unicode(p, use_solid)).unwrap_or(" ");
            let fg = piece.map(|p| Color::BLACK);
            let p = col + row * 8;
            let t = tagged_copy[p];
            let h = match t {
                2 => 25,
                1 => 50,
                _ => 0,
            };
            let color = if (row + col) % 2 == 0 {
                Color::from_rgb8(255, 255, 255 - h)
            } else {
                Color::from_rgb8(205, 205, 205 - h)
            };

            let mut lbl = label(text).text_size(96.0);
            if let Some(fg) = fg {
                lbl = lbl.text_color(fg);
            }

            sized_box(
                button(lbl, move |state: &mut AppState| {
                    match state.selected {
                        None => {
                            if state.board[row][col].is_some() {
                                state.selected = Some((row, col));
                                state.state = STATE_U0;
                                state.p0 = (col + row * 8) as i32;
                            }
                        }
                        Some((sel_row, sel_col)) => {
                            if (sel_row, sel_col) != (row, col) {
                                state.p1 = (col + row * 8) as i32;
                            }
                            state.selected = None;
                            state.state = STATE_U1;
                        }
                    }
                })
                .background_color(color)
                .corner_radius(0.0),
            )
            .expand()
            .grid_pos(col as i32, (7 - row) as i32)
        }))
        .collect();

    let widgets = grid(squares, 8, 8);

    fork(
        widgets,
        state.active.then(|| {
            task(
                |proxy| async move {
                    let mut interval = time::interval(Duration::from_millis(300));
                    loop {
                        interval.tick().await;
                        if proxy.message(()).is_err() {
                            break;
                        }
                    }
                },
                |state: &mut AppState, ()| {
                    if let Ok(mut game) = state.game.try_lock() {
                        if state.new_game {
                            engine::reset_game(&mut game);
                            state.new_game = false;
                            state.state = STATE_UZ;
                            state.tagged = [0; 64];
                        }
                        state.bbb = engine::get_board(&mut game);
                        game.secs_per_move = state.time_per_move;
                    }
                    state.board = engine_to_board(state.bbb);

                    match state.state {
                        STATE_UX => {}
                        STATE_UZ => {
                            let next = state.game.lock().unwrap().move_counter as usize % 2;
                            state.state = BOOL_TO_STATE[state.players[next] as usize];
                        }
                        STATE_U0 if state.selected.is_some() => {
                            let h = state.p0 as usize;
                            state.tagged = [0; 64];
                            for i in engine::tag(&mut state.game.lock().unwrap(), h as i64) {
                                state.tagged[i.di as usize] = 1;
                            }
                            state.tagged[h] = -1;
                            if state.rotated {
                                state.tagged.reverse();
                            }
                        }
                        STATE_U1 if state.p1 >= 0 => {
                            let p1 = state.p1 as i8;
                            let h = state.p0;
                            if h == p1 as i32 || !engine::move_is_valid2(&mut state.game.lock().unwrap(), h as i64, p1 as i64) {
                                state.msg = "invalid move, ignored.".to_string();
                                state.tagged = [0; 64];
                                state.state = STATE_UZ;
                                return;
                            }
                            let flag = engine::do_move(&mut state.game.lock().unwrap(), h as i8, p1, false);
                            state.tagged = [0; 64];
                            state.tagged[h as usize] = 2;
                            state.tagged[p1 as usize] = 2;
                            if state.rotated {
                                state.tagged.reverse();
                            }
                            state.msg = engine::move_to_str(&state.game.lock().unwrap(), h as i8, p1, flag);
                            state.state = STATE_UZ;
                        }
                        STATE_U2 => {
                            state.state = STATE_U3;
                            let (tx, rx) = mpsc::channel();
                            state.rx = Some(rx);
                            let game_clone = state.game.clone();
                            thread::spawn(move || {
                                let m = engine::reply(&mut game_clone.lock().unwrap());
                                tx.send(m).ok();
                            });
                        }
                        STATE_U3 => {
                            if let Some(rx) = &state.rx {
                                if let Ok(m) = rx.try_recv() {
                                    if m.state == engine::STATE_CHECKMATE {
                                        state.msg = "Checkmate, game terminated!".to_string();
                                        state.state = STATE_UX;
                                        return;
                                    }
                                    state.tagged = [0; 64];
                                    state.tagged[m.src as usize] = 2;
                                    state.tagged[m.dst as usize] = 2;
                                    if state.rotated {
                                        state.tagged.reverse();
                                    }
                                    let flag = engine::do_move(&mut state.game.lock().unwrap(), m.src as i8, m.dst as i8, false);
                                    state.msg = engine::move_to_str(&state.game.lock().unwrap(), m.src as i8, m.dst as i8, flag);
                                    if m.checkmate_in == 2 && m.score == engine::KING_VALUE as i64 {
                                        state.msg = "Checkmate, game terminated!".to_string();
                                        state.state = STATE_UX;
                                        return;
                                    } else if m.score.abs() > engine::KING_VALUE_DIV_2 as i64 {
                                        let checkmate_in = if m.score > 0 {
                                            m.checkmate_in / 2 - 1
                                        } else {
                                            m.checkmate_in / 2 + 1
                                        };
                                        state.msg.push_str(&format!(" Checkmate in {}", checkmate_in));
                                    }
                                    state.state = STATE_UZ;
                                    state.rx = None;
                                }
                            }
                        }
                        _ => {}
                    }
                },
            )
        }),
    )
}

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let data = AppState::default();
    let window_options = WindowOptions::new("First Xilem Chess GUI")
        .with_min_inner_size(LogicalSize::new(800.0, 800.0))
        .with_initial_inner_size(LogicalSize::new(1200.0, 1200.0));
    let app = Xilem::new_simple(data, app_logic, window_options);
    app.run_in(event_loop)
}

// Platform-specific boilerplate
fn main() -> Result<(), EventLoopError> {
    run(EventLoop::with_user_event())
}

#[cfg(target_os = "android")]
#[expect(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let mut event_loop = EventLoop::with_user_event();
    event_loop.with_android_app(app);
    run(event_loop).expect("Can create app");
}

// 356 lines
