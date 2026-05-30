use std::sync::Arc;

use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::{
    db,
    models::{ClientMsg, ServerMsg},
};

const CHANNEL_CAPACITY: usize = 256;

pub struct AppState {
    pub pool: SqlitePool,
    tx: broadcast::Sender<ServerMsg>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { pool, tx }
    }

    pub async fn broadcast(&self, msg: ServerMsg) {
        // It's fine if there are no subscribers.
        let _ = self.tx.send(msg);
    }
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Send snapshot of current state.
    match db::list_cats(&state.pool).await {
        Ok(cats) => {
            let msg = serde_json::to_string(&ServerMsg::Snapshot { cats }).unwrap();
            if sender.send(Message::Text(msg.into())).await.is_err() {
                return;
            }
        }
        Err(e) => {
            tracing::error!("ws snapshot: {e}");
            return;
        }
    }

    // Forward broadcasts to this client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let text = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };

            let client_msg: ClientMsg = match serde_json::from_str(&text) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("ws parse error: {e}");
                    continue;
                }
            };

            match client_msg {
                ClientMsg::Create { cat } => {
                    match db::create_cat(&state.pool, cat).await {
                        Ok(cat) => state.broadcast(ServerMsg::Upsert { cat }).await,
                        Err(e) => tracing::error!("ws create: {e}"),
                    }
                }
                ClientMsg::Update { id, patch } => {
                    match db::update_cat(&state.pool, id, patch).await {
                        Ok(Some(cat)) => state.broadcast(ServerMsg::Upsert { cat }).await,
                        Ok(None) => {}
                        Err(e) => tracing::error!("ws update: {e}"),
                    }
                }
                ClientMsg::Delete { id } => {
                    match db::delete_cat(&state.pool, id).await {
                        Ok(true) => state.broadcast(ServerMsg::Delete { id }).await,
                        Ok(false) => {}
                        Err(e) => tracing::error!("ws delete: {e}"),
                    }
                }
            }
        }
    });

    // Abort the other task when either side disconnects.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
