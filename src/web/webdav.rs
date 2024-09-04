use crate::app::App;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
};
use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};

use crate::utils::is_valid_username;

use super::{
    errors::{APIError, APIResult, ApiErrorExt},
    middleware::WebdavAuth,
};

pub async fn handler(
    auth: WebdavAuth,
    state: State<App>,
    req: Request,
) -> APIResult<impl IntoResponse> {
    let username = auth.username().api_error(StatusCode::UNAUTHORIZED, None)?;
    if !is_valid_username(username) {
        return Err(APIError::new(StatusCode::BAD_REQUEST, "invalid username"));
    }

    let path = state
        .config
        .user_home(username)
        .api_error(StatusCode::NOT_FOUND, None)?;

    let dav_server = DavHandler::builder()
        .strip_prefix("/api/webdav")
        .filesystem(LocalFs::new(path, false, false, false))
        .locksystem(FakeLs::new())
        .build_handler();

    let res = dav_server.handle(req).await;
    Ok(res.into_response())
}
