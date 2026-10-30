//! Desktop-only settings, stored as `{app_config_dir}/desktop.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebAccess {
    pub enabled: bool,
    pub allow_network: bool,
    pub port: u16,
}

impl Default for WebAccess {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_network: false,
            port: 9999,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Updates {
    pub check_on_start: bool,
}

impl Default for Updates {
    fn default() -> Self {
        Self { check_on_start: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DesktopSettings {
    pub workspace: PathBuf,
    /// workspace path -> bound owner user id
    pub bindings: HashMap<String, String>,
    pub close_to_tray: bool,
    pub web_access: WebAccess,
    pub updates: Updates,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl DesktopSettings {
    pub fn defaults(app_data_dir: &Path) -> Self {
        Self {
            workspace: app_data_dir.join("workspace"),
            bindings: HashMap::new(),
            close_to_tray: true,
            web_access: WebAccess::default(),
            updates: Updates::default(),
            extra: serde_json::Map::new(),
        }
    }

    fn path(app_config_dir: &Path) -> PathBuf {
        app_config_dir.join("desktop.json")
    }

    /// Loads settings; a missing or unreadable file yields defaults.
    pub fn load(app_config_dir: &Path, app_data_dir: &Path) -> Self {
        let mut settings = Self::defaults(app_data_dir);
        match std::fs::read(Self::path(app_config_dir)) {
            Ok(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(s) => settings = s,
                Err(err) => tracing::warn!("desktop.json is invalid, using defaults: {err}"),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!("cannot read desktop.json: {err}"),
        }
        settings
    }

    /// Writes atomically (temp file + rename).
    pub fn save(&self, app_config_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(app_config_dir)?;
        let path = Self::path(app_config_dir);
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)
    }
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self::defaults(Path::new("."))
    }
}
