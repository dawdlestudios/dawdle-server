use super::{
    errors::{APIResult, ApiErrorExt},
    middleware,
};
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
    let applications = state.applications.all().await.api_internal_error()?;
    Ok((Json(applications)).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct IdRequest {
    id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UsernameRequest {
    id: String,
    username: String,
}

pub async fn approve_application(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;
    state.applications.approve(&id).await.api_internal_error()?;
    Ok((Json(json!({ "success": true, "token": () }))).into_response())
}

pub async fn unapprove_application(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;
    state
        .applications
        .unapprove(&id)
        .await
        .api_internal_error()?;
    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn update_application_username(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<UsernameRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;
    state
        .applications
        .update_username(&id, &body.0.username)
        .await
        .api_internal_error()?;

    Ok((Json(json!({ "success": true, "username": () }))).into_response())
}

pub async fn delete_application(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;
    state.applications.delete(&id).await.api_internal_error()?;
    Ok((Json(json!({ "success": true }))).into_response())
}

pub async fn get_users(
    _user: middleware::Admin,
    State(state): State<App>,
) -> APIResult<impl IntoResponse> {
    let users = state.users.all().await.api_internal_error()?;
    Ok((Json(users)).into_response())
}

pub async fn delete_user(
    _user: middleware::Admin,
    State(state): State<App>,
    body: Json<IdRequest>,
) -> APIResult<impl IntoResponse> {
    let id = body.0.id;
    state.users.delete(&id).await.api_internal_error()?;
    Ok((Json(json!({ "success": true }))).into_response())
}
