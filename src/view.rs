//! The view: draws a `Game` with ratatui.
//!
//! ratatui is a framework built on top of crossterm (and other backends). We
//! describe a whole frame by painting into an in-memory `Buffer`; ratatui keeps
//! the previous frame's buffer too, diffs the two, and writes only the cells
//! that changed to the terminal. That is the same double-buffering trick that
//! termbox used in the Go version, so the view can simply draw *everything*
//! every frame and let the library work out the minimal update.
//!
//! Drawing is organized around the `Widget` trait: anything that can render
//! itself into a rectangle of a `Buffer`. ratatui ships widgets such as
//! `Paragraph` and `Block`; the board is a custom one defined below.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::model::{BOARD_HEIGHT, BOARD_WIDTH, Game, GameState, PieceKind};

// Colors. ratatui's `Color::Blue` is the dark ANSI blue; the bright variants
// are the `Light*` ones, which match the colors the Go version used.
const BACKGROUND_COLOR: Color = Color::LightBlue;
const BOARD_COLOR: Color = Color::Black;
const TEXT_COLOR: Color = Color::LightYellow;
const GHOST_COLOR: Color = Color::Black;

// Layout. Terminal coordinates in ratatui are `u16`, so the constants are
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

/// The smallest terminal the layout fits in: the instructions column plus a
/// margin, and the board plus a margin.
pub const MIN_COLUMNS: u16 = INSTRUCTIONS_X + longest(&INSTRUCTIONS) as u16 + MARGIN_WIDTH;
pub const MIN_ROWS: u16 = BOARD_Y + BOARD_HEIGHT as u16 + MARGIN_HEIGHT;

/// Length of the longest string in `lines`.
///
/// A `const fn` can be evaluated at compile time, so `MIN_COLUMNS` above is a
/// real constant. Const functions can't use iterators or `for` loops yet,
/// hence the manual `while`.
const fn longest(lines: &[&str]) -> usize {
    let mut max = 0;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].len() > max {
            max = lines[i].len();
        }
        i += 1;
    }
    max
}

/// Whether a terminal of the given size is big enough to draw the game.
pub fn fits(columns: u16, rows: u16) -> bool {
    columns >= MIN_COLUMNS && rows >= MIN_ROWS
}

/// The color used to draw a locked or falling square of the given kind.
///
/// These follow the Go version except for S, which was the same blue as the
/// background there.
fn piece_color(kind: PieceKind) -> Color {
    match kind {
        PieceKind::T => Color::LightRed,
        PieceKind::J => Color::LightGreen,
        PieceKind::L => Color::LightYellow,
        PieceKind::S => Color::Yellow,
        PieceKind::Z => Color::LightMagenta,
        PieceKind::I => Color::LightCyan,
        PieceKind::O => Color::White,
    }
}

fn text_style() -> Style {
    Style::new().fg(TEXT_COLOR).bg(BACKGROUND_COLOR)
}

/// Draw one complete frame into `frame`.
///
/// This is called from `Terminal::draw`, which hands us a `Frame` for the
/// whole terminal and, after we return, flushes the changed cells.
pub fn render(frame: &mut Frame, game: &Game) {
    let area = frame.area();

    // A `Block` with no borders and a background style is the ratatui way to
    // paint a solid background.
    frame.render_widget(Block::new().style(Style::new().bg(BACKGROUND_COLOR)), area);

    if !fits(area.width, area.height) {
        frame.render_widget(too_small_message(area), area);
        return;
    }

    frame.render_widget(
        Paragraph::new(TITLE).style(text_style()),
        Rect::new(TITLE_X, TITLE_Y, TITLE.len() as u16, TITLE_HEIGHT),
    );
    frame.render_widget(
        Board { game },
        Rect::new(
            BOARD_X,
            BOARD_Y,
            BOARD_WIDTH as u16 * CELL_WIDTH,
            BOARD_HEIGHT as u16,
        ),
    );
    let status = status_lines(game);
    let status_area = Rect::new(
        INSTRUCTIONS_X,
        INSTRUCTIONS_Y,
        area.width - INSTRUCTIONS_X,
        status.len() as u16,
    );
    frame.render_widget(Paragraph::new(status).style(text_style()), status_area);
}

