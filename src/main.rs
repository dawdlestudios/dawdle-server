mod api;
mod containers;
mod ssh;
mod state;

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
    let state = state::State::new(env)?;

    let api_addr: SocketAddr = "127.0.0.1:8008".parse()?;
    let api_server = api::run(state.clone(), api_addr);

    let ssh_addr: SocketAddr = "0.0.0.0:2222".parse()?;
    let ssh_server = SshServer::new(Containers::new()?, state);
    let ssh_server = ssh_server.run(ssh_addr);

    info!("api server listening on {}", api_addr);
    info!("ssh server listening on {}", ssh_addr);
    select! {
        _ = ssh_server => {}
        _ = api_server => {}
    }

    Ok(())
}
