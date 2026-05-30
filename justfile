[private]
default:
    @just --list

# Run development server (with auto-reload via cargo-watch)
dev:
    cargo watch -x 'run --bin care-cat-status'

# Run once
run:
    cargo run --bin care-cat-status

# Build release binary
build:
    cargo build --release

# Run release binary
start:
    ./target/release/care-cat-status

# Set the PIN required to access the app
set-pin pin:
    cargo run --bin set_pin -- {{pin}}

# Remove the PIN (disables authentication)
clear-pin:
    cargo run --bin set_pin -- --clear

# Run tests
test:
    cargo test

# Format code
fmt:
    cargo fmt

# Lint
lint:
    cargo clippy -- -D warnings

# Check (no build artifacts)
check:
    cargo check
