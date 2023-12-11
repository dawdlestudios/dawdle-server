use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

mod sftp;
mod ssh;

mod containers;
use color_eyre::eyre;
use containers::Containers;
use log::{info, LevelFilter};
use russh_keys::key::KeyPair;
use ssh::SshSession;

#[derive(Clone)]
struct Server {
    containers: Containers,
}

impl russh::server::Server for Server {
    type Handler = SshSession;

    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        SshSession::default()
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let docker = bollard::Docker::connect_with_local_defaults()?;

    // generate key in ./keys directory if it doesn't exist
    let key = if !std::path::Path::new("./.keys/id_ed25519").exists() {
        let key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
        std::fs::create_dir_all("./.keys")?;
        std::fs::write("./.keys/id_ed25519", key.to_bytes())?;
        key
    } else {
        let key = std::fs::read("./.keys/id_ed25519")?;
        ed25519_dalek::SigningKey::from_bytes(&key.try_into().expect("key is 32 bytes"))
    };

    let key = KeyPair::Ed25519(key);

    let config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(1),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: vec![key],
        ..Default::default()
    };

    let server = Server {
        containers: Containers::new()?,
    };

    let x = server.containers.attach("test").await?;
    server.containers.detatch(&x.id).await?;

    let port = "2222";
    let host = "0.0.0.0";
    let addr = SocketAddrV4::new(host.parse()?, port.parse()?);
    info!("listening on {}", addr);
    russh::server::run(Arc::new(config), addr, server).await?;

    Ok(())
}
