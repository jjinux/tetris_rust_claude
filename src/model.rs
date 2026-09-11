//! The model: all of the game state and the rules of Tetris.
//!
//! Nothing in this file knows about the terminal. That keeps the rules easy to
//! unit test (see the `tests` module at the bottom).
//!
//! Compared to the Go original, the biggest change is how the falling piece is
//! stored. Go kept it *inside* the board as negative numbers and had to erase
//! and re-place it on every move. Here the board holds only locked cells and
//! the falling piece is a separate `Option<Piece>`, so moving a piece is just
//! "build a moved copy, check that it fits, and keep it if it does".

use std::time::{Duration, Instant};

use rand::RngExt;
use rand::rngs::StdRng;

/// Width of the board in cells.
pub const BOARD_WIDTH: usize = 10;
/// Height of the board in cells.
pub const BOARD_HEIGHT: usize = 16;

const NUM_SQUARES: usize = 4;
const DEFAULT_LEVEL: u32 = 1;
const MAX_LEVEL: u32 = 10;
const ROWS_PER_LEVEL: u32 = 5;

// Pieces fall once every `speed()`; each level shaves `SPEED_STEP` off.
const SLOWEST_SPEED: Duration = Duration::from_millis(700);
const SPEED_STEP: Duration = Duration::from_millis(60);

/// The seven tetrominoes.
///
/// `#[derive(...)]` asks the compiler to generate common trait impls for us.
/// `Copy` means values are duplicated implicitly (like an `int`) instead of
/// moved, which is appropriate for a tiny enum like this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    T,
    J,
    L,
    S,
    Z,
    I,
    O,
}

impl PieceKind {
    /// Every kind, in a fixed order, so we can pick one at random by index.
    pub const ALL: [PieceKind; 7] = [
        PieceKind::T,
        PieceKind::J,
        PieceKind::L,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::I,
        PieceKind::O,
    ];

    /// The `(dx, dy)` offsets of the four squares relative to the piece's
    /// origin. These are the same numbers as `dxBank`/`dyBank` in the Go
    /// version, just zipped together per piece.
    ///
    /// A `match` on an enum must be exhaustive: if a variant is ever added,
    /// the compiler will refuse to build until it is handled here too.
    fn offsets(self) -> [(i32, i32); NUM_SQUARES] {
        match self {
            PieceKind::T => [(0, 0), (1, 0), (-1, 0), (0, 1)],
            PieceKind::J => [(0, 0), (1, 0), (-1, 0), (-1, 1)],
            PieceKind::L => [(0, 0), (1, 0), (-1, 0), (1, 1)],
            PieceKind::S => [(0, 0), (-1, 0), (1, 1), (0, 1)],
            PieceKind::Z => [(0, 0), (1, 0), (-1, 1), (0, 1)],
            PieceKind::I => [(0, 0), (1, 0), (-1, 0), (-2, 0)],
            PieceKind::O => [(0, 0), (1, 0), (1, 1), (0, 1)],
        }
    }
}

/// The piece that is currently falling.
///
/// Because this is `Copy`, the movement methods below can take `self` by
/// value and return a brand new `Piece`, leaving the original untouched. That
/// replaces the `dxPrime`/`dyPrime` "scratch" arrays in the Go code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    pub x: i32,
    pub y: i32,
    offsets: [(i32, i32); NUM_SQUARES],
}

impl Piece {
    /// A new piece of the given kind at the spawn position (top center).
    fn spawn(kind: PieceKind) -> Piece {
        Piece {
            kind,
            // `as` converts between numeric types. `usize -> i32` is fine
            // here because the board is tiny.
            x: (BOARD_WIDTH / 2) as i32,
            y: 0,
            offsets: kind.offsets(),
        }
    }

