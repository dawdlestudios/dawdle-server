use super::errors::APIError;
use crate::app::{App, Session, User};
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
pub struct WebdavAuth(Option<String>);

impl WebdavAuth {
    pub fn username(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

pub fn unauthorized(message: &str) -> Response {
    APIError::new(StatusCode::UNAUTHORIZED, message).into_response()
}

#[async_trait]
impl FromRequestParts<App> for WebdavAuth {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &App) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get("Authorization")
            .map(|inner| inner.to_str())
            .and_then(Result::ok);

        let Some(auth) = authorization else {
            return match OptionalSession::from_request_parts(parts, state)
                .await?
                .username()
            {
                Some(username) => Ok(WebdavAuth(Some(username.to_string()))),
                None => Err(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header("WWW-Authenticate", "Basic realm=\"webdav\"")
                    .body(Body::empty())
                    .unwrap()),
            };
        };

        let res = data_encoding::BASE64
            .decode(auth.strip_prefix("Basic ").unwrap_or_default().as_bytes())
            .map(String::from_utf8)
            .map_err(|_| unauthorized("invalid base64"))?
            .map_err(|_| unauthorized("invalid base64"))?;

        let (username, _password) = res
            .split_once(':')
            .ok_or_else(|| unauthorized("invalid auth header"))?;

        Ok(WebdavAuth(Some(username.to_string())))
    }
}

pub struct Admin(pub User);

#[async_trait]
impl FromRequestParts<App> for Admin {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &App) -> Result<Self, Self::Rejection> {
        let session = RequiredSession::from_request_parts(parts, state).await?;

        let user = state
            .users
            .get(session.username())
            .await
            .map_err(|_| unauthorized("user not found"))?
            .ok_or_else(|| unauthorized("user not found"))?;

        if user.role.as_deref() != Some("admin") {
            return Err(unauthorized("not an admin"));
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
impl FromRequestParts<App> for OptionalSession {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &App) -> Result<Self, Self::Rejection> {
        use axum::RequestPartsExt;

        let jar = parts
            .extract::<CookieJar>()
            .await
            .map_err(|_| unauthorized("no session cookie"))?;

        if let Some(session_token) = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string()) {
            if let Ok(session) = state.sessions.verify(&session_token).await {
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
impl FromRequestParts<App> for RequiredSession {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _state: &App) -> Result<Self, Self::Rejection> {
        match OptionalSession::from_request_parts(parts, _state).await?.0 {
            Some(session) => Ok(RequiredSession(session)),
            None => Err(unauthorized("no session")),
        }
    }
}
