//! Encrypted storage of secrets at rest (`specs/031-encrypt-agent-secrets/`).
//!
//! The storage boundary protects individual secret-bearing fields with one workspace key:
//! agent secret values, provider credentials, custom-provider keys and header values, Discord
//! and Telegram tokens, the Brave Search key, MCP environment values, and shell environment
//! values. Non-secret siblings stay readable.
//!
//! The cipher, key handling and the versioned envelope are permanent. The one-time plaintext
//! conversion in [`migrate`] and its plaintext branch are tracked for removal in #82.

pub mod cipher;
pub mod fields;
pub mod key;
pub mod migrate;

pub use cipher::{CipherFailure, SecretCipher};
pub use fields::ProtectMode;
pub use key::{KeyRequest, KeySource, default_key_path, load_or_generate};

/// Which paths in an integration's `settings` are secret. Implemented once by
/// [`crate::integrations::IntegrationKinds`] from each kind's `listing().fields`, so a new kind
/// declares its secrets once for redaction, merge and encryption alike.
///
/// `Some(paths)` for a known kind; `None` for a kind this binary does not know, which makes
/// storage protect the whole settings JSON in one envelope.
pub trait IntegrationSecretPaths: Send + Sync {
    fn secret_paths(&self, kind: &str) -> Option<Vec<String>>;
}

/// A path provider that declares no secret fields. Used by `SqliteStorage::new` for tests and
/// the pre-encryption filesystem import, where no field is encrypted.
pub struct NoIntegrationSecretPaths;

impl IntegrationSecretPaths for NoIntegrationSecretPaths {
    fn secret_paths(&self, _kind: &str) -> Option<Vec<String>> {
        Some(Vec::new())
    }
}

#[cfg(test)]
pub(crate) mod storage_tests;
