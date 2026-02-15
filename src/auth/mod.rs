pub mod middleware;
pub mod session;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use sqlx::SqlitePool;

use crate::models::user;

pub fn hash_password(password: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("password hash error: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub async fn seed_admin(pool: &SqlitePool, username: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if user::get_by_username(pool, username).await?.is_some() {
        tracing::info!("Admin user '{username}' already exists, skipping seed");
        return Ok(());
    }

    let password = session::generate_token();
    let hash = hash_password(&password)?;
    let id = user::create(pool, username, true, None).await?;
    user::set_password(pool, id, &hash).await?;

    tracing::info!("Created admin user '{username}' with password: {password}");
    tracing::info!("Please change this password after first login");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash));
    }

    #[test]
    fn wrong_password_returns_false() {
        let hash = hash_password("real_password").unwrap();
        assert!(!verify_password("wrong_password", &hash));
    }
}
