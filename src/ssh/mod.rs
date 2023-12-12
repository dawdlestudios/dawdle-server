mod session;
mod sftp;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::containers::Containers;
use color_eyre::eyre::{Ok, Result};
use russh_keys::key::KeyPair;
use session::SshSession;

#[derive(Clone)]
pub struct SshServer {
    state: crate::state::State,
    containers: Containers,
}

impl SshServer {
    pub async fn run(self, addr: SocketAddr) -> Result<()> {
        let key = self.get_key()?;
        let config = russh::server::Config {
            auth_rejection_time: Duration::from_secs(1),
            auth_rejection_time_initial: Some(Duration::from_secs(0)),
            keys: vec![key],
            ..Default::default()
        };

        russh::server::run(Arc::new(config), addr, self).await?;

        Ok(())
    }

    pub fn get_key(&self) -> Result<KeyPair> {
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
        Ok(key)
    }

    pub fn new(containers: Containers, state: crate::state::State) -> Self {
        Self { state, containers }
    }
}

impl russh::server::Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        SshSession::new(self.containers.clone())
    }
}
