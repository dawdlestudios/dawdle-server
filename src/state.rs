use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use argon2::PasswordVerifier;
use color_eyre::eyre::{eyre, Result};
use cuid2::cuid;
use okv::types::serde::SerdeRmp;
use serde::{Deserialize, Serialize};

pub type DatabaseBackend = okv::backend::rocksdb::RocksDbOptimistic;
pub type Env = okv::Env<DatabaseBackend>;
pub type DB<K, V> = okv::Database<K, V, DatabaseBackend>;

pub type PublicKeyData = String;
pub type PublicKeyName = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub password_hash: String,
    pub ssh_allow_password: bool,
    pub public_keys: Vec<(PublicKeyName, PublicKeyData)>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub username: String,
    pub created: time::OffsetDateTime,
    pub last_active: time::OffsetDateTime,
    pub logged_out: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    username: String,
    path: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Application {
    username: String,
    email: String,
    about: String,
}

#[derive(Clone)]
pub struct State {
    pub sessions: DB<String, SerdeRmp<Session>>,
    pub users: DB<String, SerdeRmp<User>>,
    // pub projects: DB<String, SerdeRmp<Project>>,
    pub applications: DB<String, SerdeRmp<(Application, time::OffsetDateTime)>>,
    pub guestbook: DB<u64, String>,
    pub guestbook_approved: DB<u64, String>,

    pub subdomains: Arc<RwLock<HashMap<String, Website>>>,
    pub custom_domains: Arc<RwLock<HashMap<String, Website>>>,
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

impl State {
    pub fn new(env: Env) -> Result<Self> {
        let users = env.open("users")?;
        let sessions = env.open("sessions")?;
        // let projects = env.open("projects")?;
        let guestbook = env.open("guestbook")?;
        let guestbook_approved = env.open("guestbook_approved")?;
        let applications = env.open("applications")?;

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
            users,
            sessions,
            // projects,
            guestbook,
            guestbook_approved,
            applications,
            subdomains,
            custom_domains,
        })
    }

    pub fn add_guestbook_entry(&self, entry: &str) -> Result<()> {
        self.guestbook.set(
            &(time::OffsetDateTime::now_utc().unix_timestamp() as u64),
            entry,
        )?;
        Ok(())
    }

    pub fn guestbook(&self) -> Result<Vec<(u64, String)>> {
        let mut guestbook = self
            .guestbook_approved
            .iter()?
            .collect::<Result<Vec<_>, _>>()?;

        guestbook.sort_by_key(|(k, _)| *k);
        Ok(guestbook)
    }

    pub fn create_session(&self, username: &str) -> Result<String> {
        let session_token = cuid();
        let session = Session {
            logged_out: false,
            username: username.to_string(),
            created: time::OffsetDateTime::now_utc(),
            last_active: time::OffsetDateTime::now_utc(),
        };
        self.sessions.set(&session_token, &session)?;
        Ok(session_token)
    }

    pub fn logout_session(&self, session_token: &str) -> Result<()> {
        let session: Session = self
            .sessions
            .get(session_token)?
            .ok_or_else(|| eyre!("session not found"))?;

        let session = Session {
            logged_out: true,
            ..session
        };

        self.sessions.set(session_token, &session)?;
        Ok(())
    }

    pub fn verify_session(&self, session_token: &str) -> Result<Option<Session>> {
        let session: Session = self
            .sessions
            .get(session_token)?
            .ok_or_else(|| eyre!("session not found"))?;

        if session.logged_out {
            return Ok(None);
        }

        const SESSION_TIMEOUT: i64 = 60 * 60 * 24 * 7; // 7 days
        let now = time::OffsetDateTime::now_utc();
        let last_active = session.last_active;
        if now.unix_timestamp() - last_active.unix_timestamp() > SESSION_TIMEOUT {
            return Ok(None);
        }

        let session = Session {
            last_active: now,
            ..session
        };

        self.sessions.set(session_token, &session)?;
        Ok(Some(session))
    }

    pub fn verify_password(&self, username: &str, password: &str) -> Result<bool> {
        let user = self.users.get(username)?;
        let user = match user {
            Some(user) => user,
            None => return Ok(false),
        };

        let hasher = argon2::Argon2::default(); // argon2id
        let password_hash = argon2::PasswordHash::new(&user.password_hash)?;
        match hasher.verify_password(password.as_bytes(), &password_hash) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
}
