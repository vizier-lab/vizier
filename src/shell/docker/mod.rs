use std::{collections::HashMap, sync::Arc};

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
    env: Option<HashMap<String, String>>,
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
                        let pb = ProgressBar::new_spinner();
                        pb.set_style(
                            ProgressStyle::with_template("{spinner:.green} {msg}")
                                .unwrap()
                                .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]),
                        );
                        pb.set_message(format!("Building Docker image '{}'...", name));
                        pb.enable_steady_tick(std::time::Duration::from_millis(100));

                        // Create tar archive of the build context (directory containing Dockerfile)
                        let path = std::path::Path::new(&path);
                        let build_context =
                            crate::utils::tar::create_tar_archive(path.parent().unwrap_or(path))?;

                        let dockerfile_path = path
                            .file_name()
                            .unwrap_or_default()
                            .to_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Dockerfile".to_string());

                        let mut stream = docker.build_image(
                            bollard::query_parameters::BuildImageOptions {
                                dockerfile: dockerfile_path,
                                t: Some(name.clone()),
                                ..Default::default()
                            },
                            None,
                            Some(bollard::body_full(build_context.into())),
                        );

                        while let Some(result) = stream.next().await {
                            if let Ok(info) = result {
                                if let Some(stream) = info.stream {
                                    pb.set_message(format!("Building '{}': {}", name, stream));
                                } else if let Some(error_detail) = info.error_detail {
                                    let error_msg = error_detail.message.unwrap_or_default();
                                    pb.finish_with_message(format!(
                                        "Failed to build '{}': {}",
                                        name, error_msg
                                    ));
                                    return Err(anyhow::anyhow!(
                                        "Docker build failed: {}",
                                        error_msg
                                    ));
                                }
                            }
                        }

                        pb.finish_with_message(format!("Docker image '{}' ready", name));

                        name
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
            env: config.env,
        })
    }
}

#[async_trait::async_trait]
impl ShellProvider for DockerShell {
    async fn exec(&self, commands: String) -> Result<String> {
        let env: Option<Vec<String>> = self.env.as_ref().map(|env| {
            env.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect()
        });

        let exec = self
            .docker
            .create_exec(
                &self.container_id,
                ExecConfig {
                    attach_stdout: Some(true),
                    attach_stdin: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec!["sh".into(), "-c".into(), commands]),
                    env,
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
                res.push(msg.to_string());
            }
        }

        Ok(res.iter().map(|s| s.clone()).collect::<Vec<_>>().join("\n"))
    }
}
