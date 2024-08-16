use crate::app::App;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

pub mod state;

type Room = String;
type Username = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    username: Username,
    room: Room,
    message: String,
    time: u64,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatRequest {
    Message { room: Room, message: String },
    Join { room: Room },
    History { room: Room },
    Info,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatResponse {
    // Join {
    //     username: Username,
    //     room: Room,
    //     time: u64,
    // },
    // Leave {
    //     username: Username,
    //     room: Room,
    //     time: u64,
    // },
    Message(ChatMessage),
    #[serde(rename_all = "camelCase")]
    Info {
        default_room: Room,
        public_rooms: Vec<Room>,
        private_rooms: Option<Vec<Room>>,
    },

    // Room {
    //     room: Room,
    //     users: Vec<Username>,
    // },
    RoomHistory {
        room: Room,
        history: Vec<ChatMessage>,
    },

    Error {
        message: String,
    },
}

fn response(chat_response: ChatResponse) -> Message {
    Message::Text(serde_json::to_string(&chat_response).unwrap())
}

pub async fn handle_chat_socket(stream: WebSocket, username: Option<String>, state: App) {
    let chat = state.chat;
    let (mut sender, mut receiver) = stream.split();

    let connection = chat.connect(username.clone());
    chat.join_room("general", &connection.username);

    let mut rx = connection.channel.subscribe();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(response(msg)).await.is_err() {
                break;
            }
        }
    });

    connection.send_info("general", Vec::new());
    connection.send_room_history("general", chat.room_history("general"));

    let recv_connection = connection.clone();
    let recv_chat = chat.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let request: ChatRequest = match serde_json::from_str(&text) {
                Ok(request) => request,
                Err(err) => {
                    let _ = recv_connection.channel.send(ChatResponse::Error {
                        message: format!("Invalid request: {}", err),
                    });

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
