//! A person's API keys. All of it needs `settings:api_keys`.

use base64::Engine;
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};

use crate::{
    access,
    auth::Principal,
    service::{ServiceError, Services, users::fail},
    storage::user::{ApiKey, UserStorage},
};

const NEEDS: &str = "settings:api_keys";

/// A new key: `(raw_key, key_hash)`. The raw key is shown once; only the hash is stored.
pub fn generate() -> (String, String) {
    let bytes: Vec<u8> = rand::random_iter::<u8>().take(32).collect();
    let raw = format!("vk_{}", base64::engine::general_purpose::URL_SAFE.encode(&bytes));
    let key_hash = hash(&raw);
    (raw, key_hash)
}

/// SHA-256 as lowercase hex.
pub fn hash(api_key: &str) -> String {
    Sha256::digest(api_key.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Returns the stored key and the raw key (shown once).
pub async fn create(
    services: &Services,
    principal: &Principal,
    name: &str,
    expires_in_days: Option<i64>,
) -> Result<(ApiKey, String), ServiceError> {
    access::require(principal, NEEDS)?;
    let (raw, key_hash) = generate();
    let expires_at = expires_in_days.map(|days| Utc::now() + Duration::days(days));
    let key = services
        .deps()
        .storage
        .create_api_key(&principal.user_id, name, &key_hash, &raw[..raw.len().min(7)], expires_at)
        .await
        .map_err(|e| fail("Failed to create API key", e))?;
    Ok((key, raw))
}

pub async fn list(services: &Services, principal: &Principal) -> Result<Vec<ApiKey>, ServiceError> {
    access::require(principal, NEEDS)?;
    services
        .deps()
        .storage
        .list_api_keys(&principal.user_id)
        .await
        .map_err(|e| fail("Failed to list API keys", e))
}

pub async fn delete(services: &Services, principal: &Principal, key_id: &str) -> Result<(), ServiceError> {
    access::require(principal, NEEDS)?;
    let storage = &services.deps().storage;
    let keys = storage
        .list_api_keys(&principal.user_id)
        .await
        .map_err(|e| fail("Failed to verify API key ownership", e))?;
    if !keys.iter().any(|k| k.id == key_id) {
        return Err(ServiceError::NotFound("API key not found".into()));
    }
    storage
        .delete_api_key(key_id)
        .await
        .map_err(|e| fail("Failed to delete API key", e))
}
