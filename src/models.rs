use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum CatColor {
    Green,
    Orange,
    Blue,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CatLocation {
    Foster,
    #[sqlx(rename = "adoption center")]
    #[serde(rename = "adoption center")]
    AdoptionCenter,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Cat {
    pub id: Uuid,
    pub name: String,
    pub color: CatColor,
    pub location: CatLocation,
    #[serde(default)]
    pub room: String,
    pub notes: String,
    pub food_notes: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCat {
    pub name: String,
    pub color: CatColor,
    pub location: CatLocation,
    #[serde(default)]
    pub room: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub food_notes: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCat {
    pub name: Option<String>,
    pub color: Option<CatColor>,
    pub location: Option<CatLocation>,
    pub room: Option<String>,
    pub notes: Option<String>,
    pub food_notes: Option<String>,
}

/// Result of a bulk CSV import.
#[derive(Debug, Default, Serialize, ToSchema)]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
    pub errors: Vec<String>,
}

/// WebSocket messages sent from server → clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Sent once, first, so the client can recognize its own lock broadcasts.
    Welcome { conn_id: Uuid },
    /// Full state snapshot sent on initial connection.
    Snapshot { cats: Vec<Cat> },
    /// A cat was created or updated.
    Upsert { cat: Cat },
    /// A cat was deleted.
    Delete { id: Uuid },
    /// A cat's edit lock was acquired or renewed. Broadcast to everyone.
    Locked { id: Uuid, by: String, by_conn: Uuid, expires_at: DateTime<Utc> },
    /// A cat's edit lock was released (explicitly, or lock holder disconnected).
    Unlocked { id: Uuid },
    /// Sent only to the requester when a lock request is refused.
    LockDenied { id: Uuid, by: String, expires_at: DateTime<Utc> },
}

/// WebSocket messages sent from client → server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Create { cat: CreateCat },
    Update { id: Uuid, patch: UpdateCat },
    Delete { id: Uuid },
    /// Request (or renew) the edit lock on a cat.
    Lock { id: Uuid },
    /// Release the edit lock on a cat.
    Unlock { id: Uuid },
}
