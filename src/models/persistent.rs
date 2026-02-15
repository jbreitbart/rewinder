use sqlx::SqlitePool;

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct PersistentOwner {
    pub media_id: i64,
    pub user_id: i64,
    pub persisted_at: String,
}

pub async fn set_owner(pool: &SqlitePool, media_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO persistent_media (media_id, user_id)
         VALUES (?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           user_id = excluded.user_id,
           persisted_at = datetime('now')",
    )
    .bind(media_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_owner(pool: &SqlitePool, media_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM persistent_media WHERE media_id = ?")
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_owner(
    pool: &SqlitePool,
    media_id: i64,
) -> Result<Option<PersistentOwner>, sqlx::Error> {
    sqlx::query_as::<_, PersistentOwner>("SELECT * FROM persistent_media WHERE media_id = ?")
        .bind(media_id)
        .fetch_optional(pool)
        .await
}

pub async fn owner_for_media_ids(
    pool: &SqlitePool,
    media_ids: &[i64],
) -> Result<Vec<PersistentOwner>, sqlx::Error> {
    if media_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut conn = pool.acquire().await?;
    sqlx::query("CREATE TEMP TABLE IF NOT EXISTS _owner_ids (id INTEGER NOT NULL)")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM _owner_ids")
        .execute(&mut *conn)
        .await?;

    for chunk in media_ids.chunks(500) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "(?)").collect();
        let query = format!(
            "INSERT INTO _owner_ids (id) VALUES {}",
            placeholders.join(",")
        );
        let mut q = sqlx::query(&query);
        for id in chunk {
            q = q.bind(id);
        }
        q.execute(&mut *conn).await?;
    }

    let rows = sqlx::query_as::<_, PersistentOwner>(
        "SELECT pm.media_id, pm.user_id, pm.persisted_at
         FROM persistent_media pm
         JOIN _owner_ids t ON t.id = pm.media_id",
    )
    .fetch_all(&mut *conn)
    .await?;

    sqlx::query("DELETE FROM _owner_ids")
        .execute(&mut *conn)
        .await?;

    Ok(rows)
}

pub async fn list_media_ids_by_owner(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT media_id FROM persistent_media WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
