mod app;
mod chat;
mod config;
mod containers;
mod minecraft;
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
    let app = app::App::new(config.clone()).await?;

    if let Some((username, password)) = config.clone().create_admin_user {
        let _ = app.users.create(&username, &password, Some("admin")).await;
    }

    let containers = Containers::new(config)?;
    containers.init().await?;

    let api_addr = SocketAddr::new(
        IpAddr::from_str(&app.config.web.interface)?,
        app.config.web.port,
    );

    let api_server = web::run(app.clone(), api_addr);

    let ssh_addr = SocketAddr::new(
        IpAddr::from_str(&app.config.ssh.interface)?,
        app.config.ssh.port,
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
