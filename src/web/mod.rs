use crate::{state::State as AppState, web::errors::APIError};
use axum::{
    body::Body, extract::Request, handler::HandlerWithoutStateExt, http::StatusCode,
    response::IntoResponse, routing::*, Router,
};

use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tower::ServiceExt;

use self::{
    errors::APIResult,
    middleware::{extract_session, require_session},
};

mod api;
mod errors;
mod middleware;

pub async fn run(state: AppState, addr: SocketAddr) -> Result<()> {
    let api_router = Router::new()
        // unauthenticated routes
        .nest(
            "/api",
            Router::new()
                .route("/login", post(api::login))
                .route("/logout", post(api::logout))
                .route("/guestbook", get(api::get_guestbook)),
        )
        // require authentication
        .nest(
            "/api",
            Router::new()
                .route("/test", get((StatusCode::OK, "test")))
                .route_layer(axum::middleware::from_fn(require_session))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    extract_session,
                )),
        )
        .fallback(|| async { APIResult::<Body>::Err(APIError::NotFound) })
        .with_state(state.clone());

    let sites_router = Router::new()
        .fallback(|| async { APIResult::<Body>::Err(APIError::NotFound) })
        .with_state(state.clone());

    // Use a different router based on the hostname.
    let app = |request: Request| async move {
        _ = state.projects.get("test");

        // We don't use the Host extractor as it returns X-Forwarded-Host if present,
        // which can be spoofed by the client.
        let hostname_header = request
            .headers()
            .get("HOST")
            .ok_or_else(|| APIError::BadRequest("no hostname".to_string()))?
            .to_str()
            .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

        let (hostname, port) = if let Some(colon) = hostname_header.find(':') {
            let (hostname, port) = hostname_header.split_at(colon);
            (hostname, &port[1..])
        } else {
            (hostname_header, "80")
        };

        let domain = addr::parse_domain_name(hostname)
            .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

        let is_api = {
            if cfg!(debug_assertions) {
                domain.prefix().is_none() && domain.suffix() == "localhost"
            } else {
                // this should get redirected by our reverse proxy
                if port != "80" {
                    return APIResult::Err(APIError::BadRequest("insecure connection".to_string()));
                }

                domain.root() == Some("dawdle.space")
            }
        };

        if is_api {
            APIResult::Ok(api_router.oneshot(request).await.into_response())
        } else {
            // TODO: insert the project name into the request extensions.
            // request.extensions_mut().insert("asdf");
            APIResult::Ok(sites_router.oneshot(request).await.into_response())
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_service()).await?;

    Ok(())
}
