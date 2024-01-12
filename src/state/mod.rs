use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};

mod guestbook;
mod users;
use guestbook::GuestbookState;
use users::UserState;
pub use users::*;

pub type DatabaseBackend = okv::backend::rocksdb::RocksDbOptimistic;
pub type Env = okv::Env<DatabaseBackend>;
pub type DB<K, V> = okv::Database<K, V, DatabaseBackend>;

#[derive(Clone)]
pub struct AppState {
    pub user: UserState,
    pub guestbook: GuestbookState,

    // pub projects: DB<String, SerdeJson<Project>>,
    pub subdomains: Arc<RwLock<HashMap<String, Website>>>,
    pub custom_domains: Arc<RwLock<HashMap<String, Website>>>,

    pub config: Arc<Config>,
    pub chat: Arc<crate::chat::state::ChatState>,
}

pub struct Config {
    pub base_dir: String,
    pub home_dirs: String,
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    username: String,
    path: String,
    name: String,
}

impl Default for Config {
    fn default() -> Self {
        let cwd = std::env::current_dir().expect("failed to get current dir");
        Self {
            home_dirs: ".files/home".to_string(),
            base_dir: cwd
                .to_str()
                .expect("failed to convert cwd to str")
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Website {
    User(String),
    Project(String, String),
}

pub fn create_env() -> Result<Env> {
    let env = Env::new(DatabaseBackend::new("./.db")?);
    Ok(env)
}

impl AppState {
    pub fn new(env: Env) -> Result<Self> {
        let users = env.open("users")?;
        let sessions = env.open("sessions")?;
        let guestbook = env.open("guestbook")?;
        let applications = env.open("applications")?;
        let claim_tokens = env.open("claim_tokens")?;

        let subdomains = Arc::new(RwLock::new(HashMap::new()));
        let custom_domains = Arc::new(RwLock::new(HashMap::new()));

        {
            subdomains
                .write()
                .expect("failed to lock subdomains")
                .extend(
                    users
                        .iter()?
                        .map(|user| {
                            user.map(|(username, _): (String, _)| {
                                (username.clone(), Website::User(username))
                            })
                        })
                        .collect::<Result<HashMap<_, _>, _>>()?,
                );
        }

        Ok(Self {
            user: UserState {
                users,
                sessions,
                applications,
                claim_tokens,
            },
            guestbook: GuestbookState { guestbook },
            subdomains,
            custom_domains,
            config: Arc::new(Config::default()),
            chat: Arc::new(crate::chat::state::ChatState::new()),
        })
    }
}
