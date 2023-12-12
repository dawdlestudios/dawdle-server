use std::net::SocketAddr;

use crate::state::State;
use axum::{routing::get, Router};
use color_eyre::eyre::Result;

pub async fn run(state: State, addr: SocketAddr) -> Result<()> {
    let app = Router::new()
        .with_state(state)
        // .route("/api", get(api))
        .route("/", get(root));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}
