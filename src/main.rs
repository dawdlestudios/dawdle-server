mod api;
mod containers;
mod ssh;
mod state;

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

    let _ = state.users.set(
        "henry",
        &crate::state::User {
            public_keys: vec!["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDfx0dXF4OM2HiE550bb7VnN/aKVTK+bZud1EVB4WYRX henry@tempora".to_string()],
            ssh_allow_password: true,
            password_hash: argon2::Argon2::default()
                .hash_password(
                    "password".as_bytes(),
                    &argon2::password_hash::SaltString::generate(&mut rand::rngs::OsRng),
                )?
                .to_string(),
        },
    );

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
