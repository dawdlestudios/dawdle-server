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

    let ssh_server = SshServer::new(Containers::new()?);

    let ssh_addr: SocketAddr = "0.0.0.0:2222".parse()?;
    info!("ssh server listening on {}", ssh_addr);
    let ssh_server = ssh_server.run(ssh_addr);

    select! {
        _ = ssh_server => {}
    }

    Ok(())
}
