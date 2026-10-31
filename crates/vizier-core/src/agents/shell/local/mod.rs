use std::{collections::HashMap, path::PathBuf, process::Stdio, time::Instant};

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

use crate::{
    agents::shell::{
        ShellProvider,
        outcome::{ShellCommand, ShellOutcome},
        process_tree,
    },
    config::shell::LocalShellConfig,
    schema::OUTPUT_CAP_BYTES,
};

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

    /// Run one command with the richer outcome the workspace shell reports (`specs/040`,
    /// contracts/workspace-shell.md): both streams capped, the exit code, duration, timeout and a
    /// process-tree kill.
    pub async fn run(&self, command: ShellCommand) -> Result<ShellOutcome> {
        let (program, args) = invocation(&command.command);
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.current_dir(&command.workdir);
        if let Some(ref env) = self.env {
            cmd.envs(env);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        process_tree::isolate(&mut cmd);

        let started = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|err| anyhow::anyhow!("failed to spawn the shell: {err}"))?;
        let out = tokio::spawn(read_capped(child.stdout.take()));
        let err = tokio::spawn(read_capped(child.stderr.take()));

        let waited = tokio::time::timeout(command.timeout, child.wait()).await;
        let (exit_code, timed_out) = match waited {
            Ok(status) => (Some(status?.code()), false),
            Err(_) => {
                process_tree::kill(&mut child).await;
                (None, true)
            }
        };

        let (stdout, out_cut) = out.await.unwrap_or_default();
        let (stderr, err_cut) = err.await.unwrap_or_default();
        Ok(ShellOutcome {
            stdout,
            stderr,
            exit_code: exit_code.flatten(),
            duration_ms: started.elapsed().as_millis() as u64,
            timed_out,
            truncated: out_cut || err_cut,
        })
    }
}

/// Read a stream to the end, keeping at most [`OUTPUT_CAP_BYTES`] and reporting whether it was
/// cut. The rest is drained so a chatty child never blocks on a full pipe.
async fn read_capped<R: AsyncRead + Unpin>(reader: Option<R>) -> (String, bool) {
    let Some(mut reader) = reader else {
        return (String::new(), false);
    };
    let mut buffer = [0u8; 8192];
    let mut kept: Vec<u8> = Vec::new();
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                let room = OUTPUT_CAP_BYTES.saturating_sub(kept.len());
                if room == 0 {
                    truncated = true;
                    continue;
                }
                let take = room.min(n);
                kept.extend_from_slice(&buffer[..take]);
                if take < n {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (String::from_utf8_lossy(&kept).into_owned(), truncated)
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
        // The addon path keeps its text contract and its effectively unbounded wait; the
        // workspace shell is the caller that sets a real limit.
        let outcome = self
            .run(ShellCommand {
                command: commands,
                workdir: self.workdir.clone(),
                timeout: std::time::Duration::from_secs(24 * 60 * 60),
            })
            .await?;
        Ok(outcome.to_text())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::schema::{DEFAULT_COMMAND_TIMEOUT, OUTPUT_CAP_BYTES};

    fn shell(dir: &std::path::Path) -> LocalShell {
        LocalShell::from_config(LocalShellConfig {
            path: dir.to_string_lossy().into_owned(),
            env: None,
        })
    }

    fn command(dir: &std::path::Path, line: &str) -> ShellCommand {
        ShellCommand {
            command: line.into(),
            workdir: dir.to_path_buf(),
            timeout: DEFAULT_COMMAND_TIMEOUT,
        }
    }

    #[tokio::test]
    async fn runs_a_command_in_the_workdir() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = shell(dir.path()).run(command(dir.path(), "echo hello")).await.unwrap();
        assert!(outcome.stdout.contains("hello"), "got {outcome:?}");
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
        assert!(!outcome.truncated);
    }

    #[tokio::test]
    async fn reports_stderr_and_a_non_zero_exit_as_a_normal_result() {
        let dir = tempfile::tempdir().unwrap();
        let line = if cfg!(windows) {
            "[Console]::Error.WriteLine('oops'); exit 3"
        } else {
            "echo oops 1>&2; exit 3"
        };
        let outcome = shell(dir.path()).run(command(dir.path(), line)).await.unwrap();
        assert!(outcome.stderr.contains("oops"), "got {outcome:?}");
        assert_eq!(outcome.exit_code, Some(3));
        assert!(!outcome.timed_out);
    }

    #[tokio::test]
    async fn cuts_a_stream_at_the_cap_and_flags_it() {
        let dir = tempfile::tempdir().unwrap();
        let line = if cfg!(windows) {
            "('x' * 70000)"
        } else {
            "printf 'x%.0s' {1..70000}"
        };
        let outcome = shell(dir.path()).run(command(dir.path(), line)).await.unwrap();
        assert!(outcome.truncated, "got {outcome:?}");
        assert!(outcome.stdout.len() <= OUTPUT_CAP_BYTES);
    }

    #[tokio::test]
    async fn a_timeout_kills_the_process_tree() {
        let dir = tempfile::tempdir().unwrap();
        let line = if cfg!(windows) {
            "Start-Sleep -Seconds 30"
        } else {
            "sleep 30"
        };
        let started = std::time::Instant::now();
        let outcome = shell(dir.path())
            .run(ShellCommand {
                command: line.into(),
                workdir: dir.path().to_path_buf(),
                timeout: Duration::from_secs(1),
            })
            .await
            .unwrap();
        assert!(outcome.timed_out, "got {outcome:?}");
        assert_eq!(outcome.exit_code, None);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
