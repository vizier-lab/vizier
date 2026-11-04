//! Starting child processes without a console window of their own.
//!
//! The desktop app is linked as a GUI program (`app/src-tauri/src/main.rs`), so on Windows it has
//! no console to hand down: a child started with the default flags asks the system for a new one,
//! and a window flashes over the app for every shell command, `taskkill` and `git` call. Every
//! spawn in the engine therefore goes through the helpers here, which are no-ops off Windows.

use tokio::process::Command as AsyncCommand;

/// `CREATE_NO_WINDOW`: start the child with no console window of its own.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Start `command` with no console window of its own. Piped stdio keeps working, so a hidden child
/// is read exactly like a visible one.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn no_window(command: &mut AsyncCommand) {
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
}

/// [`no_window`] for a blocking [`std::process::Command`].
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn no_window_blocking(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::process::Stdio;

    use super::*;

    /// A child that reports whether Windows gave it a console: `GetConsoleWindow` is 0 when there
    /// is none. It runs from a file so the script needs no shell quoting.
    const PROBE: &str = r#"$sig = '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();'
Add-Type -Name W -Namespace N -MemberDefinition $sig
[N.W]::GetConsoleWindow().ToInt64()
"#;

    fn probe(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("probe.ps1");
        std::fs::write(&path, PROBE).unwrap();
        path
    }

    #[tokio::test]
    async fn a_hidden_child_has_no_console_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = AsyncCommand::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(probe(dir.path()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_window(&mut command);
        let output = command.output().await.unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0", "{output:?}");
    }

    #[tokio::test]
    async fn a_hidden_blocking_child_has_no_console_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = std::process::Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(probe(dir.path()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_window_blocking(&mut command);
        let output = command.output().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0", "{output:?}");
    }
}
