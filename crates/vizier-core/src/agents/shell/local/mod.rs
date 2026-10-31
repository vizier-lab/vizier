use std::{collections::HashMap, path::PathBuf, process::Command};

use anyhow::Result;

use crate::{config::shell::LocalShellConfig, agents::shell::ShellProvider};

pub struct LocalShell {
    workdir: PathBuf,
    env: Option<HashMap<String, String>>,
}

impl LocalShell {
    /// Build from saved settings without touching the filesystem: an addon contributes its tool
    /// synchronously, and a local shell has nothing to prepare (the addons redesign).
    pub fn from_config(config: LocalShellConfig) -> Self {
        Self {
            workdir: PathBuf::from(config.path),
            env: config.env,
        }
    }

    pub async fn new(config: LocalShellConfig) -> Result<Self> {
        Ok(Self::from_config(config))
    }
}

/// The program and leading arguments that run one command string: `sh -c` on Unix, PowerShell
/// on Windows (its `ls`, `cat`, `echo`… aliases are what a model writing sh-style commands
/// expects, and it ships with every supported Windows). Output is forced to UTF-8 there, since
/// the console would otherwise answer in the OEM code page.
fn invocation(commands: &str) -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        let script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {commands}");
        (
            "powershell.exe",
            vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), script],
        )
    }
    #[cfg(not(windows))]
    {
        ("sh", vec!["-c".into(), commands.to_string()])
    }
}

#[async_trait::async_trait]
impl ShellProvider for LocalShell {
    async fn exec(&self, commands: String) -> Result<String> {
        let (program, args) = invocation(&commands);
        let mut cmd = Command::new(program);
        cmd.args(args);

        cmd.current_dir(self.workdir.clone());

        if let Some(ref env) = self.env {
            cmd.envs(env);
        }

        // `output()` blocks until the command exits. On an async worker that stalls every
        // other task on the runtime for as long as the command runs, and parallel background
        // pieces make several long commands at once ordinary.
        let output = tokio::task::spawn_blocking(move || cmd.output()).await??;

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runs_a_command_in_the_workdir() {
        let dir = tempfile::tempdir().unwrap();
        let shell = LocalShell::new(LocalShellConfig {
            path: dir.path().to_string_lossy().into_owned(),
            env: None,
        })
        .await
        .unwrap();
        // `echo` means the same under sh and PowerShell.
        let out = shell.exec("echo hello".into()).await.unwrap();
        assert!(out.contains("hello"), "got {out:?}");
    }
}
