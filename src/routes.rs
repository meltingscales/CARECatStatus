use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch},
};
use uuid::Uuid;

use crate::{
    db,
    models::{Cat, CreateCat, ServerMsg, UpdateCat},
    ws::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/cats", get(list_cats).post(create_cat))
        .route("/cats/:id", patch(update_cat).delete(delete_cat))
}

/// List all cats.
#[utoipa::path(
    get,
    path = "/api/cats",
    responses(
        (status = 200, description = "List of all cats", body = Vec<Cat>)
    )
)]
pub async fn list_cats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Cat>>, StatusCode> {
    db::list_cats(&state.pool)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("list_cats: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Create a cat.
#[utoipa::path(
    post,
    path = "/api/cats",
    request_body = CreateCat,
    responses(
        (status = 201, description = "Created cat", body = Cat)
    )
)]
pub async fn create_cat(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateCat>,
) -> Result<(StatusCode, Json<Cat>), StatusCode> {
    let cat = db::create_cat(&state.pool, body).await.map_err(|e| {
        tracing::error!("create_cat: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    state.broadcast(ServerMsg::Upsert { cat: cat.clone() }).await;
    Ok((StatusCode::CREATED, Json(cat)))
}

/// Update a cat (partial update).
#[utoipa::path(
    patch,
    path = "/api/cats/{id}",
    params(("id" = Uuid, Path, description = "Cat ID")),
    request_body = UpdateCat,
    responses(
        (status = 200, description = "Updated cat", body = Cat),
        (status = 404, description = "Cat not found")
    )
)]
pub async fn update_cat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<UpdateCat>,
) -> Result<Json<Cat>, StatusCode> {
    match db::update_cat(&state.pool, id, patch).await {
        Ok(Some(cat)) => {
            state.broadcast(ServerMsg::Upsert { cat: cat.clone() }).await;
            Ok(Json(cat))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("update_cat: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete a cat.
#[utoipa::path(
    delete,
    path = "/api/cats/{id}",
    params(("id" = Uuid, Path, description = "Cat ID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Cat not found")
    )
)]
pub async fn delete_cat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match db::delete_cat(&state.pool, id).await {
        Ok(true) => {
            state.broadcast(ServerMsg::Delete { id }).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("delete_cat: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
