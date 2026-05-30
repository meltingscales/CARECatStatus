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

# Systemd Service (run with sudo)
# ================================

# Install and start the systemd service on port 3007
systemd-install:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ $EUID -ne 0 ]]; then
        echo "Error: run with sudo."
        exit 1
    fi
    REPO_DIR="$(pwd)"
    SERVICE_NAME="care-cat-status"
    USER="${SUDO_USER:-root}"
    if [[ ! -f "${REPO_DIR}/target/release/care-cat-status" ]]; then
        echo "Building release binary..."
        sudo -u "$USER" cargo build --release
    fi
    sed -e "s|USER_PLACEHOLDER|${USER}|g" \
        -e "s|REPO_DIR_PLACEHOLDER|${REPO_DIR}|g" \
        "${REPO_DIR}/systemd/${SERVICE_NAME}.service" \
        > /etc/systemd/system/${SERVICE_NAME}.service
    systemctl daemon-reload
    systemctl enable ${SERVICE_NAME}
    systemctl restart ${SERVICE_NAME}
    echo "Service installed and started on port 3007."
    echo "  sudo systemctl status ${SERVICE_NAME}"
    echo "  sudo journalctl -u ${SERVICE_NAME} -f"

# Remove the systemd service
systemd-uninstall:
    #!/usr/bin/env bash
    if [[ $EUID -ne 0 ]]; then echo "Error: run with sudo."; exit 1; fi
    SERVICE_NAME="care-cat-status"
    systemctl stop ${SERVICE_NAME} 2>/dev/null || true
    systemctl disable ${SERVICE_NAME} 2>/dev/null || true
    rm -f /etc/systemd/system/${SERVICE_NAME}.service
    systemctl daemon-reload
    echo "Service uninstalled."

# Show service status
systemd-status:
    systemctl status care-cat-status

# Tail service logs
systemd-logs:
    journalctl -u care-cat-status -f

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
