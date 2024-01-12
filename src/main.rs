mod chat;
mod config;
mod containers;
mod ssh;
mod state;
mod utils;
mod web;

use color_eyre::eyre;
use containers::Containers;
use log::{info, LevelFilter};
use ssh::SshServer;
use std::net::SocketAddr;
use tokio::select;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let env = state::create_env()?;
    let state = state::AppState::new(env)?;

    let containers = Containers::new()?;
    containers.init().await?;

    if cfg!(debug_assertions) {
        log::warn!("running in debug mode! this is not secure!");

        let _ = state.user.create(
            "henry",
            crate::state::User {
                role: Some("admin".to_string()),
                public_keys: vec![("main".to_string(), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDfx0dXF4OM2HiE550bb7VnN/aKVTK+bZud1EVB4WYRX henry@tempora".to_string())],
                ssh_allow_password: true,
                password_hash: crate::utils::hash_pw("password")?,
            },
        );
    }

    let api_addr: SocketAddr = "127.0.0.1:8008".parse()?;
    let api_server = web::run(state.clone(), api_addr);

    let ssh_addr: SocketAddr = "0.0.0.0:2222".parse()?;
    let ssh_server = SshServer::new(containers, state);
    let ssh_server = ssh_server.run(ssh_addr);

    info!("api server listening on {}", api_addr);
    info!("ssh server listening on {}", ssh_addr);

    select! {
        r = ssh_server => r,
        r = api_server => r
    }
}
