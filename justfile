# Task runner for the project. Run `just` to list recipes, `just check` before
# committing. Install with `brew install just`.

# List available recipes.
default:
    @just --list

# Build the debug binary.
build:
    cargo build

# Build and run the game.
run:
    cargo run

# Run the unit tests.
test:
    cargo test

# Run clippy on all targets (including tests) and fail on any warning.
lint:
    cargo clippy --all-targets -- -D warnings

# Format the code in place.
fmt:
    cargo fmt

# Fail if the code is not formatted.
fmt-check:
    cargo fmt --check

# Everything CI runs: formatting, lints, and tests.
check: fmt-check lint test
