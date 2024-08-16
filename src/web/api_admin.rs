use super::{errors::APIResult, middleware};
use crate::app::App;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

pub async fn is_admin(_user: middleware::Admin) -> impl IntoResponse {
    (Json(json!({ "success": true }))).into_response()
}

pub async fn get_applications(
    _user: middleware::Admin,
    State(state): State<App>,
) -> APIResult<impl IntoResponse> {
    let applications = state
        .applications
        .all()
        .await
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(applications)).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct IdRequest {
    id: String,
}

pub async fn approve_application(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    let token = state
        .applications
        .approve(&id)
        .await
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true, "token": token }))).into_response())
}

pub async fn delete_application(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    state
        .applications
        .delete(&id)
        .await
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_users(
    _user: middleware::Admin,
    State(state): State<App>,
) -> APIResult<impl IntoResponse> {
    let users = state
        .users
        .all()
        .await
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(users)).into_response())
}

pub async fn delete_user(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;

    state
        .users
        .delete(&id)
        .await
        .map_err(|_| super::errors::APIError::InternalServerError)?;

    Ok((Json(json!({ "success": true }))).into_response())
}
