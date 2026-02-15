use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum AppError {
    Database(sqlx::Error),
    NotFound,
    Forbidden,
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Database(e) => write!(f, "Database error: {e}"),
            AppError::NotFound => write!(f, "Not found"),
            AppError::Forbidden => write!(f, "Forbidden"),
            AppError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        tracing::error!(
            error.kind = kind,
            error.message = %self,
            status = status.as_u16(),
            "request error"
        );
        (status, self.to_string()).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e)
    }
}
