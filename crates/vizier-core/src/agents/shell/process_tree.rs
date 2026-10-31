//! Killing a timed-out command's whole process tree (`specs/040-projects-mode/contracts/workspace-shell.md`).
//!
//! The platform-specific parts live here and nowhere else (Principle IV): on Unix the child is
//! started in its own process group and the group is signalled, so grandchildren die with it;
//! on Windows `taskkill /T /F` walks the tree.

use tokio::process::{Child, Command};

/// Put a command in its own process group, so the timeout can signal everything it spawned.
#[cfg(unix)]
pub fn isolate(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
pub fn isolate(_command: &mut Command) {}

/// Stop a child and everything it started, then reap it.
#[cfg(unix)]
pub async fn kill(child: &mut Child) {
    if let Some(pid) = child.id() {
        // The child's process group id is its pid (see `isolate`), so `killpg` reaches
        // grandchildren even after the shell itself exits.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(windows)]
pub async fn kill(child: &mut Child) {
    if let Some(pid) = child.id() {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(all(not(unix), not(windows)))]
pub async fn kill(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}
