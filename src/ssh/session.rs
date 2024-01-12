use std::sync::Arc;

use argon2::PasswordVerifier;
use bollard::container::LogOutput;
use color_eyre::eyre::{self, bail, Result};
use dashmap::DashMap;
use futures::TryStreamExt;
use log::{debug, error, info};
use russh_keys::key::parse_public_key;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::sftp::SftpSession;
use crate::containers::{AttachInput, Containers};
use crate::state::{AppState, User};
use async_trait::async_trait;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};

struct Pty {
    id: String,
    input: Mutex<AttachInput>,
}

pub struct SshChannel {
    handle: Channel<Msg>,

    pty: Option<Pty>,
    pty_term: Option<String>,
    pty_modes: Option<Vec<(russh::Pty, u32)>>,
    pty_size: Option<(u16, u16)>,
}

#[derive(Debug)]
struct SshUser {
    username: String,
    user: User,
    keys: Vec<russh_keys::key::PublicKey>,
}

pub struct SshSession {
    /// The command to run, if any.
    state: AppState,
    user: Option<SshUser>,

    command: Option<String>,
    term: String,
    containers: Containers,
    channels: Arc<DashMap<ChannelId, SshChannel>>,
}

impl SshSession {
    pub fn new(containers: Containers, state: AppState) -> Self {
        Self {
            state,
            user: None,
            command: None,
            term: "xterm".to_string(),
            containers,
            channels: Arc::new(DashMap::new()),
        }
    }

    pub async fn remove_channel(&mut self, channel_id: ChannelId) -> Option<SshChannel> {
        Some(self.channels.remove(&channel_id)?.1)
    }

    async fn get_user(&mut self, username: &str) -> Result<&SshUser> {
        let user = match self.user {
            Some(ref user) => {
                if user.username == username {
                    return Ok(user);
                } else {
                    bail!("user mismatch");
                }
            }
            None => {
                let Some(user) = self.state.user.get(username)? else {
                    bail!("user not found");
                };

                user
            }
        };

        let keys = user
            .public_keys
            .iter()
            .map(|k| {
                // kinda wastefull to parse it twice
                // hopefully solved someday: https://github.com/warp-tech/russh/issues/140
                let key = ssh_key::PublicKey::from_openssh(&k.1)
                    .map_err(|e| eyre::eyre!("failed to parse public key: {}", e))?;
                if !key.algorithm().is_ed25519() {
                    eyre::bail!("only ed25519 keys are supported")
                }
                let x = parse_public_key(&key.to_bytes()?)?;
                Ok(x)
            })
            .collect::<Result<Vec<_>>>()?;

        let user = SshUser {
            username: username.to_string(),
            user: user.clone(),
            keys,
        };

        self.user = Some(user);

        match self.user {
            #[allow(clippy::needless_borrow)] // false positive
            Some(ref user) => Ok(&user),
            None => unreachable!(),
        }
    }
}

#[async_trait]
impl russh::server::Handler for SshSession {
    type Error = eyre::Error;

    async fn auth_password(
        mut self,
        user: &str,
        password: &str,
    ) -> Result<(Self, Auth), Self::Error> {
        let user = self.get_user(user).await?;
        if !user.user.ssh_allow_password {
            return Ok((
                self,
                Auth::Reject {
                    proceed_with_methods: None,
                },
            ));
        }

        let hash = argon2::PasswordHash::new(&user.user.password_hash)?;
        argon2::Argon2::default().verify_password(password.as_bytes(), &hash)?;
        Ok((self, Auth::Accept))
    }

    /// just check if the user has the offered public key
    async fn auth_publickey_offered(
        mut self,
        user: &str,
        public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, Auth), Self::Error> {
        debug!("offered credentials: {}, {:?}", user, public_key);
        let user = self.get_user(user).await?;

        for key in user.keys.iter() {
            if key == public_key {
                info!("key accepted");
                return Ok((self, Auth::Accept));
            }
        }

        Ok((
            self,
            Auth::Reject {
                proceed_with_methods: None,
            },
        ))
    }

    /// actually authenticate the user
    /// Signature has been verified, now we need to check if the user is allowed to login
    async fn auth_publickey(
        mut self,
        _user: &str,
        _public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, Auth), Self::Error> {
        // let user = self.get_user(user).await?;
        Ok((self, Auth::Accept))
    }

