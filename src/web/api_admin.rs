use super::{errors::APIResult, middleware};
use crate::state::AppState;
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
        .guestbook
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
        .guestbook
        .approve_guestbook_entry(&id)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_applications(
    _user: middleware::Admin,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let applications = state
        .user
        .applications()
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(applications)).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct IdRequest {
    id: String,
}

pub async fn approve_application(
    _user: middleware::Admin,
    State(state): State<AppState>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    let token = state
        .user
        .approve_application(&id)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true, "token": token }))).into_response())
}

pub async fn delete_application(
    _user: middleware::Admin,
    State(state): State<AppState>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    state
        .user
        .delete_application(&id)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_users(
    _user: middleware::Admin,
    State(state): State<AppState>,
) -> APIResult<impl IntoResponse> {
    let users = state
        .user
        .users()
        .map_err(|_| super::errors::APIError::InternalServerError)?
        .iter()
        .map(|(username, user)| json!({ "username":  username, "role": user.role }))
        .collect::<Vec<_>>();

    Ok((Json(users)).into_response())
}

pub async fn delete_user(
    _user: middleware::Admin,
    State(state): State<AppState>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    state
        .user
        .delete(&id)
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}
