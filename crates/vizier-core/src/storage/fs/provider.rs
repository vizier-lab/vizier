use anyhow::Result;

use crate::storage::fs::FileSystemStorage;
use crate::utils::build_path;

const PROVIDERS_PATH: &str = "providers";

impl FileSystemStorage {
    /// The provider files of a pre-sqlite deployment, as `(legacy variant, raw JSON)`, for
    /// `migrate_filesystem_backend_to_sqlite` to copy into `provider_config` as they are;
    /// `migrate_provider_ids_v1` then rewrites them.
    pub fn legacy_provider_rows(&self) -> Result<Vec<(String, String)>> {
        let dir = build_path(&self.workspace, &[PROVIDERS_PATH]);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut rows = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Some(variant) = path.file_stem().and_then(|s| s.to_str())
            {
                rows.push((variant.to_string(), std::fs::read_to_string(&path)?));
            }
        }
        Ok(rows)
    }
}
