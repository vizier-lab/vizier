use anyhow::Result;

use crate::config::provider::ProviderVariant;
use crate::schema::ProviderEntry;
use crate::storage::{fs::FileSystemStorage, provider::ProviderStorage};
use crate::utils::build_path;

const PROVIDERS_PATH: &str = "providers";

#[async_trait::async_trait]
impl ProviderStorage for FileSystemStorage {
    async fn list_providers(&self) -> Result<Vec<ProviderEntry>> {
        let dir = build_path(&self.workspace, &[PROVIDERS_PATH]);

        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut providers = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let raw = std::fs::read_to_string(&path)?;
                if let Ok(provider) = serde_json::from_str::<ProviderEntry>(&raw) {
                    providers.push(provider);
                }
            }
        }

        Ok(providers)
    }

    async fn get_provider(&self, variant: &ProviderVariant) -> Result<Option<ProviderEntry>> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let path = build_path(&self.workspace, &[PROVIDERS_PATH, &format!("{}.json", variant_str)]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let entry = serde_json::from_str::<ProviderEntry>(&raw)?;
        Ok(Some(entry))
    }

    async fn upsert_provider(&self, entry: &ProviderEntry) -> Result<()> {
        let dir = build_path(&self.workspace, &[PROVIDERS_PATH]);
        let _ = std::fs::create_dir_all(&dir)?;

        let variant_str = serde_json::to_string(&entry.variant)?.trim_matches('"').to_string();
        let path = dir.join(format!("{}.json", variant_str));

        std::fs::write(path, serde_json::to_string_pretty(entry)?)?;
        Ok(())
    }

    async fn delete_provider(&self, variant: &ProviderVariant) -> Result<()> {
        let variant_str = serde_json::to_string(variant)?.trim_matches('"').to_string();
        let path = build_path(&self.workspace, &[PROVIDERS_PATH, &format!("{}.json", variant_str)]);

        if !path.exists() {
            return Err(anyhow::anyhow!("Provider '{:?}' not found", variant));
        }

        std::fs::remove_file(path)?;
        Ok(())
    }
}
