use super::errors::APIError;
use crate::state::{AppState, Session, User};
use crate::web::api::SESSION_COOKIE_NAME;
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

#[derive(Debug)]
pub struct BasicAuth(Option<String>);

impl BasicAuth {
    pub fn username(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

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

pub struct Admin(pub User);

#[async_trait]
impl FromRequestParts<AppState> for Admin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = RequiredSession::from_request_parts(parts, state).await?;

        let user = state
            .user
            .get(session.username())
            .map_err(|_| APIError::Unauthorized.into_response())?
            .ok_or_else(|| APIError::Unauthorized.into_response())?;

        if user.role.as_deref() != Some("admin") {
            return Err(APIError::Unauthorized.into_response());
        }

        Ok(Admin(user))
    }
}

#[derive(Debug)]
pub struct OptionalSession(Option<Session>);

impl OptionalSession {
    pub fn username(&self) -> Option<&str> {
        self.0.as_ref().map(|s| &s.username[..])
    }
}

#[async_trait]
impl FromRequestParts<AppState> for OptionalSession {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        use axum::RequestPartsExt;

        let jar = parts.extract::<CookieJar>().await.map_err(|_| {
            APIError::Custom(StatusCode::UNAUTHORIZED, "no session cookie".to_string())
                .into_response()
        })?;

        if let Some(session_token) = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string()) {
            if let Ok(session) = state.user.verify_session(&session_token) {
                if let Some(ref session) = session {
                    parts.extensions.insert(RequiredSession(session.clone()));
                }
                return Ok(OptionalSession(session));
            }
        }

        Ok(OptionalSession(None))
    }
}

#[derive(Debug, Clone)]
pub struct RequiredSession(pub Session);

impl RequiredSession {
    pub fn username(&self) -> &str {
        &self.0.username
    }
}

#[async_trait]
impl FromRequestParts<AppState> for RequiredSession {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match OptionalSession::from_request_parts(parts, _state).await?.0 {
            Some(session) => Ok(RequiredSession(session)),
            None => Err(APIError::Unauthorized.into_response()),
        }
    }
}