/// The key legend followed by the level and line counters and, when the game
/// is over, "GAME OVER!".
///
/// The `'static` lifetime on `Line` says these lines own their text (or
/// borrow string literals, which live forever) rather than borrowing from
/// `game`.
fn status_lines(game: &Game) -> Vec<Line<'static>> {
    // `iter().copied()` yields `&str` values instead of `&&str` references.
    let mut lines: Vec<Line> = INSTRUCTIONS.iter().copied().map(Line::from).collect();
    lines.push(Line::from(""));
    lines.push(Line::from(format!("Level: {}", game.level())));
    lines.push(Line::from(format!("Lines: {}", game.num_lines())));
    if game.state() == GameState::Over {
        lines.push(Line::from(""));
        lines.push(Line::from("GAME OVER!"));
    }
    lines
}

/// Tell the player the window is too small instead of drawing a mangled board.
fn too_small_message(area: Rect) -> Paragraph<'static> {
    let lines = vec![
        Line::from("Terminal too small."),
        Line::from(format!(
            "Need at least {MIN_COLUMNS}x{MIN_ROWS}, have {}x{}.",
            area.width, area.height
        )),
        Line::from("Resize the window, or press q to quit."),
    ];
    Paragraph::new(lines).style(text_style())
}

/// The playing field: locked cells, the falling piece, and its ghost.
///
/// A custom widget is just a struct plus an `impl Widget`. It borrows the game
/// for the duration of one frame, which is what the `'a` lifetime expresses:
/// a `Board<'a>` cannot outlive the `&'a Game` it holds.
struct Board<'a> {
    game: &'a Game,
}

impl Widget for Board<'_> {
    /// `render` takes `self` by value: widgets are cheap, throwaway values
    /// built fresh each frame.
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                // `map_or` handles both halves of the `Option` in one
                // expression: the default for `None`, and a function to
                // apply to `Some`.
                let color = self.game.cell(x, y).map_or(BOARD_COLOR, piece_color);
                draw_cell(buf, area, x, y, color, ' ');
            }
        }

        let Some(piece) = self.game.piece() else {
            return;
        };
        let color = piece_color(piece.kind);
        for (x, y) in self.game.ghost_cells() {
            // Ghost cells can only be on the board, but `try_from` keeps the
            // conversion honest instead of `as`-casting a possibly negative
            // value.
            if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
                draw_cell(buf, area, x, y, color, '*');
            }
        }
    }
}

/// Draw one board cell (two characters wide) at board coordinates `(x, y)`.
fn draw_cell(buf: &mut Buffer, area: Rect, x: usize, y: usize, color: Color, ch: char) {
    let screen_x = area.x + x as u16 * CELL_WIDTH;
    let screen_y = area.y + y as u16;
    for i in 0..CELL_WIDTH {
        // `cell_mut` returns `None` for positions outside the buffer, so a
        // too-small buffer is clipped rather than causing a panic.
        if let Some(cell) = buf.cell_mut((screen_x + i, screen_y)) {
            cell.set_char(ch).set_fg(GHOST_COLOR).set_bg(color);
        }
    }
}

