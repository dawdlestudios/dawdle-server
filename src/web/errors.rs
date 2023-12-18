use axum::{body::Body, http::StatusCode, response::IntoResponse, Json};
use color_eyre::eyre::Result;
use serde_json::json;

pub type APIResult<T> = Result<T, APIError>;

pub enum APIError {
    NotFound,
    BadRequest(String),
    InternalServerError,
    Custom(StatusCode, String),
}

impl IntoResponse for APIError {
    fn into_response(self) -> axum::http::Response<Body> {
        match self {
            APIError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not found",
                })),
            )
                .into_response(),
            APIError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            APIError::InternalServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))).into_response()
            }
            APIError::Custom(status, message) => {
                (status, Json(json!({ "error": message }))).into_response()
            }
        }
    }
}
