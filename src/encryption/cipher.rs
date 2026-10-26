//! The workspace cipher (`specs/031-encrypt-agent-secrets/contracts/encryption.md` §3–4): one
//! ChaCha20-Poly1305 key, a fresh random nonce per value, and a self-describing envelope
//! `enc:v1:<key-id>:<base64url(nonce ‖ ciphertext ‖ tag)>`.
//!
//! The associated data is not stored: callers pass the same derived string on read and write
//! (`src/encryption/fields.rs`), so a value copied to another row or field fails authentication.
//! The cipher is deliberately concrete rather than a trait — one workspace, one key, one format.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    aead::{Aad, CHACHA20_POLY1305, LessSafeKey, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::encryption::key::KeySource;

/// Literal prefix of every protected value. `v1` is the only version this release writes or
/// reads; any other `enc:` version is reported as an unknown format, never guessed at.
pub const ENVELOPE_PREFIX: &str = "enc:v1:";
const ENVELOPE_FAMILY: &str = "enc:";
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

/// Why a stored value could not be turned back into plaintext. The storage helpers in
/// `fields.rs` turn these into the table/record/field messages from the contract; none of them
/// ever carries the value, the ciphertext or key material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherFailure {
    /// The envelope is not `enc:v1:…`.
    UnknownVersion,
    /// The key id in the envelope is not this cipher's.
    WrongKey,
    /// The payload is not valid base64, is too short, or has no nonce.
    Malformed,
    /// AEAD authentication failed: the value was altered, truncated or moved.
    Integrity,
    /// A cipher without a key was asked to read a protected value.
    Disabled,
}

impl CipherFailure {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnknownVersion => "uses an unknown format version",
            Self::WrongKey => "was written by a different encryption key",
            Self::Malformed => "is not a valid protected value",
            Self::Integrity => "failed its integrity check",
            Self::Disabled => "cannot be read because encryption is disabled",
        }
    }
}

/// The one cipher for a workspace. Cloning is cheap (`Arc`); a disabled cipher is the
/// test/legacy-import mode and stores values unchanged.
#[derive(Clone)]
pub struct SecretCipher {
    inner: Option<Arc<Inner>>,
}

impl std::fmt::Debug for SecretCipher {
    /// Never print key material: only whether it is enabled, its id and its source.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretCipher")
            .field("enabled", &self.is_enabled())
            .field("key_id", &self.key_id())
            .field("source", &self.source())
            .finish()
    }
}

struct Inner {
    key: LessSafeKey,
    key_id: String,
    source: KeySource,
}

impl SecretCipher {
    /// Build an enabled cipher from a validated 32-byte key.
    pub fn from_key(key: [u8; 32], source: KeySource) -> Result<Self> {
        let key = Zeroizing::new(key);
        let key_id = key_id_for(&key);
        let unbound = UnboundKey::new(&CHACHA20_POLY1305, key.as_ref())
            .map_err(|_| anyhow!("could not initialize the encryption cipher"))?;
        Ok(Self {
            inner: Some(Arc::new(Inner {
                key: LessSafeKey::new(unbound),
                key_id,
                source,
            })),
        })
    }

    /// An enabled cipher with a fixed key, for tests only.
    #[cfg(test)]
    pub fn from_key_for_test(key: [u8; 32]) -> Self {
        Self::from_key(key, KeySource::Test).expect("a 32-byte test key is always valid")
    }

    /// No encryption: reads and writes are plaintext. Used by `SqliteStorage::new` for the
    /// existing tests and the pre-encryption filesystem import, never for production data.
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// The 16-hex identifier stored in every envelope and in `state:secrets.key_id`. Empty for
    /// a disabled cipher.
    pub fn key_id(&self) -> &str {
        self.inner.as_ref().map(|i| i.key_id.as_str()).unwrap_or("")
    }

    pub fn source(&self) -> KeySource {
        self.inner
            .as_ref()
            .map(|i| i.source.clone())
            .unwrap_or(KeySource::Disabled)
    }

    /// Whether a stored string claims to be protected. Any `enc:` prefix is reserved so an
    /// unknown future version is refused rather than treated as plaintext.
    pub fn is_protected(stored: &str) -> bool {
        stored.starts_with(ENVELOPE_FAMILY)
    }

