# CLAUDE.md

Notes for Claude Code when working in this repository.

## What this is

"Tetris in Rust (by Claude Code)" is a console Tetris. It is a port of
https://github.com/jjinux/gotetris (Go, also by JJ) that Claude Code produced
to help JJ learn Rust and Claude Code. JJ knows many languages but only a
little Rust.

## Coding style

- Write code the way an intermediate Rust programmer would: idiomatic, but not
  clever. Prefer clarity over brevity.
- When using an interesting language construct (RAII/`Drop`, let chains,
  `impl Trait`, struct update syntax, generics with `where`, `?`, etc.), leave
  a short comment explaining what it does and why it is useful.
- Keep the MVC split: rules in `src/model.rs`, drawing in `src/view.rs`, the
  event loop in `src/main.rs`. The model must not depend on crossterm.
- The model's tests live in `src/model.rs` under `#[cfg(test)] mod tests`.
  Add or update tests when changing the rules. Use `Game::with_seed` for
  deterministic pieces.
- Clippy runs with `pedantic` on and `-D warnings`. The cast lints are allowed
  in `Cargo.toml` because the board is tiny. Fix lints rather than adding
  `#[allow]` attributes unless there is a good reason, and comment the reason.

## Commands

    just check   # fmt-check + clippy + test; run before every commit
    just test
    just lint
    just fmt
    just run

If `just` is not installed, the recipes are one-line `cargo` commands; see
`justfile`.

## Workflow

- `TODO.md` is the plan. Check items off as they are finished and add new
  items as they come up.
- Never commit directly to `main`. Create a branch, push it, and open a pull
  request with `gh pr create` against https://github.com/jjinux/tetris_rust_claude.
  JJ reviews and merges.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org):
  `<type>(<optional scope>): <description>`, e.g. `feat(model): add hold piece`,
  `fix(view): pad status lines`, `docs: explain PR workflow`, `chore: bump crossterm`,
  `test(model): cover wall kicks`, `refactor`, `ci`. Use the imperative mood and
  keep the subject under 72 characters; put details in the body.
- CI (`.github/workflows/ci.yml`) runs `just check` on every push and pull request.

## Dependencies

- `ratatui` (0.30) for drawing: `Terminal::draw`, `Frame`, `Buffer`, the `Widget`
  trait, `Paragraph`, `Block`. It double-buffers and diffs frames for us. View
  tests use `ratatui::backend::TestBackend`.
- `crossterm` (0.29, the version ratatui's `crossterm` feature uses) for key events,
  `event::poll`, and the synchronized-update markers around each frame.
- `rand` (0.10) for piece selection: `rand::make_rng()` for entropy seeding,
  `StdRng::seed_from_u64` in tests, `RngExt::random_range` to pick a piece.
