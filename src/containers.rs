use bollard::{
    container::{CreateContainerOptions, LogOutput, StartContainerOptions},
    exec::StartExecResults,
    Docker,
};
use color_eyre::eyre::{eyre, Result};
use futures::Stream;
use log::info;
use std::{collections::HashMap, pin::Pin};
use tokio::io::AsyncWrite;

#[derive(Clone)]
pub struct Containers {
    docker: Docker,
}

pub struct Attach {
    pub id: String,
    pub output: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
    pub input: Pin<Box<dyn AsyncWrite + Send>>,
}

impl Containers {
    pub fn new() -> Result<Self> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
        })
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
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: format!("dawdle-{}", user),
                    ..Default::default()
                }),
                bollard::container::Config {
                    image: Some("debian:bookworm"),
                    cmd: Some(vec!["/bin/bash"]),
                    attach_stdin: Some(true),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    tty: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        Ok(container.id)
    }

    // get a container id for a user
    pub async fn get_container(&self, user: &str) -> Result<Option<String>> {
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
                c.names
                    .as_ref()
                    .map(|n| n.contains(&format!("/dawdle-{}", user)))
                    == Some(true)
            })
            .map(|c| c.id.clone())
            .flatten();

        info!("container: {:?}", container);
        Ok(container)
    }

    // attach a new exec process to the user's container
    // create a container if one doesn't exist
    pub async fn attach(&self, user: &str) -> Result<Attach> {
        // get or create the container
        let container_id = match self.get_container(user).await? {
            Some(container) => container,
            None => self.create_container(user).await?,
        };

        // ensure the container is running
        self.docker
            .start_container::<String>(&container_id, Some(StartContainerOptions::default()))
            .await?;

        let exec = self
            .docker
            .create_exec(
                &container_id,
                bollard::exec::CreateExecOptions {
                    cmd: Some(vec!["/bin/bash"]),
                    attach_stderr: Some(true),
                    attach_stdout: Some(true),
                    attach_stdin: Some(true),
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
                    output_capacity: Some(8 * 1024),
                }),
            )
            .await?
        else {
            panic!("expected Attached");
        };

        Ok(Attach {
            id: exec.id,
            output,
            input,
        })
    }
}
