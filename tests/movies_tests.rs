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

#[tokio::test]
async fn movies_hides_mark_counts_for_non_admins() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;
    insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    assert!(!body.contains("media-card__marks"));
}

#[tokio::test]
async fn movies_shows_mark_counts_for_admins() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;
    insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    assert!(body.contains("media-card__marks"));
}

#[tokio::test]
async fn movies_sort_by_year_desc() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    rewinder::models::media::upsert(
        &pool,
        "movie",
        "Old Movie",
        Some(1990),
        None,
        "/movies/Old Movie (1990)",
        1_000_000,
    )
    .await
    .unwrap();
    rewinder::models::media::upsert(
        &pool,
        "movie",
        "New Movie",
        Some(2022),
        None,
        "/movies/New Movie (2022)",
        1_000_000,
    )
    .await
    .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies?sort=year&dir=desc", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    let new_idx = body.find("New Movie").unwrap();
    let old_idx = body.find("Old Movie").unwrap();
    assert!(new_idx < old_idx, "expected New Movie before Old Movie");
}

#[tokio::test]
async fn movies_sort_by_added_desc() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let first_id = insert_movie(&pool, "First Movie", "/movies/First Movie (2000)").await;
    let second_id = insert_movie(&pool, "Second Movie", "/movies/Second Movie (2001)").await;

    // Set distinct first_seen values without sleeping
    sqlx::query("UPDATE media SET first_seen = '2024-01-01 00:00:00' WHERE id = ?")
        .bind(first_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE media SET first_seen = '2024-06-01 00:00:00' WHERE id = ?")
        .bind(second_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies?sort=added&dir=desc", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    let second_idx = body.find("Second Movie").unwrap();
    let first_idx = body.find("First Movie").unwrap();
    assert!(second_idx < first_idx, "expected most recently added first");
}

#[tokio::test]
async fn set_and_read_poster_path() {
    let pool = test_pool().await;
    let movie_id = insert_movie(&pool, "Inception", "/movies/Inception (2010)").await;

    // Initially needs poster
    assert!(rewinder::models::media::needs_poster(&pool, movie_id)
        .await
        .unwrap());

    // Set poster
    rewinder::models::media::set_poster(&pool, movie_id, "/abc123.jpg")
        .await
        .unwrap();

    // No longer needs poster
    assert!(!rewinder::models::media::needs_poster(&pool, movie_id)
        .await
        .unwrap());

    // Verify stored value
    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.poster_path.as_deref(), Some("/abc123.jpg"));
}