    /// The absolute `(x, y)` position of each of the four squares.
    ///
    /// Returning `impl Iterator` lets callers use `for`, `.any()`, `.all()`,
    /// etc. without us allocating a `Vec`. The `+ '_` lifetime says the
    /// iterator borrows `self` and cannot outlive it.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.offsets
            .iter()
            .map(move |&(dx, dy)| (self.x + dx, self.y + dy))
    }

    /// A copy of this piece shifted by `(dx, dy)`.
    fn moved(self, dx: i32, dy: i32) -> Piece {
        // Struct update syntax: copy every field from `self` except the ones
        // listed explicitly.
        Piece {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    /// A copy of this piece rotated 90 degrees: `(dx, dy)` becomes `(dy, -dx)`.
    fn rotated(self) -> Piece {
        Piece {
            offsets: self.offsets.map(|(dx, dy)| (dy, -dx)),
            ..self
        }
    }
}

/// Where we are in the life cycle of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Intro,
    Started,
    Paused,
    Over,
}

/// One row of locked cells. `None` is empty; `Some(kind)` tells the view what
/// color to draw. Using `Option` instead of a sentinel value like `0` means
/// the compiler forces us to handle the empty case.
type Row = [Option<PieceKind>; BOARD_WIDTH];
type Board = [Row; BOARD_HEIGHT];

const EMPTY_ROW: Row = [None; BOARD_WIDTH];
const EMPTY_BOARD: Board = [EMPTY_ROW; BOARD_HEIGHT];

/// All of the game state. Indexed as `board[y][x]`, like the Go version.
pub struct Game {
    board: Board,
    state: GameState,
    num_lines: u32,
    piece: Option<Piece>,
    /// When the piece should next fall on its own. `None` means "the timer is
    /// stopped" (intro, paused, or game over); this replaces Go's
    /// `time.Timer` plus `Stop()`.
    next_fall: Option<Instant>,
    /// Owning the random number generator (instead of using a global one)
    /// lets tests seed it for a repeatable sequence of pieces.
    rng: StdRng,
}

impl Game {
    /// A new game seeded from the operating system's entropy.
    pub fn new() -> Game {
        Game::from_rng(rand::make_rng())
    }

    /// A new game with a fixed seed, so the sequence of pieces is repeatable.
    ///
    /// `#[cfg(test)]` compiles this only for `cargo test`. Without it, the
    /// compiler would warn that the function is never used in the real binary.
    #[cfg(test)]
    pub fn with_seed(seed: u64) -> Game {
        // A `use` can be scoped to a function. Doing it here keeps the trait
        // import test-only too, so the real build has no unused-import warning.
        use rand::SeedableRng;
        Game::from_rng(StdRng::seed_from_u64(seed))
    }

    fn from_rng(rng: StdRng) -> Game {
        Game {
            board: EMPTY_BOARD,
            state: GameState::Intro,
            num_lines: 0,
            piece: None,
            next_fall: None,
            rng,
        }
    }

    /// Reset everything except the random number generator, to play again.
    fn reset(&mut self) {
        self.board = EMPTY_BOARD;
        self.state = GameState::Intro;
        self.num_lines = 0;
        self.piece = None;
        self.next_fall = None;
    }

    // ----- Read-only accessors used by the view -----------------------------

    pub fn state(&self) -> GameState {
        self.state
    }

    pub fn num_lines(&self) -> u32 {
        self.num_lines
    }

    /// The level is derived from the number of lines cleared rather than
    /// stored, so it can never get out of sync.
    pub fn level(&self) -> u32 {
        (DEFAULT_LEVEL + self.num_lines / ROWS_PER_LEVEL).min(MAX_LEVEL)
    }

    /// How long a piece rests before falling one row.
    pub fn speed(&self) -> Duration {
        // `Duration` subtraction panics on underflow; `saturating_sub` clamps
        // to zero instead. It can't underflow with today's constants, but
        // clippy (rightly) prefers the version that can't panic.
        SLOWEST_SPEED.saturating_sub(SPEED_STEP * self.level())
    }

    /// When the next automatic fall is due, if the timer is running.
    pub fn next_fall(&self) -> Option<Instant> {
        self.next_fall
    }

    /// The currently falling piece, if any.
    pub fn piece(&self) -> Option<&Piece> {
        // `as_ref` turns `&Option<Piece>` into `Option<&Piece>` so the caller
        // borrows the piece instead of copying it out.
        self.piece.as_ref()
    }

