use crate::app::App;
use axum::{
    extract::{Request, State},
    response::IntoResponse,
};
use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};

use crate::utils::is_valid_username;

use super::{errors::APIResult, middleware::WebdavAuth};

pub async fn handler(
    auth: WebdavAuth,
    state: State<App>,
    req: Request,
) -> APIResult<impl IntoResponse> {
    let username = auth
        .username()
        .ok_or(crate::web::errors::APIError::Unauthorized)?;

    log::info!("yay");

    if !is_valid_username(username) {
        return Err(crate::web::errors::APIError::Unauthorized);
    }

    let path = std::path::Path::new(&state.config.base_dir)
        .join(crate::config::FILES_FOLDER)
        .join(crate::config::FILES_HOME)
        .join(username);

    let dav_server = DavHandler::builder()
        .strip_prefix("/api/webdav")
        .filesystem(LocalFs::new(path, false, false, false))
        .locksystem(FakeLs::new())
        .build_handler();

    let res = dav_server.handle(req).await;
    Ok(res.into_response())
}
