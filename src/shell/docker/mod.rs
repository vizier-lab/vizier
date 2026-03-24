use std::sync::Arc;

use anyhow::Result;
use bollard::{
    Docker,
    exec::{StartExecOptions, StartExecResults},
    plugin::{ContainerCreateBody, ExecConfig},
    query_parameters::{CreateContainerOptions, CreateImageOptions},
};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};

use crate::{
    config::shell::{DockerShellConfig, DockerSourceConfig},
    shell::ShellProvider,
};

pub struct DockerShell {
    container_id: String,
    docker: Arc<Docker>,
}

impl DockerShell {
    pub async fn new(config: DockerShellConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        // find existing container
        let container_id = match docker.inspect_container(&config.container_name, None).await {
            Ok(inspect) => inspect.id.unwrap(),
            Err(_) => {
                let image = match &config.image {
                    DockerSourceConfig::Pull { name } => {
                        let pb = ProgressBar::new_spinner();
                        pb.set_style(
                            ProgressStyle::with_template("{spinner:.green} {msg}")
                                .unwrap()
                                .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]),
                        );
                        pb.set_message(format!("Pulling Docker image '{}'...", name.clone()));
                        pb.enable_steady_tick(std::time::Duration::from_millis(100));

                        let mut stream = docker.create_image(
                            Some(CreateImageOptions {
                                from_image: Some(name.clone()),
                                ..Default::default()
                            }),
                            None,
                            None,
                        );

                        while let Some(result) = stream.next().await {
                            if let Ok(info) = result {
                                if let Some(status) = info.status {
                                    pb.set_message(format!(
                                        "Pulling '{}': {}",
                                        name.clone(),
                                        status
                                    ));
                                }
                            }
                        }

                        pb.finish_with_message(format!("Docker image '{}' ready", name.clone()));

                        name
                    }
                    DockerSourceConfig::Dockerfile { path, name } => {
                        unimplemented!("Not implemented for now")
                    }
                };

                let create_config = ContainerCreateBody {
                    image: Some(image.into()),
                    tty: Some(true),
                    ..Default::default()
                };

                let container_id = docker
                    .create_container(
                        Some(CreateContainerOptions {
                            name: Some(config.container_name),
                            ..Default::default()
                        }),
                        create_config,
                    )
                    .await?
                    .id;

                container_id
            }
        };

        let _ = docker.start_container(&container_id, None).await?;

        Ok(Self {
            container_id,
            docker: Arc::new(docker),
        })
    }
}

#[async_trait::async_trait]
impl ShellProvider for DockerShell {
    async fn exec(&self, commands: String) -> Result<String> {
        let exec = self
            .docker
            .create_exec(
                &self.container_id,
                ExecConfig {
                    attach_stdout: Some(true),
                    attach_stdin: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec!["sh".into(), "-c".into(), commands]),
                    ..Default::default()
                },
            )
            .await;

        let exec_id = exec?.id;

        let mut res = vec![];
        if let StartExecResults::Attached { mut output, .. } = self
            .docker
            .start_exec(
                &exec_id,
                Some(StartExecOptions {
                    detach: false,
                    tty: false,
                    output_capacity: Some(1024 * 8),
                }),
            )
            .await?
        {
            while let Some(Ok(msg)) = output.next().await {
                println!("{msg}");
                res.push(msg.to_string());
            }
        }

        Ok(res.iter().map(|s| s.clone()).collect::<Vec<_>>().join("\n"))
    }
}
