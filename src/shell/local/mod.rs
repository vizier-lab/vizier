use std::{collections::HashMap, path::PathBuf, process::Command};

use anyhow::Result;

use crate::{config::shell::LocalShellConfig, shell::ShellProvider};

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

        let output = cmd.output()?;

        Ok(String::from_utf8(output.stdout)?)
    }
}
