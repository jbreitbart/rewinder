use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use sqlx::SqlitePool;

use crate::auth::session;
use crate::models::user;
use crate::routes::AppState;

pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

pub struct AdminUser(pub AuthUser);

impl std::ops::Deref for AdminUser {
    type Target = AuthUser;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum AuthRejection {
    Redirect(Redirect),
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        match self {
            AuthRejection::Redirect(r) => r.into_response(),
        }
    }
}

async fn extract_auth_user(
    parts: &mut Parts,
    pool: &SqlitePool,
) -> Result<AuthUser, AuthRejection> {
    let jar = CookieJar::from_headers(&parts.headers);

    let token = jar
        .get("session")
        .map(|c| c.value().to_string())
        .ok_or(AuthRejection::Redirect(Redirect::to("/login")))?;

    let user_id = session::validate(pool, &token)
        .await
        .map_err(|_| AuthRejection::Redirect(Redirect::to("/login")))?
        .ok_or(AuthRejection::Redirect(Redirect::to("/login")))?;

    let u = user::get_by_id(pool, user_id)
        .await
        .map_err(|_| AuthRejection::Redirect(Redirect::to("/login")))?
        .ok_or(AuthRejection::Redirect(Redirect::to("/login")))?;

    Ok(AuthUser {
        id: u.id,
        username: u.username,
        is_admin: u.is_admin,
    })
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        extract_auth_user(parts, &state.pool).await
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(AuthRejection::Redirect(Redirect::to("/")));
        }
        Ok(AdminUser(user))
    }
}
