#![allow(dead_code, unused_imports)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

use rewinder::config::AppConfig;
use rewinder::routes::{build_router, AppState};

pub async fn test_pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to create test pool");

    rewinder::db::run_migrations(&pool)
        .await
        .expect("failed to run migrations");

    pool
}

pub fn test_config(media_dirs: Vec<PathBuf>) -> AppConfig {
    let media_dirs = if media_dirs.is_empty() {
        vec![PathBuf::from("/movies"), PathBuf::from("/tv")]
    } else {
        media_dirs
    };

    AppConfig {
        database_url: ":memory:".to_string(),
        listen_addr: "127.0.0.1:0".to_string(),
        media_dirs,
        grace_period_days: 7,
        cleanup_interval_hours: 1,
        initial_admin_user: None,
    }
}

pub fn test_app(pool: SqlitePool, config: AppConfig, dry_run: bool) -> Router {
    let state = AppState {
        pool,
        config: Arc::new(config),
        dry_run,
    };
    build_router(state)
}

pub async fn create_test_user(pool: &SqlitePool, username: &str, is_admin: bool) -> (i64, String) {
    let password = "testpass123";
    let hash = rewinder::auth::hash_password(password).expect("hash failed");
    let id = rewinder::models::user::create(pool, username, is_admin, None)
        .await
        .expect("create user failed");
    rewinder::models::user::set_password(pool, id, &hash)
        .await
        .expect("set password failed");
    (id, password.to_string())
}

pub async fn login_cookie(pool: &SqlitePool, user_id: i64) -> String {
    let token = rewinder::auth::session::create(pool, user_id, 720)
        .await
        .expect("create session failed");
    format!("session={token}")
}

pub async fn insert_movie(pool: &SqlitePool, title: &str, path: &str) -> i64 {
    rewinder::models::media::upsert(pool, "movie", title, Some(2020), None, path, 1_000_000)
        .await
        .expect("insert movie failed")
}

pub async fn insert_tv_season(pool: &SqlitePool, title: &str, season: i64, path: &str) -> i64 {
    rewinder::models::media::upsert(
        pool,
        "tv_season",
        title,
        None,
        Some(season),
        path,
        2_000_000,
    )
    .await
    .expect("insert tv season failed")
}

pub fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

pub fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

pub fn post_form(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn post_form_with_cookie(uri: &str, body: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("cookie", cookie)
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn delete_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

pub async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("failed to read body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("body not utf8")
}

#[allow(dead_code)]
pub async fn assert_redirect(response: &axum::http::Response<Body>, expected_location: &str) {
    assert!(
        response.status() == StatusCode::SEE_OTHER
            || response.status() == StatusCode::TEMPORARY_REDIRECT,
        "expected redirect status, got {}",
        response.status()
    );
    let location = response
        .headers()
        .get("location")
        .expect("no location header")
        .to_str()
        .unwrap();
    assert_eq!(location, expected_location);
}
