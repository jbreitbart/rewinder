use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::models::media::Media;
use crate::models::user::User;

/// Helper to convert any Askama template into an axum Response
fn render_template(t: &impl Template) -> Response {
    match t.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Template render error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

impl IntoResponse for LoginTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "setup_password.html")]
pub struct SetupPasswordTemplate {
    pub token: String,
    pub username: String,
    pub error: Option<String>,
}

impl IntoResponse for SetupPasswordTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

pub struct MediaRow {
    pub media: Media,
    pub marked: bool,
    pub mark_count: i64,
    pub total_users: i64,
}

#[derive(Template)]
#[template(path = "movies.html")]
pub struct MoviesTemplate {
    pub username: String,
    pub is_admin: bool,
    pub items: Vec<MediaRow>,
    pub show_marked: bool,
}

impl IntoResponse for MoviesTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "tv.html")]
pub struct TvTemplate {
    pub username: String,
    pub is_admin: bool,
    pub items: Vec<MediaRow>,
    pub show_marked: bool,
}

impl IntoResponse for TvTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "partials/media_row.html")]
pub struct MediaRowPartial {
    pub item: MediaRow,
}

impl IntoResponse for MediaRowPartial {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboardTemplate {
    pub username: String,
    pub is_admin: bool,
    pub active_count: i64,
    pub trashed_count: i64,
    pub active_size: String,
    pub trashed_size: String,
    pub user_count: i64,
}

impl IntoResponse for AdminDashboardTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "admin/users.html")]
pub struct AdminUsersTemplate {
    pub username: String,
    pub is_admin: bool,
    pub users: Vec<User>,
    pub invite_url: Option<String>,
}

impl IntoResponse for AdminUsersTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

#[derive(Template)]
#[template(path = "admin/trash.html")]
pub struct AdminTrashTemplate {
    pub username: String,
    pub is_admin: bool,
    pub items: Vec<Media>,
}

impl IntoResponse for AdminTrashTemplate {
    fn into_response(self) -> Response {
        render_template(&self)
    }
}

pub fn format_size(bytes: &i64) -> String {
    let bytes = *bytes;
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_gb() {
        let size: i64 = 2_147_483_648; // 2 GB
        assert_eq!(format_size(&size), "2.0 GB");
    }

    #[test]
    fn format_size_mb() {
        let size: i64 = 524_288_000; // 500 MB
        assert_eq!(format_size(&size), "500 MB");
    }

    #[test]
    fn format_size_small() {
        let size: i64 = 1_048_576; // 1 MB
        assert_eq!(format_size(&size), "1 MB");
    }
}
