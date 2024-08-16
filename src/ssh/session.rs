use dashmap::mapref::one::RefMut;
use dashmap::DashMap;
use eyre::{bail, eyre, Result};
use futures::TryStreamExt;
use log::{debug, info};
use russh_keys::key::parse_public_key;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::app::App;
use crate::containers::{AttachInput, Containers, Pty};
use async_trait::async_trait;
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};

#[derive(Default)]
struct UserContainer {
    _container_id: Option<String>,
    exec_id: Option<String>,
    stream: Option<Mutex<AttachInput>>,
}

impl UserContainer {
    async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let Some(stream) = &self.stream else {
            bail!("stream not found")
        };

        let mut stream = stream.lock().await;
        stream.0.write_all(data).await?;
        Ok(())
    }

    fn exec_id(&self) -> Result<&str> {
        let Some(id) = &self.exec_id else {
            bail!("exec_id not found")
        };

        Ok(id)
    }
}

#[derive(Default)]
pub struct SshChannel {
    pty: Option<Pty>,
    env: Option<Vec<(String, String)>>,
    shell: UserContainer,
}

#[derive(Debug)]
struct SshUser {
    username: String,
    keys: Vec<russh_keys::key::PublicKey>,
}

pub struct SshSession {
    state: App,
    containers: Containers,
    user: Option<SshUser>,
    channels: DashMap<ChannelId, SshChannel>,
}

impl SshSession {
    pub fn new(containers: Containers, state: App) -> Self {
        Self {
            state,
            containers,
            user: None,
            channels: DashMap::new(),
        }
    }

    fn user(&mut self) -> Result<&SshUser> {
        match self.user {
            Some(ref user) => Ok(user),
            None => bail!("user not found"),
        }
    }

