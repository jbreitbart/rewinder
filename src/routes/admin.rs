use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

use crate::auth::middleware::AdminUser;
use crate::auth::session;
use crate::error::AppError;
use crate::models::{mark, media, persistent, user};
use crate::routes::AppState;
use crate::templates;
use crate::templates::{AdminDashboardTemplate, AdminTrashTemplate, AdminUsersTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/users", get(users_page).post(create_user))
        .route("/admin/users/{id}/delete", post(delete_user))
        .route("/admin/trash", get(trash_page))
        .route("/admin/trash/{id}/rescue", post(rescue_item))
        .route("/admin/scan", post(trigger_scan))
}

async fn dashboard(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Result<impl IntoResponse, AppError> {
    let active_count = media::count_by_status(&state.pool, "active").await?;
    let trashed_count = media::count_by_status(&state.pool, "trashed").await?;
    let active_size = media::total_active_size(&state.pool).await?;
    let trashed_size = media::total_trashed_size(&state.pool).await?;
    let user_count = user::count(&state.pool).await?;

    Ok(AdminDashboardTemplate {
        username: admin.username.clone(),
        is_admin: true,
        active_count,
        trashed_count,
        active_size: templates::format_size(&active_size),
        trashed_size: templates::format_size(&trashed_size),
        user_count,
    })
}

async fn users_page(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Result<impl IntoResponse, AppError> {
    let users = user::list_all(&state.pool).await?;

    Ok(AdminUsersTemplate {
        username: admin.username.clone(),
        is_admin: true,
        users,
        invite_url: None,
    })
}

#[derive(Deserialize)]
struct CreateUserForm {
    username: String,
}

async fn create_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<CreateUserForm>,
) -> Result<impl IntoResponse, AppError> {
    let token = session::generate_token();
    user::create(&state.pool, &form.username, false, Some(&token)).await?;

    let users = user::list_all(&state.pool).await?;
    let invite_url = format!("/invite/{token}");

    Ok(AdminUsersTemplate {
        username: admin.username.clone(),
        is_admin: true,
        users,
        invite_url: Some(invite_url),
    })
}

async fn delete_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let owned_persistent = persistent::list_media_ids_by_owner(&state.pool, id).await?;
    for media_id in owned_persistent {
        crate::persistent::restore_from_permanent_unchecked(
            &state.pool,
            media_id,
            &state.config,
            state.dry_run,
        )
        .await
        .map_err(|e| AppError::Internal(format!("failed to restore persistent media: {e}")))?;
    }

    user::delete(&state.pool, id).await?;

    // After deleting a user, check if any media now has all users marked
    let eligible = mark::media_ids_with_all_marked(&state.pool).await?;
    for media_id in eligible {
        let _ = crate::trash::check_and_trash(&state.pool, media_id, &state.config, state.dry_run)
            .await;
    }

    Ok(Redirect::to("/admin/users").into_response())
}

async fn trash_page(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Result<impl IntoResponse, AppError> {
    let items = media::list_trashed(&state.pool).await?;

    Ok(AdminTrashTemplate {
        username: admin.username.clone(),
        is_admin: true,
        items,
    })
}

async fn rescue_item(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    crate::trash::rescue_from_trash(&state.pool, id, &state.config, state.dry_run)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Redirect::to("/admin/trash").into_response())
}

async fn trigger_scan(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Result<Response, AppError> {
    let pool = state.pool.clone();
    let media_dirs = state.config.media_dirs.clone();

    tokio::spawn(async move {
        if let Err(e) = crate::scanner::full_scan(&pool, &media_dirs, None).await {
            tracing::error!("Manual scan failed: {e}");
        }
    });

    Ok(Redirect::to("/admin").into_response())
}
