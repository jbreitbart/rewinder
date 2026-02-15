use sqlx::SqlitePool;

#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub invite_token: Option<String>,
    pub created_at: String,
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await
}

pub async fn get_by_invite_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE invite_token = ?")
        .bind(token)
        .fetch_optional(pool)
        .await
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn create(
    pool: &SqlitePool,
    username: &str,
    is_admin: bool,
    invite_token: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let result =
        sqlx::query("INSERT INTO users (username, is_admin, invite_token) VALUES (?, ?, ?)")
            .bind(username)
            .bind(is_admin)
            .bind(invite_token)
            .execute(pool)
            .await?;
    Ok(result.last_insert_rowid())
}

pub async fn set_password(
    pool: &SqlitePool,
    id: i64,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = ?, invite_token = NULL WHERE id = ?")
        .bind(password_hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}
