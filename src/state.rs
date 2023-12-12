use argon2::PasswordVerifier;
use color_eyre::eyre::{eyre, Result};
use cuid2::cuid;
use okv::types::serde::SerdeRmp;
use serde::{Deserialize, Serialize};

pub type DatabaseBackend = okv::backend::rocksdb::RocksDb;
pub type Env = okv::Env<DatabaseBackend>;
pub type DB<K, V> = okv::Database<K, V, DatabaseBackend>;

#[derive(Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub public_keys: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub username: String,
    pub created: time::OffsetDateTime,
    pub last_active: time::OffsetDateTime,
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    username: String,
    path: String,
    name: String,
}

#[derive(Clone)]
pub struct State {
    pub sessions: DB<String, SerdeRmp<Session>>,
    pub users: DB<String, SerdeRmp<User>>,
    pub projects: DB<String, SerdeRmp<Project>>,

    pub guestbook: DB<u64, String>,
    pub guestbook_approved: DB<u64, String>,
}

pub fn create_env() -> Result<Env> {
    let env = Env::new(DatabaseBackend::new("./.db")?);
    Ok(env)
}

impl State {
    pub fn new(env: Env) -> Result<Self> {
        let users = env.open("users")?;
        let sessions = env.open("sessions")?;
        let projects = env.open("projects")?;
        let guestbook = env.open("guestbook")?;
        let guestbook_approved = env.open("guestbook_approved")?;

        Ok(Self {
            users,
            sessions,
            projects,
            guestbook,
            guestbook_approved,
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
            username: username.to_string(),
            created: time::OffsetDateTime::now_utc(),
            last_active: time::OffsetDateTime::now_utc(),
        };
        self.sessions.set(&session_token, &session)?;
        Ok(session_token)
    }

    pub fn verify_session(&self, session_token: &str) -> Result<Option<Session>> {
        let session: Session = self
            .sessions
            .get(session_token)?
            .ok_or_else(|| eyre!("session not found"))?;

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
