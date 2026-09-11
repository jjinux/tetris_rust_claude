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

use std::io::{self, Write};
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::model::{Game, GameState};

/// How long to wait for a key when no fall is scheduled (intro, pause, over).
const IDLE_TIMEOUT: Duration = Duration::from_millis(100);

/// Puts the terminal into "game mode" and guarantees it is put back.
///
/// This is the RAII pattern (Go would use `defer`). Creating the guard sets up
/// the terminal, and its `Drop` impl tears it down when the guard goes out of
/// scope, whether `main` returns normally, returns an error via `?`, or
/// unwinds from a panic.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<TerminalGuard> {
        // Raw mode: keys reach us immediately, one at a time, without echo,
        // and Ctrl-C is delivered as a key instead of killing the process.
        enable_raw_mode()?;
        // The alternate screen is a second buffer; leaving it restores
        // whatever the user had in the terminal before the game.
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;

        // If we panic, Rust prints the message *before* unwinding runs our
        // `Drop`, so it would land on the alternate screen and vanish. This
        // hook restores the terminal first, then runs the default printer.
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            default_hook(info);
        }));

        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Errors are ignored here on purpose: this runs during cleanup, and there is
/// nothing useful left to do if restoring the terminal fails.
fn restore_terminal() {
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// `main` can return a `Result`. On `Err`, Rust prints the error (via `Debug`)
/// and exits with a non-zero status, so `?` works all the way up here.
fn main() -> io::Result<()> {
    let _guard = TerminalGuard::new()?;
    // Buffer everything so each frame is written in one system call. The
    // default 8 KiB buffer is smaller than a frame, which would split it and
    // let the terminal paint a half-drawn screen.
    let mut out = io::BufWriter::with_capacity(64 * 1024, io::stdout());
    let mut game = Game::new();

    view::clear(&mut out)?;
    // Only redraw when something changed. Redrawing every time `poll` wakes
    // up would be wasted work and, on many terminals, visible flicker.
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
            view::render(&mut out, &game)?;
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
                        break;
                    }
                    dirty = true;
                }
                // The terminal was resized, so the background needs
                // repainting before the next frame. If it is now too small
                // to show the board, pause so the player doesn't lose a
                // piece they can't see.
                Event::Resize(columns, rows) => {
                    if !view::fits(columns, rows) && game.state() == GameState::Started {
                        game.pause();
                    }
                    view::clear(&mut out)?;
                    dirty = true;
                }
                _ => {}
            }
        }
    }
    out.flush()
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