    fn channel(&self, channel_id: ChannelId) -> Result<RefMut<'_, ChannelId, SshChannel>> {
        self.channels
            .get_mut(&channel_id)
            .ok_or_else(|| eyre::eyre!("channel not found"))
    }

    async fn get_user(&mut self, username: &str) -> Result<&SshUser> {
        let public_keys = match self.user {
            Some(ref user) => {
                if user.username == username {
                    return Ok(user);
                }
                return Err(eyre!("user mismatch"));
            }
            None => {
                let Ok(public_keys) = self.state.users.get_public_keys(username).await else {
                    bail!("user not found")
                };
                public_keys
            }
        };

        let keys = public_keys
            .iter()
            .map(|(_name, key)| {
                // kinda wastefull to parse it twice
                // hopefully solved someday: https://github.com/warp-tech/russh/issues/140
                let key = ssh_key::PublicKey::from_openssh(&key)
                    .map_err(|e| eyre::eyre!("failed to parse public key: {}", e))?;
                if !key.algorithm().is_ed25519() {
                    eyre::bail!("only ed25519 keys are supported")
                }
                let x = parse_public_key(&key.to_bytes()?, None)?;
                Ok(x)
            })
            .collect::<Result<Vec<_>>>()?;

        let user = SshUser {
            username: username.to_string(),
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

    /// just check if the user has the offered public key
    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        public_key: &russh_keys::key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        debug!("offered credentials: {}, {:?}", user, public_key);
        let user = self.get_user(user).await?;

        let res = match user.keys.iter().any(|k| k == public_key) {
            true => Auth::Accept,
            false => Auth::Reject {
                proceed_with_methods: None,
            },
        };

        Ok(res)
    }

    /// actually authenticate the user
    /// Signature has been verified, now we need to check if the user is allowed to login
    async fn auth_publickey(
        &mut self,
        user: &str,
        _public_key: &russh_keys::key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        let _ = self.get_user(user).await?;
        Ok(Auth::Accept)
    }

    /// A new channel has been opened by the client.
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        info!("channel_open_session");
        self.channels.insert(channel.id(), SshChannel::default());
        Ok(true)
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.alter(&channel, |_, mut v| {
            v.env
                .get_or_insert_with(Vec::new)
                .push((variable_name.to_string(), variable_value.to_string()));
            v
        });
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        modes: &[(russh::Pty, u32)],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        log::debug!("pty_request: {:?}", modes);
        self.channels.alter(&channel, |_k, mut v| {
            v.pty = Some(Pty {
                pty_term: Some(term.to_string()),
                pty_modes: Some(modes.to_vec()),
                pty_size: Some((col_width as u16, row_height as u16)),
            });
            v
        });
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        log::debug!("exec_request");
        let command = String::from_utf8(data.to_vec())?;

        let username = self.user()?.username.clone();
        let attach = self
            .containers
            .attach(
                &username,
                Some(command),
                self.channel(channel_id)?.pty.clone(),
            )
            .await?;

        self.channels.alter(&channel_id, |_, mut v| {
            v.shell = UserContainer {
                _container_id: Some(attach.container_id.clone()),
                exec_id: Some(attach.id.clone()),
                stream: Some(Mutex::new(attach.input)),
            };
            v
        });

        let attach_output = attach.output;
        let session_handle = session.handle();

        tokio::spawn(async move {
            info!("attach_output reader spawned");

            let res = attach_output
                .0
                .into_stream()
                .try_for_each(|output| async {
                    session_handle
                        .data(channel_id, CryptoVec::from_slice(&output.into_bytes()))
                        .await
                        .map_err(|e| {
                            println!("data failed: {:?}", String::from_utf8_lossy(e.as_ref()))
                        })
                        .unwrap();
                    Ok(())
                })
                .await;

            info!("attach_output reader done: {:?}", res);
            if let Err(e) = res {
                log::error!("attach_output reader failed: {}", e);
            } else {
                session_handle.channel_success(channel_id).await.unwrap();
            }

            let _ = session_handle.exit_status_request(channel_id, 0).await;
            let _ = session_handle.channel_success(channel_id).await;
            let _ = session_handle.close(channel_id).await;
        });

        log::debug!("exec_request done");
        session.request_success();
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        log::debug!("shell_request");
        let username = self.user()?.username.clone();
        let attach = self
            .containers
            .attach(&username, None, self.channel(channel_id)?.pty.clone())
            .await?;

        self.channels.alter(&channel_id, |_, mut v| {
            v.shell = UserContainer {
                _container_id: Some(attach.container_id.clone()),
                exec_id: Some(attach.id.clone()),
                stream: Some(Mutex::new(attach.input)),
            };
            v
        });

        let attach_output = attach.output;
        // Read bytes from the PTY and send them to the SSH client
        let session_handle = session.handle();

        tokio::spawn(async move {
            info!("attach_output reader spawned");

            let res = attach_output
                .0
                .into_stream()
                .try_for_each(|output| async {
                    let out = output.into_bytes();
                    if !out.is_empty() {
                        session_handle
                            .data(channel_id, CryptoVec::from_slice(&out))
                            .await
                            .map_err(|e| {
                                println!("data failed: {:?}", String::from_utf8_lossy(e.as_ref()))
                            })
                            .unwrap();
                    }

                    Ok(())
                })
                .await;

            info!("attach_output reader done: {:?}", res);
            if let Err(e) = res {
                log::error!("attach_output reader failed: {}", e);
            }

            let _ = session_handle.exit_status_request(channel_id, 0).await;
            let _ = session_handle.channel_success(channel_id).await;
            let _ = session_handle.close(channel_id).await;
        });

        Ok(())
    }

    async fn data(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // SSH client sends data, pipe it to the corresponding PTY
        // info!("data packet: {:?}", String::from_utf8_lossy(data));
        {
            match self.channel(channel_id)?.shell.write_all(data).await {
                Ok(_) => {}
                Err(e) => log::error!("failed to write to pty: {}", e),
            }
        }

        Ok(())
    }

    /// The client's pseudo-terminal window size has changed.
    async fn window_change_request(
        &mut self,
        channel_id: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        {
            let Some(mut channel) = self.channels.get_mut(&channel_id) else {
                bail!("channel not found")
            };

            if let Some(pty) = channel.pty.as_mut() {
                pty.pty_size = Some((col_width as u16, row_height as u16));
                self.containers
                    .resize(
                        channel.shell.exec_id()?,
                        col_width as u16,
                        row_height as u16,
                    )
                    .await?;
            };
        }

        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel_id: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        log::debug!("channel_close");
        // Clean up
        if let Some((_, channel)) = self.channels.remove(&channel_id) {
            let _ = self.containers.detatch(channel.shell.exec_id()?).await;
        }

        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel_id: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Clean up
        if let Some((_, channel)) = self.channels.remove(&channel_id) {
            let _ = self.containers.detatch(channel.shell.exec_id()?).await;
        }

        Ok(())
    }
}
