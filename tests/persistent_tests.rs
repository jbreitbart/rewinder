mod common;

use axum::http::StatusCode;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn persist_hides_item_from_other_users() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (alice_id, _) = create_test_user(&pool, "alice", false).await;
    let (bob_id, _) = create_test_user(&pool, "bob", false).await;
    let alice_cookie = login_cookie(&pool, alice_id).await;
    let bob_cookie = login_cookie(&pool, bob_id).await;

    let movie_id = insert_movie(&pool, "Private Movie", "/movies/Private Movie (2020)").await;

    let app = test_app(pool.clone(), config.clone(), true);
    let response = app
        .oneshot(post_form_with_cookie(
            &format!("/movies/{movie_id}/persist"),
            "",
            &alice_cookie,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let app = test_app(pool.clone(), config.clone(), true);
    let body_alice = body_string(
        app.oneshot(get_with_cookie("/movies", &alice_cookie))
            .await
            .unwrap(),
    )
    .await;
    assert!(body_alice.contains("Private Movie"));
    assert!(body_alice.contains("Persisted by you"));

    let app = test_app(pool, config, true);
    let body_bob = body_string(
        app.oneshot(get_with_cookie("/movies", &bob_cookie))
            .await
            .unwrap(),
    )
    .await;
    assert!(!body_bob.contains("Private Movie"));
}

#[tokio::test]
async fn non_owner_cannot_unpersist() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (alice_id, _) = create_test_user(&pool, "alice", false).await;
    let (bob_id, _) = create_test_user(&pool, "bob", false).await;
    let alice_cookie = login_cookie(&pool, alice_id).await;
    let bob_cookie = login_cookie(&pool, bob_id).await;

    let movie_id = insert_movie(&pool, "Locked Movie", "/movies/Locked Movie (2020)").await;

    let app = test_app(pool.clone(), config.clone(), true);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/persist"),
        "",
        &alice_cookie,
    ))
    .await
    .unwrap();

    let app = test_app(pool, config, true);
    let response = app
        .oneshot(delete_with_cookie(
            &format!("/movies/{movie_id}/persist"),
            &bob_cookie,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn owner_delete_restores_persisted_to_active() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (owner_id, _) = create_test_user(&pool, "owner", false).await;
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let owner_cookie = login_cookie(&pool, owner_id).await;
    let admin_cookie = login_cookie(&pool, admin_id).await;

    let movie_id = insert_movie(&pool, "Restorable", "/movies/Restorable (2020)").await;

    let app = test_app(pool.clone(), config.clone(), true);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/persist"),
        "",
        &owner_cookie,
    ))
    .await
    .unwrap();

    let app = test_app(pool.clone(), config, true);
    app.oneshot(post_form_with_cookie(
        &format!("/admin/users/{owner_id}/delete"),
        "",
        &admin_cookie,
    ))
    .await
    .unwrap();

    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "active");
    let owner = rewinder::models::persistent::get_owner(&pool, movie_id)
        .await
        .unwrap();
    assert!(owner.is_none());
}

#[tokio::test]
async fn persist_then_unpersist_moves_real_filesystem() {
    let media_dir = tempfile::tempdir().unwrap();
    let movie_path = media_dir.path().join("Keep Forever (2020)");
    std::fs::create_dir(&movie_path).unwrap();
    std::fs::write(movie_path.join("movie.mkv"), "fake video content").unwrap();

    let pool = test_pool().await;
    let config = test_config(vec![media_dir.path().to_path_buf()]);
    let permanent_dir =
        rewinder::config::AppConfig::permanent_dir_for_media_dir(media_dir.path()).unwrap();

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = rewinder::models::media::upsert(
        &pool,
        "movie",
        "Keep Forever",
        Some(2020),
        None,
        movie_path.to_str().unwrap(),
        100,
    )
    .await
    .unwrap();

    let app = test_app(pool.clone(), config.clone(), false);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/persist"),
        "",
        &cookie,
    ))
    .await
    .unwrap();

    assert!(!movie_path.exists(), "original should be moved");
    let persisted_path = permanent_dir.join("Keep Forever (2020)");
    assert!(persisted_path.exists(), "should be in permanent");

    let app = test_app(pool, config, false);
    app.oneshot(delete_with_cookie(
        &format!("/movies/{movie_id}/persist"),
        &cookie,
    ))
    .await
    .unwrap();

    assert!(movie_path.exists(), "should be restored to media dir");
    assert!(!persisted_path.exists(), "permanent path should be empty");
}

#[tokio::test]
async fn tv_persist_all_series_persists_every_season() {
    let pool = test_pool().await;
    let config = test_config(vec![]);
    let (alice_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, alice_id).await;

    let s1 = insert_tv_season(&pool, "Slow Horses", 1, "/tv/Slow Horses/Season 1").await;
    let s2 = insert_tv_season(&pool, "Slow Horses", 2, "/tv/Slow Horses/Season 2").await;

    let app = test_app(pool.clone(), config, true);
    let response = app
        .oneshot(post_form_with_cookie(
            "/tv/series/Slow%20Horses/persist-all",
            "",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let media1 = rewinder::models::media::get_by_id(&pool, s1)
        .await
        .unwrap()
        .unwrap();
    let media2 = rewinder::models::media::get_by_id(&pool, s2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media1.status, "permanent");
    assert_eq!(media2.status, "permanent");
}
