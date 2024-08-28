pub const DOCKER_IMAGE: &str = "ghcr.io/dawdlestudios/container";
pub const DOCKER_TAG: &str = "latest";
pub const DOCKER_CONTAINER_PREFIX: &str = "dawdle-home-";

#[cfg(target_os = "macos")]
pub const DOCKER_SOCKET_MACOS: &str = "unix:///Users/henry/.colima/default/docker.sock";

pub const FILES_FOLDER: &str = ".files";
// pub const KEYS_FOLDER: &str = ".keys";
pub const DB_FOLDER: &str = ".db";

pub const FILES_HOME: &str = "home/";
pub const FILES_DEFAULT_HOME: &str = "default-home";

use serde::{Deserialize, Serialize};

use crate::utils::{is_valid_project_path, is_valid_username};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_base_dir")]
    pub base_dir: String,

    pub ssh_port: u16,
    pub ssh_interface: String,
    pub www_port: u16,
    pub www_interface: String,
}

fn default_base_dir() -> String {
    std::env::current_dir()
        .expect("failed to get current dir")
        .to_str()
        .expect("failed to convert cwd to str")
        .to_string()
}

impl Config {
    pub fn user_public_path(&self, username: &str) -> Option<std::path::PathBuf> {
        if !is_valid_username(username) {
            return None;
        }

        Some(
            std::path::Path::new(&self.base_dir)
                .join(crate::config::FILES_FOLDER)
                .join(crate::config::FILES_HOME)
                .join(username.to_ascii_lowercase())
                .join("public"),
        )
    }

    pub fn db_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.base_dir)
            .join(crate::config::DB_FOLDER)
            .join("db.sqlite")
    }

    pub fn project_path(&self, username: &str, project_path: &str) -> Option<std::path::PathBuf> {
        if !is_valid_username(username) || !is_valid_project_path(project_path) {
            return None;
        }

        Some(
            std::path::Path::new(&self.base_dir)
                .join(crate::config::FILES_FOLDER)
                .join(crate::config::FILES_HOME)
                .join(username.to_ascii_lowercase())
                .join(project_path),
        )
    }

    pub fn load() -> eyre::Result<Self> {
        let config_path = std::env::var("DAWDLE_HOME_CONFIG").unwrap_or_else(|_| {
            std::env::current_dir()
                .expect("failed to get current dir")
                .join("./dawdle.config.toml")
                .to_str()
                .expect("failed to convert cwd to str")
                .to_string()
        });

        let config = std::fs::read_to_string(config_path.clone())?;
        let config: Config = toml::from_str(&config)?;

        log::info!("loaded config from {}", config_path);
        Ok(config)
    }
}
