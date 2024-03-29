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
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use tokio::select;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let config = config::Config::load()?;

    let env = state::create_env()?;
    let state = state::AppState::new(env, config)?;

    let containers = Containers::new()?;
    containers.init().await?;

    if let Some((username, password)) = &state.config.initial_user {
        let _ = state.user.create(
            username,
            crate::state::User {
                role: Some("admin".to_string()),
                public_keys: vec![],
                password_hash: crate::utils::hash_pw(password)?,
            },
        );
    }

    let api_addr = SocketAddr::new(
        IpAddr::from_str(&state.config.www_interface)?,
        state.config.www_port,
    );

    let api_server = web::run(state.clone(), api_addr);

    let ssh_addr = SocketAddr::new(
        IpAddr::from_str(&state.config.ssh_interface)?,
        state.config.ssh_port,
    );

    let ssh_server = SshServer::new(containers, state);
    let ssh_server = ssh_server.run(ssh_addr);

    info!("api server listening on {}", api_addr);
    info!("ssh server listening on {}", ssh_addr);

    select! {
        r = ssh_server => r,
        r = api_server => r
    }
}
