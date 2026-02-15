use sqlx::SqlitePool;

#[allow(dead_code)] // fields used by sqlx::FromRow deserialization
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Media {
    pub id: i64,
    pub media_type: String,
    pub title: String,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub path: String,
    pub size_bytes: i64,
    pub status: String,
    pub trashed_at: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
}

pub async fn list_by_type(
    pool: &SqlitePool,
    media_type: &str,
) -> Result<Vec<Media>, sqlx::Error> {
    sqlx::query_as::<_, Media>(
        "SELECT * FROM media WHERE media_type = ? AND status = 'active' ORDER BY title, season",
    )
    .bind(media_type)
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Media>, sqlx::Error> {
    sqlx::query_as::<_, Media>("SELECT * FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn upsert(
    pool: &SqlitePool,
    media_type: &str,
    title: &str,
    year: Option<i64>,
    season: Option<i64>,
    path: &str,
    size_bytes: i64,
) -> Result<i64, sqlx::Error> {
    // Try insert first
    let result = sqlx::query(
        "INSERT INTO media (media_type, title, year, season, path, size_bytes)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
           last_seen = datetime('now'),
           status = 'active',
           size_bytes = excluded.size_bytes",
    )
    .bind(media_type)
    .bind(title)
    .bind(year)
    .bind(season)
    .bind(path)
    .bind(size_bytes)
    .execute(pool)
    .await?;

    if result.last_insert_rowid() != 0 {
        Ok(result.last_insert_rowid())
    } else {
        // Was an update, fetch the id
        let row: (i64,) = sqlx::query_as("SELECT id FROM media WHERE path = ?")
            .bind(path)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

pub async fn mark_gone_except(pool: &SqlitePool, seen_paths: &[String]) -> Result<(), sqlx::Error> {
    if seen_paths.is_empty() {
        sqlx::query("UPDATE media SET status = 'gone' WHERE status = 'active'")
            .execute(pool)
            .await?;
        return Ok(());
    }

    // Mark active items as gone if their path wasn't seen
    // SQLite doesn't support array binds, so we build placeholders
    let placeholders: Vec<&str> = seen_paths.iter().map(|_| "?").collect();
    let query = format!(
        "UPDATE media SET status = 'gone' WHERE status = 'active' AND path NOT IN ({})",
        placeholders.join(",")
    );
    let mut q = sqlx::query(&query);
    for path in seen_paths {
        q = q.bind(path);
    }
    q.execute(pool).await?;
    Ok(())
}

pub async fn mark_gone_by_path(pool: &SqlitePool, path: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET status = 'gone' WHERE path = ? AND status = 'active'")
        .bind(path)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_trashed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE media SET status = 'trashed', trashed_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET status = 'active', trashed_at = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_trashed(pool: &SqlitePool) -> Result<Vec<Media>, sqlx::Error> {
    sqlx::query_as::<_, Media>(
        "SELECT * FROM media WHERE status = 'trashed' ORDER BY trashed_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_expired_trash(
    pool: &SqlitePool,
    grace_period_days: u64,
) -> Result<Vec<Media>, sqlx::Error> {
    sqlx::query_as::<_, Media>(
        "SELECT * FROM media WHERE status = 'trashed'
         AND trashed_at <= datetime('now', ? || ' days')",
    )
    .bind(-(grace_period_days as i64))
    .fetch_all(pool)
    .await
}

pub async fn set_gone(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET status = 'gone' WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn total_active_size(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(size_bytes), 0) FROM media WHERE status = 'active'")
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn total_trashed_size(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(size_bytes), 0) FROM media WHERE status = 'trashed'")
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn count_by_status(pool: &SqlitePool, status: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media WHERE status = ?")
        .bind(status)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn cleanup_gone_marks(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM marks WHERE media_id IN (SELECT id FROM media WHERE status = 'gone')",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
