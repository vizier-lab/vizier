use anyhow::Result;

use crate::schema::GlobalConfigEntry;
use crate::storage::{fs::FileSystemStorage, global_config::GlobalConfigStorage};
use crate::utils::build_path;

const GLOBAL_CONFIG_PATH: &str = "global_config";

#[async_trait::async_trait]
impl GlobalConfigStorage for FileSystemStorage {
    async fn list_global_configs(&self) -> Result<Vec<GlobalConfigEntry>> {
        let dir = build_path(&self.workspace, &[GLOBAL_CONFIG_PATH]);

        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let raw = std::fs::read_to_string(&path)?;
                if let Ok(config) = serde_json::from_str::<GlobalConfigEntry>(&raw) {
                    entries.push(config);
                }
            }
        }

        Ok(entries)
    }

    async fn get_global_config(&self, key: &str) -> Result<Option<GlobalConfigEntry>> {
        let path = build_path(
            &self.workspace,
            &[GLOBAL_CONFIG_PATH, &format!("{}.json", key)],
        );

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let entry = serde_json::from_str::<GlobalConfigEntry>(&raw)?;
        Ok(Some(entry))
    }

    async fn upsert_global_config(&self, entry: &GlobalConfigEntry) -> Result<()> {
        let dir = build_path(&self.workspace, &[GLOBAL_CONFIG_PATH]);
        let _ = std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.json", entry.key));
        std::fs::write(path, serde_json::to_string_pretty(entry)?)?;
        Ok(())
    }

    async fn delete_global_config(&self, key: &str) -> Result<()> {
        let path = build_path(
            &self.workspace,
            &[GLOBAL_CONFIG_PATH, &format!("{}.json", key)],
        );

        if !path.exists() {
            return Err(anyhow::anyhow!("Global config '{}' not found", key));
        }

        std::fs::remove_file(path)?;
        Ok(())
    }
}
