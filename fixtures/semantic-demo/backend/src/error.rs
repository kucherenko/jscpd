//! Errors returned by the API, and how they become HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, serde::Serialize)]
pub struct FieldError {
    pub field: &'static str,
    pub message: &'static str,
}

impl FieldError {
    pub fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Invalid(Vec<FieldError>),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::Database(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, serde_json::json!({ "error": message })),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, serde_json::json!({ "error": message })),
            ApiError::Invalid(fields) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                serde_json::json!({ "error": "validation failed", "fields": fields }),
            ),
            ApiError::Database(err) => {
                tracing::error!(?err, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, serde_json::json!({ "error": "internal error" }))
            }
        };
        (status, Json(body)).into_response()
    }
}
