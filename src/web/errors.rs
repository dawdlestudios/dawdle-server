use axum::{
    body::Body,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use eyre::Result;
use serde_json::json;

pub type APIResult<T> = Result<T, APIError>;

pub trait ApiErrorExt<T> {
    fn api_error(self, status: StatusCode, message: Option<&str>) -> Result<T, APIError>;
    fn api_internal_error(self) -> Result<T, APIError>
    where
        Self: Sized,
    {
        self.api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("internal server error"),
        )
    }
    fn api_not_found(self) -> Result<T, APIError>
    where
        Self: Sized,
    {
        self.api_error(StatusCode::NOT_FOUND, None)
    }
    fn api_bad_request(self) -> Result<T, APIError>
    where
        Self: Sized,
    {
        self.api_error(StatusCode::BAD_REQUEST, None)
    }
    fn api_unauthorized(self) -> Result<T, APIError>
    where
        Self: Sized,
    {
        self.api_error(StatusCode::UNAUTHORIZED, None)
    }
}

impl<T, E: Into<eyre::Error>> ApiErrorExt<T> for Result<T, E> {
    fn api_error(self, status: StatusCode, message: Option<&str>) -> Result<T, APIError> {
        self.map_err(|e| {
            let message = message.unwrap_or(status.canonical_reason().unwrap_or("unknown"));
            log::warn!("api error: {message}: {}", e.into());
            APIError(status, message.to_string())
        })
    }
}

impl<T> ApiErrorExt<T> for Option<T> {
    fn api_error(self, status: StatusCode, message: Option<&str>) -> Result<T, APIError> {
        self.ok_or_else(|| {
            let message = message.unwrap_or(status.canonical_reason().unwrap_or("unknown"));
            log::warn!("api error: {message}");
            APIError(status, message.to_string())
        })
    }
}

pub const NOT_FOUND: (StatusCode, Html<&str>) =
    (StatusCode::NOT_FOUND, Html(include_str!("./404.html")));

pub struct APIError(StatusCode, String);

impl APIError {
    pub fn new(status: StatusCode, message: &str) -> Self {
        Self(status, message.to_string())
    }
}

impl Default for APIError {
    fn default() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_string(),
        )
    }
}

impl IntoResponse for APIError {
    fn into_response(self) -> axum::http::Response<Body> {
        let body = json!({
            "status": self.0.as_u16(),
            "message": self.1
        })
        .to_string();
        (self.0, body).into_response()
    }
}
