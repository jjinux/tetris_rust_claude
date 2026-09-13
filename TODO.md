# TODO: Tetris in Rust (by Claude Code)

A port of [gotetris](https://github.com/jjinux/gotetris) (Go) to Rust.
Decisions JJ made are marked "Decision (JJ)".

- [x] Come up with a plan:
  - [x] Decide on a library to use to do the terminal-based graphics.
    - Recommendation: `crossterm`. It is the closest analog to termbox-go: a low-level,
      cross-platform crate for raw mode, alternate screen, colored cells, and key events.
    - Considered `ratatui` (widgets/layout on top of crossterm) but it is heavier than a
      16x10 grid needs and would hide the cell-by-cell drawing the Go version does.
    - Considered `termion` but it is Unix-only and less actively maintained.
  - [x] Is there any reason we should use threads?
    - No. The Go version needs a goroutine + channel because `termbox.PollEvent()` blocks.
      `crossterm::event::poll(timeout)` does not block, so a single-threaded loop can wait
      for "the next key OR the next fall tick", replacing Go's `select`.
    - Replace Go's `time.Timer` with an `Instant` deadline (`next_fall_at`) and compute the
      poll timeout from it.
    - Nice teaching point: this is a place where Rust and Go idioms diverge. Note it in comments.
  - [x] Figure out a strategy for testing.
    - Use the built-in `cargo test` with `#[cfg(test)] mod tests` inside `model.rs`.
    - The Go version has zero tests. Add tests for the pure game logic (model) only: piece
      fits / collides, rotation, line removal, level progression, game over, pause/resume.
    - Decision (JJ): `Game` owns a seeded `rand::StdRng`. `Game::new()` seeds from entropy;
      tests use `Game::with_seed(n)` for deterministic piece sequences.
    - Do not test the terminal view; it is thin and hard to test meaningfully.
  - [x] Are there standard things in Rust to control linting and formatting?
    - Yes, and both are already installed: `cargo fmt` (rustfmt) and `cargo clippy`.
    - Config: `rustfmt.toml` for formatting, `[lints.clippy]` table in `Cargo.toml` for lints.
    - Run clippy with `-D warnings` so lints fail the build in CI / the check script.
  - [x] Is there an equivalent of package.json for Rust that has simple scripts?
    - Not built in. `Cargo.toml` has no "scripts" section. Cargo aliases (`.cargo/config.toml`)
      only alias other cargo subcommands.
    - Common answers: a `Makefile`, a `justfile` (`just`), or `cargo-make` (`Makefile.toml`).
    - Decision (JJ): a `justfile`. JJ will `brew install just` himself.
  - [x] Decide on project structure: mirror the Go MVC layout.
    - `src/main.rs` (controller / event loop), `src/model.rs` (game state and rules),
      `src/view.rs` (rendering). Use Rust 2024 edition.
  - [x] Decide how faithful vs. how idiomatic the port should be.
    - Decision (JJ): idiomatic where it teaches. Keep the algorithm and MVC structure
      recognizable, but use `enum Cell`, `enum GameState`, a `Piece` struct with offset
      arrays, `Option<...>` instead of sentinel values, and `Result`/`?` for terminal errors.
- [x] Use Git.
  - [x] `git init` with `main` as the default branch.
  - [x] Add a `.gitignore` for Rust (`/target`) and macOS (`.DS_Store`).
  - [x] Commit after each milestone with clear messages.
- [x] Use GitHub. Store things under @jjinux.
  - [x] Create a public repo `jjinux/tetris_rust_claude` with `gh repo create`.
  - [x] Early work was pushed straight to `main`. Decision (JJ, 2026-09-11): from now on use
        branches + pull requests, and Conventional Commits messages. Documented in CLAUDE.md.
  - [x] Add an MIT LICENSE.
- [x] Get the test harness running.
  - [x] `cargo init --name tetris_rust_claude`.
  - [x] Add a trivial `#[test]` and confirm `cargo test` passes.
- [x] Set up linting and formatting.
  - [x] Add `rustfmt.toml`.
  - [x] Add `[lints.clippy]` to `Cargo.toml` (e.g. `pedantic = "warn"` with a few allows).
  - [x] Confirm `cargo fmt --check` and `cargo clippy -- -D warnings` pass.
- [x] Add a `justfile` with recipes: `build`, `run`, `test`, `lint`, `fmt`, `check` (all of the above).
- [x] Port the main game play.
  - [x] Add dependencies: `crossterm`, `rand`.
  - [x] Port `model.go` -> `src/model.rs`.
    - [x] Constants: board size, speeds, piece shape tables (`dx_bank`/`dy_bank`).
    - [x] `GameState` enum (Intro, Started, Paused, Over).
    - [x] `Game` struct and `Game::new()` / `reset()`.
    - [x] Movement: `move_left`, `move_right`, `move_down`, `rotate`, `fall` (hard drop).
    - [x] `piece_fits`, `erase_piece`, `place_piece`, `fill_matrix`, `lock_piece`.
      - Note: `erase_piece`/`place_piece` are gone. The falling piece is an `Option<Piece>`
        kept separate from the board instead of being stored in it as negative numbers.
    - [x] `remove_lines`, skyline tracking, level progression (`rows_per_level`, `max_level`).
      - Note: skyline tracking was dropped (the board is 16 rows; scanning it is cheap) and
        `level()` is computed from `num_lines` instead of stored.
    - [x] `start`, `pause`, `resume`, `play` (tick), `speed()`.
    - [x] Falling timer as an `Instant` deadline instead of `time.Timer`.
    - [x] Unit tests for the above.
  - [x] Port `view.go` -> `src/view.rs`.
    - [x] Layout constants (margins, title, board position, instructions column).
    - [x] Colors: blue background, black board, yellow text, per-piece colors.
    - [x] Draw the board with 2-character-wide cells.
    - [x] Draw the ghost/preview piece (`*` cells showing where the piece will land).
    - [x] Draw the instructions, level, lines, and "GAME OVER!".
    - [x] Title text: "TETRIS IN RUST (BY CLAUDE CODE)".
    - [x] Handle terminal resize (re-render on `Event::Resize`).
  - [x] Port `controller.go` -> `src/main.rs`.
    - [x] Enter raw mode + alternate screen; restore the terminal on exit, including on panic
          (use a guard struct with `Drop`, a good Rust construct to explain).
    - [x] Event loop: `poll(timeout)` -> key events or fall tick -> render.
    - [x] Keys: arrows, space, `s`, `p`, `q`/Esc/Ctrl-C/Ctrl-D.
  - [x] Smoke-tested in a pseudo-terminal (starts, takes keys, exits cleanly).
  - [x] Claude play-tested via a pseudo-terminal + pyte screen dump; found and fixed a
        one-interval render lag after each fall tick.
  - [x] JJ play-tested: worked but flickered. Fixed by clearing the screen only at startup and
        on resize, redrawing only when state changed, and wrapping frames in synchronized updates.
  - [x] JJ re-tested the flicker fix in a real terminal: fixed.
- [x] Write a basic CLAUDE.md.
  - [x] Project purpose, layout, how to build/test/lint, coding-style expectations
        (intermediate Rust, comment interesting constructs), workflow (update TODO.md, commit to main).
- [x] Write a basic README.md that explains what the project is and how to use it.
  - [x] Explain that this project is a port of a version JJ wrote in Golang
        (https://github.com/jjinux/gotetris) that Claude Code ported to help JJ learn Rust and Claude Code.
  - [x] Install / build / run / test / lint instructions.
  - [x] Controls.
  - [x] Credits (termbox-go -> crossterm, Alexei Kourbatov, the Dart port).
  - [x] Add a screenshot like the Go README has (JJ provided it).
- [x] Add a GitHub Actions workflow (`.github/workflows/ci.yml`) running fmt, clippy, and tests on push.
- [x] Nice-to-haves:
  - [x] Guard against terminals that are too small to draw the board. Shows a message with the
        required size, and auto-pauses a running game when a resize makes it too small.
- [x] Rendering follow-ups (2026-09-11):
  - [x] Investigate a hand-rolled double buffer (draw into a back buffer, diff against the front
        buffer, write only the changes, swap).
  - [x] Investigate adopting ratatui, which provides exactly that plus widgets and a test backend.
  - [x] Decision (JJ): adopt ratatui because it is a better base for future games.
  - [x] Port the view to ratatui: `Board` is a custom `Widget`, text is `Paragraph`s, the
        controller uses `ratatui::try_init` / `restore` and `Terminal::draw`.
  - [x] Add view tests with `TestBackend` (intro screen, piece and ghost colors, too-small message).
- [x] Announce it (2026-09-12):
  - [x] Blog about it: https://www.jjinux.com/2026/09/rust-tetris-in-rust-by-claude-code.html
  - [x] Post it to Bluesky, CC'ing the This Week in Rust and Ratatui teams:
        https://bsky.app/profile/jjinux.bsky.social/post/3mveiu2gqc52g
  - [x] Share it with friends.
