use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;

use crate::auth;
use crate::auth::session;
use crate::models::user;
use crate::routes::AppState;
use crate::templates::{LoginTemplate, SetupPasswordTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_page).post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/invite/{token}", get(invite_page).post(invite_handler))
}

async fn login_page() -> impl IntoResponse {
    LoginTemplate { error: None }
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let user = match user::get_by_username(&state.pool, &form.username).await {
        Ok(Some(u)) => u,
        _ => {
            return LoginTemplate {
                error: Some("Invalid username or password".into()),
            }
            .into_response();
        }
    };

    let hash = match &user.password_hash {
        Some(h) => h,
        None => {
            return LoginTemplate {
                error: Some("Account not activated. Use your invite link.".into()),
            }
            .into_response();
        }
    };

    if !auth::verify_password(&form.password, hash) {
        return LoginTemplate {
            error: Some("Invalid username or password".into()),
        }
        .into_response();
    }

    let token = match session::create(&state.pool, user.id, session::DEFAULT_SESSION_TTL_HOURS).await {
        Ok(t) => t,
        Err(_) => {
            return LoginTemplate {
                error: Some("Internal error".into()),
            }
            .into_response();
        }
    };

    let cookie = Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .same_site(axum_extra::extract::cookie::SameSite::Strict);

    (jar.add(cookie), Redirect::to("/movies")).into_response()
}

async fn logout_handler(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(cookie) = jar.get("session") {
        let _ = session::delete(&state.pool, cookie.value()).await;
    }

    let removal = Cookie::build(("session", ""))
        .path("/")
        .http_only(true);

    (jar.remove(removal), Redirect::to("/login")).into_response()
}

async fn invite_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    match user::get_by_invite_token(&state.pool, &token).await {
        Ok(Some(u)) => SetupPasswordTemplate {
            token,
            username: u.username,
            error: None,
        }
        .into_response(),
        _ => Redirect::to("/login").into_response(),
    }
}

#[derive(Deserialize)]
struct SetPasswordForm {
    password: String,
    password_confirm: String,
}

async fn invite_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(token): Path<String>,
    Form(form): Form<SetPasswordForm>,
) -> Response {
    let user = match user::get_by_invite_token(&state.pool, &token).await {
        Ok(Some(u)) => u,
        _ => return Redirect::to("/login").into_response(),
    };

    if form.password != form.password_confirm {
        return SetupPasswordTemplate {
            token,
            username: user.username,
            error: Some("Passwords do not match".into()),
        }
        .into_response();
    }

    if form.password.len() < 8 {
        return SetupPasswordTemplate {
            token,
            username: user.username,
            error: Some("Password must be at least 8 characters".into()),
        }
        .into_response();
    }

    let hash = match auth::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => {
            return SetupPasswordTemplate {
                token,
                username: user.username,
                error: Some("Internal error".into()),
            }
            .into_response();
        }
    };

    if user::set_password(&state.pool, user.id, &hash).await.is_err() {
        return SetupPasswordTemplate {
            token,
            username: user.username,
            error: Some("Internal error".into()),
        }
        .into_response();
    }

    // Auto-login
    let session_token = match session::create(&state.pool, user.id, session::DEFAULT_SESSION_TTL_HOURS).await {
        Ok(t) => t,
        Err(_) => return Redirect::to("/login").into_response(),
    };

    let cookie = Cookie::build(("session", session_token))
        .path("/")
        .http_only(true)
        .same_site(axum_extra::extract::cookie::SameSite::Strict);

    (jar.add(cookie), Redirect::to("/movies")).into_response()
}
