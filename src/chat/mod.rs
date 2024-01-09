use crate::state::State as AppState;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

pub mod state;

type Channel = String;
type Username = String;
type ChatMessage = String;

#[derive(Clone, Deserialize)]
pub enum ChatRequest {
    Message(ChatMessage),
    Join(Channel),
    History,
    Info,
}

#[derive(Clone, Serialize)]
pub enum ChatResponse {
    Join(Username, Channel, time::OffsetDateTime),
    Leave(Username, Channel, time::OffsetDateTime),
    Message(Username, ChatMessage, Channel, time::OffsetDateTime),

    Error(String),
}

fn response(chat_response: ChatResponse) -> Message {
    Message::Text(serde_json::to_string(&chat_response).unwrap())
}

pub async fn handle_chat_socket(stream: WebSocket, username: Option<String>, state: AppState) {
    let chat = state.chat;
    let (mut sender, mut receiver) = stream.split();

    let connection = chat.connect(username.clone());

    let mut rx = connection.channel.subscribe();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(response(msg)).await.is_err() {
                break;
            }
        }
    });

    let recv_connection = connection.clone();
    let recv_chat = chat.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let request: ChatRequest = match serde_json::from_str(&text) {
                Ok(request) => request,
                Err(err) => {
                    let _ = recv_connection
                        .channel
                        .send(ChatResponse::Error(err.to_string()));

                    continue;
                }
            };
            recv_chat.handle_req(request, recv_connection.clone());
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    chat.disconnect(&connection.username)
}
