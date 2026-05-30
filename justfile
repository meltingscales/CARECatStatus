default: dev

# Run development server (with auto-reload via cargo-watch)
dev:
    cargo watch -x run

# Run once
run:
    cargo run

# Build release binary
build:
    cargo build --release

# Run release binary
start:
    ./target/release/care-cat-status

# Run database migrations only
migrate:
    cargo run --bin migrate

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
