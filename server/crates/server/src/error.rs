use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("nicht angemeldet")]
    Unauthorized,
    #[error("keine Berechtigung")]
    Forbidden,
    #[error("nicht gefunden")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Datenbankfehler: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

pub type ApiResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Db(e) => {
                tracing::error!(error = ?e, "db error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let msg = match &self {
            AppError::Db(_) | AppError::Internal(_) => "Interner Fehler".to_string(),
            other => other.to_string(),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub fn bad(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}
