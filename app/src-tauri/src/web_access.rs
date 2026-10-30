//! Opt-in browser access (FR-017..019): the server edition's listener over the app's own
//! runtime, with the server's login rules.

use std::path::Path;

use serde::Serialize;
use tokio::{sync::broadcast, task::JoinHandle};
use vizier_core::{config::HTTPChannelConfig, runtime::VizierRuntime};

use crate::settings::WebAccess;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WebAccessStatus {
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WebAccessStatus {
    pub fn running(url: String) -> Self {
        Self { running: true, url: Some(url), error: None }
    }
    pub fn stopped() -> Self {
        Self { running: false, url: None, error: None }
    }
    pub fn failed(error: String) -> Self {
        Self { running: false, url: None, error: Some(error) }
    }
}

pub struct WebAccessHandle {
    pub url: String,
    task: JoinHandle<()>,
}

impl WebAccessHandle {
    pub fn stop(self) {
        self.task.abort();
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `{workspace}/.runtime/web-access.key`: 32 random bytes, generated once, mode 0600. The app
/// creates it at start (it is also the runtime's JWT secret), not only when web access is
/// first enabled, because the router reads the secret from the runtime's config.
pub fn ensure_key(workspace: &Path) -> Result<Vec<u8>, String> {
    use ring::rand::SecureRandom;
    let dir = workspace.join(".runtime");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join("web-access.key");
    if let Ok(existing) = std::fs::read(&path)
        && existing.len() == 32
    {
        return Ok(existing);
    }
    let mut key = vec![0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut key).map_err(|_| "cannot generate a key".to_string())?;
    std::fs::write(&path, &key).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

/// The listener's config. Loopback unless the person allowed other devices.
pub fn listener_config(runtime: &VizierRuntime, wa: &WebAccess) -> HTTPChannelConfig {
    let base = runtime.deps().config.channels.http.clone().unwrap_or(HTTPChannelConfig {
        port: wa.port as u32,
        jwt_secret: String::new(),
        jwt_expiry_hours: 720,
        ws_idle_timeout_secs: 300,
        cors_origins: None,
        host: None,
    });
    HTTPChannelConfig {
        port: wa.port as u32,
        cors_origins: Some(vec![
            format!("http://127.0.0.1:{}", wa.port),
            format!("http://localhost:{}", wa.port),
        ]),
        host: Some(if wa.allow_network { "0.0.0.0" } else { "127.0.0.1" }.to_string()),
        ..base
    }
}

/// Starts the listener. A port that cannot be bound is an error here, not a stopped engine.
pub fn start(
    runtime: &VizierRuntime,
    wa: &WebAccess,
    status: broadcast::Sender<WebAccessStatus>,
) -> Result<WebAccessHandle, String> {
    let config = listener_config(runtime, wa);
    let host = config.host.clone().unwrap_or_default();
    // Fail fast and readably when the port is taken, instead of inside the spawned task.
    std::net::TcpListener::bind((host.as_str(), wa.port))
        .map(drop)
        .map_err(|e| format!("cannot listen on {host}:{}: {e}", wa.port))?;
    let url = format!("http://{}:{}", if wa.allow_network { "0.0.0.0" } else { "127.0.0.1" }, wa.port);
    let serving = vizier_http::serve(runtime, config);
    let task = tokio::spawn(async move {
        if let Err(e) = serving.await {
            let _ = status.send(WebAccessStatus::failed(e.0));
        }
    });
    Ok(WebAccessHandle { url, task })
}
