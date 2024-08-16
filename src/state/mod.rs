use std::sync::Arc;

use color_eyre::eyre::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

mod guestbook;
mod users;
use guestbook::GuestbookState;
use users::UserState;
pub use users::*;

use crate::config::{Config, DB_FOLDER};

#[derive(Clone)]
pub struct AppState {
    pub user: UserState,
    pub guestbook: GuestbookState,

    // pub projects: DB<String, SerdeJson<Project>>,
    pub sites: DashMap<String, Website>,

    pub config: Arc<Config>,
    pub chat: Arc<crate::chat::state::ChatState>,
}

type Username = String;
type RelativeProjectPath = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Website {
    User(Username), // always at ~/public
    Site(Username, RelativeProjectPath),
}

impl AppState {
    pub fn new(env: Env, config: Arc<Config>) -> Result<Self> {
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

        sites.insert(
            "lastfm-iceberg".to_string(),
            Website::Site("henry".to_string(), "sites/lastfm-iceberg".to_string()),
        );

        Ok(Self {
            user: UserState {
                config: config.clone(),
                users,
                sessions,
                applications,
                claim_tokens,
            },
            guestbook: GuestbookState { guestbook },
            sites,
            config,
            chat: Arc::new(crate::chat::state::ChatState::new()),
        })
    }

    // only needs to be called manually if e.g. a new user is added or a site is created
    // otherwise, it will be called automatically when the server starts
    pub fn set_site(&self, subdomain: String, website: Website) {
        self.sites.insert(subdomain, website);
    }
}
