use crate::state::State as AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use serde_json::json;
use time::Duration;

use super::errors::{APIError, APIResult};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

const SESSION_COOKIE_MAX_AGE: Duration = Duration::days(7);

#[axum::debug_handler]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    body: axum::extract::Json<LoginRequest>,
) -> APIResult<impl IntoResponse> {
    let LoginRequest { username, password } = body.0;

    let valid = state
        .verify_password(&username, &password)
        .map_err(|_| APIError::InternalServerError)?;

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

    let session_cookie = Cookie::build(("session_id", session))
        .max_age(SESSION_COOKIE_MAX_AGE)
        .http_only(true)
        .path("/api")
        .secure(!cfg!(debug_assertions))
        .same_site(SameSite::Strict)
        .build();

    let username_cookie = Cookie::build(("session_username", username))
        .max_age(SESSION_COOKIE_MAX_AGE)
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
    let session_token = jar.get("session_id").map(|c| c.value().to_string());

    if let Some(session_token) = session_token {
        state
            .logout_session(&session_token)
            .map_err(|_| APIError::InternalServerError)?;
    }

    Ok((
        StatusCode::OK,
        jar.remove("session_id").remove("session_username"),
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

pub async fn get_guestbook(State(state): State<AppState>) -> APIResult<impl IntoResponse> {
    let entries = state
        .guestbook_approved
        .iter()
        .map_err(|_| APIError::InternalServerError)?
        .map(|val| {
            val.map(|(k, v)| GuestbookEntry {
                date: k,
                message: v,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| APIError::InternalServerError)?;

    println!("{:?}", entries);

    Ok((Json(entries)).into_response())
}
