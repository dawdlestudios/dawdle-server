use crate::utils::RingBuffer;
use dashmap::DashMap;
use std::{collections::HashSet, sync::atomic::AtomicU64};
use tokio::sync::broadcast;

use super::{ChatMessage, ChatRequest, ChatResponse};

#[derive(Debug)]
pub struct Room {
    connected_users: HashSet<String>,
    message_history: RingBuffer<ChatMessage>,
}

impl Default for Room {
    fn default() -> Self {
        Self {
            connected_users: HashSet::new(),
            message_history: RingBuffer::new(20),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub username: String,
    pub(super) channel: broadcast::Sender<ChatResponse>,
}

impl Connection {
    pub fn send_room_history(&self, room: &str, history: Vec<ChatMessage>) {
        let _ = self.channel.send(ChatResponse::RoomHistory {
            room: room.to_string(),
            history: history,
        });
    }

    pub fn send_info(&self, default_room: &str, public_rooms: Vec<String>) {
        let _ = self.channel.send(ChatResponse::Info {
            default_room: default_room.to_string(),
            public_rooms,
            private_rooms: None,
        });
    }

    // send a message only to this connection
    pub fn send_msg(&self, username: &str, room: &str, message: &str) {
        let _ = self.channel.send(ChatResponse::Message(ChatMessage {
            username: username.to_string(),
            message: message.to_string(),
            room: room.to_string(),
            time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
        }));
    }
}

#[derive(Debug)]
pub struct ChatState {
    pub connections: DashMap<String, Connection>,
    pub rooms: DashMap<String, Room>,
    pub guest_id_counter: AtomicU64,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            guest_id_counter: AtomicU64::new(0),
            rooms: DashMap::from_iter(vec![("general".to_string(), Room::default())].into_iter()),
            connections: DashMap::new(),
        }
    }

    // only if the room is exists
    pub fn join_room(&self, room: &str, username: &str) {
        if let Some(mut room) = self.rooms.get_mut(room) {
            room.connected_users.insert(username.to_string());
        }
    }

    pub fn connect(&self, username: Option<String>) -> Connection {
        let username = username.unwrap_or_else(|| format!("guest-{}", self.new_guest_id()));

        let connection = Connection {
            username: username.clone(),
            channel: broadcast::channel(100).0,
        };

        self.connections
            .insert(username.clone(), connection.clone());

        connection
    }

    pub fn disconnect(&self, username: &str) {
        for mut room in self.rooms.iter_mut() {
            room.connected_users.remove(username);
        }
    }

    pub fn send_message(&self, room_name: &str, username: &str, message: String) {
        let room = {
            let Some(mut room) = self.rooms.get_mut(room_name) else {
                log::error!("room {} does not exist", room_name);
                return;
            };

            room.message_history.push(ChatMessage {
                username: username.to_string(),
                message: message.clone(),
                room: room_name.to_string(),
                time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
            });

            // don't keep the mut locked for longer than necessary
            room.downgrade()
        };

        for user in &room.connected_users {
            if let Some(connection) = self.connections.get(user) {
                let _ = connection.channel.send(ChatResponse::Message(ChatMessage {
                    username: username.to_string(),
                    message: message.clone(),
                    room: room_name.to_string(),
                    time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
                }));
            }
        }
    }

    pub fn room_history(&self, room_name: &str) -> Vec<ChatMessage> {
        if let Some(room) = self.rooms.get(room_name) {
            room.message_history.to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn handle_req(&self, req: ChatRequest, connection: Connection) {
        match req {
            ChatRequest::Message { room, message } => {
                if message.starts_with('/') {
                    self.handle_command(&room, &message, connection);
                    return;
                }
                self.send_message(&room, &connection.username, message);
            }
            ChatRequest::History { room: room_name } => {
                if let Some(room) = self.rooms.get(&room_name) {
                    let history = room.message_history.to_vec();
                    connection.send_room_history(&room_name, history);
                }
            }
            _ => {
                let _ = connection.channel.send(ChatResponse::Error {
                    message: "unimplemented".to_string(),
                });
            }
        };
    }

    pub fn handle_command(&self, room: &str, message: &str, connection: Connection) {
        let mut parts = message.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args = parts.collect::<Vec<_>>();

        match command {
            "/help" => {
                let _ = connection.send_msg("system", room, "no commands available");
            }
            _ => {
                let _ =
                    connection.send_msg("system", room, &format!("unknown command: {}", command));
            }
        }
    }

    pub fn new_guest_id(&self) -> u64 {
        self.guest_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}
