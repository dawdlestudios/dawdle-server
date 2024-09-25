use argon2::PasswordVerifier;
use eyre::{eyre, Result};
use futures::{StreamExt, TryStreamExt};
use libsql::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::{
    minecraft,
    utils::{hash_pw, is_valid_username, to_time},
};

#[derive(Clone)]
pub struct AppUsers {
    conn: Connection,
    config: crate::config::Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
    pub role: Option<String>,

    pub minecraft_username: Option<String>,
    pub minecraft_uuid: Option<String>,
}

impl AppUsers {
    pub fn new(conn: Connection, config: crate::config::Config) -> Self {
        Self { conn, config }
    }

    pub async fn all_usernames(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT username FROM users").await?;

        let rows = stmt.query(()).await?;
        let usernames = rows.into_stream().map(|row| row?.get::<String>(0));
        Ok(usernames.try_collect::<Vec<_>>().await?)
    }

    pub async fn all(&self) -> Result<Vec<User>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT username, created_at, role, minecraft_username, minecraft_uuid FROM users",
            )
            .await?;

        let rows = stmt.query(()).await?;
        let users = rows.into_stream().map(|row| {
            let row = row?;
            eyre::Ok(User {
                username: row.get(0)?,
                created_at: to_time(row.get(1)?)?,
                role: row.get(2)?,
                minecraft_username: row.get(3)?,
                minecraft_uuid: row.get(4)?,
            })
        });

        users.try_collect::<Vec<_>>().await
    }

    pub async fn verify_password(&self, username: &str, password: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT password_hash FROM users WHERE username = ?")
            .await?;

        let row = stmt.query_row([username]).await?;
        let password_hash = row.get::<String>(0)?;
        let password_hash = argon2::PasswordHash::new(&password_hash)?;

        let hasher = argon2::Argon2::default();
        match hasher.verify_password(password.as_bytes(), &password_hash) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn create(&self, username: &str, password: &str, role: Option<&str>) -> Result<()> {
        let username = username.to_lowercase();
        if !is_valid_username(&username) {
            return Err(eyre!("invalid username"));
        }

        let password_hash = hash_pw(password)?;
        self.conn
            .execute(
                "INSERT INTO users (username, password_hash, role) VALUES (?, ?, ?)",
                params![username, password_hash, role],
            )
            .await?;

        Ok(())
    }

    pub async fn delete(&self, username: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM users WHERE username = ?", [username])
            .await?;

        Ok(())
    }

    pub async fn get(&self, username: &str) -> Result<Option<User>> {
        let mut stmt = self
            .conn
            .prepare("SELECT created_at, role, minecraft_username, minecraft_uuid FROM users WHERE username = ?")
            .await?;

        let Ok(row) = stmt.query_row([username]).await else {
            return Ok(None);
        };

        let user = User {
            username: username.to_string(),
            created_at: to_time(row.get(0)?)?,
            role: row.get(1)?,
            minecraft_username: row.get(2)?,
            minecraft_uuid: row.get(3)?,
        };

        Ok(Some(user))
    }

    pub async fn get_public_keys(&self, username: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT public_key, name FROM user_public_keys WHERE username = ?")
            .await?;

        let rows = stmt.query([username]).await?;
        let public_keys = rows.into_stream().map(|row| {
            let row = row?;
            let name = row.get::<String>(1)?;
            let key = row.get::<String>(0)?;
            eyre::Ok((name, key))
        });
        public_keys.try_collect::<Vec<_>>().await
    }

    pub async fn add_public_key(&self, username: &str, public_key: &str, name: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO user_public_keys (username, name, public_key) VALUES (?, ?, ?)",
                [username, name, public_key],
            )
            .await?;

        Ok(())
    }

    pub async fn remove_public_key(&self, username: &str, name: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM user_public_keys WHERE username = ? AND name = ?",
                [username, name],
            )
            .await?;

        Ok(())
    }

    pub async fn update_password(&self, username: &str, password: &str) -> Result<()> {
        let password_hash = hash_pw(password)?;
        self.conn
            .execute(
                "UPDATE users SET password_hash = ? WHERE username = ?",
                [&password_hash, username],
            )
            .await?;

        Ok(())
    }

    pub async fn update_role(&self, username: &str, role: Option<&str>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE users SET role = ? WHERE username = ?",
                params![role, username],
            )
            .await?;
        Ok(())
    }

    pub async fn update_minecraft_username(
        &self,
        username: &str,
        new_minecraft_username: &str,
    ) -> Result<()> {
        let user = self
            .get(username)
            .await?
            .ok_or_else(|| eyre!("user not found"))?;

        // if it's an uuid, error
        if new_minecraft_username.replace("-", "").len() == 32 {
            return Err(eyre!("minecraft username cannot be an UUID"));
        }

        let new_minecraft_user =
            minecraft::whitelist_add(new_minecraft_username, &self.config.minecraft).await?;

        log::info!(
            "minecraft user {} ({}) added to whitelist",
            new_minecraft_user.name,
            new_minecraft_user.id
        );

        // check if another user already has this minecraft username
        let mut exists_stmt = self
            .conn
            .prepare("SELECT username FROM users WHERE minecraft_uuid = ?")
            .await?;

        let existing_user = exists_stmt
            .query_row([new_minecraft_user.id.clone()])
            .await
            .map(|row| row.get::<String>(0));

        match existing_user {
            Ok(Ok(existing_user)) => {
                if existing_user != username {
                    return Err(eyre!("minecraft user already registered"));
                }
            }
            Ok(_) => return Err(eyre!("unexpected error")),
            Err(libsql::Error::QueryReturnedNoRows) => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(old_uuid) = &user.minecraft_uuid {
            if old_uuid != &new_minecraft_user.id {
                log::info!(
                    "minecraft user {} ({}) removed from whitelist",
                    user.minecraft_username.as_deref().unwrap_or("unknown"),
                    old_uuid
                );

                minecraft::whitelist_remove(old_uuid, &self.config.minecraft).await?;
            }
        }

        self.conn
            .execute(
                "UPDATE users SET minecraft_username = ?, minecraft_uuid = ? WHERE username = ?",
                params![
                    new_minecraft_user.name.clone(),
                    new_minecraft_user.id.clone(),
                    username
                ],
            )
            .await?;

        log::info!(
            "updated minecraft username for user {} to {} ({})",
            username,
            new_minecraft_user.name,
            new_minecraft_user.id
        );

        Ok(())
    }
}
