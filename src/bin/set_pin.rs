/// CLI tool to set or clear the app PIN stored in SQLite.
///
/// Usage:
///   set-pin <pin>   — set the PIN
///   set-pin --clear — remove PIN (disables auth)
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "cats.db".into());
    let url = if database_url.starts_with("sqlite:") {
        database_url.clone()
    } else {
        format!("sqlite:{database_url}?mode=rwc")
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;

    // Ensure migrations have run.
    sqlx::migrate!("./migrations").run(&pool).await?;

    match args.first().map(String::as_str) {
        Some("--clear") => {
            sqlx::query("DELETE FROM settings WHERE key = 'pin_hash'")
                .execute(&pool)
                .await?;
            println!("PIN cleared — authentication is now disabled.");
        }
        Some(pin) if !pin.is_empty() => {
            let hash = bcrypt::hash(pin, bcrypt::DEFAULT_COST)?;
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES ('pin_hash', ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(&hash)
            .execute(&pool)
            .await?;
            println!("PIN set successfully.");
        }
        _ => {
            eprintln!("Usage: set-pin <pin>\n       set-pin --clear");
            std::process::exit(1);
        }
    }

    Ok(())
}
