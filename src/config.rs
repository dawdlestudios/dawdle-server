use crate::utils::{is_valid_project_path, is_valid_username};
use serde::{Deserialize, Serialize};

pub const DOCKER_IMAGE: &str = "ghcr.io/dawdlestudios/container";
pub const DOCKER_TAG: &str = "latest";
pub const DOCKER_CONTAINER_PREFIX: &str = "dawdle-home-";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub fs: FileSystemConfig,
    pub ssh: SSHConfig,
    pub web: WebConfig,
    pub minecraft: MinecraftConfig,
    pub create_admin_user: Option<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileSystemConfig {
    /// The directory for all database files, ssh keys, etc
    pub data_dir: String,

    /// The directory for all user files
    pub user_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SSHConfig {
    pub port: u16,
    pub interface: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebConfig {
    pub port: u16,
    pub interface: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinecraftConfig {
    pub restadmin_url: String,
    pub restadmin_token: String,
}

impl Config {
    pub fn load() -> eyre::Result<Self> {
        let config_path = std::env::var("DAWDLE_CONFIG").unwrap_or_else(|_| {
            // check if the config file exists in the current directory
            // otherwise, check if ~/.config/dawdle.config.toml exists
            let cwd = std::env::current_dir().expect("failed to get current dir");
            let cwd_config = cwd.join("dawdle.config.toml");
            if cwd_config.exists() {
                return cwd_config.to_str().unwrap().to_string();
            }

            let home = std::env::var("HOME").expect("HOME env var not set");
            let home = std::path::Path::new(&home);
            let home_config = home.join(".config").join("dawdle.config.toml");
            if home_config.exists() {
                return home_config.to_str().unwrap().to_string();
            }

            panic!("config file not found");
        });

        let config = std::fs::read_to_string(config_path.clone())?;
        let config: Config = toml::from_str(&config)?;

        log::info!("loaded config from {}", config_path);
        Ok(config)
    }

    pub fn user_bin_dir(&self) -> std::path::PathBuf {
        resolve_path(&self.fs.user_dir).join("bin")
    }

    pub fn default_user_home(&self) -> std::path::PathBuf {
        resolve_path(&self.fs.user_dir).join("default-home")
    }

    pub fn user_home(&self, username: &str) -> Option<std::path::PathBuf> {
        if !is_valid_username(username) {
            return None;
        }
        let path = resolve_path(&self.fs.user_dir)
            .join("home")
            .join(username.to_ascii_lowercase());
        Some(path)
    }

    pub fn user_public_path(&self, username: &str) -> Option<std::path::PathBuf> {
        self.user_home(username).map(|path| path.join("public"))
    }

    pub fn db_path(&self) -> std::path::PathBuf {
        resolve_path(&self.fs.data_dir)
            .join("database")
            .join("db.sqlite")
    }

    pub fn ssh_key_path(&self) -> std::path::PathBuf {
        resolve_path(&self.fs.data_dir)
            .join("ssh")
            .join("id_ed25519")
    }

    pub fn project_path(&self, username: &str, project_path: &str) -> Option<std::path::PathBuf> {
        if !is_valid_username(username) || !is_valid_project_path(project_path) {
            return None;
        }
        let path = resolve_path(&self.fs.user_dir)
            .join("home")
            .join(username.to_ascii_lowercase())
            .join(project_path);
        Some(path)
    }
}

// takes either an absolute path or a path relative to the current directory
// returns an absolute path (PathBuf)
fn resolve_path(path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .expect("failed to get current dir")
        .join(path)
}
