use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header},
    routing::{get, patch, post},
};
use uuid::Uuid;

use crate::{
    csv, db,
    models::{Cat, CreateCat, ImportResult, ServerMsg, UpdateCat},
    ws::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/cats", get(list_cats).post(create_cat))
        .route("/cats/{id}", patch(update_cat).delete(delete_cat))
        .route("/cats/export.csv", get(export_cats_csv))
        .route("/cats/import", post(import_cats_csv))
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
        (status = 404, description = "Cat not found"),
        (status = 423, description = "Cat is locked for editing by another session")
    )
)]
pub async fn update_cat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<UpdateCat>,
) -> Result<Json<Cat>, StatusCode> {
    if state.is_locked_by_other(id, None).await {
        return Err(StatusCode::LOCKED);
    }
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

const CSV_COLUMNS: [&str; 6] = ["name", "color", "location", "room", "notes", "food_notes"];

/// Export all cats as CSV.
#[utoipa::path(
    get,
    path = "/api/cats/export.csv",
    responses((status = 200, description = "CSV of all cats", content_type = "text/csv"))
)]
pub async fn export_cats_csv(
    State(state): State<Arc<AppState>>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let cats = db::list_cats(&state.pool).await.map_err(|e| {
        tracing::error!("export_cats_csv: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut out = format!("{}\n", CSV_COLUMNS.join(","));
    for cat in &cats {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv::field(&cat.name),
            csv::field(db::color_to_str(&cat.color)),
            csv::field(db::location_to_str(&cat.location)),
            csv::field(&cat.room),
            csv::field(&cat.notes),
            csv::field(&cat.food_notes),
        ));
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"cats.csv\""),
        ],
        out,
    ))
}

/// Import cats from CSV, matching existing cats by name (case-sensitive).
/// Unknown names are created; known names are updated in place.
#[utoipa::path(
    post,
    path = "/api/cats/import",
    request_body(content = String, content_type = "text/csv"),
    responses((status = 200, description = "Import summary", body = ImportResult))
)]
pub async fn import_cats_csv(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Json<ImportResult>, StatusCode> {
    let rows = csv::parse(&body);
    let mut rows = rows.into_iter();
    let Some(header_row) = rows.next() else {
        return Ok(Json(ImportResult::default()));
    };

    let col = |name: &str| header_row.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    let Some(name_col) = col("name") else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let color_col = col("color");
    let location_col = col("location");
    let room_col = col("room");
    let notes_col = col("notes");
    let food_col = col("food_notes");

    let existing = db::list_cats(&state.pool).await.map_err(|e| {
        tracing::error!("import_cats_csv: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let mut by_name: HashMap<String, Uuid> = existing.into_iter().map(|c| (c.name, c.id)).collect();

    let mut result = ImportResult::default();
    for (i, row) in rows.enumerate() {
        let line = i + 2; // 1-indexed, plus header row
        let get = |c: Option<usize>| c.and_then(|c| row.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();

        let name = get(Some(name_col));
        if name.is_empty() {
            continue;
        }
        let color = match db::parse_color(&get(color_col)) {
            Ok(c) => c,
            Err(e) => {
                result.errors.push(format!("line {line}: {e}"));
                continue;
            }
        };
        let location = match db::parse_location(&get(location_col)) {
            Ok(l) => l,
            Err(e) => {
                result.errors.push(format!("line {line}: {e}"));
                continue;
            }
        };
        let room = get(room_col);
        let notes = get(notes_col);
        let food_notes = get(food_col);

        if let Some(&id) = by_name.get(&name) {
            let patch = UpdateCat {
                name: None,
                color: Some(color),
                location: Some(location),
                room: Some(room),
                notes: Some(notes),
                food_notes: Some(food_notes),
            };
            match db::update_cat(&state.pool, id, patch).await {
                Ok(Some(cat)) => {
                    state.broadcast(ServerMsg::Upsert { cat }).await;
                    result.updated += 1;
                }
                Ok(None) => {}
                Err(e) => result.errors.push(format!("line {line}: {e}")),
            }
        } else {
            let create = CreateCat { name: name.clone(), color, location, room, notes, food_notes };
            match db::create_cat(&state.pool, create).await {
                Ok(cat) => {
                    by_name.insert(name, cat.id);
                    state.broadcast(ServerMsg::Upsert { cat }).await;
                    result.created += 1;
                }
                Err(e) => result.errors.push(format!("line {line}: {e}")),
            }
        }
    }

    Ok(Json(result))
}
