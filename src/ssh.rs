use color_eyre::eyre::{self, bail, Result};
use dashmap::DashMap;
use log::info;

use crate::sftp::SftpSession;
use async_trait::async_trait;
use pty_process::OwnedWritePty;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId};

pub struct SshChannel {
    handle: Channel<Msg>,
    pty: Option<OwnedWritePty>,
    pty_term: Option<String>,
    pty_modes: Option<Vec<(russh::Pty, u32)>>,
    pty_size: Option<(u16, u16)>,
}

pub struct SshSession {
    channels: DashMap<ChannelId, SshChannel>,
}

impl Default for SshSession {
    fn default() -> Self {
        Self {
            channels: DashMap::new(),
        }
    }
}

impl SshSession {
    pub async fn remove_channel(&mut self, channel_id: ChannelId) -> Option<SshChannel> {
        Some(self.channels.remove(&channel_id)?.1)
    }
}

#[async_trait]
impl russh::server::Handler for SshSession {
    type Error = eyre::Error;

    async fn auth_password(self, user: &str, password: &str) -> Result<(Self, Auth), Self::Error> {
        info!("credentials: {}, {}", user, password);
        Ok((self, Auth::Accept))
    }

    async fn auth_publickey(
        self,
        user: &str,
        public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, Auth), Self::Error> {
        info!("credentials: {}, {:?}", user, public_key);
        Ok((self, Auth::Accept))
    }

    async fn channel_open_session(
        mut self,
        channel: Channel<Msg>,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        {
            let id = channel.id();
            let client = SshChannel {
                handle: channel,
                pty: None,
                pty_size: None,
                pty_modes: None,
                pty_term: None,
            };

            self.channels.insert(id, client);
        }

        Ok((self, true, session))
    }

    #[allow(unused_variables, clippy::too_many_arguments)]
    async fn pty_request(
        self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        modes: &[(russh::Pty, u32)],
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        info!(
            "pty_request: {}, {}, {}, {}, {}, {:?}",
            term, col_width, row_height, pix_width, pix_height, modes
        );

        self.channels.alter(&channel, |k, mut v| {
            v.pty_size = Some((col_width as u16, row_height as u16));
            v.pty_modes = Some(modes.to_vec());
            v.pty_term = Some(term.to_string());
            v
        });

        Ok((self, session))
    }

    async fn subsystem_request(
        mut self,
        channel_id: ChannelId,
        name: &str,
        mut session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        info!("subsystem: {}", name);

        if name == "sftp" {
            let Some(channel) = self.remove_channel(channel_id).await else {
                bail!("channel not found");
            };

            let sftp = SftpSession::default();
            session.channel_success(channel_id);
            russh_sftp::server::run(channel.handle.into_stream(), sftp).await;
        } else {
            session.channel_failure(channel_id);
        }

        Ok((self, session))
    }
}
