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

# Add a user (e.g. just add-user jane-doe 1234)
add-user name pin:
    cargo run --bin manage-users -- add {{name}} {{pin}}

# Change a user's PIN
modify-user name pin:
    cargo run --bin manage-users -- modify {{name}} {{pin}}

# Rename a user
rename-user old new:
    cargo run --bin manage-users -- rename {{old}} {{new}}

# Delete a user
delete-user name:
    cargo run --bin manage-users -- delete {{name}}

# List all users
list-users:
    cargo run --bin manage-users -- list

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