    /// What is visible at `(x, y)`: a locked cell, a square of the falling
    /// piece, or nothing.
    pub fn cell(&self, x: usize, y: usize) -> Option<PieceKind> {
        let on_piece = self
            .piece
            .filter(|p| p.cells().any(|(px, py)| px == x as i32 && py == y as i32))
            .map(|p| p.kind);
        // `or` returns the first `Some` of the two.
        on_piece.or(self.board[y][x])
    }

    /// The squares the falling piece would occupy if it dropped straight
    /// down, excluding squares it already covers. The view draws these as a
    /// "ghost" so the player can see where the piece will land.
    pub fn ghost_cells(&self) -> Vec<(i32, i32)> {
        // `let ... else` is an early return for when a pattern doesn't match.
        let Some(piece) = self.piece else {
            return Vec::new();
        };
        let mut ghost = piece;
        while self.fits(ghost.moved(0, 1)) {
            ghost = ghost.moved(0, 1);
        }
        ghost
            .cells()
            .filter(|cell| !piece.cells().any(|c| c == *cell))
            .collect()
    }

    // ----- Player input -----------------------------------------------------

    /// The 's' key: start, resume, or restart depending on the state.
    pub fn start(&mut self) {
        match self.state {
            GameState::Started => {}
            GameState::Paused => self.resume(),
            GameState::Over => {
                self.reset();
                self.begin();
            }
            GameState::Intro => self.begin(),
        }
    }

    fn begin(&mut self) {
        self.state = GameState::Started;
        if !self.spawn_piece() {
            self.state = GameState::Over;
        }
    }

    /// The 'p' key toggles pause.
    pub fn pause(&mut self) {
        match self.state {
            GameState::Started => {
                self.state = GameState::Paused;
                self.next_fall = None;
            }
            GameState::Paused => self.resume(),
            GameState::Intro | GameState::Over => {}
        }
    }

    fn resume(&mut self) {
        self.state = GameState::Started;
        self.reset_falling_timer();
    }

    pub fn move_left(&mut self) {
        self.try_move(|p| p.moved(-1, 0));
    }

    pub fn move_right(&mut self) {
        self.try_move(|p| p.moved(1, 0));
    }

    pub fn rotate(&mut self) {
        self.try_move(Piece::rotated);
    }

    /// Move the piece down one row. Returns whether it moved.
    pub fn move_down(&mut self) -> bool {
        self.try_move(|p| p.moved(0, 1))
    }

    /// The space bar: drop the piece to the bottom and lock it immediately.
    pub fn fall(&mut self) {
        if self.state != GameState::Started {
            return;
        }
        while self.move_down() {}
        self.lock_piece();
    }

    /// Apply `transform` to the falling piece and keep the result if it fits.
    ///
    /// This takes a *generic* function parameter: `F` can be a closure or a
    /// plain function like `Piece::rotated`, and the compiler generates a
    /// specialized copy of `try_move` for each one (no dynamic dispatch).
    fn try_move<F>(&mut self, transform: F) -> bool
    where
        F: Fn(Piece) -> Piece,
    {
        if self.state != GameState::Started {
            return false;
        }
        let Some(piece) = self.piece else {
            return false;
        };
        let candidate = transform(piece);
        if self.fits(candidate) {
            self.piece = Some(candidate);
            true
        } else {
            false
        }
    }

    // ----- Timer ------------------------------------------------------------

    /// Called by the controller when `next_fall` has passed. Moves the piece
    /// down or, if it can't move, locks it in place.
    pub fn tick(&mut self) {
        if self.state != GameState::Started {
            return;
        }
        if self.move_down() {
            self.reset_falling_timer();
        } else {
            self.lock_piece();
        }
    }

    fn reset_falling_timer(&mut self) {
        self.next_fall = Some(Instant::now() + self.speed());
    }

    // ----- Rules ------------------------------------------------------------

