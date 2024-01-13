pub const DOCKER_IMAGE: &str = "dawdle-home";
pub const DOCKER_TAG: &str = "latest";
pub const DOCKER_CONTAINER_PREFIX: &str = "dawdle-home-";

#[cfg(target_os = "darwin")]
pub const DOCKER_SOCKET_MACOS: &str = "unix:///Users/henry/.colima/default/docker.sock";

pub const HOME_SUBFOLDER: &str = "./home";

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
    pub db_path: String,
    pub files_path: String,
    pub keys_path: String,
    // "ssh_port": 2222,
    // "ssh_interface": "127.0.0.1",
    // "www_port": 8080,
    // "www_interface": "127.0.0.1",
    // "db_path": "./.db",
    // "files_path": "./.files",
    // "keys_path": "./.keys"
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