    /// Encrypt `plaintext` with `aad`, returning a fresh envelope. A disabled cipher returns the
    /// value unchanged (test/legacy-import mode).
    pub fn encrypt(&self, plaintext: &str, aad: &str) -> Result<String> {
        let Some(inner) = &self.inner else {
            return Ok(plaintext.to_string());
        };

        let mut nonce_bytes = [0u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| anyhow!("could not read random bytes for the encryption nonce"))?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut in_out = plaintext.as_bytes().to_vec();
        inner
            .key
            .seal_in_place_append_tag(nonce, Aad::from(aad.as_bytes()), &mut in_out)
            .map_err(|_| anyhow!("could not encrypt the value"))?;

        let mut payload = Vec::with_capacity(NONCE_LEN + in_out.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&in_out);
        Ok(format!(
            "{ENVELOPE_PREFIX}{}:{}",
            inner.key_id,
            URL_SAFE_NO_PAD.encode(payload)
        ))
    }

    /// Decrypt a protected value. Plaintext is refused: normal reads require an envelope, and
    /// only the migration uses [`Self::open`].
    pub fn decrypt(&self, stored: &str, aad: &str) -> std::result::Result<String, CipherFailure> {
        let Some(inner) = &self.inner else {
            return if Self::is_protected(stored) {
                Err(CipherFailure::Disabled)
            } else {
                Ok(stored.to_string())
            };
        };

        let Some(rest) = stored.strip_prefix(ENVELOPE_PREFIX) else {
            return Err(if Self::is_protected(stored) {
                CipherFailure::UnknownVersion
            } else {
                CipherFailure::Malformed
            });
        };
        let (key_id, payload) = rest.split_once(':').ok_or(CipherFailure::Malformed)?;
        if key_id != inner.key_id {
            return Err(CipherFailure::WrongKey);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| CipherFailure::Malformed)?;
        if bytes.len() < NONCE_LEN + TAG_LEN {
            return Err(CipherFailure::Malformed);
        }

        let (nonce_bytes, ciphertext) = bytes.split_at(NONCE_LEN);
        let nonce =
            Nonce::try_assume_unique_for_key(nonce_bytes).map_err(|_| CipherFailure::Malformed)?;
        let mut in_out = ciphertext.to_vec();
        let opened = inner
            .key
            .open_in_place(nonce, Aad::from(aad.as_bytes()), &mut in_out)
            .map_err(|_| CipherFailure::Integrity)?;
        String::from_utf8(opened.to_vec()).map_err(|_| CipherFailure::Malformed)
    }

    /// `Some(plaintext)` for a valid envelope, `None` for a value that is still plaintext.
    /// Migration-only: no normal read may accept plaintext after the marker is set.
    pub fn open(
        &self,
        stored: &str,
        aad: &str,
    ) -> std::result::Result<Option<String>, CipherFailure> {
        if !Self::is_protected(stored) {
            return Ok(None);
        }
        self.decrypt(stored, aad).map(Some)
    }
}

