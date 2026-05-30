use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum CatColor {
    Green,
    Yellow,
    Blue,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Cat {
    pub id: Uuid,
    pub name: String,
    pub color: CatColor,
    pub notes: String,
    pub food_notes: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCat {
    pub name: String,
    pub color: CatColor,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub food_notes: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCat {
    pub name: Option<String>,
    pub color: Option<CatColor>,
    pub notes: Option<String>,
    pub food_notes: Option<String>,
}

/// WebSocket messages sent from server → clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Full state snapshot sent on initial connection.
    Snapshot { cats: Vec<Cat> },
    /// A cat was created or updated.
    Upsert { cat: Cat },
    /// A cat was deleted.
    Delete { id: Uuid },
}

/// WebSocket messages sent from client → server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Create { cat: CreateCat },
    Update { id: Uuid, patch: UpdateCat },
    Delete { id: Uuid },
}
