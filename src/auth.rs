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
use sqlx::SqlitePool;
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

pub async fn get_pin_hash(pool: &SqlitePool) -> anyhow::Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = 'pin_hash'")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| {
        use sqlx::Row;
        r.get::<String, _>("value")
    }))
}

#[allow(dead_code)]
pub async fn set_pin_hash(pool: &SqlitePool, hash: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('pin_hash', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(hash)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn clear_pin(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM settings WHERE key = 'pin_hash'")
        .execute(pool)
        .await?;
    Ok(())
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
    let pin_hash = get_pin_hash(state.pool()).await.unwrap_or(None);
    let required = pin_hash.is_some();
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
    let pin_hash = get_pin_hash(state.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(hash) = pin_hash else {
        // No PIN set — auth not required, issue a session anyway.
        let token = Uuid::new_v4().to_string();
        state.sessions().insert(token.clone()).await;
        let jar = jar.add(Cookie::build((SESSION_COOKIE, token)).path("/").build());
        return Ok((jar, Json(LoginResponse { ok: true })));
    };

    let valid = bcrypt::verify(&body.pin, &hash).unwrap_or(false);
    if !valid {
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
    // If no PIN is set, allow all requests.
    let pin_hash = get_pin_hash(state.pool()).await.unwrap_or(None);
    if pin_hash.is_none() {
        return next.run(req).await;
    }

    // Extract the session cookie.
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
