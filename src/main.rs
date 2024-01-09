mod config;
mod containers;
mod ssh;
mod state;
mod utils;
mod web;
mod chat;

use argon2::PasswordHasher;
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

    let containers = Containers::new()?;
    containers.init().await?;

    if cfg!(debug_assertions) {
        log::warn!("running in debug mode! this is not secure!");

        let _ = state.users.set(
            "henry",
            &crate::state::User {
                role: Some("admin".to_string()),
                public_keys: vec![("main".to_string(), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHKpHLbfvXYR+OUXeh4GSpX26FJUUbT4UV2lOunYNH3a henry@macaroni".to_string())],
                ssh_allow_password: true,
                password_hash: argon2::Argon2::default()
                    .hash_password(
                        "password".as_bytes(),
                        &argon2::password_hash::SaltString::generate(&mut rand::rngs::OsRng),
                    )? 
                    .to_string(),
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
        _ = ssh_server => {}
        _ = api_server => {}
    }

    Ok(())
}
