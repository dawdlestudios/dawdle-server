use crate::{
    app::{App, Website},
    utils::valid_public_key,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use serde_json::json;
use time::Duration;

use super::{
    errors::{APIError, APIResult, ApiErrorExt},
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
pub const ROLE_COOKIE_NAME: &str = "clientside_role";
pub const SESSION_COOKIE_NAME: &str = "session_token";

pub async fn login(
    State(state): State<App>,
    jar: CookieJar,
    body: axum::extract::Json<LoginRequest>,
) -> APIResult<impl IntoResponse> {
    let LoginRequest { username, password } = body.0;
    let username = username.to_lowercase();

    let valid = state
        .users
        .verify_password(&username, &password)
        .await
        .api_unauthorized()?;

    if !valid {
        return Err(APIError::new(StatusCode::UNAUTHORIZED, "invalid password"));
    };

    let user = state
        .users
        .get(&username)
        .await
        .api_internal_error()?
        .api_unauthorized()?;

    let session = state
        .sessions
        .create(&username)
        .await
        .api_internal_error()?;

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

    let mut cookies = jar.add(session_cookie).add(username_cookie);

    if let Some(role) = user.role {
        let role_cookie = Cookie::build((ROLE_COOKIE_NAME, role.to_string()))
            .max_age(USERNAME_COOKIE_MAX_AGE)
            .http_only(false)
            .path("/")
            .secure(!cfg!(debug_assertions))
            .same_site(SameSite::Strict)
            .build();
        cookies = cookies.add(role_cookie);
    }

    Ok((
        StatusCode::OK,
        cookies,
        Json(json!({
            "success": true,
        })),
    )
        .into_response())
}

pub async fn logout(State(state): State<App>, jar: CookieJar) -> APIResult<impl IntoResponse> {
    let session_token = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());

    if let Some(session_token) = session_token {
        let _ = state.sessions.logout(&session_token).await;
    }

    let remove_cookies = jar
        .remove(Cookie::from(SESSION_COOKIE_NAME))
        .remove(Cookie::build((USERNAME_COOKIE_NAME, "")).path("/").build())
        .remove(Cookie::build((ROLE_COOKIE_NAME, "")).path("/").build());

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

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct MeResponse {
    username: String,
    minecraft_username: Option<String>,
    public_keys: Vec<(String, String)>,
}

pub async fn get_me(
    session: RequiredSession,
    State(state): State<App>,
) -> APIResult<impl IntoResponse> {
    let keys = state
        .users
        .get_public_keys(session.username())
        .await
        .api_internal_error()?;

    let user = state
        .users
        .get(session.username())
        .await
        .api_internal_error()?
        .api_not_found()?;

    Ok((Json(MeResponse {
        username: session.username().to_string(),
        public_keys: keys,
        minecraft_username: user.minecraft_username,
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
    State(state): State<App>,
    body: Json<AddPublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let AddPublicKeyRequest { name, key } = body.0;

    if !valid_public_key(&key) {
        return Err(APIError::new(StatusCode::BAD_REQUEST, "invalid public key"));
    }

    state
        .users
        .add_public_key(session.username(), &key, &name)
        .await
        .api_internal_error()?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemovePublicKeyRequest {
    name: String,
}

pub async fn remove_public_key(
    session: RequiredSession,
    State(state): State<App>,
    body: Json<RemovePublicKeyRequest>,
) -> APIResult<impl IntoResponse> {
    let RemovePublicKeyRequest { name } = body.0;
    state
        .users
        .remove_public_key(session.username(), &name)
        .await
        .map_err(|_| APIError::new(StatusCode::BAD_REQUEST, "key name does not exist"))?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplicationRequest {
    pub username: String,
    pub email: String,
    pub about: String,
}

pub async fn apply(
    State(state): State<App>,
    body: Json<ApplicationRequest>,
) -> APIResult<impl IntoResponse> {
    let application = body.0;

    state
        .applications
        .apply(
            &application.username,
            &application.email,
            &application.about,
        )
        .await
        .api_internal_error()?;

    Ok((Json(json!({ "success": true }))).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct ClaimRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

pub async fn claim(
    State(state): State<App>,
    body: Json<ClaimRequest>,
) -> APIResult<impl IntoResponse> {
    let token = body.0;

    state
        .applications
        .claim(&token.token, &token.username, &token.password)
        .await
        .api_internal_error()?;

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
    State(state): State<App>,
    body: Json<ChangePasswordRequest>,
) -> APIResult<impl IntoResponse> {
    let password = body.0;

    state
        .users
        .verify_password(session.username(), &password.old_password)
        .await
        .api_internal_error()?;

    state
        .users
        .update_password(session.username(), &password.new_password)
        .await
        .api_internal_error()?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_sites(State(state): State<App>) -> APIResult<impl IntoResponse> {
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

#[derive(Debug, serde::Deserialize)]
pub struct UpdateMinecraftUsernameRequest {
    pub username: String,
}

pub async fn update_minecraft_username(
    session: RequiredSession,
    State(state): State<App>,
    body: Json<UpdateMinecraftUsernameRequest>,
) -> APIResult<impl IntoResponse> {
    let username = body.0.username;

    state
        .users
        .update_minecraft_username(session.username(), &username)
        .await
        .api_internal_error()?;

    Ok((Json(json!({ "success": true }))).into_response())
}
