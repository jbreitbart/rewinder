mod common;

use axum::http::StatusCode;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn list_movies_empty() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_movies_shows_items() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;
    insert_movie(&pool, "The Matrix", "/movies/The Matrix (1999)").await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Inception"));
    assert!(body.contains("The Matrix"));
}

#[tokio::test]
async fn mark_movie() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    let app = test_app(pool.clone(), config, true);
    let response = app
        .oneshot(post_form_with_cookie(
            &format!("/movies/{movie_id}/mark"),
            "",
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify mark count increased
    let count = rewinder::models::mark::mark_count(&pool, movie_id)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn unmark_movie() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    // Mark first
    rewinder::models::mark::mark(&pool, user_id, movie_id)
        .await
        .unwrap();

    let app = test_app(pool.clone(), config, true);
    let response = app
        .oneshot(delete_with_cookie(
            &format!("/movies/{movie_id}/mark"),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let count = rewinder::models::mark::mark_count(&pool, movie_id)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn mark_nonexistent_movie() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(post_form_with_cookie("/movies/9999/mark", "", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn movies_hides_marked_by_default() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    // Create a second user so marking doesn't trash the movie
    create_test_user(&pool, "bob", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;
    insert_movie(&pool, "The Matrix", "/movies/The Matrix (1999)").await;

    // Mark Inception
    rewinder::models::mark::mark(&pool, user_id, movie_id)
        .await
        .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    assert!(!body.contains("Inception"));
    assert!(body.contains("The Matrix"));
}

#[tokio::test]
async fn movies_show_marked_param() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    // Create a second user so marking doesn't trash the movie
    create_test_user(&pool, "bob", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    // Mark Inception
    rewinder::models::mark::mark(&pool, user_id, movie_id)
        .await
        .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies?show_marked=true", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    assert!(body.contains("Inception"));
}
