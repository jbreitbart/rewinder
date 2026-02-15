use sqlx::SqlitePool;

pub async fn mark(pool: &SqlitePool, user_id: i64, media_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO marks (user_id, media_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn unmark(pool: &SqlitePool, user_id: i64, media_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM marks WHERE user_id = ? AND media_id = ?")
        .bind(user_id)
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_count(pool: &SqlitePool, media_id: i64) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM marks WHERE media_id = ?")
        .bind(media_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn all_users_marked(pool: &SqlitePool, media_id: i64) -> Result<bool, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users
         WHERE id NOT IN (SELECT user_id FROM marks WHERE media_id = ?)",
    )
    .bind(media_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0 == 0)
}

pub async fn clear_marks(pool: &SqlitePool, media_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM marks WHERE media_id = ?")
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get list of media IDs that a user has marked
pub async fn user_marks(pool: &SqlitePool, user_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT media_id FROM marks WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// After a user is deleted, check all media for auto-trash eligibility
pub async fn media_ids_with_all_marked(pool: &SqlitePool) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT m.id FROM media m
         WHERE m.status = 'active'
         AND NOT EXISTS (
             SELECT 1 FROM users u
             WHERE u.id NOT IN (SELECT mk.user_id FROM marks mk WHERE mk.media_id = m.id)
         )",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