    /// A new channel has been opened by the client.
    async fn channel_open_session(
        mut self,
        channel: Channel<Msg>,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        info!("channel_open_session");

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

    #[allow(unused_variables)]
    async fn env_request(
        self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        Ok((self, session))
    }

    #[allow(unused_variables, clippy::too_many_arguments)]
    async fn pty_request(
        mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        modes: &[(russh::Pty, u32)],
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        // TODO: handle different pty types
        self.term = term.to_string();
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

    async fn exec_request(
        mut self,
        channel: ChannelId,
        data: &[u8],
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        let Ok(command) = String::from_utf8(data.to_vec()) else {
            bail!("command is not valid utf8");
        };

        // we disable echo here to prevent ssh from echoing bootstrap commands
        // TODO: this is a hack, we should probably do something else
        self.command = Some("stty -echo\n".to_string() + &command);
        // self.command = Some("".to_string() + &command);

        let (self, session) = self.shell_request(channel, session).await?;
        Ok((self, session))
    }

    async fn shell_request(
        mut self,
        channel_id: ChannelId,
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        let Some(user) = self.user.as_ref() else {
            bail!("user not found");
        };

        let (_attach_id, attach_output) = {
            let Some(ref mut channel) = self.channels.get_mut(&channel_id) else {
                log::error!("channel not found");
                bail!("channel not found");
            };

            let attach = self
                .containers
                .attach(&user.username, self.command.clone())
                .await
                .map_err(|e| {
                    log::error!("attach failed: {}", e);
                    e
                })?;

            channel.pty = Some(Pty {
                id: attach.id.clone(),
                input: Mutex::new(attach.input),
            });

            (attach.id, attach.output)
        };

        // Read bytes from the PTY and send them to the SSH client
        let session_handle = session.handle();

        tokio::spawn(async move {
            let reader = attach_output.0.into_stream();
            info!("attach_output reader spawned");

            let res = reader
                .try_for_each(|output| async {
                    match output {
                        LogOutput::StdErr { message } | LogOutput::StdOut { message } => {
                            session_handle
                                .data(channel_id, CryptoVec::from_slice(&message))
                                .await
                                .map_err(|e| {
                                    println!(
                                        "data failed: {:?}",
                                        String::from_utf8_lossy(e.as_ref())
                                    )
                                })
                                .unwrap();
                        }
                        _ => {}
                    };

                    Ok(())
                })
                .await;

            info!("attach_output reader done: {:?}", res);
            if let Err(e) = res {
                log::error!("attach_output reader failed: {}", e);
            }

            session_handle.eof(channel_id).await.unwrap();
            session_handle.close(channel_id).await.unwrap();

            // TODO: Clean up
        });

        // todo: initial shell size
        {
            let Some(mut channel) = self.channels.get_mut(&channel_id) else {
                bail!("channel not found");
            };
            let (col_width, row_height) = channel.pty_size.unwrap_or((80, 24));

            if let Some(pty) = channel.pty.as_mut() {
                self.containers
                    .resize(&pty.id, col_width, row_height)
                    .await?;
            };
        }

        Ok((self, session))
    }

    async fn data(
        self,
        channel_id: ChannelId,
        data: &[u8],
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        // SSH client sends data, pipe it to the corresponding PTY

        // info!("data: {:?}", String::from_utf8_lossy(data));

        {
            let Some(channel) = self.channels.get_mut(&channel_id) else {
                error!("channel not found: {}", channel_id);
                bail!("channel not found: {}", channel_id);
            };

            if let Some(pty) = &channel.pty {
                let mut input = pty.input.lock().await;

                match input.0.write_all(data).await {
                    Ok(_) => {}
                    Err(e) => log::error!("failed to write to pty: {}", e),
                }

                // TODO: maybe we don't need to block here:
                // match pty.input.try_lock() {
                //     Ok(mut input) => {
                //         input.0.write_all(data).await?;
                //     }
                //     Err(e) => {
                //         log::error!("pty.input.lock() failed: {}", e);
                //     }
                // }
            } else {
                error!("no pty for channel {}", channel_id);
            }
        }

        Ok((self, session))
    }

    /// The client's pseudo-terminal window size has changed.
    async fn window_change_request(
        self,
        channel_id: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        {
            let Some(mut channel) = self.channels.get_mut(&channel_id) else {
                bail!("channel not found");
            };

            channel.pty_size = Some((col_width as u16, row_height as u16));
            if let Some(pty) = channel.pty.as_mut() {
                self.containers
                    .resize(&pty.id, col_width as u16, row_height as u16)
                    .await?;
            };
        }

        Ok((self, session))
    }

    async fn channel_close(
        self,
        channel_id: ChannelId,
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        // Clean up
        if let Some((_, channel)) = self.channels.remove(&channel_id) {
            if let Some(pty) = channel.pty {
                self.containers.detatch(&pty.id).await?;
            }
        }

        Ok((self, session))
    }
}
