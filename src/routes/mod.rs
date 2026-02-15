pub mod admin;
pub mod auth;
pub mod movies;
pub mod sort;
pub mod tv;

use crate::config::AppConfig;
use axum::Router;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<AppConfig>,
    pub dry_run: bool,
}

impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(auth::router())
        .merge(movies::router())
        .merge(tv::router())
        .merge(admin::router())
        .with_state(state)
}
