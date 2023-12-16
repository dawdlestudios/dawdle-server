use crate::state::{Session, State as AppState};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::extract::CookieJar;

use super::errors::{APIError, APIResult};

pub async fn extract_session(
    State(state): State<AppState>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> APIResult<impl IntoResponse> {
    if let Some(session_token) = jar.get("session_id").map(|c| c.value().to_string()) {
        if let Ok(Some(session)) = state.verify_session(&session_token) {
            request.extensions_mut().insert(session);
        };
    };

    // Ok(request)
    Ok(next.run(request).await)
}

pub async fn require_session(request: Request, next: Next) -> APIResult<impl IntoResponse> {
    if request.extensions().get::<Session>().is_none() {
        return Err(APIError::Custom(
            StatusCode::UNAUTHORIZED,
            "not logged in".to_string(),
        ));
    };

    // Ok(request)
    Ok(next.run(request).await)
}
