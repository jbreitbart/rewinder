use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::auth::middleware::AuthUser;
use crate::error::AppError;
use crate::models::{mark, media, user};
use crate::routes::AppState;
use crate::templates::{MediaRow, MediaRowPartial, TvTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tv", get(list_tv))
        .route("/tv/{id}/mark", post(mark_tv).delete(unmark_tv))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    show_marked: Option<String>,
}

async fn list_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let show_marked = query.show_marked.as_deref() == Some("true");
    let all_media = media::list_by_type(&state.pool, "tv_season").await?;
    let user_marks = mark::user_marks(&state.pool, auth.id).await?;
    let total_users = user::count(&state.pool).await?;

    let mut items = Vec::new();
    for m in all_media {
        let marked = user_marks.contains(&m.id);
        if !show_marked && marked {
            continue;
        }
        let mark_count = mark::mark_count(&state.pool, m.id).await?;
        items.push(MediaRow {
            media: m,
            marked,
            mark_count,
            total_users,
        });
    }

    Ok(TvTemplate {
        username: auth.username,
        is_admin: auth.is_admin,
        items,
        show_marked,
    })
}

async fn mark_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    mark::mark(&state.pool, auth.id, id).await?;

    let _ = crate::trash::check_and_trash(&state.pool, id, &state.config.trash_dir, state.dry_run).await;

    let media_item = media::get_by_id(&state.pool, id).await?.unwrap_or(m);
    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaRowPartial {
        item: MediaRow {
            media: media_item,
            marked: true,
            mark_count,
            total_users,
        },
    })
}

async fn unmark_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    mark::unmark(&state.pool, auth.id, id).await?;

    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaRowPartial {
        item: MediaRow {
            media: m,
            marked: false,
            mark_count,
            total_users,
        },
    })
}
