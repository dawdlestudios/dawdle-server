use crate::state::State as AppState;
use axum::{
    extract::{Request, State},
    response::IntoResponse,
};
use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};

use crate::utils::is_valid_username;

use super::{
    errors::APIResult,
    middleware::{BasicAuth, OptionalSession},
};

pub async fn handler(
    session: OptionalSession,
    basic_auth: BasicAuth,
    state: State<AppState>,
    req: Request,
) -> APIResult<impl IntoResponse> {
    let username = if let Some(username) = basic_auth.username() {
        username
    } else if let Some(session) = session.username() {
        session
    } else {
        return Err(crate::web::errors::APIError::Unauthorized);
    };

    if !is_valid_username(username) {
        return Err(crate::web::errors::APIError::Unauthorized);
    }

    let path = std::path::Path::new(&state.config.base_dir)
        .join(&state.config.home_dirs)
        .join(username);

    let dav_server = DavHandler::builder()
        .strip_prefix("/api/webdav")
        .filesystem(LocalFs::new(path, false, false, false))
        .locksystem(FakeLs::new())
        .build_handler();

    let res = dav_server.handle(req).await;
    Ok(res.into_response())
}
