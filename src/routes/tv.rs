use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

use crate::auth::middleware::AuthUser;
use crate::error::AppError;
use crate::models::{mark, media, persistent, user};
use crate::routes::sort::{apply_sort_dir, SortDir};
use crate::routes::AppState;
use crate::templates::{poster_image_url, MediaCardPartial, MediaRow, TvSeriesGroup, TvTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tv", get(list_tv))
        .route("/tv/series/{series}/mark-all", post(mark_series))
        .route("/tv/series/{series}/persist-all", post(persist_series))
        .route("/tv/{id}/mark", post(mark_tv).delete(unmark_tv))
        .route("/tv/{id}/persist", post(persist_tv).delete(unpersist_tv))
}

#[derive(Deserialize, Clone)]
struct ListQuery {
    #[serde(default)]
    show_marked: Option<String>,
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    dir: Option<String>,
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

fn build_tv_groups(
    items: Vec<MediaRow>,
    sort_by: TvSortBy,
    sort_dir: SortDir,
) -> Vec<TvSeriesGroup> {
    let mut grouped: BTreeMap<String, Vec<MediaRow>> = BTreeMap::new();
    for item in items {
        grouped
            .entry(item.media.title.clone())
            .or_default()
            .push(item);
    }

    let mut groups = Vec::new();
    for (title, mut seasons) in grouped {
        seasons.sort_by(|a, b| {
            let ordering = a
                .media
                .season
                .cmp(&b.media.season)
                .then_with(|| a.media.id.cmp(&b.media.id));
            match sort_by {
                TvSortBy::Season => apply_sort_dir(ordering, sort_dir),
                _ => ordering,
            }
        });
        let marked_count = seasons.iter().filter(|s| s.marked).count() as i64;
        let total_count = seasons.len() as i64;
        let poster_url = seasons
            .first()
            .and_then(|s| poster_image_url(&s.media.poster_path));
        groups.push(TvSeriesGroup {
            title,
            seasons,
            marked_count,
            total_count,
            poster_url,
        });
    }

    groups.sort_by(|a, b| {
        let ordering = match sort_by {
            TvSortBy::Name => a.title.cmp(&b.title),
            TvSortBy::Season => a.title.cmp(&b.title),
            TvSortBy::Marked => a
                .marked_count
                .cmp(&b.marked_count)
                .then_with(|| a.title.cmp(&b.title)),
            TvSortBy::Added => {
                let a_added = a
                    .seasons
                    .iter()
                    .map(|s| s.media.first_seen.as_str())
                    .max()
                    .unwrap_or("");
                let b_added = b
                    .seasons
                    .iter()
                    .map(|s| s.media.first_seen.as_str())
                    .max()
                    .unwrap_or("");
                a_added.cmp(b_added).then_with(|| a.title.cmp(&b.title))
            }
        };
        apply_sort_dir(ordering, sort_dir)
    });

    groups
}

async fn list_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let show_marked = query.show_marked.as_deref() == Some("true");
    let sort_by = TvSortBy::parse(query.sort.as_deref());
    let sort_dir = SortDir::parse(query.dir.as_deref());
    let all_media = media::list_visible_for_user(&state.pool, "tv_season", auth.id).await?;
    let user_marks = mark::user_marks(&state.pool, auth.id).await?;
    let total_users = user::count(&state.pool).await?;
    let media_ids: Vec<i64> = all_media.iter().map(|m| m.id).collect();
    let owners = persistent::owner_for_media_ids(&state.pool, &media_ids).await?;
    let owner_map: HashMap<i64, i64> = owners
        .into_iter()
        .map(|o| (o.media_id, o.user_id))
        .collect();

    let mut items = Vec::new();
    for m in all_media {
        let owner = owner_map.get(&m.id).copied();
        let persisted = m.status == "permanent";
        let persisted_by_me = owner == Some(auth.id);
        let marked = !persisted && user_marks.contains(&m.id);
        if !show_marked && marked {
            continue;
        }
        let mark_count = mark::mark_count(&state.pool, m.id).await?;
        items.push(MediaRow {
            media: m,
            marked,
            mark_count,
            total_users,
            persisted,
            persisted_by_me,
        });
    }

    let series_groups = build_tv_groups(items, sort_by, sort_dir);