/// View tests render into ratatui's `TestBackend`, an in-memory terminal, and
/// then inspect the resulting `Buffer`. No real terminal is involved.
#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn render_to_buffer(game: &Game, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, game)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// The text on one row of the buffer, with trailing spaces removed.
    fn row_text(buf: &Buffer, y: u16) -> String {
        let text: String = (0..buf.area().width)
            .map(|x| buf[(x, y)].symbol())
            .collect();
        text.trim_end().to_string()
    }

    #[test]
    fn longest_finds_the_longest_line() {
        assert_eq!(longest(&["a", "abc", "ab"]), 3);
        assert_eq!(longest(&[]), 0);
    }

    #[test]
    fn minimum_size_covers_the_layout() {
        // Margin 2 + board 10 * 2 + margin 2 = 24 columns, then the legend
        // and a trailing margin.
        assert_eq!(MIN_COLUMNS, 24 + "Goal: Fill in 5 lines!".len() as u16 + 2);
        assert_eq!(MIN_ROWS, 3 + 16 + 1);
        assert!(fits(MIN_COLUMNS, MIN_ROWS));
        assert!(!fits(MIN_COLUMNS - 1, MIN_ROWS));
        assert!(!fits(MIN_COLUMNS, MIN_ROWS - 1));
    }

    #[test]
    fn intro_screen_shows_title_legend_and_empty_board() {
        let game = Game::with_seed(1);
        let buf = render_to_buffer(&game, 60, 24);

        assert_eq!(row_text(&buf, TITLE_Y), format!("  {TITLE}"));
        assert_eq!(
            &row_text(&buf, INSTRUCTIONS_Y)[24..],
            "Goal: Fill in 5 lines!"
        );
        assert_eq!(&row_text(&buf, INSTRUCTIONS_Y + 11)[24..], "Level: 1");
        assert_eq!(&row_text(&buf, INSTRUCTIONS_Y + 12)[24..], "Lines: 0");
        assert!(!row_text(&buf, INSTRUCTIONS_Y + 14).contains("GAME OVER"));

        // Every board cell is black; the margin around it is the background.
        for y in 0..BOARD_HEIGHT as u16 {
            for x in 0..BOARD_WIDTH as u16 * CELL_WIDTH {
                assert_eq!(buf[(BOARD_X + x, BOARD_Y + y)].bg, BOARD_COLOR);
            }
        }
        assert_eq!(buf[(0, 0)].bg, BACKGROUND_COLOR);
        assert_eq!(buf[(BOARD_X - 1, BOARD_Y)].bg, BACKGROUND_COLOR);
        assert_eq!(buf[(BOARD_END_X, BOARD_Y)].bg, BACKGROUND_COLOR);
    }

    #[test]
    fn falling_piece_and_ghost_are_drawn_in_the_piece_color() {
        let mut game = Game::with_seed(1);
        game.start();
        let piece = *game.piece().expect("start spawns a piece");
        let buf = render_to_buffer(&game, 60, 24);

        for (x, y) in piece.cells() {
            let cell = &buf[(BOARD_X + x as u16 * CELL_WIDTH, BOARD_Y + y as u16)];
            assert_eq!(cell.bg, piece_color(piece.kind));
            assert_eq!(cell.symbol(), " ");
        }
        assert!(!game.ghost_cells().is_empty());
        for (x, y) in game.ghost_cells() {
            let cell = &buf[(BOARD_X + x as u16 * CELL_WIDTH, BOARD_Y + y as u16)];
            assert_eq!(cell.bg, piece_color(piece.kind));
            assert_eq!(cell.symbol(), "*");
        }
    }

    #[test]
    fn too_small_terminal_shows_a_message_instead_of_the_board() {
        let game = Game::with_seed(1);
        let buf = render_to_buffer(&game, 40, 15);
        assert_eq!(row_text(&buf, 0), "Terminal too small.");
        assert_eq!(row_text(&buf, 1), "Need at least 48x20, have 40x15.");
        assert_eq!(row_text(&buf, 2), "Resize the window, or press q to quit.");
        // Nothing below the message: no board, no legend.
        assert_eq!(row_text(&buf, BOARD_Y), "");
        assert_eq!(buf[(BOARD_X, BOARD_Y)].bg, BACKGROUND_COLOR);
    }
}
