use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicU64, RwLock},
};

use tokio::sync::broadcast;

use super::{ChatRequest, ChatResponse};

#[derive(Debug, Clone, Default)]
pub struct Room {
    connected_users: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub username: String,
    pub(super) channel: broadcast::Sender<ChatResponse>,
}

#[derive(Debug)]
pub struct ChatState {
    pub connections: RwLock<HashMap<String, Connection>>,
    pub rooms: RwLock<HashMap<String, Room>>,
    pub guest_id_counter: AtomicU64,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            guest_id_counter: AtomicU64::new(0),
            rooms: RwLock::new(HashMap::from_iter(
                vec![("general".to_string(), Room::default())].into_iter(),
            )),
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub fn room(&self, room: &str) -> Option<Room> {
        self.rooms.read().unwrap().get(room).cloned()
    }

    // only if the room is exists
    pub fn join_room(&self, room: &str, username: &str) {
        let mut rooms = self.rooms.write().unwrap();
        if let Some(room) = rooms.get_mut(room) {
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
            .write()
            .unwrap()
            .insert(username, connection.clone());

        connection
    }

    pub fn disconnect(&self, username: &str) {
        let mut rooms = self.rooms.write().unwrap();
        for (_, room) in rooms.iter_mut() {
            room.connected_users.remove(username);
        }
    }

    pub fn send_message(&self, room_name: &str, username: &str, message: String) {
        let connections = self.connections.read().unwrap();
        let Some(room) = self.room(room_name) else {
            log::error!("room {} does not exist", room_name);
            return;
        };

        for user in room.connected_users {
            if let Some(connection) = connections.get(&user) {
                log::info!("sending message to {}", user);
                let _ = connection.channel.send(ChatResponse::Message {
                    username: username.to_string(),
                    message: message.clone(),
                    room: room_name.to_string(),
                    time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
                });
            }
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
                let _ = connection.channel.send(ChatResponse::Message {
                    username: "system".to_string(),
                    message: "no commands available".to_string(),
                    room: room.to_string(),
                    time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
                });
            }
            _ => {
                let _ = connection.channel.send(ChatResponse::Message {
                    username: "system".to_string(),
                    message: format!("unknown command: {}", command),
                    room: room.to_string(),
                    time: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
                });
            }
        }
    }

    pub fn new_guest_id(&self) -> u64 {
        self.guest_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}
