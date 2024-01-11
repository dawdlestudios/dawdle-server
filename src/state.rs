use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use argon2::PasswordVerifier;
use color_eyre::eyre::{eyre, Result};
use cuid2::cuid;
use okv::types::serde::SerdeJson;
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
    pub role: Option<String>,
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
    pub id: String,
    pub username: String,
    pub email: String,
    pub about: String,
    pub date: u64,
    pub approved: bool,
    pub claimed: bool,
    pub claim_token: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GuestbookEntry {
    pub id: String,
    pub date: u64,
    pub by: String,
    pub message: String,
    pub approved: bool,
}

type ApplicationID = String;

#[derive(Clone)]
pub struct State {
    pub sessions: DB<String, SerdeJson<Session>>,
    pub users: DB<String, SerdeJson<User>>,
    // pub projects: DB<String, SerdeJson<Project>>,
    pub applications: DB<String, SerdeJson<Application>>,
    pub claim_tokens: DB<String, ApplicationID>,
    pub guestbook: DB<String, SerdeJson<GuestbookEntry>>,

    pub subdomains: Arc<RwLock<HashMap<String, Website>>>,
    pub custom_domains: Arc<RwLock<HashMap<String, Website>>>,

    pub config: Arc<Config>,
    pub chat: Arc<crate::chat::state::ChatState>,
}

pub struct Config {
    pub base_dir: String,
    pub home_dirs: String,
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

impl State {
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
            users,
            sessions,
            guestbook,
            applications,
            subdomains,
            claim_tokens,
            custom_domains,
            config: Arc::new(Config::default()),
            chat: Arc::new(crate::chat::state::ChatState::new()),
        })
    }

    pub fn add_guestbook_entry(&self, entry: &str) -> Result<()> {
        let id = cuid();

        let entry = GuestbookEntry {
            id: id.clone(),
            date: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
            by: "guest".to_string(),
            message: entry.to_string(),
            approved: false,
        };

        self.guestbook.set(&id, &entry)?;
        Ok(())
    }

    pub fn guestbook_entries(&self) -> Result<Vec<GuestbookEntry>> {
        let entries = self.guestbook.iter()?.collect::<Result<Vec<_>, _>>()?;
        Ok(entries.into_iter().map(|(_, v)| v).collect())
    }

    pub fn approved_guestbook_entries(&self) -> Result<Vec<GuestbookEntry>> {
        let entries = self.guestbook.iter()?.collect::<Result<Vec<_>, _>>()?;
        Ok(entries
            .into_iter()
            .map(|(_, v)| v)
            .filter(|entry| entry.approved)
            .collect())
    }

    pub fn approve_guestbook_entry(&self, id: &str) -> Result<()> {
        let entry: GuestbookEntry = self
            .guestbook
            .get(id)?
            .ok_or_else(|| eyre!("entry not found"))?;

        let entry = GuestbookEntry {
            approved: true,
            ..entry
        };

        self.guestbook.set(id, &entry)?;
        Ok(())
    }

    pub fn applications(&self) -> Result<Vec<Application>> {
        let applications = self.applications.iter()?.collect::<Result<Vec<_>, _>>()?;
        Ok(applications.into_iter().map(|(_, v)| v).collect())
    }

    pub fn approve_application(&self, id: &str) -> Result<String> {
        let application: Application = self
            .applications
            .get(id)?
            .ok_or_else(|| eyre!("application not found"))?;

        let token = cuid();
        let application = Application {
            approved: true,
            claimed: false,
            claim_token: Some(token.clone()),
            ..application
        };

        self.applications.set(id, &application)?;
        self.claim_tokens.set(&token, &application.id)?;

        Ok(token)
    }

    pub fn apply(&self, username: &str, email: &str, about: &str) -> Result<()> {
        let id = cuid();
        let application = Application {
            id: id.clone(),
            username: username.to_string(),
            email: email.to_string(),
            about: about.to_string(),
            date: time::OffsetDateTime::now_utc().unix_timestamp() as u64,
            approved: false,
            claimed: false,
            claim_token: None,
        };

        self.applications.set_nx(&id, &application)?;
        Ok(())
    }

    pub fn claim(&self, token: &str, username: &str, pw: &str) -> Result<()> {
        let application_id = self
            .claim_tokens
            .get(token)?
            .ok_or_else(|| eyre!("invalid claim token"))?;

        let application: Application = self
            .applications
            .get(&application_id)?
            .ok_or_else(|| eyre!("application not found"))?;

        if application.claim_token != Some(token.to_string()) {
            return Err(eyre!("invalid claim token"));
        }

        if application.username != username {
            return Err(eyre!("invalid username"));
        }

        let application = Application {
            claimed: true,
            ..application
        };

        self.users.set_nx(
            &application.username,
            &User {
                password_hash: crate::utils::hash_pw(pw).map_err(|_| eyre!("failed to hash pw"))?,
                ssh_allow_password: false,
                public_keys: vec![],
                role: None,
            },
        )?;

        self.applications.set(&application_id, &application)?;
        Ok(())
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
