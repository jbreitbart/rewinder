use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::auth::middleware::AuthUser;
use crate::error::AppError;
use crate::models::{mark, media, user};
use crate::routes::sort::{apply_sort_dir, SortDir};
use crate::routes::AppState;
use crate::templates::{MediaRow, MediaRowPartial, MoviesTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/movies") }),
        )
        .route("/movies", get(list_movies))
        .route("/movies/{id}/mark", post(mark_movie).delete(unmark_movie))
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
enum MovieSortBy {
    Name,
    Year,
    Marked,
    Added,
}

impl MovieSortBy {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("year") => MovieSortBy::Year,
            Some("marked") => MovieSortBy::Marked,
            Some("added") => MovieSortBy::Added,
            _ => MovieSortBy::Name,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            MovieSortBy::Name => "name",
            MovieSortBy::Year => "year",
            MovieSortBy::Marked => "marked",
            MovieSortBy::Added => "added",
        }
    }
}

async fn list_movies(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let show_marked = query.show_marked.as_deref() == Some("true");
    let sort_by = MovieSortBy::parse(query.sort.as_deref());
    let sort_dir = SortDir::parse(query.dir.as_deref());
    let all_media = media::list_by_type(&state.pool, "movie").await?;
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
            MovieSortBy::Name => a
                .media
                .title
                .cmp(&b.media.title)
                .then_with(|| a.media.id.cmp(&b.media.id)),
            MovieSortBy::Year => a
                .media
                .year
                .cmp(&b.media.year)
                .then_with(|| a.media.title.cmp(&b.media.title)),
            MovieSortBy::Marked => a
                .marked
                .cmp(&b.marked)
                .then_with(|| a.media.title.cmp(&b.media.title)),
            MovieSortBy::Added => a
                .media
                .first_seen
                .cmp(&b.media.first_seen)
                .then_with(|| a.media.title.cmp(&b.media.title)),
        };
        apply_sort_dir(ordering, sort_dir)
    });

    Ok(MoviesTemplate {
        username: auth.username,
        is_admin: auth.is_admin,
        items,
        show_marked,
        sort_by: sort_by.as_str().to_string(),
        sort_dir: sort_dir.as_str().to_string(),
    })
}

async fn mark_movie(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    mark::mark(&state.pool, auth.id, id).await?;

    // Check if all users marked → move to trash
    crate::trash::check_and_trash(&state.pool, id, &state.config, state.dry_run)
        .await
        .map_err(|e| AppError::Internal(format!("trash operation failed: {e}")))?;

    // Re-fetch to get updated state
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

async fn unmark_movie(
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
