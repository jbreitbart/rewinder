use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

const MIGRATIONS: [(&str, &str); 3] = [
    ("001_initial", include_str!("../migrations/001_initial.sql")),
    (
        "002_add_permanent_media",
        include_str!("../migrations/002_add_permanent_media.sql"),
    ),
    (
        "003_poster_path",
        include_str!("../migrations/003_poster_path.sql"),
    ),
];

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    for (version, sql) in MIGRATIONS {
        let already_applied: Option<(String,)> =
            sqlx::query_as("SELECT version FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_optional(pool)
                .await?;
        if already_applied.is_some() {
            continue;
        }

        sqlx::raw_sql(sql).execute(pool).await?;
        sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
            .bind(version)
            .execute(pool)
            .await?;
    }

    Ok(())
}

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}
