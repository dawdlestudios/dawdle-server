use crate::state::{AppState, Website};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use serde_json::json;
use time::Duration;

use super::{
    errors::{APIError, APIResult},
    middleware::RequiredSession,
};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub const SESSION_COOKIE_MAX_AGE: Duration = Duration::days(7);
pub const USERNAME_COOKIE_MAX_AGE: Duration = Duration::days(7);
pub const USERNAME_COOKIE_NAME: &str = "clientside_username";
pub const SESSION_COOKIE_NAME: &str = "session_token";

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    body: axum::extract::Json<LoginRequest>,
) -> APIResult<impl IntoResponse> {
    let LoginRequest { username, password } = body.0;

    let valid = state
        .user
        .verify_password(&username, &password)
        .map_err(|e| {
            log::error!("error verifying password: {:?}", e);
            APIError::Unauthorized
        })?;

    if !valid {
        return Err(APIError::custom(
            StatusCode::UNAUTHORIZED,
            "invalid password",
        ));
    };

    let session = state
        .user
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
        let _ = state.user.logout_session(&session_token);
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
struct GuestbookEntryResponse {
    date: u64,
    by: String,
    message: String,
}

pub async fn add_guestbook_entry(
    State(state): State<AppState>,
    body: Json<String>,
) -> APIResult<impl IntoResponse> {
    let entry = body.0;

    state
        .guestbook
        .add_guestbook_entry(&entry)
        .map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_guestbook(State(state): State<AppState>) -> APIResult<impl IntoResponse> {
    let entries = state
        .guestbook
        .approved_guestbook_entries()
        .map_err(|_| APIError::InternalServerError)?
        .iter()
        .map(|entry| GuestbookEntryResponse {
            date: entry.date,
            by: entry.by.clone(),
            message: entry.message.clone(),
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
    session: RequiredSession,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let user = state
        .user
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
    session: RequiredSession,
    State(state): State<AppState>,
    body: Json<AddPublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let AddPublicKeyRequest { name, key } = body.0;

    state
        .user
        .add_public_key(session.username(), &name, &key)
        .map_err(|_| {
            log::error!("error adding public key");
            APIError::InternalServerError
        })?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemovePublicKeyRequest {
    name: String,
}

pub async fn remove_public_key(
    session: RequiredSession,
    State(state): State<AppState>,
    body: Json<RemovePublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let RemovePublicKeyRequest { name } = body.0;
    state
        .user
        .remove_public_key(session.username(), &name)
        .map_err(|_| APIError::custom(StatusCode::BAD_REQUEST, "key name does not exist"))?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplicationRequest {
    pub username: String,
    pub email: String,
    pub about: String,
}

pub async fn apply(
    State(state): State<AppState>,
    body: Json<ApplicationRequest>,
) -> APIResult<impl IntoResponse> {
    let application = body.0;

    state
        .user
        .apply(
            &application.username,
            &application.email,
            &application.about,
        )
        .map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct ClaimRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

pub async fn claim(
    State(state): State<AppState>,
    body: Json<ClaimRequest>,
) -> APIResult<impl IntoResponse> {
    let token = body.0;

    state
        .user
        .claim(&token.token, &token.username, &token.password)
        .map_err(|e| {
            log::error!("error claiming application: {:?}", e);
            APIError::InternalServerError
        })?;

    state.set_site(token.username.clone(), Website::User(token.username));
    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

pub async fn change_password(
    session: RequiredSession,
    State(state): State<AppState>,
    body: Json<ChangePasswordRequest>,
) -> APIResult<impl IntoResponse> {
    let password = body.0;

    state
        .user
        .verify_password(session.username(), &password.old_password)
        .map_err(|_| APIError::InternalServerError)?;

    state
        .user
        .change_password(session.username(), &password.new_password)
        .map_err(|_| APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_sites(State(state): State<AppState>) -> APIResult<impl IntoResponse> {
    let sites = state
        .sites
        .iter()
        .map(|site| {
            let website = site.value();
            let hostname = site.key();

            match website {
                Website::User(username) => json!({
                    "type": "user",
                    "username": username,
                }),
                Website::Site(username, _path) => json!({
                    "type": "site",
                    "hostname": hostname,
                    "username": username,
                }),
            }
        })
        .collect::<serde_json::Value>();

    Ok((Json(sites)).into_response())
}
