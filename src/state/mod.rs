use std::sync::Arc;

use color_eyre::eyre::Result;
use dashmap::DashMap;
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
    pub sites: DashMap<String, Website>,

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

        let sites = {
            let domains = users.iter()?.collect::<Result<Vec<(String, User)>, _>>()?;
            DashMap::from_iter(
                domains
                    .into_iter()
                    .map(|(k, _)| (k.clone(), Website::User(k))),
            )
        };

        Ok(Self {
            user: UserState {
                users,
                sessions,
                applications,
                claim_tokens,
            },
            guestbook: GuestbookState { guestbook },
            sites,
            config: Arc::new(Config::default()),
            chat: Arc::new(crate::chat::state::ChatState::new()),
        })
    }

    // only needs to be called manually if e.g. a new user is added or a site is created
    // otherwise, it will be called automatically when the server starts
    pub fn set_site(&self, subdomain: String, website: Website) {
        self.sites.insert(subdomain, website);
    }
}