    /// Whether every square of `piece` is inside the walls and above the
    /// floor, and not on top of a locked cell. Squares above the top of the
    /// board (negative `y`) are allowed, which can happen right after a
    /// rotation at spawn.
    fn fits(&self, piece: Piece) -> bool {
        piece.cells().all(|(x, y)| {
            let in_walls = (0..BOARD_WIDTH as i32).contains(&x);
            let above_floor = y < BOARD_HEIGHT as i32;
            // `&&` short-circuits, so the board is only indexed once we know
            // `x` and `y` are in range (and `y` is not above the board).
            in_walls && above_floor && (y < 0 || self.board[y as usize][x as usize].is_none())
        })
    }

    /// Copy the falling piece into the board, clear lines, and spawn the
    /// next piece (or end the game).
    fn lock_piece(&mut self) {
        // `take()` moves the value out of the `Option`, leaving `None`.
        if let Some(piece) = self.piece.take() {
            self.fill_board(piece);
        }
        self.remove_lines();
        let top_row_blocked = self.board[0].iter().any(Option::is_some);
        if top_row_blocked || !self.spawn_piece() {
            self.state = GameState::Over;
            self.next_fall = None;
        }
    }

    fn fill_board(&mut self, piece: Piece) {
        for (x, y) in piece.cells() {
            // `usize::try_from` fails for negative numbers, so this single
            // `if let` handles both "above the board" and bounds checking.
            if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y))
                && y < BOARD_HEIGHT
                && x < BOARD_WIDTH
            {
                self.board[y][x] = Some(piece.kind);
            }
        }
    }

    /// Remove every full row, shifting the rows above it down.
    fn remove_lines(&mut self) {
        for y in 0..BOARD_HEIGHT {
            if self.board[y].iter().all(Option::is_some) {
                // Rows are `Copy`, so this is a plain array-element copy.
                for k in (1..=y).rev() {
                    self.board[k] = self.board[k - 1];
                }
                self.board[0] = EMPTY_ROW;
                self.num_lines += 1;
            }
        }
    }

    /// Pick a random piece and place it at the top. Returns `false` if it
    /// doesn't fit, which means the game is over.
    fn spawn_piece(&mut self) -> bool {
        let index = self.rng.random_range(0..PieceKind::ALL.len());
        let piece = Piece::spawn(PieceKind::ALL[index]);
        if !self.fits(piece) {
            return false;
        }
        self.piece = Some(piece);
        self.reset_falling_timer();
        true
    }
}

/// `Default` is the conventional trait for "give me a reasonable empty
/// value". Clippy asks for it whenever a type has a zero-argument `new()`.
impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}

/// Unit tests live in the same file as the code, inside a module that is only
/// compiled for `cargo test`. Because they are a child module, they can call
/// private functions and read private fields, which makes setting up board
/// positions easy.
#[cfg(test)]
mod tests {
    use super::*;

    /// A started game with a known piece, ignoring whatever the RNG chose.
    fn game_with(kind: PieceKind) -> Game {
        let mut game = Game::with_seed(1);
        game.start();
        game.piece = Some(Piece::spawn(kind));
        game
    }

    fn fill_row(game: &mut Game, y: usize, except: &[usize]) {
        for x in 0..BOARD_WIDTH {
            if !except.contains(&x) {
                game.board[y][x] = Some(PieceKind::O);
            }
        }
    }

    #[test]
    fn new_game_is_in_intro_state() {
        let game = Game::with_seed(1);
        assert_eq!(game.state(), GameState::Intro);
        assert_eq!(game.level(), 1);
        assert_eq!(game.num_lines(), 0);
        assert!(game.piece().is_none());
        assert!(game.next_fall().is_none());
    }

    #[test]
    fn start_spawns_a_piece_and_starts_the_timer() {
        let mut game = Game::with_seed(1);
        game.start();
        assert_eq!(game.state(), GameState::Started);
        assert!(game.piece().is_some());
        assert!(game.next_fall().is_some());
    }

