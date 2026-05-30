/// CLI tool to manage users for CARECatStatus.
///
/// Usage:
///   manage-users add <username> <pin>
///   manage-users modify <username> <new-pin>
///   manage-users rename <username> <new-username>
///   manage-users delete <username>
///   manage-users list
use sqlx::{Row, sqlite::SqlitePoolOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let pool = connect().await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    match args.as_slice() {
        [cmd, username, pin] if cmd == "add" => {
            validate_username(username)?;
            let exists: bool = sqlx::query("SELECT 1 FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&pool)
                .await?
                .is_some();
            if exists {
                anyhow::bail!("User '{username}' already exists. Use `modify` to change their PIN.");
            }
            let hash = bcrypt::hash(pin, bcrypt::DEFAULT_COST)?;
            sqlx::query("INSERT INTO users (username, pin_hash) VALUES (?, ?)")
                .bind(username)
                .bind(&hash)
                .execute(&pool)
                .await?;
            println!("User '{username}' added.");
        }

        [cmd, username, new_pin] if cmd == "modify" => {
            let hash = bcrypt::hash(new_pin, bcrypt::DEFAULT_COST)?;
            let rows = sqlx::query("UPDATE users SET pin_hash = ? WHERE username = ?")
                .bind(&hash)
                .bind(username)
                .execute(&pool)
                .await?
                .rows_affected();
            if rows == 0 {
                anyhow::bail!("User '{username}' not found.");
            }
            println!("PIN updated for '{username}'.");
        }

        [cmd, old_name, new_name] if cmd == "rename" => {
            validate_username(new_name)?;
            let exists: bool = sqlx::query("SELECT 1 FROM users WHERE username = ?")
                .bind(new_name)
                .fetch_optional(&pool)
                .await?
                .is_some();
            if exists {
                anyhow::bail!("Username '{new_name}' is already taken.");
            }
            let rows = sqlx::query(
                "INSERT INTO users (username, pin_hash)
                 SELECT ?, pin_hash FROM users WHERE username = ?",
            )
            .bind(new_name)
            .bind(old_name)
            .execute(&pool)
            .await?
            .rows_affected();
            if rows == 0 {
                anyhow::bail!("User '{old_name}' not found.");
            }
            sqlx::query("DELETE FROM users WHERE username = ?")
                .bind(old_name)
                .execute(&pool)
                .await?;
            println!("Renamed '{old_name}' → '{new_name}'.");
        }

        [cmd, username] if cmd == "delete" => {
            let rows = sqlx::query("DELETE FROM users WHERE username = ?")
                .bind(username)
                .execute(&pool)
                .await?
                .rows_affected();
            if rows == 0 {
                anyhow::bail!("User '{username}' not found.");
            }
            println!("User '{username}' deleted.");
        }

        [cmd] if cmd == "list" => {
            let rows = sqlx::query("SELECT username FROM users ORDER BY username ASC")
                .fetch_all(&pool)
                .await?;
            if rows.is_empty() {
                println!("No users.");
            } else {
                for row in &rows {
                    println!("{}", row.get::<String, _>("username"));
                }
            }
        }

        _ => {
            eprintln!(
                "Usage:
  manage-users add <username> <pin>
  manage-users modify <username> <new-pin>
  manage-users rename <username> <new-username>
  manage-users delete <username>
  manage-users list

Usernames must be lowercase letters and hyphens (e.g. jane-doe)."
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn connect() -> anyhow::Result<sqlx::SqlitePool> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "cats.db".into());
    let url = if database_url.starts_with("sqlite:") {
        database_url
    } else {
        format!("sqlite:{database_url}?mode=rwc")
    };
    Ok(SqlitePoolOptions::new().max_connections(1).connect(&url).await?)
}

fn validate_username(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
        || name.starts_with('-')
        || name.ends_with('-')
    {
        anyhow::bail!(
            "Invalid username '{name}'. Use lowercase letters and hyphens only (e.g. jane-doe)."
        );
    }
    Ok(())
}
