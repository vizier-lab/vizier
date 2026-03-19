use std::env;

use anyhow::Result;

use crate::config::VizierConfig;

pub async fn init() -> Result<()> {
    let current_dir = env::current_dir()?;

    let mut config = VizierConfig::default();
    config.workspace = format!("{}/.vizier", current_dir.to_str().unwrap());

    let mut config_path = current_dir.clone();
    config_path.push(".vizier.yaml");
    config.save(config_path.clone(), "".into())?;

    let _ = VizierConfig::load(Some(config_path));

    Ok(())
}
