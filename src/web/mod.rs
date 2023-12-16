use crate::state::State as AppState;
use axum::{
    extract::Request, handler::HandlerWithoutStateExt, http::StatusCode, response::IntoResponse,
    routing::*, Router,
};

use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tower::ServiceExt;

use self::middleware::{extract_session, require_session};

mod api;
mod errors;
mod middleware;

pub async fn run(state: AppState, addr: SocketAddr) -> Result<()> {
    let api_router = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/login", post(api::login))
                .route("/logout", post(api::logout)),
        )
        .nest(
            "/api",
            Router::new()
                // .route("/self", get(api::get_self))
                .route_layer(axum::middleware::from_fn(require_session))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    extract_session,
                )),
        )
        .fallback((StatusCode::NOT_FOUND, "not found"))
        .with_state(state.clone());

    let sites_router = Router::new().with_state(state.clone());

    // Use a different router based on the hostname.
    let app = |request: Request| async move {
        _ = state.projects.get("test");

        // We don't use the Host extractor as it returns X-Forwarded-Host if present,
        // which can be spoofed by the client.
        let Some(hostname) = request.headers().get("HOST").cloned() else {
            return (StatusCode::BAD_REQUEST, "no hostname").into_response();
        };

        // TODO: parse the hostname with `addr` (or `publicsuffix` if I decide to get on the list)
        // TODO: custom domains
        if hostname == "api" {
            api_router.oneshot(request).await.into_response()
        } else {
            // TODO: insert the project name into the request extensions.
            // request.extensions_mut().insert("asdf");
            sites_router.oneshot(request).await.into_response()
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_service()).await?;

    Ok(())
}
