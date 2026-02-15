mod common;

use axum::http::StatusCode;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn list_tv_shows_seasons() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    insert_tv_season(&pool, "Breaking Bad", 1, "/tv/Breaking Bad/Season 1").await;
    insert_tv_season(&pool, "Breaking Bad", 2, "/tv/Breaking Bad/Season 2").await;

    let app = test_app(pool, config, true);
    let response = app.oneshot(get_with_cookie("/tv", &cookie)).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Breaking Bad"));
}

#[tokio::test]
async fn mark_unmark_tv() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    // Second user so marking doesn't trash
    create_test_user(&pool, "bob", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let tv_id = insert_tv_season(&pool, "Breaking Bad", 1, "/tv/Breaking Bad/Season 1").await;

    // Mark
    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(post_form_with_cookie(
            &format!("/tv/{tv_id}/mark"),
            "",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let count = rewinder::models::mark::mark_count(&pool, tv_id)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Unmark
    let app = test_app(pool.clone(), config, true);
    let response = app
        .oneshot(delete_with_cookie(&format!("/tv/{tv_id}/mark"), &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let count = rewinder::models::mark::mark_count(&pool, tv_id)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn tv_hides_mark_counts_for_non_admins() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;
    insert_tv_season(&pool, "Breaking Bad", 1, "/tv/Breaking Bad/Season 1").await;

    let app = test_app(pool, config, true);
    let response = app.oneshot(get_with_cookie("/tv", &cookie)).await.unwrap();

    let body = body_string(response).await;
    assert!(!body.contains("<th>Marked</th>"));
}

#[tokio::test]
async fn tv_shows_mark_counts_for_admins() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;
    insert_tv_season(&pool, "Breaking Bad", 1, "/tv/Breaking Bad/Season 1").await;

    let app = test_app(pool, config, true);
    let response = app.oneshot(get_with_cookie("/tv", &cookie)).await.unwrap();

    let body = body_string(response).await;
    assert!(body.contains(">Marked</a></th>"));
}

#[tokio::test]
async fn tv_sort_by_season_desc() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    insert_tv_season(&pool, "Breaking Bad", 1, "/tv/Breaking Bad/Season 1").await;
    insert_tv_season(&pool, "Breaking Bad", 2, "/tv/Breaking Bad/Season 2").await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/tv?sort=season&dir=desc", &cookie))
        .await
        .unwrap();

    let body = body_string(response).await;
    let season2_idx = body.find("Season 2").unwrap();
    let season1_idx = body.find("Season 1").unwrap();
    assert!(season2_idx < season1_idx, "expected Season 2 before Season 1");
}
