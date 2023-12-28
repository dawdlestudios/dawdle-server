use crate::state::{Session, State as AppState};
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

use super::errors::APIError;

pub struct ValidSession(pub Session);
impl ValidSession {
    pub fn username(&self) -> &str {
        &self.0.username
    }
}

#[async_trait]
impl FromRequestParts<AppState> for ValidSession {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        use axum::RequestPartsExt;
        let jar = parts
            .extract::<CookieJar>()
            .await
            .map_err(|err| err.into_response())?;

        if let Some(session_token) = jar.get("session_id").map(|c| c.value().to_string()) {
            if let Ok(Some(session)) = state.verify_session(&session_token) {
                return Ok(Self(session));
            };
        };

        Err(APIError::Unauthorized.into_response())
    }
}
