use core::{AppApplications, AppSessions, AppUsers};
use std::sync::Arc;

use dashmap::DashMap;
use eyre::Result;
use refinery_libsql::LibsqlConn;
use serde::{Deserialize, Serialize};

mod core;
mod refinery_libsql;
pub use core::{Session, User};

use crate::{chat::state::ChatState, config::Config};

#[derive(Clone)]
pub struct App {
    pub users: AppUsers,
    pub applications: AppApplications,
    pub sessions: AppSessions,
    pub chat: Arc<crate::chat::state::ChatState>,

    pub config: Config,
    pub sites: Arc<DashMap<String, Website>>,
}

type Username = String;
type RelativeProjectPath = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Website {
    User(Username), // always at ~/public
    Site(Username, RelativeProjectPath),
}

refinery::embed_migrations!("src/migrations");

impl App {
    pub async fn new(config: Config) -> Result<Self> {
        std::fs::create_dir_all(config.db_path().parent().unwrap())?;
        let db = libsql::Builder::new_local(config.db_path()).build().await?;
        let conn = db.connect()?;

        let mut runner = migrations::runner();
        runner.set_migration_table_name("migrations");
        runner.run_async(&mut LibsqlConn(conn.clone())).await?;

        let users = AppUsers::new(conn.clone(), config.clone());
        let applications = AppApplications::new(conn.clone(), config.clone());
        let sessions = AppSessions::new(conn.clone());

        let sites = {
            DashMap::from_iter(
                users
                    .all_usernames()
                    .await?
                    .into_iter()
                    .map(|username| (username.clone(), Website::User(username))),
            )
        };

        sites.insert(
            "lastfm-iceberg".to_string(),
            Website::Site("henry".to_string(), "sites/lastfm-iceberg".to_string()),
        );

        Ok(Self {
            users,
            applications,
            sessions,
            config,
            sites: Arc::new(sites),
            chat: Arc::new(ChatState::new()),
        })
    }

    // only needs to be called manually if e.g. a new user is added or a site is created
    // otherwise, it will be called automatically when the server starts
    pub fn set_site(&self, subdomain: String, website: Website) {
        self.sites.insert(subdomain, website);
    }
}
