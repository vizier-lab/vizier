use std::{collections::HashMap, path::PathBuf, process::Command};

use anyhow::Result;

use crate::{config::shell::LocalShellConfig, agents::shell::ShellProvider};

pub struct LocalShell {
    workdir: PathBuf,
    env: Option<HashMap<String, String>>,
}

impl LocalShell {
    pub async fn new(config: LocalShellConfig) -> Result<Self> {
        Ok(Self {
            workdir: PathBuf::from(config.path),
            env: config.env,
        })
    }
}

#[async_trait::async_trait]
impl ShellProvider for LocalShell {
    async fn exec(&self, commands: String) -> Result<String> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").args([&commands]);

        cmd.current_dir(self.workdir.clone());

        if let Some(ref env) = self.env {
            cmd.envs(env);
        }

        // `output()` blocks until the command exits. On an async worker that stalls every
        // other task on the runtime for as long as the command runs, and parallel background
        // pieces make several long commands at once ordinary.
        let output = tokio::task::spawn_blocking(move || cmd.output()).await??;

        Ok(String::from_utf8(output.stdout)?)
    }
}
