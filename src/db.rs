use chrono::Utc;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use uuid::Uuid;

use crate::models::{Cat, CatColor, CatLocation, CreateCat, UpdateCat};

pub async fn init(database_url: &str) -> anyhow::Result<SqlitePool> {
    let url = if database_url.starts_with("sqlite:") {
        database_url.to_owned()
    } else {
        format!("sqlite:{database_url}?mode=rwc")
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn row_to_cat(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<Cat> {
    let id: &str = row.try_get("id")?;
    let color: &str = row.try_get("color")?;
    let location: &str = row.try_get("location")?;
    let updated_at: &str = row.try_get("updated_at")?;
    Ok(Cat {
        id: id.parse()?,
        name: row.try_get("name")?,
        color: parse_color(color)?,
        location: parse_location(location)?,
        notes: row.try_get("notes")?,
        food_notes: row.try_get("food_notes")?,
        updated_at: updated_at.parse()?,
    })
}

pub async fn list_cats(pool: &SqlitePool) -> anyhow::Result<Vec<Cat>> {
    let rows = sqlx::query(
        "SELECT id, name, color, location, notes, food_notes, updated_at FROM cats ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await?;

    rows.iter().map(row_to_cat).collect()
}

pub async fn create_cat(pool: &SqlitePool, req: CreateCat) -> anyhow::Result<Cat> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let id_str = id.to_string();
    let now_str = now.to_rfc3339();
    let color_str = color_to_str(&req.color);
    let location_str = location_to_str(&req.location);

    sqlx::query(
        "INSERT INTO cats (id, name, color, location, notes, food_notes, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id_str)
    .bind(&req.name)
    .bind(color_str)
    .bind(location_str)
    .bind(&req.notes)
    .bind(&req.food_notes)
    .bind(&now_str)
    .execute(pool)
    .await?;

    Ok(Cat {
        id,
        name: req.name,
        color: req.color,
        location: req.location,
        notes: req.notes,
        food_notes: req.food_notes,
        updated_at: now,
    })
}

pub async fn update_cat(
    pool: &SqlitePool,
    id: Uuid,
    patch: UpdateCat,
) -> anyhow::Result<Option<Cat>> {
    let now = Utc::now();
    let id_str = id.to_string();

    let Some(existing) = sqlx::query(
        "SELECT id, name, color, location, notes, food_notes, updated_at FROM cats WHERE id = ?",
    )
    .bind(&id_str)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let name: String      = existing.try_get("name")?;
    let color: String     = existing.try_get("color")?;
    let location: String  = existing.try_get("location")?;
    let notes: String     = existing.try_get("notes")?;
    let food_notes: String = existing.try_get("food_notes")?;

    let name       = patch.name.unwrap_or(name);
    let color      = patch.color.map(|c| color_to_str(&c).to_owned()).unwrap_or(color);
    let location   = patch.location.map(|l| location_to_str(&l).to_owned()).unwrap_or(location);
    let notes      = patch.notes.unwrap_or(notes);
    let food_notes = patch.food_notes.unwrap_or(food_notes);
    let now_str    = now.to_rfc3339();

    sqlx::query(
        "UPDATE cats SET name=?, color=?, location=?, notes=?, food_notes=?, updated_at=? WHERE id=?",
    )
    .bind(&name)
    .bind(&color)
    .bind(&location)
    .bind(&notes)
    .bind(&food_notes)
    .bind(&now_str)
    .bind(&id_str)
    .execute(pool)
    .await?;

    Ok(Some(Cat {
        id,
        name,
        color: parse_color(&color)?,
        location: parse_location(&location)?,
        notes,
        food_notes,
        updated_at: now,
    }))
}

pub async fn delete_cat(pool: &SqlitePool, id: Uuid) -> anyhow::Result<bool> {
    let id_str = id.to_string();
    let result = sqlx::query("DELETE FROM cats WHERE id = ?")
        .bind(&id_str)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

fn color_to_str(c: &CatColor) -> &'static str {
    match c {
        CatColor::Green => "green",
        CatColor::Yellow => "yellow",
        CatColor::Blue => "blue",
    }
}

fn parse_color(s: &str) -> anyhow::Result<CatColor> {
    match s {
        "green" => Ok(CatColor::Green),
        "yellow" => Ok(CatColor::Yellow),
        "blue" => Ok(CatColor::Blue),
        other => anyhow::bail!("unknown color: {other}"),
    }
}

fn location_to_str(l: &CatLocation) -> &'static str {
    match l {
        CatLocation::Foster => "foster",
        CatLocation::AdoptionCenter => "adoption center",
    }
}

fn parse_location(s: &str) -> anyhow::Result<CatLocation> {
    match s {
        "foster" => Ok(CatLocation::Foster),
        "adoption center" => Ok(CatLocation::AdoptionCenter),
        other => anyhow::bail!("unknown location: {other}"),
    }
}
