//! Password hashing and verification (Argon2), shared by every edition.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

use crate::{Result, error::VizierError, storage::user::User};

pub fn hash(password: &str) -> Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| VizierError(format!("Failed to hash password: {e}")))
}

/// `false` for an empty or malformed hash, so a user without a password never verifies.
pub fn verify(password: &str, hash: &str) -> bool {
    if hash.is_empty() {
        return false;
    }
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

pub fn has_password(user: &User) -> bool {
    !user.password_hash.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_empty_hash() {
        let h = hash("hunter2").unwrap();
        assert!(verify("hunter2", &h));
        assert!(!verify("hunter3", &h));
        assert!(!verify("", ""));
    }
}
