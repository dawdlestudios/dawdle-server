mod app;
mod chat;
mod config;
mod containers;
mod ssg;
mod ssh;
mod utils;
mod web;

use containers::Containers;
use log::{info, LevelFilter};
use ssh::SshServer;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use tokio::select;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let config = config::Config::load()?;
    let app = app::App::new(config).await?;

    let containers = Containers::new()?;
    containers.init().await?;

    #[cfg(debug_assertions)]
    let _ = app.users.create("admin", "admin", Some("admin")).await;

    let api_addr = SocketAddr::new(
        IpAddr::from_str(&app.config.www_interface)?,
        app.config.www_port,
    );

    let api_server = web::run(app.clone(), api_addr);

    let ssh_addr = SocketAddr::new(
        IpAddr::from_str(&app.config.ssh_interface)?,
        app.config.ssh_port,
    );

    let ssh_server = SshServer::new(containers, app);
    let ssh_server = ssh_server.run(ssh_addr);

    info!("api server listening on {}", api_addr);
    info!("ssh server listening on {}", ssh_addr);

    select! {
        r = ssh_server => r,
        r = api_server => r
    }
}
