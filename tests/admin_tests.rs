mod common;

use axum::http::StatusCode;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn non_admin_redirected_from_admin() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/admin", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn admin_dashboard() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/admin", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_create_user() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(post_form_with_cookie(
            "/admin/users",
            "username=newuser",
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("/invite/"));
}

#[tokio::test]
async fn admin_delete_user() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;

    let (user_id, _) = create_test_user(&pool, "victim", false).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(post_form_with_cookie(
            &format!("/admin/users/{user_id}/delete"),
            "",
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("location").unwrap().to_str().unwrap(),
        "/admin/users"
    );
}

#[tokio::test]
async fn admin_trash_page() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/admin/trash", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_rescue_from_trash() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let cookie = login_cookie(&pool, admin_id).await;

    // Insert and trash a movie
    let movie_id = insert_movie(&pool, "Old Movie", "/movies/Old Movie (2010)").await;
    rewinder::models::media::set_trashed(&pool, movie_id)
        .await
        .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(post_form_with_cookie(
            &format!("/admin/trash/{movie_id}/rescue"),
            "",
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("location").unwrap().to_str().unwrap(),
        "/admin/trash"
    );
}
