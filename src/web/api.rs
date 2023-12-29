use crate::state::State as AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use serde_json::json;
use time::Duration;

use super::{
    errors::{APIError, APIResult},
    middleware::ValidSession,
};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

const SESSION_COOKIE_MAX_AGE: Duration = Duration::days(7);
const USERNAME_COOKIE_MAX_AGE: Duration = Duration::days(7);
const USERNAME_COOKIE_NAME: &str = "clientside_username";
const SESSION_COOKIE_NAME: &str = "session_token";

#[axum::debug_handler]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    body: axum::extract::Json<LoginRequest>,
) -> APIResult<impl IntoResponse> {
    let LoginRequest { username, password } = body.0;

    let valid = state.verify_password(&username, &password).map_err(|e| {
        log::error!("error verifying password: {:?}", e);
        APIError::Unauthorized
    })?;

    if !valid {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "invalid username or password",
            })),
        )
            .into_response());
    };

    let session = state
        .create_session(&username)
        .map_err(|_| APIError::InternalServerError)?;

    let session_cookie = Cookie::build((SESSION_COOKIE_NAME, session))
        .max_age(SESSION_COOKIE_MAX_AGE)
        .http_only(true)
        .path("/api")
        .secure(!cfg!(debug_assertions))
        .same_site(SameSite::Strict)
        .build();

    let username_cookie = Cookie::build((USERNAME_COOKIE_NAME, username))
        .max_age(USERNAME_COOKIE_MAX_AGE)
        .http_only(false)
        .path("/")
        .secure(!cfg!(debug_assertions))
        .same_site(SameSite::Strict)
        .build();

    Ok((
        StatusCode::OK,
        jar.add(session_cookie).add(username_cookie),
        Json(json!({
            "success": true,
        })),
    )
        .into_response())
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> APIResult<impl IntoResponse> {
    let session_token = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());

    if let Some(session_token) = session_token {
        state
            .logout_session(&session_token)
            .map_err(|_| APIError::InternalServerError)?;
    }

    let remove_cookies = jar
        .remove(Cookie::from(SESSION_COOKIE_NAME))
        .remove(Cookie::build((USERNAME_COOKIE_NAME, "")).path("/").build());

    Ok((
        StatusCode::OK,
        remove_cookies,
        Json(json!({
            "success": true,
        })),
    )
        .into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct GuestbookEntry {
    date: u64,
    message: String,
}

pub async fn add_guestbook_entry(
    State(state): State<AppState>,
    body: Json<String>,
) -> APIResult<impl IntoResponse> {
    let entry = body.0;

    state
        .add_guestbook_entry(&entry)
        .map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_guestbook(State(state): State<AppState>) -> APIResult<impl IntoResponse> {
    let entries = state
        .guestbook()
        .map_err(|_| APIError::InternalServerError)?
        .iter()
        .map(|(date, msg)| GuestbookEntry {
            date: *date,
            message: msg.clone(),
        })
        .collect::<Vec<_>>();

    Ok((Json(entries)).into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct MeResponse {
    username: String,
    public_keys: Vec<(String, String)>,
}

pub async fn get_me(
    session: ValidSession,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let user = state
        .users
        .get(session.username())
        .map_err(|_| APIError::InternalServerError)?
        .ok_or(APIError::InternalServerError)?;

    Ok((Json(MeResponse {
        username: session.username().to_string(),
        public_keys: user.public_keys,
    }))
    .into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AddPublicKeyRequest {
    name: String,
    key: String,
}

pub async fn add_public_key(
    session: ValidSession,
    State(state): State<AppState>,
    body: Json<AddPublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let AddPublicKeyRequest { name, key } = body.0;

    let tx = state
        .users
        .transaction()
        .map_err(|_| APIError::InternalServerError)?;

    let mut user = state
        .users
        .get(session.username())
        .map_err(|_| APIError::InternalServerError)?
        .ok_or(APIError::InternalServerError)?;

    if user.public_keys.iter().any(|(n, _)| n == &name) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "key name already exists",
            })),
        )
            .into_response());
    }

    user.public_keys.push((name, key));
    tx.set(session.username(), &user)
        .map_err(|_| APIError::InternalServerError)?;
    tx.commit().map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemovePublicKeyRequest {
    name: String,
}

pub async fn remove_public_key(
    session: ValidSession,
    State(state): State<AppState>,
    body: Json<RemovePublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let RemovePublicKeyRequest { name } = body.0;

    let tx = state
        .users
        .transaction()
        .map_err(|_| APIError::InternalServerError)?;

    let mut user = state
        .users
        .get(session.username())
        .map_err(|_| APIError::InternalServerError)?
        .ok_or(APIError::InternalServerError)?;

    if !user.public_keys.iter().any(|(n, _)| n == &name) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "key name does not exist",
            })),
        )
            .into_response());
    }

    user.public_keys.retain(|(n, _)| n != &name);
    tx.set(session.username(), &user)
        .map_err(|_| APIError::InternalServerError)?;
    tx.commit().map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}