    #[test]
    fn same_seed_gives_same_pieces() {
        let mut a = Game::with_seed(42);
        let mut b = Game::with_seed(42);
        a.start();
        b.start();
        assert_eq!(a.piece(), b.piece());
    }

    #[test]
    fn pause_stops_the_timer_and_resume_restarts_it() {
        let mut game = Game::with_seed(1);
        game.start();
        game.pause();
        assert_eq!(game.state(), GameState::Paused);
        assert!(game.next_fall().is_none());
        game.pause();
        assert_eq!(game.state(), GameState::Started);
        assert!(game.next_fall().is_some());
        game.pause();
        game.start();
        assert_eq!(game.state(), GameState::Started);
    }

    #[test]
    fn input_is_ignored_before_the_game_starts() {
        let mut game = Game::with_seed(1);
        game.move_left();
        game.rotate();
        game.fall();
        assert!(!game.move_down());
        assert_eq!(game.state(), GameState::Intro);
        assert!(game.piece().is_none());
    }

    #[test]
    fn pieces_stop_at_the_walls() {
        let mut game = game_with(PieceKind::O);
        for _ in 0..BOARD_WIDTH {
            game.move_left();
        }
        let min_x = game.piece().unwrap().cells().map(|(x, _)| x).min().unwrap();
        assert_eq!(min_x, 0);

        for _ in 0..BOARD_WIDTH {
            game.move_right();
        }
        let max_x = game.piece().unwrap().cells().map(|(x, _)| x).max().unwrap();
        assert_eq!(max_x, BOARD_WIDTH as i32 - 1);
    }

    #[test]
    fn rotating_four_times_returns_to_the_start() {
        let mut game = game_with(PieceKind::T);
        game.move_down(); // Make room above so every rotation fits.
        let original = *game.piece().unwrap();
        game.rotate();
        assert_ne!(*game.piece().unwrap(), original);
        for _ in 0..3 {
            game.rotate();
        }
        assert_eq!(*game.piece().unwrap(), original);
    }

    #[test]
    fn rotation_is_blocked_by_a_wall() {
        // A vertical I piece against the left wall cannot rotate back to
        // horizontal, because the horizontal shape extends 2 to the left.
        let mut game = game_with(PieceKind::I);
        game.move_down();
        game.move_down();
        game.rotate(); // Now vertical.
        for _ in 0..BOARD_WIDTH {
            game.move_left();
        }
        let before = *game.piece().unwrap();
        game.rotate();
        assert_eq!(*game.piece().unwrap(), before);
    }

    #[test]
    fn move_down_returns_false_at_the_floor() {
        let mut game = game_with(PieceKind::O);
        let mut moves = 0;
        while game.move_down() {
            moves += 1;
        }
        // The O piece is 2 tall and spawns at y = 0, so it can drop 14 rows.
        assert_eq!(moves, BOARD_HEIGHT - 2);
        assert!(!game.move_down());
    }

    #[test]
    fn fall_locks_the_piece_and_spawns_a_new_one() {
        let mut game = game_with(PieceKind::O);
        game.fall();
        let locked: usize = game
            .board
            .iter()
            .flatten()
            .filter(|cell| cell.is_some())
            .count();
        assert_eq!(locked, NUM_SQUARES);
        assert_eq!(game.board[BOARD_HEIGHT - 1][5], Some(PieceKind::O));
        assert_eq!(game.board[BOARD_HEIGHT - 2][6], Some(PieceKind::O));
        assert!(game.piece().is_some(), "a new piece should have spawned");
        assert_eq!(game.piece().unwrap().y, 0);
        assert_eq!(game.state(), GameState::Started);
    }

    #[test]
    fn tick_moves_the_piece_down_one_row() {
        let mut game = game_with(PieceKind::O);
        game.tick();
        assert_eq!(game.piece().unwrap().y, 1);
    }

    #[test]
    fn tick_locks_a_piece_that_cannot_move() {
        let mut game = game_with(PieceKind::O);
        while game.move_down() {}
        game.tick();
        assert_eq!(game.board[BOARD_HEIGHT - 1][5], Some(PieceKind::O));
        assert_eq!(game.piece().unwrap().y, 0, "a fresh piece spawned");
    }

