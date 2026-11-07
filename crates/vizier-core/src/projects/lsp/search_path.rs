//! Finding a server binary, and the environment it runs with (R4, R11).
//!
//! A GUI app gets a minimal `PATH`, so a lookup that misses reads the login shell's `PATH` once
//! and adds the usual tool folders. A lookup never searches the project folder: a binary a
//! repository ships is not run unless a person sets it as an override.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Names removed from a server's environment, besides anything matching `secret_name`.
const REMOVED_EXACT: &[&str] = &["AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "DATABASE_URL"];

/// Is this an environment variable a server must not inherit?
pub fn secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("VIZIER_")
        || REMOVED_EXACT.contains(&upper.as_str())
        || upper.ends_with("_API_KEY")
        || upper.ends_with("_TOKEN")
        || upper.contains("_SECRET")
        || upper.ends_with("_SECRET")
}

/// The environment for a server: inherited minus secrets, then the override's own values.
pub fn filtered_env(
    inherited: impl IntoIterator<Item = (String, String)>,
    extra_path: Option<&str>,
    overrides: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = inherited
        .into_iter()
        .filter(|(name, _)| !secret_name(name))
        .collect();
    if let Some(path) = extra_path
        && let Some(entry) = env.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
    {
        entry.1 = path.to_owned();
    }
    for (name, value) in overrides {
        env.retain(|(k, _)| k != name);
        env.push((name.clone(), value.clone()));
    }
    env
}

fn well_known_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        dirs.push(home.join(".cargo").join("bin"));
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join("go").join("bin"));
    }
    #[cfg(unix)]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs
}

static LOGIN_PATH: OnceLock<Option<String>> = OnceLock::new();

/// The login shell's `PATH`, read once per process with a 3 s limit.
#[cfg(unix)]
pub async fn login_shell_path() -> Option<String> {
    if let Some(cached) = LOGIN_PATH.get() {
        return cached.clone();
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut command = tokio::process::Command::new(shell);
    command
        .args(["-lc", "printf '%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(3), command.output())
        .await
        .ok()
        .and_then(|r| r.ok())
        .filter(|o| o.status.success());
    let value = output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|p| !p.is_empty());
    let _ = LOGIN_PATH.set(value.clone());
    value
}

#[cfg(not(unix))]
pub async fn login_shell_path() -> Option<String> {
    None
}

fn executable_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        let mut names = vec![name.to_owned()];
        names.extend(
            exts.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{name}{}", e.to_ascii_lowercase())),
        );
        names
    }
    #[cfg(not(windows))]
    {
        vec![name.to_owned()]
    }
}

/// The command as a path to look for, or nothing when it is a bare name. A command that names a
/// path is written whole on Windows (`…/examples/fake_lsp`), while the program itself carries the
/// extension (`fake_lsp.exe`), so the extension has to be tried on it too.
fn named_path(command: &str) -> Vec<PathBuf> {
    let path = Path::new(command);
    if !path.is_absolute() {
        return vec![];
    }
    let mut paths = vec![path.to_path_buf()];
    if cfg!(windows) && path.extension().is_none() {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        paths.extend(
            exts.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| path.with_extension(e.trim_start_matches('.').to_ascii_lowercase())),
        );
    }
    paths
}

fn is_executable(path: &Path) -> bool {    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Look `command` up in `search` (a `PATH`-style string). Relative entries are ignored.
pub fn find_in(command: &str, search: &OsString) -> Option<PathBuf> {
    let names = executable_names(command);
    std::env::split_paths(search)
        .filter(|dir| dir.is_absolute())
        .find_map(|dir| {
            names
                .iter()
                .map(|n| dir.join(n))
                .find(|candidate| is_executable(candidate))
        })
}

/// Where a command is, and the `PATH` the server should be given.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub program: PathBuf,
    /// Set when the login shell's `PATH` was needed, so the child inherits it too.
    pub path: Option<String>,
}

/// Resolve a command. A command with a separator is taken as given (an override may name one);
/// a bare name is searched for, never in the project folder.
pub async fn resolve(command: &str) -> Option<Found> {
    if command.contains('/') || command.contains('\\') {
        let named = named_path(command);
        return named.into_iter().find(|p| is_executable(p)).map(|program| Found { program, path: None });
    }
    let process_path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(program) = find_in(command, &process_path) {
        return Some(Found { program, path: None });
    }
    let mut extended: Vec<PathBuf> = std::env::split_paths(&process_path).collect();
    if let Some(login) = login_shell_path().await {
        extended.extend(std::env::split_paths(&login));
    }
    extended.extend(well_known_dirs());
    let joined = std::env::join_paths(extended).ok()?;
    let program = find_in(command, &joined)?;
    Some(Found {
        program,
        path: Some(joined.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_removed_and_toolchain_variables_kept() {
        let inherited = vec![
            ("FOO_API_KEY".to_string(), "x".to_string()),
            ("VIZIER_JWT_SECRET".into(), "x".into()),
            ("GITHUB_TOKEN".into(), "x".into()),
            ("AWS_SECRET_ACCESS_KEY".into(), "x".into()),
            ("DATABASE_URL".into(), "x".into()),
            ("RUSTUP_HOME".into(), "/r".into()),
            ("GOPATH".into(), "/g".into()),
            ("PATH".into(), "/bin".into()),
        ];
        let env = filtered_env(inherited, None, &HashMap::new());
        let names: Vec<_> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["RUSTUP_HOME", "GOPATH", "PATH"]);
    }

    #[test]
    fn an_override_env_wins_and_can_restore_a_token() {
        let inherited = vec![("RA_LOG".to_string(), "info".to_string())];
        let mut extra = HashMap::new();
        extra.insert("RA_LOG".to_string(), "warn".to_string());
        extra.insert("MY_TOKEN".to_string(), "t".to_string());
        let env = filtered_env(inherited, None, &extra);
        assert!(env.contains(&("RA_LOG".into(), "warn".into())));
        assert!(env.contains(&("MY_TOKEN".into(), "t".into())));
        assert_eq!(env.iter().filter(|(k, _)| k == "RA_LOG").count(), 1);
    }

    #[test]
    fn the_login_path_replaces_path() {
        let env = filtered_env(
            vec![("PATH".to_string(), "/bin".to_string())],
            Some("/bin:/x"),
            &HashMap::new(),
        );
        assert_eq!(env, vec![("PATH".to_string(), "/bin:/x".to_string())]);
    }

    #[cfg(unix)]
    #[test]
    fn lookup_finds_an_executable_and_skips_relative_entries() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("my-ls");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let search = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in("my-ls", &search), Some(bin));
        assert_eq!(find_in("absent", &search), None);
        let relative = OsString::from(".");
        assert_eq!(find_in("my-ls", &relative), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_executable_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("plain"), "x").unwrap();
        let search = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in("plain", &search), None);
    }
}
