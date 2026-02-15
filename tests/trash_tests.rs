mod common;

use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn all_users_mark_triggers_trash() {
    let pool = test_pool().await;
    let config = test_config(vec![]);

    let (user1_id, _) = create_test_user(&pool, "alice", false).await;
    let (user2_id, _) = create_test_user(&pool, "bob", false).await;
    let cookie1 = login_cookie(&pool, user1_id).await;
    let cookie2 = login_cookie(&pool, user2_id).await;

    let movie_id = insert_movie(&pool, "Old Movie", "/movies/Old Movie (2010)").await;

    // User 1 marks
    let app = test_app(pool.clone(), config.clone(), true);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/mark"),
        "",
        &cookie1,
    ))
    .await
    .unwrap();

    // Should still be active (only 1 of 2 users marked)
    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "active");

    // User 2 marks
    let app = test_app(pool.clone(), config, true);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/mark"),
        "",
        &cookie2,
    ))
    .await
    .unwrap();

    // Should now be trashed (all users marked)
    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "trashed");
}

#[tokio::test]
async fn single_user_mark_trashes_immediately() {
    let pool = test_pool().await;
    let config = test_config(vec![]);

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = insert_movie(&pool, "Solo Movie", "/movies/Solo Movie (2020)").await;

    let app = test_app(pool.clone(), config, true);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/mark"),
        "",
        &cookie,
    ))
    .await
    .unwrap();

    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "trashed");
}

#[tokio::test]
async fn delete_user_triggers_auto_trash() {
    // Setup: 3 users (alice, bob, admin). Alice and admin mark a movie.
    // Bob hasn't marked, so movie stays active. Delete bob → now all
    // remaining users have marked → movie gets trashed.
    let pool = test_pool().await;
    let config = test_config(vec![]);

    let (user_a, _) = create_test_user(&pool, "alice", false).await;
    let (user_b, _) = create_test_user(&pool, "bob", false).await;
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let admin_cookie = login_cookie(&pool, admin_id).await;

    let movie_id = insert_movie(&pool, "Some Movie", "/movies/Some Movie (2020)").await;

    // Alice and admin mark
    rewinder::models::mark::mark(&pool, user_a, movie_id)
        .await
        .unwrap();
    rewinder::models::mark::mark(&pool, admin_id, movie_id)
        .await
        .unwrap();

    // Not trashed yet because bob hasn't marked
    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "active");

    // Delete bob → now all remaining users (alice, admin) have marked
    let app = test_app(pool.clone(), config, true);
    app.oneshot(post_form_with_cookie(
        &format!("/admin/users/{user_b}/delete"),
        "",
        &admin_cookie,
    ))
    .await
    .unwrap();

    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "trashed");
}

