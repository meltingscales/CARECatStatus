use std::{
    collections::HashSet,
    sync::Arc,
};

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "care_session";

// ── Session store ─────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Sessions(RwLock<HashSet<String>>);

impl Sessions {
    pub async fn insert(&self, token: String) {
        self.0.write().await.insert(token);
    }

    pub async fn contains(&self, token: &str) -> bool {
        self.0.read().await.contains(token)
    }
}

// ── DB helpers ────────────────────────────────────────────────────────────────

/// Returns true if at least one user exists (i.e. auth is required).
pub async fn auth_required(pool: &SqlitePool) -> bool {
    sqlx::query("SELECT 1 FROM users LIMIT 1")
        .fetch_optional(pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
}

/// Look up the bcrypt hash for a username.
async fn fetch_pin_hash(pool: &SqlitePool, username: &str) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT pin_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("pin_hash")))
}

// ── Route handlers ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AuthStatus {
    pub required: bool,
    pub authenticated: bool,
}

pub async fn status_handler<S>(
    State(state): State<Arc<S>>,
    jar: CookieJar,
) -> Json<AuthStatus>
where
    S: HasAuth + Send + Sync + 'static,
{
    let required = auth_required(state.pool()).await;
    let authenticated = if required {
        match jar.get(SESSION_COOKIE) {
            Some(c) => state.sessions().contains(c.value()).await,
            None => false,
        }
    } else {
        true
    };
    Json(AuthStatus { required, authenticated })
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub pin: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub ok: bool,
}

pub async fn login_handler<S>(
    State(state): State<Arc<S>>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), StatusCode>
where
    S: HasAuth + Send + Sync + 'static,
{
    if !auth_required(state.pool()).await {
        // No users exist — open access, issue session.
        let token = Uuid::new_v4().to_string();
        state.sessions().insert(token.clone()).await;
        let jar = jar.add(Cookie::build((SESSION_COOKIE, token)).path("/").build());
        return Ok((jar, Json(LoginResponse { ok: true })));
    }

    let hash = fetch_pin_hash(state.pool(), &body.username)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(hash) = hash else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !bcrypt::verify(&body.pin, &hash).unwrap_or(false) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = Uuid::new_v4().to_string();
    state.sessions().insert(token.clone()).await;
    let jar = jar.add(Cookie::build((SESSION_COOKIE, token)).path("/").build());
    Ok((jar, Json(LoginResponse { ok: true })))
}

// ── Middleware ────────────────────────────────────────────────────────────────

pub async fn require_auth<S>(
    State(state): State<Arc<S>>,
    req: Request<Body>,
    next: Next,
) -> Response
where
    S: HasAuth + Send + Sync + 'static,
{
    if !auth_required(state.pool()).await {
        return next.run(req).await;
    }

    let cookie_header = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&format!("{SESSION_COOKIE}=")));

    match token {
        Some(t) if state.sessions().contains(t).await => next.run(req).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

// ── Trait so auth can access AppState fields without a circular dep ───────────

pub trait HasAuth {
    fn pool(&self) -> &SqlitePool;
    fn sessions(&self) -> &Sessions;
}
