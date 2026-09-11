# Tetris in Rust (by Claude Code)

A console-based version of Tetris written in Rust.

![Screen shot](screen_shot.png)

This project is a port of [gotetris](https://github.com/jjinux/gotetris), a
version of Tetris that I ([@jjinux](https://github.com/jjinux)) wrote in Go.
[Claude Code](https://claude.com/claude-code) did the port under my direction
as an exercise to help me learn both Rust and Claude Code. The code is written
the way an intermediate Rust programmer would write it, and comments call out
Rust constructs that are worth knowing about (RAII guards, `Option` instead of
sentinel values, `impl Iterator`, let chains, and so on).

## Requirements

- A Rust toolchain (install with [rustup](https://rustup.rs)). The project uses
  the 2024 edition.
- Optionally [`just`](https://github.com/casey/just) for the task-runner
  recipes (`brew install just`). Everything it runs is plain `cargo`, so you
  can also type those commands directly.

## Install and run

    git clone https://github.com/jjinux/tetris_rust_claude.git
    cd tetris_rust_claude
    cargo run --release

Or, with `just`:

    just run

## Controls

| Key            | Action                          |
| -------------- | ------------------------------- |
| left / right   | Move the piece                  |
| up             | Rotate                          |
| down           | Move down one row               |
| space          | Drop the piece and lock it      |
| s              | Start (or restart after a loss) |
| p              | Pause / resume                  |
| q, esc, ctrl-c | Quit                            |

Clear five lines to advance a level. Each level makes the pieces fall faster,
up to level 10. The `*` cells show where the current piece will land.

## Working on the code

    just            # list recipes
    just test       # cargo test
    just lint       # cargo clippy --all-targets -- -D warnings
    just fmt        # cargo fmt
    just check      # fmt-check + lint + test (what CI runs)

The code follows the same loose MVC split as the Go original:

- `src/model.rs`: the game state and rules. No terminal code, so it is unit
  tested in the same file.
- `src/view.rs`: draws a `Game` with [crossterm](https://github.com/crossterm-rs/crossterm).
- `src/main.rs`: the controller. Sets up the terminal and runs the event loop.

`TODO.md` is the running plan for the project and `CLAUDE.md` holds the notes
Claude Code works from.

## Credits

- The Go original, [gotetris](https://github.com/jjinux/gotetris), was built on
  [termbox-go](https://github.com/nsf/termbox-go). This port uses
  [crossterm](https://github.com/crossterm-rs/crossterm) instead.
- gotetris was inspired by [Alexei Kourbatov](http://www.javascripter.net) and
  by my earlier port of Tetris to Dart.

## License

MIT. See [LICENSE](LICENSE).
