use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageConfig {
    /// No longer a selectable runtime backend (research.md §10, FR-025) — this variant survives
    /// only so an existing `.vizier.yaml`/`VIZIER_STORAGE=filesystem` from before this feature
    /// still parses, so `VizierDependencies::new` can detect it and run the one-time migration
    /// into sqlite. It is never used to construct a live storage backend; `--storage filesystem`
    /// on the CLI is rejected outright (see `cli::run::StorageKind`).
    Filesystem,
    Sqlite,
}
