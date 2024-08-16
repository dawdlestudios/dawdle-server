use bollard::{
    container::{CreateContainerOptions, LogOutput, StartContainerOptions},
    exec::StartExecResults,
    Docker,
};
use eyre::{eyre, Result};
use futures::Stream;
use std::pin::Pin;
use tokio::io::AsyncWrite;

use crate::utils::is_valid_username;

#[derive(Clone)]
pub struct Containers {
    docker: Docker,
}

pub struct Attach {
    pub container_id: String,
    pub id: String,
    pub output: AttachOutput,
    pub input: AttachInput,
}

#[derive(Clone)]
pub struct Pty {
    pub pty_term: Option<String>,
    pub pty_modes: Option<Vec<(russh::Pty, u32)>>,
    pub pty_size: Option<(u16, u16)>,
}

pub struct AttachInput(pub Pin<Box<dyn AsyncWrite + Send>>);

pub struct AttachOutput(
    pub Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
);

impl Containers {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        let docker = Docker::connect_with_socket(
            crate::config::DOCKER_SOCKET_MACOS,
            30,
            bollard::API_DEFAULT_VERSION,
        )?;

        #[cfg(not(target_os = "macos"))]
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self { docker })
    }

    pub async fn init(&self) -> Result<()> {
        let _ = self.docker.info().await?;
        std::fs::create_dir_all("./.files/home")?;
        std::fs::create_dir_all("./.files/bin")?;
        Ok(())
    }

    pub async fn resize(&self, id: &str, width: u16, height: u16) -> Result<()> {
        self.docker
            .resize_exec(id, bollard::exec::ResizeExecOptions { height, width })
            .await?;

        Ok(())
    }

    // ensure the exec process is killed
    pub async fn detatch(&self, id: &str) -> Result<()> {
        let exec = self.docker.inspect_exec(id).await?;
        if exec.running.is_some() {
            // kill the process
            let pid = exec.pid.ok_or_else(|| eyre!("no pid"))?;
            let kill = self
                .docker
                .create_exec(
                    exec.container_id
                        .as_ref()
                        .ok_or_else(|| eyre!("no container id"))?,
                    bollard::exec::CreateExecOptions {
                        cmd: Some(vec!["/bin/kill", "-9", &pid.to_string()]),
                        ..Default::default()
                    },
                )
                .await?;

            self.docker.start_exec(&kill.id, None).await?;
        }

        Ok(())
    }

    pub async fn create_container(&self, user: &str) -> Result<String> {
        assert!(is_valid_username(user));

        println!("creating container for {}", user);
        println!("name: {}{}", crate::config::DOCKER_CONTAINER_PREFIX, user);

        let binds = vec![
            format!(
                "{}/.files/home/{}:/home/{}:rw",
                std::env::current_dir()?.display(),
                user,
                user
            ),
            format!(
                "{}/.files/bin:/usr/local/dawdle/bin:ro",
                std::env::current_dir()?.display()
            ),
        ];

        std::fs::create_dir_all(format!(
            "{}/.files/home/{}",
            std::env::current_dir()?.display(),
            user
        ))?;

        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: format!("{}{}", crate::config::DOCKER_CONTAINER_PREFIX, user),
                    ..Default::default()
                }),
                bollard::container::Config {
                    host_config: Some(bollard::models::HostConfig {
                        binds: Some(binds),
                        ..Default::default()
                    }),
                    hostname: Some("dawdle.space"),
                    image: Some(
                        format!(
                            "{}:{}",
                            crate::config::DOCKER_IMAGE,
                            crate::config::DOCKER_TAG
                        )
                        .as_str(),
                    ),
                    env: Some(vec![&format!("DAWDLE_USER={}", user)]),
                    ..Default::default()
                },
            )
            .await?;

        println!("created container: {:?}", container.id);

        Ok(container.id)
    }

    // get a container id for a user
    pub async fn get_container(&self, user: &str) -> Result<Option<String>> {
        assert!(is_valid_username(user));

        let containers = self
            .docker
            .list_containers::<String>(Some(bollard::container::ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await?;

        let container = containers
            .iter()
            .find(|c| {
                c.names.as_ref().map(|n| {
                    n.contains(&format!(
                        "/{}{}",
                        crate::config::DOCKER_CONTAINER_PREFIX,
                        user
                    ))
                }) == Some(true)
            })
            .and_then(|c| c.id.clone());
        Ok(container)
    }

    // attach a new exec process to the user's container
    // create a container if one doesn't exist
    pub async fn attach(
        &self,
        user: &str,
        command: Option<String>,
        tty: Option<Pty>,
    ) -> Result<Attach> {
        assert!(is_valid_username(user));

        // get or create the container
        let container_id = match self.get_container(user).await? {
            Some(container) => container,
            None => self.create_container(user).await?,
        };

        // ensure the container is running
        self.docker
            .start_container::<String>(&container_id, Some(StartContainerOptions::default()))
            .await?;

        let command = command.unwrap_or("".to_string());
        let exec_command = format!("set -e; {}", command);

        let cmd = match command.is_empty() {
            true => vec!["/bin/zsh", "-l", "-i"],
            false => vec!["/bin/bash", "-c", &exec_command],
        };

        let exec = self
            .docker
            .create_exec(
                &container_id,
                bollard::exec::CreateExecOptions {
                    cmd: Some(cmd),
                    attach_stderr: Some(true),
                    attach_stdout: Some(true),
                    attach_stdin: Some(true),
                    working_dir: Some(&format!("/home/{}", user)),
                    tty: Some(tty.is_some()),
                    env: Some(vec!["TERM=xterm-256color"]),
                    ..Default::default()
                },
            )
            .await?;

        let StartExecResults::Attached { input, output } = self
            .docker
            .start_exec(
                &exec.id,
                Some(bollard::exec::StartExecOptions {
                    detach: false,
                    tty: tty.is_some(),
                    output_capacity: Some(8 * 1024),
                }),
            )
            .await?
        else {
            panic!("expected Attached");
        };

        if let Some(tty) = tty {
            if let Some((width, height)) = tty.pty_size {
                self.resize(&exec.id, width, height).await?;
            }
        }

        Ok(Attach {
            id: exec.id,
            container_id,
            output: AttachOutput(output),
            input: AttachInput(input),
        })
    }
}
