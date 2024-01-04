use crate::state::{Session, State as AppState};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

use super::errors::APIError;

#[derive(Debug)]
pub struct BasicAuth(Option<String>);

#[async_trait]
impl FromRequestParts<AppState> for BasicAuth {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        let Some(auth) = parts
            .headers
            .get("Authorization")
            .map(|inner| inner.to_str())
            .and_then(Result::ok)
        else {
            if parts.extensions.get::<RequiredSession>().is_some() {
                return Ok(BasicAuth(None));
            }
            return Err(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("WWW-Authenticate", "Basic realm=\"webdav\"")
                .body(Body::empty())
                .unwrap());
        };

        let res = data_encoding::BASE64
            .decode(auth.strip_prefix("Basic ").unwrap_or_default().as_bytes())
            .map(String::from_utf8)
            .map_err(|_| APIError::Unauthorized.into_response())?
            .map_err(|_| APIError::Unauthorized.into_response())?;

        let (username, _password) = res
            .split_once(':')
            .ok_or(APIError::Unauthorized.into_response())?;

        Ok(BasicAuth(Some(username.to_string())))
    }
}

#[derive(Debug)]
pub struct RequiredSession(pub Session);

impl RequiredSession {
    pub fn username(&self) -> &str {
        &self.0.username
    }
}

#[derive(Debug)]
pub struct OptionalSession(pub Option<RequiredSession>);

#[async_trait]
impl FromRequestParts<AppState> for OptionalSession {
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
            if let Ok(session) = state.verify_session(&session_token) {
                return Ok(OptionalSession(session.map(RequiredSession)));
            }
        }

        Ok(OptionalSession(None))
    }
}

#[async_trait]
impl FromRequestParts<AppState> for RequiredSession {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match OptionalSession::from_request_parts(parts, state).await {
            Ok(OptionalSession(Some(valid_session))) => Ok(valid_session),
            Ok(OptionalSession(None)) | Err(_) => Err(APIError::Unauthorized.into_response()),
        }
    }
}
