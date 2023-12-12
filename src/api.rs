use std::net::SocketAddr;

use crate::state::State as AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use color_eyre::eyre::Result;
use tower::ServiceExt;

pub async fn run(state: AppState, addr: SocketAddr) -> Result<()> {
    let api = Router::new()
        .route("/api", get(root))
        .with_state(state.clone());

    let pages = Router::new()
        .route("/", get(root))
        .with_state(state.clone());

    // Use a different router based on the hostname.
    let app = |State(state): State<AppState>, request: Request<Body>| async move {
        _ = state.projects.get("test");

        // We don't use the Host extractor as it returns X-Forwarded-Host if present,
        // which can be spoofed by the client.
        let Some(hostname) = request.headers().get("HOST").cloned() else {
            return (StatusCode::BAD_REQUEST, "no hostname").into_response();
        };

        // TODO: parse the hostname with `addr` (or `publicsuffix` if I decide to get on the list)
        // TODO: custom domains
        if hostname == "api" {
            return api.oneshot(request).await.into_response();
        } else {
            // TODO: insert the project name into the request extensions.
            // request.extensions_mut().insert("asdf");
            return pages.oneshot(request).await.into_response();
        }
    };

    // .fallback() as .route("/*", _) doesn't match / and .nest("/", _) doesn't want to use my closure :(
    let app = Router::new().fallback(app).with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> impl IntoResponse {
    "Hello, World!"
}
