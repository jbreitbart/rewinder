use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::cmp::Ordering;

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
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    dir: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("desc") => SortDir::Desc,
            _ => SortDir::Asc,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TvSortBy {
    Name,
    Season,
    Marked,
    Added,
}

impl TvSortBy {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("season") => TvSortBy::Season,
            Some("marked") => TvSortBy::Marked,
            Some("added") => TvSortBy::Added,
            _ => TvSortBy::Name,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            TvSortBy::Name => "name",
            TvSortBy::Season => "season",
            TvSortBy::Marked => "marked",
            TvSortBy::Added => "added",
        }
    }
}

fn apply_sort_dir(ordering: Ordering, sort_dir: SortDir) -> Ordering {
    match sort_dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    }
}

async fn list_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let show_marked = query.show_marked.as_deref() == Some("true");
    let sort_by = TvSortBy::parse(query.sort.as_deref());
    let sort_dir = SortDir::parse(query.dir.as_deref());
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

    items.sort_by(|a, b| {
        let ordering = match sort_by {
            TvSortBy::Name => a
                .media
                .title
                .cmp(&b.media.title)
                .then_with(|| a.media.season.cmp(&b.media.season)),
            TvSortBy::Season => a
                .media
                .season
                .cmp(&b.media.season)
                .then_with(|| a.media.title.cmp(&b.media.title)),
            TvSortBy::Marked => a
                .marked
                .cmp(&b.marked)
                .then_with(|| a.media.title.cmp(&b.media.title)),
            TvSortBy::Added => a
                .media
                .first_seen
                .cmp(&b.media.first_seen)
                .then_with(|| a.media.title.cmp(&b.media.title)),
        };
        apply_sort_dir(ordering, sort_dir)
    });

    Ok(TvTemplate {
        username: auth.username,
        is_admin: auth.is_admin,
        items,
        show_marked,
        sort_by: sort_by.as_str().to_string(),
        sort_dir: sort_dir.as_str().to_string(),
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

    crate::trash::check_and_trash(&state.pool, id, &state.config, state.dry_run)
        .await
        .map_err(|e| AppError::Internal(format!("trash operation failed: {e}")))?;

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
        is_admin: auth.is_admin,
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
        is_admin: auth.is_admin,
    })
}
