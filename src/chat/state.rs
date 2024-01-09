use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicU64, RwLock},
};

use tokio::sync::broadcast;

use super::{ChatRequest, ChatResponse};

#[derive(Debug, Clone)]
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
            rooms: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub fn room(&self, room: &str) -> Option<Room> {
        self.rooms.read().unwrap().get(room).cloned()
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

    pub fn handle_req(&self, req: ChatRequest, connection: Connection) {
        match req {
            _ => {
                let _ = connection
                    .channel
                    .send(ChatResponse::Error("not implemented".to_string()));
            }
        };
    }

    pub fn new_guest_id(&self) -> u64 {
        self.guest_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}