#[tokio::test]
async fn rescue_restores_and_clears_marks() {
    let pool = test_pool().await;
    let config = test_config(vec![]);

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let admin_cookie = login_cookie(&pool, admin_id).await;

    let movie_id = insert_movie(&pool, "Rescued Movie", "/movies/Rescued Movie (2020)").await;

    // Trash it
    rewinder::models::mark::mark(&pool, user_id, movie_id)
        .await
        .unwrap();
    rewinder::models::mark::mark(&pool, admin_id, movie_id)
        .await
        .unwrap();
    rewinder::models::media::set_trashed(&pool, movie_id)
        .await
        .unwrap();

    // Rescue (dry_run mode won't try filesystem)
    let app = test_app(pool.clone(), config, true);
    app.oneshot(post_form_with_cookie(
        &format!("/admin/trash/{movie_id}/rescue"),
        "",
        &admin_cookie,
    ))
    .await
    .unwrap();

    // Should be active again
    let media = rewinder::models::media::get_by_id(&pool, movie_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media.status, "active");

    // Marks should be cleared
    let count = rewinder::models::mark::mark_count(&pool, movie_id)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn trash_with_real_filesystem() {
    let media_dir = tempfile::tempdir().unwrap();

    // Create a movie directory with a file
    let movie_path = media_dir.path().join("Test Movie (2020)");
    std::fs::create_dir(&movie_path).unwrap();
    std::fs::write(movie_path.join("movie.mkv"), "fake video content").unwrap();

    let pool = test_pool().await;
    let config = test_config(vec![media_dir.path().to_path_buf()]);
    let trash_dir = rewinder::config::AppConfig::trash_dir_for_media_dir(media_dir.path()).unwrap();

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let movie_id = rewinder::models::media::upsert(
        &pool,
        "movie",
        "Test Movie",
        Some(2020),
        None,
        movie_path.to_str().unwrap(),
        100,
    )
    .await
    .unwrap();

    // Mark with dry_run: false — single user, should trash immediately
    let app = test_app(pool.clone(), config.clone(), false);
    app.oneshot(post_form_with_cookie(
        &format!("/movies/{movie_id}/mark"),
        "",
        &cookie,
    ))
    .await
    .unwrap();

    // File should have moved to trash
    assert!(!movie_path.exists(), "original should be gone");
    assert!(
        trash_dir.join("Test Movie (2020)").exists(),
        "should be in trash"
    );

    // Rescue
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let admin_cookie = login_cookie(&pool, admin_id).await;

    let app = test_app(pool.clone(), config, false);
    app.oneshot(post_form_with_cookie(
        &format!("/admin/trash/{movie_id}/rescue"),
        "",
        &admin_cookie,
    ))
    .await
    .unwrap();

    // File should be back
    assert!(movie_path.exists(), "movie should be restored");
    assert!(
        !trash_dir.join("Test Movie (2020)").exists(),
        "trash should be empty"
    );
}

#[tokio::test]
async fn tv_trash_preserves_show_subdirectory() {
    let media_dir = tempfile::tempdir().unwrap();

    // Create a TV show season directory with a file
    let show_path = media_dir.path().join("Breaking Bad");
    let season_path = show_path.join("Season 1");
    std::fs::create_dir_all(&season_path).unwrap();
    std::fs::write(season_path.join("episode1.mkv"), "fake video content").unwrap();

    let pool = test_pool().await;
    let config = test_config(vec![media_dir.path().to_path_buf()]);
    let trash_dir = rewinder::config::AppConfig::trash_dir_for_media_dir(media_dir.path()).unwrap();

    let (user_id, _) = create_test_user(&pool, "alice", false).await;
    let cookie = login_cookie(&pool, user_id).await;

    let tv_id = rewinder::models::media::upsert(
        &pool,
        "tv_season",
        "Breaking Bad",
        None,
        Some(1),
        season_path.to_str().unwrap(),
        100,
    )
    .await
    .unwrap();

    // Mark with dry_run: false — single user, should trash immediately
    let app = test_app(pool.clone(), config.clone(), false);
    app.oneshot(post_form_with_cookie(
        &format!("/tv/{tv_id}/mark"),
        "",
        &cookie,
    ))
    .await
    .unwrap();

    // Season path should be preserved under trash
    assert!(!season_path.exists(), "original season path should be gone");
    assert!(
        trash_dir.join("Breaking Bad").join("Season 1").exists(),
        "season should be in nested trash path"
    );

    // Rescue
    let (admin_id, _) = create_test_user(&pool, "admin", true).await;
    let admin_cookie = login_cookie(&pool, admin_id).await;

    let app = test_app(pool.clone(), config, false);
    app.oneshot(post_form_with_cookie(
        &format!("/admin/trash/{tv_id}/rescue"),
        "",
        &admin_cookie,
    ))
    .await
    .unwrap();

    assert!(season_path.exists(), "season path should be restored");
    assert!(
        !trash_dir.join("Breaking Bad").join("Season 1").exists(),
        "nested trash path should be empty after rescue"
    );
}
