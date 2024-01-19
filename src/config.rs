pub const DOCKER_IMAGE: &str = "ghcr.io/dawdlestudios/container";
pub const DOCKER_TAG: &str = "latest";
pub const DOCKER_CONTAINER_PREFIX: &str = "dawdle-home-";

#[cfg(target_os = "darwin")]
pub const DOCKER_SOCKET_MACOS: &str = "unix:///Users/henry/.colima/default/docker.sock";

pub const FILES_FOLDER: &str = ".files";
// pub const KEYS_FOLDER: &str = ".keys";
pub const DB_FOLDER: &str = ".db";

pub const FILES_HOME: &str = "home/";
pub const FILES_DEFAULT_HOME: &str = "default-home";

use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_base_dir")]
    pub base_dir: String,

    pub ssh_port: u16,
    pub ssh_interface: String,
    pub www_port: u16,
    pub www_interface: String,

    pub initial_user: Option<(String, String)>,
}

fn default_base_dir() -> String {
    std::env::current_dir()
        .expect("failed to get current dir")
        .to_str()
        .expect("failed to convert cwd to str")
        .to_string()
}

impl Config {
    pub fn load() -> color_eyre::eyre::Result<Arc<Self>> {
        let config_path = std::env::var("DAWDLE_HOME_CONFIG").unwrap_or_else(|_| {
            std::env::current_dir()
                .expect("failed to get current dir")
                .join("./dawdle.config.json")
                .to_str()
                .expect("failed to convert cwd to str")
                .to_string()
        });

        let config = std::fs::read_to_string(config_path.clone())?;
        let config: Config = serde_json::from_str(&config)?;

        log::info!("loaded config from {}", config_path);
        Ok(Arc::new(config))
    }
}