    Ok(TvTemplate {
        username: auth.username,
        is_admin: auth.is_admin,
        series_groups,
        show_marked,
        sort_by: sort_by.as_str().to_string(),
        sort_dir: sort_dir.as_str().to_string(),
    })
}

async fn mark_series(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(series): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let all_media = media::list_by_type(&state.pool, "tv_season").await?;
    let ids: Vec<i64> = all_media
        .into_iter()
        .filter(|m| m.title == series && m.status == "active")
        .map(|m| m.id)
        .collect();

    for id in ids {
        mark::mark(&state.pool, auth.id, id).await?;
        crate::trash::check_and_trash(&state.pool, id, &state.config, state.dry_run)
            .await
            .map_err(|e| AppError::Internal(format!("trash operation failed: {e}")))?;
    }

    list_tv(State(state), auth, Query(query)).await
}

async fn mark_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if m.status != "active" {
        return Err(AppError::NotFound);
    }

    mark::mark(&state.pool, auth.id, id).await?;

    crate::trash::check_and_trash(&state.pool, id, &state.config, state.dry_run)
        .await
        .map_err(|e| AppError::Internal(format!("trash operation failed: {e}")))?;

    let media_item = media::get_by_id(&state.pool, id).await?.unwrap_or(m);

    // If the item was trashed (all users marked), remove it from the DOM
    if media_item.status != "active" {
        return Ok(axum::response::Html(String::new()).into_response());
    }

    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaCardPartial {
        item: MediaRow {
            media: media_item,
            marked: true,
            mark_count,
            total_users,
            persisted: false,
            persisted_by_me: false,
        },
        is_admin: auth.is_admin,
    }
    .into_response())
}

async fn unmark_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if m.status != "active" {
        return Err(AppError::NotFound);
    }

    mark::unmark(&state.pool, auth.id, id).await?;

    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaCardPartial {
        item: MediaRow {
            media: m,
            marked: false,
            mark_count,
            total_users,
            persisted: false,
            persisted_by_me: false,
        },
        is_admin: auth.is_admin,
    })
}

async fn persist_series(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(series): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let all_media = media::list_by_type(&state.pool, "tv_season").await?;
    let ids: Vec<i64> = all_media
        .into_iter()
        .filter(|m| m.title == series && m.status == "active")
        .map(|m| m.id)
        .collect();

    for id in ids {
        crate::persistent::move_to_permanent(
            &state.pool,
            id,
            auth.id,
            &state.config,
            state.dry_run,
        )
        .await
        .map_err(|e| AppError::Internal(format!("persist operation failed: {e}")))?;
    }

    list_tv(State(state), auth, Query(query)).await
}

async fn persist_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if m.status != "active" {
        return Err(AppError::NotFound);
    }

    crate::persistent::move_to_permanent(&state.pool, id, auth.id, &state.config, state.dry_run)
        .await
        .map_err(|e| AppError::Internal(format!("persist operation failed: {e}")))?;

    let media_item = media::get_by_id(&state.pool, id).await?.unwrap_or(m);
    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaCardPartial {
        item: MediaRow {
            media: media_item,
            marked: false,
            mark_count,
            total_users,
            persisted: true,
            persisted_by_me: true,
        },
        is_admin: auth.is_admin,
    })
}

async fn unpersist_tv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let m = media::get_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if m.status != "permanent" {
        return Err(AppError::NotFound);
    }
    let owner = persistent::get_owner(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if owner.user_id != auth.id {
        return Err(AppError::Forbidden);
    }

    crate::persistent::restore_from_permanent(
        &state.pool,
        id,
        auth.id,
        &state.config,
        state.dry_run,
    )
    .await
    .map_err(|e| AppError::Internal(format!("unpersist operation failed: {e}")))?;

    let media_item = media::get_by_id(&state.pool, id).await?.unwrap_or(m);
    let mark_count = mark::mark_count(&state.pool, id).await?;
    let total_users = user::count(&state.pool).await?;

    Ok(MediaCardPartial {
        item: MediaRow {
            media: media_item,
            marked: false,
            mark_count,
            total_users,
            persisted: false,
            persisted_by_me: false,
        },
        is_admin: auth.is_admin,
    })
}
