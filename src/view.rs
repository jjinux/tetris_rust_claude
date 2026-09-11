//! The view: draws a `Game` onto the terminal with crossterm.
//!
//! crossterm works by writing ANSI escape sequences to a `Write` (normally
//! stdout). `queue!` appends commands to the writer's buffer; nothing is sent
//! until we `flush()`.
//!
//! Unlike termbox in the Go version, crossterm has no back buffer: it does not
//! diff frames and send only the changed cells, it sends exactly what we
//! queue. To avoid flicker we therefore (1) paint the background only once, in
//! `clear`, rather than every frame, (2) let the controller skip frames when
//! nothing changed, and (3) wrap each frame in a "synchronized update" so
//! terminals that support it paint the frame atomically.

use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, BeginSynchronizedUpdate, EndSynchronizedUpdate};

use crate::model::{BOARD_HEIGHT, BOARD_WIDTH, Game, GameState, PieceKind};

// Colors
const BACKGROUND_COLOR: Color = Color::Blue;
const BOARD_COLOR: Color = Color::Black;
const TEXT_COLOR: Color = Color::Yellow;
const GHOST_COLOR: Color = Color::Black;

// Layout. Terminal coordinates in crossterm are `u16`, so the constants are
// `u16` too and we only convert at the edges.
const MARGIN_WIDTH: u16 = 2;
const MARGIN_HEIGHT: u16 = 1;
const TITLE_X: u16 = MARGIN_WIDTH;
const TITLE_Y: u16 = MARGIN_HEIGHT;
const TITLE_HEIGHT: u16 = 1;
const BOARD_X: u16 = MARGIN_WIDTH;
const BOARD_Y: u16 = TITLE_Y + TITLE_HEIGHT + MARGIN_HEIGHT;
/// Each cell is two characters wide so the board looks roughly square.
const CELL_WIDTH: u16 = 2;
const BOARD_END_X: u16 = BOARD_X + BOARD_WIDTH as u16 * CELL_WIDTH;
const INSTRUCTIONS_X: u16 = BOARD_END_X + MARGIN_WIDTH;
const INSTRUCTIONS_Y: u16 = BOARD_Y;

const TITLE: &str = "TETRIS IN RUST (BY CLAUDE CODE)";

const INSTRUCTIONS: [&str; 10] = [
    "Goal: Fill in 5 lines!",
    "",
    "left   Left",
    "right  Right",
    "up     Rotate",
    "down   Down",
    "space  Fall",
    "s      Start",
    "p      Pause",
    "esc,q  Exit",
];

/// The color used to draw a locked or falling square of the given kind.
fn piece_color(kind: PieceKind) -> Color {
    match kind {
        PieceKind::T => Color::Red,
        PieceKind::J => Color::Green,
        PieceKind::L => Color::Yellow,
        PieceKind::S => Color::Blue,
        PieceKind::Z => Color::Magenta,
        PieceKind::I => Color::Cyan,
        PieceKind::O => Color::White,
    }
}

/// Paint the whole terminal in the background color and draw the title.
///
/// Call this once at startup and again whenever the terminal is resized. It is
/// deliberately *not* part of `render`: repainting everything every frame is
/// what makes a terminal flicker.
///
/// We paint every row explicitly instead of using crossterm's `Clear` because
/// not all terminals fill cleared cells with the current background color.
pub fn clear(out: &mut impl Write) -> io::Result<()> {
    let (columns, rows) = terminal::size()?;
    let blank_row = " ".repeat(usize::from(columns));
    queue!(
        out,
        BeginSynchronizedUpdate,
        SetBackgroundColor(BACKGROUND_COLOR)
    )?;
    for row in 0..rows {
        queue!(out, MoveTo(0, row), Print(&blank_row))?;
    }
    print_at(out, TITLE_X, TITLE_Y, TEXT_COLOR, BACKGROUND_COLOR, TITLE)?;
    queue!(out, EndSynchronizedUpdate)?;
    out.flush()
}

