use std::sync::Arc;

use argon2::PasswordVerifier;
use color_eyre::eyre::{bail, eyre, Result};
use cuid2::cuid;
use okv::types::serde::SerdeJson;
use serde::{Deserialize, Serialize};

use crate::utils::valid_public_key;

use super::DB;

pub type PublicKeyData = String;
pub type PublicKeyName = String;
pub type ApplicationID = String;

#[derive(Clone)]
pub struct UserState {
    pub config: Arc<crate::config::Config>,
    pub sessions: DB<String, SerdeJson<Session>>,
    pub users: DB<String, SerdeJson<User>>,
    pub applications: DB<String, SerdeJson<Application>>,
    pub claim_tokens: DB<String, ApplicationID>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub username: String,
    pub created: time::OffsetDateTime,
    pub last_active: time::OffsetDateTime,
    pub logged_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub password_hash: String,
    pub public_keys: Vec<(PublicKeyName, PublicKeyData)>,
    pub role: Option<String>,
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

impl UserState {
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

        if !application.approved {
            return Err(eyre!("application not approved"));
        }

        if application.claim_token != Some(token.to_string()) {
            return Err(eyre!("invalid claim token"));
        }

        if application.username != username {
            return Err(eyre!("invalid username"));
        }

        let application = Application {
            claimed: true,
            claim_token: None,
            ..application
        };

        self.users.set_nx(
            &application.username,
            &User {
                password_hash: crate::utils::hash_pw(pw).map_err(|_| eyre!("failed to hash pw"))?,
                public_keys: vec![],
                role: None,
            },
        )?;
        self.applications.set(&application_id, &application)?;
        self.claim_tokens.delete(token)?;

        self.create_home(&application.username)?;

        Ok(())
    }

    fn create_home(&self, username: &str) -> Result<()> {
        // copy the default home folder to the user's new home folder
        let default_home = std::path::Path::new(&self.config.base_dir)
            .join(crate::config::FILES_FOLDER)
            .join(crate::config::FILES_DEFAULT_HOME);

        let user_home = std::path::Path::new(&self.config.base_dir)
            .join(crate::config::FILES_FOLDER)
            .join(crate::config::FILES_HOME)
            .join(&username);

        if !user_home.exists() {
            std::fs::create_dir_all(&user_home)?;
        }

        for entry in std::fs::read_dir(default_home)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().unwrap();
            let dest = user_home.join(filename);
            if !dest.exists() {
                std::fs::copy(path, dest)?;
            }
        }

        Ok(())
    }
}

// General
impl UserState {
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

    pub fn create(&self, username: &str, user: User) -> Result<()> {
        self.users.set_nx(&username, &user)?;
        Ok(())
    }

    pub fn get(&self, username: &str) -> Result<Option<User>> {
        let user = self.users.get(username)?;
        Ok(user)
    }

    pub fn add_public_key(&self, username: &str, name: &str, data: &str) -> Result<()> {
        let tx = self.users.transaction()?;
        let mut user: User = tx.get(username)?.ok_or_else(|| eyre!("user not found"))?;

        // try to parse the key to make sure it's valid
        if !valid_public_key(data) {
            bail!("invalid public key");
        }

        if user.public_keys.iter().any(|(n, _)| n == name) {
            bail!("key name already exists")
        }

        user.public_keys.push((name.to_string(), data.to_string()));

        tx.set(username, &user)?;
        tx.commit()?;

        Ok(())
    }

    pub fn remove_public_key(&self, username: &str, name: &str) -> Result<()> {
        let tx = self.users.transaction()?;
        let mut user: User = tx.get(username)?.ok_or_else(|| eyre!("user not found"))?;

        if !user.public_keys.iter().any(|(n, _)| n == name) {
            bail!("key name does not exist")
        }

        user.public_keys.retain(|(n, _)| n != name);
        tx.set(username, &user)?;
        tx.commit()?;
        Ok(())
    }
}

// Sessions
impl UserState {
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
}
