// Xilem GUI for the tiny Salewski chess engine
// v0.3 -- 13-AUG-2025
// (C) 2015 - 2032 Dr. Stefan Salewski

use num_traits::clamp;
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use masonry::properties::types::{AsUnit, Length};
use masonry::{dpi::LogicalSize, parley::FontStack};
use masonry_winit::app::{EventLoop, EventLoopBuilder};
use tokio::time;
use winit::error::EventLoopError;
use xilem::{
    Blob,
    style::Padding,
    view::{CrossAxisAlignment, flex_col, text_button},
};
use xilem::{
    Color, WidgetView, WindowOptions, Xilem,
    core::fork,
    style::Style,
    view::{
        FlexExt, FlexSpacer, GridExt, button, checkbox, flex_row, grid, label, sized_box, task,
    },
};
use xilem_core::lens;

mod engine;

const HUMAN: u8 = 0;
const ENGINE: u8 = 1;

const STATE_UZ: i32 = -2;
const STATE_UX: i32 = -1;
const STATE_READY: i32 = 0;
const STATE_MOVE_ATTEMPT: i32 = 1;
const STATE_ENGINE_THINKING: i32 = 2;
const STATE_ENGINE_PLAYING: i32 = 3;

const BOOL_TO_ENGINE: [u8; 2] = [HUMAN, ENGINE];
const BOOL_TO_STATE: [i32; 2] = [STATE_READY, STATE_ENGINE_THINKING];
const GAP: Length = Length::const_px(12.);

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

fn engine_to_board(engine_board: engine::Board) -> [[Option<ColoredPiece>; 8]; 8] {
    use ColorSide::{Black, White};
    use Piece::*;
    let mut board = [[None; 8]; 8];
    for (i, &val) in engine_board.iter().enumerate() {
        let piece_color = match val {
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
        if let Some((piece, color)) = piece_color {
            board[i / 8][i % 8] = Some(ColoredPiece { piece, color });
        }
    }
    board
}

fn piece_unicode(piece: ColoredPiece, solid: bool) -> &'static str {
    use ColorSide::{Black, White};
    use Piece::*;
    match (piece.piece, if solid { Black } else { piece.color }) {
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
    game: Arc<Mutex<engine::Game>>,
    rx: Option<mpsc::Receiver<engine::Move>>,
    board: [[Option<ColoredPiece>; 8]; 8],
    selected: Option<(usize, usize)>,
    tagged: engine::Board,
    state: i32,
    msg: String,
    players: [u8; 2],
    engine_plays_white: bool,
    engine_plays_black: bool,
    use_solid_unicode: bool,
    rotated: bool,
    active: bool,
    time_per_move: f32,
    p0: i32,
    p1: i32,
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
            tagged: [0; 64],
            state: STATE_UZ,
            msg: "Tiny chess".into(),
            players: [HUMAN, ENGINE],
            engine_plays_white: false,
            engine_plays_black: true,
            use_solid_unicode: false,
            rotated: false,
            active: true,
            time_per_move: 1.5,
            p0: -1,
            p1: -1,
        }
    }
}

fn time_control_slider(time: &mut f32) -> impl WidgetView<f32> + use<> {
    flex_col((
        label(format!("{:.2} Sec/move", time)),
        flex_row((
            text_button("+", |t| *t = clamp(*t + 0.1, 0.1, 5.0)),
            text_button("-", |t| *t = clamp(*t - 0.1, 0.1, 5.0)),
        )),
    ))
}

fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    let label_bar = sized_box(label(&*state.msg)).height(32.px());
    let settings_panel = flex_col((
        FlexSpacer::Fixed(GAP),
        label_bar,
        lens(time_control_slider, |s: &mut AppState| &mut s.time_per_move),
        checkbox(
            "Engine plays white",
            state.engine_plays_white,
            |s: &mut AppState, _| {
                s.engine_plays_white ^= true;
                s.players[0] = BOOL_TO_ENGINE[s.engine_plays_white as usize];
                s.state = STATE_UZ;
            },
        ),
        checkbox(
            "Engine plays black",
            state.engine_plays_black,
            |s: &mut AppState, _| {
                s.engine_plays_black ^= true;
                s.players[1] = BOOL_TO_ENGINE[s.engine_plays_black as usize];
                s.state = STATE_UZ;
            },
        ),
        text_button("Rotate", |s: &mut AppState| s.rotated ^= true),
        text_button("New game", |s: &mut AppState| {
            if let Ok(mut game) = s.game.lock() {
                engine::reset_game(&mut game);
                s.board = engine_to_board(engine::get_board(&game));
                s.tagged = [0; 64];
                s.state = STATE_UZ;
            }
        }),
        text_button("Print movelist", |s: &mut AppState| {
            if let Ok(game) = s.game.lock() {
                engine::print_move_list(&game);
            }
        }),
        FlexSpacer::Fixed(GAP),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(GAP);

    let board_grid = {
        let mut cells = vec![];
        for row in 0..8 {
            for col in 0..8 {
                let (draw_row, draw_col) = if state.rotated {
                    (row, col)
                } else {
                    (7 - row, 7 - col)
                };
                let idx = row * 8 + col;
                let shade = match state.tagged[idx] {
                    2 => 25,
                    1 => 50,
                    _ => 0,
                };
                let color = if (row + col) % 2 == 0 {
                    Color::from_rgb8(255, 255, 255 - shade)
                } else {
                    Color::from_rgb8(205, 205, 205 - shade)
                };
                let label_piece = state.board[row][col]
                    .map(|p| {
                        label(piece_unicode(p, state.use_solid_unicode))
                            .font(FontStack::Source("Noto Sans Symbols 2".into()))
                            .text_size(96.0)
                            .color(Color::BLACK)
                    })
                    .unwrap_or_else(|| label(" ").text_size(96.0).color(Color::BLACK));
                let cell = sized_box(
                    button(label_piece, move |s: &mut AppState| {
                        let clicked = (row, col);
                        let idx = row * 8 + col;
                        match s.selected {
                            None => {
                                if s.board[row][col].is_some() {
                                    s.p0 = idx as i32;
                                    s.selected = Some(clicked);
                                    s.tagged = [0; 64];
                                    for m in engine::tag(&mut s.game.lock().unwrap(), idx as i64) {
                                        s.tagged[m.di as usize] = 1;
                                    }
                                    s.tagged[idx] = -1;
                                    s.state = STATE_READY;
                                }
                            }
                            Some(prev) => {
                                if prev != clicked {
                                    s.p1 = idx as i32;
                                    s.selected = None;
                                    s.state = STATE_MOVE_ATTEMPT;
                                } else {
                                    s.selected = None;
                                    s.tagged = [0; 64];
                                }
                            }
                        }
                    })
                    .padding(Padding::all(0.0))
                    .background_color(color)
                    .corner_radius(0.0),
                )
                .expand()
                .grid_pos(draw_col as i32, draw_row as i32);
                cells.push(cell);
            }
        }
        grid(cells, 8, 8)
    };

    let full_layout = flex_row((
        FlexSpacer::Fixed(GAP),
        settings_panel,
        flex_col((
            FlexSpacer::Fixed(GAP),
            board_grid.flex(1.0),
            FlexSpacer::Fixed(GAP),
        ))
        .flex(1.0),
        FlexSpacer::Fixed(GAP),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(GAP);
    fork(
        full_layout,
        state.active.then(|| {
            task(
                |proxy| async move {
                    let mut interval = time::interval(Duration::from_millis(100));
                    while proxy.message(()).is_ok() {
                        interval.tick().await;
                    }
                },
                |s: &mut AppState, _| {
                    if let Ok(game) = s.game.try_lock() {
                        s.board = engine_to_board(engine::get_board(&game));
                    }

                    match s.state {
                        STATE_UZ => {
                            let turn = s.game.lock().unwrap().move_counter as usize % 2;
                            s.state = BOOL_TO_STATE[s.players[turn] as usize];
                        }
                        STATE_MOVE_ATTEMPT if s.p1 >= 0 => {
                            let (from, to) = (s.p0 as i8, s.p1 as i8);
                            let valid = engine::move_is_valid2(
                                &mut s.game.lock().unwrap(),
                                from as i64,
                                to as i64,
                            );
                            s.tagged = [0; 64];
                            if from == to || !valid {
                                s.msg = "Invalid move.".into();
                            } else {
                                let flag =
                                    engine::do_move(&mut s.game.lock().unwrap(), from, to, false);
                                s.msg =
                                    engine::move_to_str(&s.game.lock().unwrap(), from, to, flag);
                                s.tagged[from as usize] = 2;
                                s.tagged[to as usize] = 2;
                            }
                            s.state = STATE_UZ;
                        }
                        STATE_ENGINE_THINKING => {
                            s.state = STATE_ENGINE_PLAYING;
                            if let Ok(mut game) = s.game.try_lock() {
                                game.secs_per_move = s.time_per_move;
                            }
                            let (tx, rx) = mpsc::channel();
                            s.rx = Some(rx);
                            let game_clone = Arc::clone(&s.game);
                            thread::spawn(move || {
                                let chess_move = engine::reply(&mut game_clone.lock().unwrap());
                                let _ = tx.send(chess_move);
                            });
                        }
                        STATE_ENGINE_PLAYING => {
                            if let Some(rx) = &s.rx {
                                if let Ok(mv) = rx.try_recv() {
                                    let mut game = s.game.lock().unwrap();
                                    s.tagged = [0; 64];
                                    s.tagged[mv.src as usize] = 2;
                                    s.tagged[mv.dst as usize] = 2;
                                    let flag = engine::do_move(
                                        &mut game,
                                        mv.src as i8,
                                        mv.dst as i8,
                                        false,
                                    );
                                    s.msg = engine::move_to_str(
                                        &game,
                                        mv.src as i8,
                                        mv.dst as i8,
                                        flag,
                                    ) + &format!(" (scr: {})", mv.score);

                                    s.rx = None;
                                    s.state = match mv.state {
                                        engine::STATE_CHECKMATE => {
                                            s.msg = "Checkmate, game terminated!".into();
                                            STATE_UX
                                        }
                                        _ if mv.score.abs() > engine::KING_VALUE_DIV_2 as i64 => {
                                            let turns = mv.checkmate_in / 2
                                                + if mv.score > 0 { -1 } else { 1 };
                                            s.msg.push_str(&format!(" Checkmate in {}", turns));
                                            STATE_UZ
                                        }
                                        _ => STATE_UZ,
                                    };
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

#[cfg(target_os = "android")]
#[expect(unsafe_code)]
#[no_mangle]
unsafe fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let mut event_loop = EventLoop::with_user_event();
    event_loop.with_android_app(app);
    run(event_loop).expect("Cannot create app");
}
