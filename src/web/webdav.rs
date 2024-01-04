use axum::{extract::Request, response::IntoResponse};
use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};

use super::{
    errors::APIResult,
    middleware::{BasicAuth, OptionalSession},
};

pub async fn handler(
    session: OptionalSession,
    basic_auth: BasicAuth,
    req: Request,
) -> APIResult<impl IntoResponse> {
    let dir = "/tmp";

    println!("basicauth: {:?}", basic_auth);
    println!("session: {:?}", session);

    let dav_server = DavHandler::builder()
        .strip_prefix("/webdav")
        .filesystem(LocalFs::new(dir, false, false, false))
        .locksystem(FakeLs::new())
        .build_handler();

    let res = dav_server.handle(req).await;
    Ok(res.into_response())
}
