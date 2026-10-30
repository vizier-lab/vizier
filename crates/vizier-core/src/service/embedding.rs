//! The local embedding models an agent can choose from.

use crate::config::embedding::LOCAL_EMBEDDING_MODELS;

/// `(name, tier)` of every local embedding model.
pub fn local_models() -> Vec<(String, String)> {
    LOCAL_EMBEDDING_MODELS
        .iter()
        .map(|(name, tier)| (name.to_string(), tier.to_string()))
        .collect()
}
