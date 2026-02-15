mod common;

use axum::http::StatusCode;
use std::path::PathBuf;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn unauthenticated_redirects_to_login() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);
    let app = test_app(pool, config, true);

    let response = app.oneshot(get("/movies")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("location").unwrap().to_str().unwrap(),
        "/login"
    );
}

#[tokio::test]
async fn login_page_returns_200() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);
    let app = test_app(pool, config, true);

    let response = app.oneshot(get("/login")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_with_valid_credentials() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);
    let app = test_app(pool.clone(), config, true);

    let (_id, password) = create_test_user(&pool, "alice", false).await;

    let response = app
        .oneshot(post_form(
            "/login",
            &format!("username=alice&password={password}"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("location").unwrap().to_str().unwrap(),
        "/movies"
    );
    // Should have a Set-Cookie header
    assert!(response.headers().get("set-cookie").is_some());
}

#[tokio::test]
async fn login_with_wrong_password() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);
    let app = test_app(pool.clone(), config, true);

    create_test_user(&pool, "alice", false).await;

    let response = app
        .oneshot(post_form("/login", "username=alice&password=wrongpass"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Invalid username or password"));
}

#[tokio::test]
async fn logout_clears_session() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    // Verify we can access /movies with the cookie
    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Logout
    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(post_form_with_cookie("/logout", "", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // Subsequent request should redirect to login
    let app = test_app(pool, config, true);
    let response = app
        .oneshot(get_with_cookie("/movies", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn invite_flow() {
    let pool = test_pool().await;
    let config = test_config(PathBuf::from("/tmp/trash"), vec![]);

    // Create a user with an invite token
    let token = "test-invite-token-123";
    let user_id = rewinder::models::user::create(&pool, "bob", false, Some(token))
        .await
        .unwrap();

    // GET the invite page
    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(get(&format!("/invite/{token}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("bob"));

    // POST to set password
    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(post_form(
            &format!("/invite/{token}"),
            "password=newpassword123&password_confirm=newpassword123",
        ))
        .await
        .unwrap();

    // Should auto-login and redirect
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("location").unwrap().to_str().unwrap(),
        "/movies"
    );
    assert!(response.headers().get("set-cookie").is_some());

    // Verify user now has a password set
    let user = rewinder::models::user::get_by_id(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert!(user.password_hash.is_some());
    assert!(user.invite_token.is_none());
}