/// Draw one frame: everything that can change from one moment to the next.
///
/// Every cell this touches is overwritten with an explicit color, so there is
/// no need to erase first. `out` is generic over anything that implements
/// `Write`. In the game it is stdout; a test could pass a `Vec<u8>` and inspect
/// the bytes.
pub fn render(out: &mut impl Write, game: &Game) -> io::Result<()> {
    queue!(out, BeginSynchronizedUpdate)?;
    draw_board(out, game)?;
    draw_ghost(out, game)?;
    draw_status(out, game)?;
    queue!(out, EndSynchronizedUpdate)?;
    out.flush()
}

fn draw_board(out: &mut impl Write, game: &Game) -> io::Result<()> {
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            // `map_or` handles both halves of the `Option` in one expression:
            // the default for `None`, and a function to apply to `Some`.
            let color = game.cell(x, y).map_or(BOARD_COLOR, piece_color);
            draw_cell(out, x, y, color, ' ')?;
        }
    }
    Ok(())
}

/// Show where the falling piece will land, as `*`s in the piece's color.
fn draw_ghost(out: &mut impl Write, game: &Game) -> io::Result<()> {
    let Some(piece) = game.piece() else {
        return Ok(());
    };
    let color = piece_color(piece.kind);
    for (x, y) in game.ghost_cells() {
        // Ghost cells can only be on the board, but `try_from` keeps the
        // conversion honest instead of `as`-casting a possibly negative value.
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            draw_cell(out, x, y, color, '*')?;
        }
    }
    Ok(())
}

/// Draw one board cell (two characters wide) at board coordinates `(x, y)`.
fn draw_cell(out: &mut impl Write, x: usize, y: usize, color: Color, ch: char) -> io::Result<()> {
    let screen_x = BOARD_X + x as u16 * CELL_WIDTH;
    let screen_y = BOARD_Y + y as u16;
    let text: String = std::iter::repeat_n(ch, usize::from(CELL_WIDTH)).collect();
    print_at(out, screen_x, screen_y, GHOST_COLOR, color, &text)
}

/// Draw the key legend, the level and line counters, and "GAME OVER!".
fn draw_status(out: &mut impl Write, game: &Game) -> io::Result<()> {
    // `enumerate()` pairs each item with its index, like Go's `for i, v := range`.
    for (i, line) in INSTRUCTIONS.iter().enumerate() {
        print_at(
            out,
            INSTRUCTIONS_X,
            INSTRUCTIONS_Y + i as u16,
            TEXT_COLOR,
            BACKGROUND_COLOR,
            line,
        )?;
    }
    let status_y = INSTRUCTIONS_Y + INSTRUCTIONS.len() as u16 + 1;
    // Because nothing erases the screen between frames, each line is padded
    // to a fixed width (`{:<12}` left-aligns in 12 columns). Otherwise
    // "Lines: 12" followed by "Lines: 0" after a restart would leave a stray
    // "2" on screen.
    let level = format!("{:<12}", format!("Level: {}", game.level()));
    let lines = format!("{:<12}", format!("Lines: {}", game.num_lines()));
    let game_over = if game.state() == GameState::Over {
        "GAME OVER!"
    } else {
        "          "
    };
    print_at(
        out,
        INSTRUCTIONS_X,
        status_y,
        TEXT_COLOR,
        BACKGROUND_COLOR,
        &level,
    )?;
    print_at(
        out,
        INSTRUCTIONS_X,
        status_y + 1,
        TEXT_COLOR,
        BACKGROUND_COLOR,
        &lines,
    )?;
    print_at(
        out,
        INSTRUCTIONS_X,
        status_y + 3,
        TEXT_COLOR,
        BACKGROUND_COLOR,
        game_over,
    )
}

/// Write `text` at screen position `(x, y)` in the given colors.
fn print_at(
    out: &mut impl Write,
    x: u16,
    y: u16,
    fg: Color,
    bg: Color,
    text: &str,
) -> io::Result<()> {
    queue!(
        out,
        MoveTo(x, y),
        SetForegroundColor(fg),
        SetBackgroundColor(bg),
        Print(text)
    )
}
