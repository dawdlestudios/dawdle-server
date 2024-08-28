use cuid2::cuid;
use eyre::Result;
use libsql::{params, Connection};

use crate::utils::to_time;

#[derive(Clone)]
pub struct AppSessions {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub username: String,
    pub created_at: time::OffsetDateTime,
    pub last_active: time::OffsetDateTime,
    pub logged_out: bool,
}

impl AppSessions {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub async fn create(&self, username: &str) -> Result<String> {
        let session_token = cuid();

        self.conn
            .execute(
                "INSERT INTO sessions (session_token, username) VALUES (?, ?)",
                params![session_token.clone(), username],
            )
            .await?;

        Ok(session_token)
    }

    pub async fn logout(&self, session_token: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE sessions SET logged_out = 1 WHERE session_token = ?",
                [session_token],
            )
            .await?;
        Ok(())
    }

    pub async fn verify(&self, session_token: &str) -> Result<Option<Session>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT 
                username, 
                created_at, 
                last_active, 
                logged_out
             FROM sessions WHERE session_token = ?",
            )
            .await?;

        let row = stmt.query_row([session_token]).await?;
        let session = Session {
            username: row.get(0)?,
            created_at: to_time(row.get(1)?)?,
            last_active: to_time(row.get(2)?)?,
            logged_out: row.get(3)?,
        };

        if session.logged_out {
            return Ok(None);
        }

        const SESSION_TIMEOUT: i64 = 60 * 60 * 24 * 7; // 7 days
        let now = time::OffsetDateTime::now_utc();
        let last_active = session.last_active;
        if now.unix_timestamp() - last_active.unix_timestamp() > SESSION_TIMEOUT {
            return Ok(None);
        }

        self.conn
            .execute(
                "UPDATE sessions SET last_active = ? WHERE session_token = ?",
                params![now.unix_timestamp(), session_token],
            )
            .await?;

        Ok(Some(session))
    }
}
