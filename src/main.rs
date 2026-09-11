//! Tetris in Rust (by Claude Code): a console-based Tetris.
//!
//! This is a port of <https://github.com/jjinux/gotetris>. Like the original
//! it is loosely MVC: `model` holds the rules, `view` draws, and this file is
//! the controller that turns key presses and the clock into calls on the model.
//!
//! The Go version runs a goroutine that blocks on `termbox.PollEvent()` and
//! feeds a channel, then `select`s between that channel and a timer. Rust has
//! threads and channels too, but we don't need them: `crossterm::event::poll`
//! takes a timeout, so a single thread can wait for "a key press *or* the
//! next fall tick, whichever comes first".

mod model;
mod view;

use std::io;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::DefaultTerminal;

use crate::model::{Game, GameState};

/// How long to wait for a key when no fall is scheduled (intro, pause, over).
const IDLE_TIMEOUT: Duration = Duration::from_millis(100);

/// `main` can return a `Result`. On `Err`, Rust prints the error (via `Debug`)
/// and exits with a non-zero status, so `?` works all the way up here.
fn main() -> io::Result<()> {
    // `ratatui::try_init` enables raw mode, switches to the alternate screen,
    // and installs a panic hook that restores the terminal before the panic
    // message is printed. `restore` undoes it. This is ratatui's recommended
    // shape: run the real program in a separate function so that `restore`
    // runs whether `run` returned `Ok` or `Err`.
    let mut terminal = ratatui::try_init()?;
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

/// The event loop.
fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut game = Game::new();
    // Only redraw when something changed. ratatui would send nothing for an
    // identical frame anyway, but skipping the work entirely is free.
    let mut dirty = true;
    loop {
        // Fire the fall timer if it is due.
        if game.next_fall().is_some_and(|due| Instant::now() >= due) {
            game.tick();
            dirty = true;
        }

        // Draw *before* waiting for input, so a tick or key press shows up
        // immediately rather than after the next wait finishes.
        if dirty {
            draw(terminal, &game)?;
            dirty = false;
        }

        // Wait for a key until the next fall is due (or a short idle timeout
        // when nothing is falling). `saturating_duration_since` clamps to
        // zero instead of panicking if the deadline is already in the past.
        let timeout = game.next_fall().map_or(IDLE_TIMEOUT, |due| {
            due.saturating_duration_since(Instant::now())
        });
        if event::poll(timeout)? {
            match event::read()? {
                // Only key *presses* are handled; some platforms also report
                // releases.
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if handle_key(&mut game, key).is_break() {
                        return Ok(());
                    }
                    dirty = true;
                }
                // ratatui notices the new size on the next draw. If the
                // window is now too small to show the board, pause so the
                // player doesn't lose a piece they can't see.
                Event::Resize(columns, rows) => {
                    if !view::fits(columns, rows) && game.state() == GameState::Started {
                        game.pause();
                    }
                    dirty = true;
                }
                _ => {}
            }
        }
    }
}

/// Render one frame.
///
/// `Terminal::draw` gives the closure a `Frame` to paint into, then diffs it
/// against the previous frame and writes only the changed cells. The
/// synchronized-update markers around it ask terminals that support them to
/// paint those changes all at once.
fn draw(terminal: &mut DefaultTerminal, game: &Game) -> io::Result<()> {
    execute!(io::stdout(), BeginSynchronizedUpdate)?;
    terminal.draw(|frame| view::render(frame, game))?;
    execute!(io::stdout(), EndSynchronizedUpdate)
}

/// Dispatch one key press to the model. Returns `ControlFlow::Break` to quit.
///
/// `ControlFlow` is a small std enum (`Continue(C)` / `Break(B)`) that reads
/// better than a bare `bool` for "should the loop keep going?".
fn handle_key(game: &mut Game, key: KeyEvent) -> ControlFlow<()> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // Matching on a tuple lets us look at the key and the modifiers together.
    // `|` inside a pattern matches any of the alternatives.
    match (key.code, ctrl) {
        (KeyCode::Left, _) => game.move_left(),
        (KeyCode::Right, _) => game.move_right(),
        (KeyCode::Up, _) => game.rotate(),
        (KeyCode::Down, _) => {
            game.move_down();
        }
        (KeyCode::Char(' '), _) => game.fall(),
        (KeyCode::Char('s'), false) => game.start(),
        (KeyCode::Char('p'), false) => game.pause(),
        (KeyCode::Char('q') | KeyCode::Esc, _) | (KeyCode::Char('c' | 'd'), true) => {
            return ControlFlow::Break(());
        }
        _ => {}
    }
    ControlFlow::Continue(())
}
