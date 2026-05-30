# CARECatStatus

A real-time cat status board for shift staff at CARE Animal Shelter. Multiple staff members can view and update cat statuses simultaneously — changes sync across all connected devices instantly.

Live at: **https://carecatstatus.drakonix.systems**

## Features

- Live collaborative editing — changes sync globally via WebSocket (last-write-wins with diff resolution)
- Per-cat status tracking: color (green / yellow / blue), location (adoption center / foster), notes, food notes
- Username + PIN login — auth is optional (disabled when no users exist)
- Installable as a Progressive Web App (PWA) on phones and tablets
- OpenAPI spec served at `/openapi.json`, Swagger UI at `/docs`

## Tech Stack

| Layer    | Tech                   |
|----------|------------------------|
| Backend  | Rust + Axum            |
| Database | SQLite (via sqlx)      |
| Frontend | Vanilla JS + WebSocket |
| Realtime | WebSocket broadcast    |
| API spec | OpenAPI (utoipa)       |

## Data Model

Each cat entry has:

| Field        | Type                             |
|--------------|----------------------------------|
| `name`       | String                           |
| `color`      | `green` \| `yellow` \| `blue`   |
| `location`   | `adoption center` \| `foster`   |
| `notes`      | String                           |
| `food_notes` | String                           |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [`just`](https://just.systems/) command runner
- `cargo-watch` for live reload (`cargo install cargo-watch`)

### Run (development)

```bash
just dev       # live-reload via cargo-watch
just run       # run once
```

The server starts on `http://localhost:3000` by default.

### Build for production

```bash
just build
just start
```

### Configuration

| Environment Variable | Default   | Description          |
|----------------------|-----------|----------------------|
| `PORT`               | `3000`    | HTTP/WebSocket port  |
| `DATABASE_URL`       | `cats.db` | SQLite database path |

## User Management

Auth is disabled when no users exist. Once a user is added, everyone must sign in.

```bash
just add-user firstname-lastname 1234   # create a user
just modify-user firstname-lastname 5678  # change their PIN
just rename-user old-name new-name      # rename a user
just delete-user firstname-lastname     # remove a user
just list-users                         # show all users
```

Usernames must be lowercase letters and hyphens only (e.g. `jane-doe`).

## Login

Open the app in a browser. If users exist, a sign-in screen appears with:
- A username field (type your `firstname-lastname`)
- A PIN numpad (click buttons or press digit keys / Backspace / Enter)

## Deployment (systemd on a GCP VM)

```bash
just build                      # compile release binary
sudo just systemd-install       # install and start the service
sudo just systemd-uninstall     # remove the service
just systemd-status             # check status
just systemd-logs               # tail logs
```

The service runs on port **3007** and is proxied by nginx at
`carecatstatus.drakonix.systems`.

To apply a new release:
```bash
just build
sudo systemctl restart care-cat-status
```

## nginx

The nginx vhost config lives in
`~/Git/drakonix.systems/nginx/drakonix.systems.conf`. To deploy a config
change:

```bash
cd ~/Git/drakonix.systems
just nginx-install
```

## API

An OpenAPI spec is served at `/openapi.json` when the server is running.
Swagger UI is at `/docs`.
