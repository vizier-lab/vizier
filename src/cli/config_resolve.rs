//! Where the server's config and workspace come from (`specs/037-core-extraction/` R7). The
//! engine takes a config with an explicit workspace; deciding which file and which directory
//! is the binary's job.

use std::{env::current_dir, fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::{config::VizierConfig, constant};

/// The config, from the first of: `path` (`-c`), `$VIZIER_CONFIG`, `./.vizier.yaml`. With
/// none, the built-in defaults (`${ENV}` expanded) on `$VIZIER_DATA_DIR`, else `$HOME/.vizier`,
/// created if missing.
pub fn resolve(path: Option<PathBuf>) -> Result<VizierConfig> {
    let resolved_path: Option<PathBuf> = match path {
        Some(p) => Some(p),
        None => std::env::var("VIZIER_CONFIG")
            .ok()
            .map(PathBuf::from)
            .or_else(default_config_candidate),
    };

    match resolved_path {
        Some(p) => VizierConfig::from_path(&p),
        None => {
            tracing::info!("no config file found, using built-in defaults");
            let mut config = VizierConfig::default().expand_env()?;
            config.workspace = resolve_default_workspace()?;
            Ok(config)
        }
    }
}

fn default_config_candidate() -> Option<PathBuf> {
    let candidate = current_dir().ok()?.join(constant::DEFAULT_CONFIG_PATH);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn resolve_default_workspace() -> Result<String> {
    let data_dir = std::env::var("VIZIER_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".vizier")))
        .context("could not determine workspace: VIZIER_DATA_DIR unset and $HOME unknown")?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create workspace at {}", data_dir.display()))?;
    Ok(data_dir.to_string_lossy().to_string())
}
