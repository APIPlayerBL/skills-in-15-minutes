use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct Problem {
    #[serde(rename = "type")]
    typ: String,
    title: String,
    status: u16,
    detail: String,
    instance: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title) = match &self {
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AppError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "Validation failed"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "Forbidden"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "Not found"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "Conflict"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };
        let detail = match &self {
            AppError::Db(_) => "database error".to_string(),
            _ => self.to_string(),
        };
        let problem = Problem {
            typ: "https://errors.skills15min.app/".to_string(),
            title: title.to_string(),
            status: status.as_u16(),
            detail,
            instance: None,
        };
        (status, Json(problem)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