    #[test]
    fn completing_a_row_removes_it() {
        let mut game = game_with(PieceKind::O);
        let bottom = BOARD_HEIGHT - 1;
        // Leave a 2-wide gap where the O piece will land (x = 5 and 6).
        fill_row(&mut game, bottom, &[5, 6]);
        fill_row(&mut game, bottom - 1, &[5, 6]);
        game.fall();
        assert_eq!(game.num_lines(), 2);
        assert!(game.board.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn rows_above_a_removed_row_shift_down() {
        let mut game = game_with(PieceKind::O);
        let bottom = BOARD_HEIGHT - 1;
        fill_row(&mut game, bottom, &[5, 6]);
        // A lone cell on the row above should drop to the bottom.
        game.board[bottom - 1][0] = Some(PieceKind::I);
        game.fall();
        assert_eq!(game.num_lines(), 1);
        assert_eq!(game.board[bottom][0], Some(PieceKind::I));
        // The other half of the O piece survives.
        assert_eq!(game.board[bottom][5], Some(PieceKind::O));
        assert_eq!(game.board[bottom][6], Some(PieceKind::O));
        assert_eq!(game.board[bottom - 1][0], None);
    }

    #[test]
    fn level_rises_every_five_lines_and_caps_at_ten() {
        let mut game = Game::with_seed(1);
        assert_eq!(game.level(), 1);
        game.num_lines = 4;
        assert_eq!(game.level(), 1);
        game.num_lines = 5;
        assert_eq!(game.level(), 2);
        game.num_lines = 45;
        assert_eq!(game.level(), 10);
        game.num_lines = 500;
        assert_eq!(game.level(), 10);
    }

    #[test]
    fn speed_gets_faster_with_level() {
        let mut game = Game::with_seed(1);
        assert_eq!(game.speed(), Duration::from_millis(640));
        game.num_lines = 45;
        assert_eq!(game.speed(), Duration::from_millis(100));
    }

    #[test]
    fn game_is_over_when_the_stack_reaches_the_top() {
        let mut game = game_with(PieceKind::O);
        // Fill every row below the spawning piece, leaving column 0 open so
        // that no row completes. The O piece then locks in rows 0 and 1.
        for y in 2..BOARD_HEIGHT {
            fill_row(&mut game, y, &[0]);
        }
        game.fall();
        assert_eq!(game.num_lines(), 0);
        assert_eq!(game.state(), GameState::Over);
        assert!(game.next_fall().is_none());
    }

    #[test]
    fn starting_after_game_over_resets_everything() {
        let mut game = game_with(PieceKind::O);
        game.num_lines = 7;
        game.state = GameState::Over;
        game.start();
        assert_eq!(game.state(), GameState::Started);
        assert_eq!(game.num_lines(), 0);
        assert!(game.piece().is_some());
    }

    #[test]
    fn cell_shows_the_falling_piece_over_the_board() {
        let game = game_with(PieceKind::O);
        // O piece at (5, 0) covers (5,0) (6,0) (6,1) (5,1).
        assert_eq!(game.cell(5, 0), Some(PieceKind::O));
        assert_eq!(game.cell(6, 1), Some(PieceKind::O));
        assert_eq!(game.cell(4, 0), None);
        assert_eq!(game.cell(0, BOARD_HEIGHT - 1), None);
    }

    #[test]
    fn ghost_cells_show_where_the_piece_will_land() {
        let game = game_with(PieceKind::O);
        let mut ghost = game.ghost_cells();
        ghost.sort_unstable();
        let bottom = BOARD_HEIGHT as i32 - 1;
        assert_eq!(
            ghost,
            vec![(5, bottom - 1), (5, bottom), (6, bottom - 1), (6, bottom)]
        );
    }

    #[test]
    fn ghost_cells_exclude_squares_the_piece_already_covers() {
        let mut game = game_with(PieceKind::O);
        while game.move_down() {}
        assert!(game.ghost_cells().is_empty());
    }
}
