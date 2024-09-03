use cuid2::cuid;
use eyre::{bail, Ok, Result};
use futures::{StreamExt, TryStreamExt};
use libsql::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::utils::{hash_pw, is_valid_username, to_time};

#[derive(Clone)]
pub struct AppApplications {
    conn: Connection,
    config: crate::config::Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub username: String,
    pub email: String,
    pub about: String,
    #[serde(with = "time::serde::rfc3339")]
    pub date: time::OffsetDateTime,
    pub approved: bool,
    pub claimed: bool,
    pub claim_token: Option<String>,
}

impl AppApplications {
    pub fn new(conn: Connection, config: crate::config::Config) -> Self {
        Self { conn, config }
    }

    pub async fn all(&self) -> Result<Vec<Application>> {
        let mut stmt = self
            .conn
            .prepare("SELECT application_id, requested_username, email, about, approved, claimed, claim_token, created_at FROM applications")
            .await?;
        let rows = stmt.query(()).await?;

        let applications = rows.into_stream().map(|row| {
            let row = row?;
            Ok(Application {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                about: row.get(3)?,
                approved: row.get(4)?,
                claimed: row.get(5)?,
                claim_token: row.get(6)?,
                date: to_time(row.get(7)?)?,
            })
        });

        Ok(applications.try_collect::<Vec<_>>().await?)
    }

    pub async fn approve(&self, id: &str) -> Result<()> {
        let token = cuid();
        self.conn
            .execute(
                "UPDATE applications SET approved = 1, claim_token = ? WHERE id = ?",
                params![token, id],
            )
            .await?;

        Ok(())
    }

    pub async fn apply(&self, username: &str, email: &str, about: &str) -> Result<()> {
        let username = username.to_lowercase();
        if !is_valid_username(&username) {
            log::error!("invalid username: {}", username);
            bail!("invalid username");
        }
        let err = self
            .conn
            .execute(
                "INSERT INTO applications (application_id, requested_username, email, about) VALUES (?, ?, ?, ?)",
                params![cuid(), username, email, about],
            )
            .await?;

        Ok(())
    }

    pub async fn claim(&self, token: &str, username: &str, pw: &str) -> Result<()> {
        let username = username.to_lowercase();
        if !is_valid_username(&username) {
            bail!("invalid username");
        }

        let tx = self.conn.transaction().await?;

        let mut stmt = tx
            .prepare(
                "SELECT id, approved, claimed, username FROM applications WHERE claim_token = ?",
            )
            .await?;
        let application = stmt.query_row([token]).await?;

        let (app_id, app_approved, app_claimed, app_username) = (
            application.get::<String>(0)?,
            application.get::<bool>(1)?,
            application.get::<bool>(2)?,
            application.get::<String>(3)?,
        );

        if !app_approved {
            bail!("application not approved");
        }

        if app_claimed {
            bail!("application already claimed");
        }

        if app_username != username {
            return Ok(()); // silently ignore
        }

        tx.execute(
            "UPDATE applications SET claimed = 1 WHERE id = ?",
            params![app_id],
        )
        .await?;

        tx.execute(
            "INSERT INTO users (username, password_hash) VALUES (?, ?)",
            params![username.clone(), hash_pw(pw)?],
        )
        .await?;

        self.create_home(&username)?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM applications WHERE id = ?", params![id])
            .await?;
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
            .join(username);

        if !user_home.exists() {
            std::fs::create_dir_all(&user_home)?;
        }

        log::info!(
            "copying default home folder to {}",
            user_home.to_str().unwrap()
        );

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
