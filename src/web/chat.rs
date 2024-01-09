use super::middleware::OptionalSession;
use crate::state::State as AppState;
use axum::{
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
};

pub async fn handler(
    ws: WebSocketUpgrade,
    session: OptionalSession,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let username = session.username().map(|s| s.to_string());
    ws.on_upgrade(move |socket| crate::chat::handle_chat_socket(socket, username, state))
}
