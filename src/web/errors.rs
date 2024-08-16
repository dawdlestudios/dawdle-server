use axum::{
    body::Body,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use eyre::Result;
use serde_json::json;

pub type APIResult<T> = Result<T, APIError>;

pub const NOT_FOUND: (StatusCode, Html<&str>) =
    (StatusCode::NOT_FOUND, Html(include_str!("./404.html")));

pub enum APIError {
    NotFound,
    InternalServerError,

    Unauthorized,
    BadRequest(String),
    Custom(StatusCode, String),
}

impl APIError {
    pub fn custom(status: StatusCode, message: &str) -> Self {
        APIError::Custom(status, message.to_string())
    }
    pub fn bad_request() -> Self {
        APIError::BadRequest("bad request".to_string())
    }
    pub fn error(message: &str) -> Self {
        APIError::Custom(StatusCode::INTERNAL_SERVER_ERROR, message.to_string())
    }
}

impl IntoResponse for APIError {
    fn into_response(self) -> axum::http::Response<Body> {
        match self {
            APIError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "unauthorized" })),
            )
                .into_response(),
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
