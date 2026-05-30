# CARECatStatus

A real-time cat status board for shift staff at CARE Animal Shelter. Multiple staff members can view and update cat statuses simultaneously — changes sync across all connected devices instantly.

## Features

- Live collaborative editing — changes sync globally via WebSocket (last-write-wins with diff resolution)
- Per-cat status tracking: color (green / yellow / blue), notes, food notes
- No login required
- Installable as a Progressive Web App (PWA) on phones and tablets
- OpenAPI spec for future clients (e.g. Android app)

## Tech Stack

| Layer    | Tech                        |
|----------|-----------------------------|
| Backend  | Rust + Axum                 |
| Database | SQLite                      |
| Frontend | Vanilla JS + WebSocket      |
| Realtime | WebSocket (Axum)            |
| API spec | OpenAPI                     |

## Data Model

Each cat entry has:

| Field       | Type                        |
|-------------|-----------------------------|
| `name`      | String                      |
| `color`     | `green` \| `yellow` \| `blue` |
| `notes`     | String                      |
| `food_notes`| String                      |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)

### Run

```bash
cargo run
```

The server starts on `http://localhost:3000` by default.

### Build for production

```bash
cargo build --release
./target/release/care-cat-status
```

### Configuration

| Environment Variable | Default       | Description              |
|----------------------|---------------|--------------------------|
| `PORT`               | `3000`        | HTTP/WebSocket port      |
| `DATABASE_URL`       | `cats.db`     | SQLite database path     |

## API

An OpenAPI spec is served at `/openapi.json` when the server is running. You can import it into tools like Bruno, Postman, or use it to generate a typed client.

## PWA Installation

Open the app in a mobile browser and use "Add to Home Screen" to install it as a PWA. Staff can then launch it like a native app without needing to remember the URL.
