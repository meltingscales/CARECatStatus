use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Duration, Utc};
use futures::{SinkExt, StreamExt};
use sqlx::SqlitePool;
use tokio::sync::{Mutex, broadcast, mpsc};
use uuid::Uuid;

use crate::{
    auth::{HasAuth, Sessions, auth_required},
    db,
    models::{ClientMsg, ServerMsg},
};

const CHANNEL_CAPACITY: usize = 256;

/// How long an edit lock lasts without being renewed.
pub const LOCK_TTL_SECS: i64 = 60;

struct LockInfo {
    conn_id: Uuid,
    username: String,
    expires_at: DateTime<Utc>,
}

pub struct AppState {
    pub pool: SqlitePool,
    pub sessions: Sessions,
    tx: broadcast::Sender<ServerMsg>,
    locks: Mutex<HashMap<Uuid, LockInfo>>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { pool, sessions: Sessions::default(), tx, locks: Mutex::new(HashMap::new()) }
    }

    pub async fn broadcast(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }

    /// Acquire or renew the edit lock on `cat_id` for `conn_id`.
    /// Fails if someone else holds an unexpired lock.
    async fn try_lock(&self, cat_id: Uuid, conn_id: Uuid, username: &str) -> Result<DateTime<Utc>, (String, DateTime<Utc>)> {
        let mut locks = self.locks.lock().await;
        let now = Utc::now();
        if let Some(existing) = locks.get(&cat_id) {
            if existing.conn_id != conn_id && existing.expires_at > now {
                return Err((existing.username.clone(), existing.expires_at));
            }
        }
        let expires_at = now + Duration::seconds(LOCK_TTL_SECS);
        locks.insert(cat_id, LockInfo { conn_id, username: username.to_owned(), expires_at });
        Ok(expires_at)
    }

    /// Release the lock on `cat_id` if held by `conn_id`.
    async fn unlock(&self, cat_id: Uuid, conn_id: Uuid) -> bool {
        let mut locks = self.locks.lock().await;
        if locks.get(&cat_id).is_some_and(|l| l.conn_id == conn_id) {
            locks.remove(&cat_id);
            true
        } else {
            false
        }
    }

    /// Release every lock held by `conn_id` (e.g. on disconnect). Returns their cat ids.
    async fn release_all(&self, conn_id: Uuid) -> Vec<Uuid> {
        let mut locks = self.locks.lock().await;
        let ids: Vec<Uuid> = locks.iter().filter(|(_, l)| l.conn_id == conn_id).map(|(id, _)| *id).collect();
        for id in &ids {
            locks.remove(id);
        }
        ids
    }

    /// True if `cat_id` is under an unexpired lock held by someone other than `conn_id`.
    pub async fn is_locked_by_other(&self, cat_id: Uuid, conn_id: Option<Uuid>) -> bool {
        let locks = self.locks.lock().await;
        match locks.get(&cat_id) {
            Some(l) => l.expires_at > Utc::now() && Some(l.conn_id) != conn_id,
            None => false,
        }
    }

    /// All unexpired locks, for briefing a newly (re)connected client.
    async fn active_locks(&self) -> Vec<ServerMsg> {
        let locks = self.locks.lock().await;
        let now = Utc::now();
        locks
            .iter()
            .filter(|(_, l)| l.expires_at > now)
            .map(|(id, l)| ServerMsg::Locked { id: *id, by: l.username.clone(), by_conn: l.conn_id, expires_at: l.expires_at })
            .collect()
    }
}

impl HasAuth for AppState {
    fn pool(&self) -> &SqlitePool { &self.pool }
    fn sessions(&self) -> &Sessions { &self.sessions }
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> impl IntoResponse {
    // Gate the upgrade behind auth if a PIN is set.
    let pin_set = auth_required(&state.pool).await;
    let token = jar.get(crate::auth::SESSION_COOKIE).map(|c| c.value().to_string());
    if pin_set {
        let authed = match &token {
            Some(t) => state.sessions.contains(t).await,
            None => false,
        };
        if !authed {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    // Display name used in lock notifications; falls back for anonymous deployments.
    let username = match &token {
        Some(t) => state.sessions.username(t).await,
        None => None,
    }
    .unwrap_or_else(|| "a volunteer".to_string());

    ws.on_upgrade(move |socket| handle_socket(socket, state, username))
        .into_response()
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, username: String) {
    let conn_id = Uuid::new_v4();
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = state.tx.subscribe();
    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel::<ServerMsg>();

    // Tell the client its own connection id, so it can recognize its own lock broadcasts.
    let welcome = serde_json::to_string(&ServerMsg::Welcome { conn_id }).unwrap();
    if sender.send(Message::Text(welcome.into())).await.is_err() {
        return;
    }

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

    // Brief the new connection on locks already in effect.
    for lock_msg in state.active_locks().await {
        let text = serde_json::to_string(&lock_msg).unwrap();
        if sender.send(Message::Text(text.into())).await.is_err() {
            return;
        }
    }

    // Forward broadcasts and direct replies to this client.
    let mut send_task = tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                r = broadcast_rx.recv() => match r {
                    Ok(m) => m,
                    Err(_) => break,
                },
                r = direct_rx.recv() => match r {
                    Some(m) => m,
                    None => break,
                },
            };
            let text = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client.
    let state_recv = state.clone();
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
                    match db::create_cat(&state_recv.pool, cat).await {
                        Ok(cat) => state_recv.broadcast(ServerMsg::Upsert { cat }).await,
                        Err(e) => tracing::error!("ws create: {e}"),
                    }
                }
                ClientMsg::Update { id, patch } => {
                    if state_recv.is_locked_by_other(id, Some(conn_id)).await {
                        continue; // stale UI on the sender's end; silently drop.
                    }
                    match db::update_cat(&state_recv.pool, id, patch).await {
                        Ok(Some(cat)) => state_recv.broadcast(ServerMsg::Upsert { cat }).await,
                        Ok(None) => {}
                        Err(e) => tracing::error!("ws update: {e}"),
                    }
                }
                ClientMsg::Delete { id } => {
                    match db::delete_cat(&state_recv.pool, id).await {
                        Ok(true) => state_recv.broadcast(ServerMsg::Delete { id }).await,
                        Ok(false) => {}
                        Err(e) => tracing::error!("ws delete: {e}"),
                    }
                }
                ClientMsg::Lock { id } => {
                    match state_recv.try_lock(id, conn_id, &username).await {
                        Ok(expires_at) => {
                            state_recv
                                .broadcast(ServerMsg::Locked { id, by: username.clone(), by_conn: conn_id, expires_at })
                                .await;
                        }
                        Err((by, expires_at)) => {
                            let _ = direct_tx.send(ServerMsg::LockDenied { id, by, expires_at });
                        }
                    }
                }
                ClientMsg::Unlock { id } => {
                    if state_recv.unlock(id, conn_id).await {
                        state_recv.broadcast(ServerMsg::Unlocked { id }).await;
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Release any locks this connection held so other clients aren't stuck waiting.
    for id in state.release_all(conn_id).await {
        state.broadcast(ServerMsg::Unlocked { id }).await;
    }
}
