use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "indexer", rename_all = "snake_case")]
pub enum DocumentIndexerConfig {
    InMem,
    Surreal,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageConfig {
    Filesystem(DocumentIndexerConfig),
    Surreal,
}