/// 16 lowercase hex characters: the first 8 bytes of SHA-256(key). A one-way identifier, safe
/// to store and show; it is not key material.
pub fn key_id_for(key: &[u8; 32]) -> String {
    Sha256::digest(key)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The key id of an `enc:`-prefixed value, when its header is well formed. Used by the startup
/// scan that detects protected data before `state:secrets.key_id` exists.
pub fn envelope_key_id(stored: &str) -> Option<&str> {
    let rest = stored.strip_prefix(ENVELOPE_PREFIX)?;
    let (key_id, _) = rest.split_once(':')?;
    if key_id.len() == 16
        && key_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Some(key_id)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];
    const AAD: &str = "vizier:secrets:v1\x1fagent_secret\x1fdp-a\x1fTOKEN";

    fn cipher() -> SecretCipher {
        SecretCipher::from_key_for_test(KEY)
    }

    #[test]
    fn round_trips_exactly() {
        let cipher = cipher();
        let value = "  line one\nline two\r\n 日本語 🎉  ";
        let envelope = cipher.encrypt(value, AAD).unwrap();
        assert!(envelope.starts_with(ENVELOPE_PREFIX));
        assert_eq!(cipher.decrypt(&envelope, AAD).unwrap(), value);
    }

    #[test]
    fn the_same_plaintext_encrypts_differently_every_time() {
        let cipher = cipher();
        let a = cipher.encrypt("same", AAD).unwrap();
        let b = cipher.encrypt("same", AAD).unwrap();
        assert_ne!(a, b);
        assert_eq!(cipher.decrypt(&a, AAD).unwrap(), "same");
        assert_eq!(cipher.decrypt(&b, AAD).unwrap(), "same");
    }

    #[test]
    fn the_envelope_carries_the_key_id_and_version() {
        let cipher = cipher();
        let envelope = cipher.encrypt("x", AAD).unwrap();
        assert!(envelope.starts_with(&format!("{ENVELOPE_PREFIX}{}:", cipher.key_id())));
        assert_eq!(envelope_key_id(&envelope), Some(cipher.key_id()));
        assert_eq!(cipher.key_id().len(), 16);
    }

    #[test]
    fn tampering_is_rejected() {
        let cipher = cipher();
        let envelope = cipher.encrypt("secret", AAD).unwrap();
        let truncated = &envelope[..envelope.len() - 8];
        assert!(matches!(
            cipher.decrypt(truncated, AAD),
            Err(CipherFailure::Malformed) | Err(CipherFailure::Integrity)
        ));

        let mut bytes = envelope.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert_eq!(
            cipher.decrypt(&tampered, AAD),
            Err(CipherFailure::Integrity)
        );
    }

    #[test]
    fn another_key_or_another_aad_is_rejected() {
        let envelope = cipher().encrypt("secret", AAD).unwrap();
        let other = SecretCipher::from_key_for_test([8u8; 32]);
        assert_eq!(other.decrypt(&envelope, AAD), Err(CipherFailure::WrongKey));

        let moved = AAD.replace("TOKEN", "OTHER");
        assert_eq!(
            cipher().decrypt(&envelope, &moved),
            Err(CipherFailure::Integrity)
        );
    }

    #[test]
    fn unknown_versions_and_malformed_values_are_rejected() {
        let cipher = cipher();
        assert_eq!(
            cipher.decrypt("enc:v2:0000000000000000:AAAA", AAD),
            Err(CipherFailure::UnknownVersion)
        );
        assert_eq!(
            cipher.decrypt("not protected", AAD),
            Err(CipherFailure::Malformed)
        );
        assert_eq!(
            cipher.decrypt(
                &format!("{ENVELOPE_PREFIX}{}:not base64!", cipher.key_id()),
                AAD
            ),
            Err(CipherFailure::Malformed)
        );
        assert_eq!(
            cipher.decrypt(&format!("{ENVELOPE_PREFIX}{}:AAAA", cipher.key_id()), AAD),
            Err(CipherFailure::Malformed)
        );
    }

    #[test]
    fn open_allows_plaintext_but_decrypt_does_not() {
        let cipher = cipher();
        assert_eq!(cipher.open("plain", AAD).unwrap(), None);
        assert_eq!(cipher.decrypt("plain", AAD), Err(CipherFailure::Malformed));
        let envelope = cipher.encrypt("secret", AAD).unwrap();
        assert_eq!(cipher.open(&envelope, AAD).unwrap(), Some("secret".into()));
    }

    #[test]
    fn a_disabled_cipher_stores_plaintext_and_refuses_envelopes() {
        let cipher = SecretCipher::disabled();
        assert!(!cipher.is_enabled());
        assert_eq!(cipher.encrypt("plain", AAD).unwrap(), "plain");
        assert_eq!(cipher.decrypt("plain", AAD).unwrap(), "plain");
        assert_eq!(
            cipher.decrypt("enc:v1:0000000000000000:AAAA", AAD),
            Err(CipherFailure::Disabled)
        );
        assert_eq!(cipher.open("plain", AAD).unwrap(), None);
    }

    #[test]
    fn a_key_id_is_stable_and_lowercase_hex() {
        let id = key_id_for(&KEY);
        assert_eq!(id, key_id_for(&KEY));
        assert_eq!(id.len(), 16);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
        assert_ne!(id, key_id_for(&[9u8; 32]));
    }
}
