use super::{errors::APIResult, middleware};
use crate::state::State as AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

pub async fn is_admin(_user: middleware::Admin) -> impl IntoResponse {
    (Json(json!({ "success": true }))).into_response()
}

pub async fn get_guestbook_requests(
    _user: middleware::Admin,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let entries = state
        .guestbook_entries()
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(entries)).into_response())
}

pub async fn approve_guestbook_entry(
    _user: middleware::Admin,
    State(state): State<AppState>,
    body: Json<String>,
) -> APIResult<impl IntoResponse> {
    let id = body.0;

    state
        .approve_guestbook_entry(&id)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_applications(
    _user: middleware::Admin,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let applications = state
        .applications()
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(applications)).into_response())
}

pub async fn approve_application(
    _user: middleware::Admin,
    State(state): State<AppState>,
    body: Json<String>,
) -> APIResult<impl IntoResponse> {
    let username = body.0;

    let token = state
        .approve_application(&username)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true, "token": token }))).into_response())
}
