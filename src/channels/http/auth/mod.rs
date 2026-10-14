use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{config::HTTPChannelConfig, storage::user::Role};

pub mod middleware;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub permissions: Vec<String>,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct AuthService {
    jwt_secret: String,
    jwt_expiry_hours: i64,
}

impl AuthService {
    pub fn new(config: &HTTPChannelConfig) -> Self {
        Self {
            jwt_secret: config.jwt_secret.clone(),
            jwt_expiry_hours: config.jwt_expiry_hours as i64,
        }
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String> {
        let argon2 = Argon2::default();
        let password_hash: PasswordHash = argon2
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| anyhow::anyhow!("Invalid password hash: {}", e))?;
        let argon2 = Argon2::default();
        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Generate a JWT token
    pub fn generate_token(&self, user_id: &str, username: &str, permissions: Vec<String>) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            permissions,
            iat: now.timestamp() as usize,
            exp: exp.timestamp() as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;

        Ok(token)
    }

    /// Validate a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| anyhow::anyhow!("Invalid token: {}", e))?;

        Ok(token_data.claims)
    }

    /// Generate a new API key
    /// Returns (raw_key, key_hash) - raw_key is shown once to user
    pub fn generate_api_key(&self) -> (String, String) {
        let raw_key_bytes: Vec<u8> = rand::random_iter::<u8>().take(32).collect();
        let raw_key = format!("vk_{}", base64::engine::general_purpose::URL_SAFE.encode(&raw_key_bytes));
        let key_hash = self.hash_api_key(&raw_key);
        (raw_key, key_hash)
    }

    /// Hash an API key using SHA-256
    pub fn hash_api_key(&self, api_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// How a request was authenticated — a JWT bearer token (the WebUI's login flow) or an API
/// key (programmatic callers). Used to attribute document revisions (`RevisionOrigin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub permissions: Vec<String>,
    pub auth_method: AuthMethod,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> AuthService {
        AuthService {
            jwt_secret: "test-secret".into(),
            jwt_expiry_hours: 1,
        }
    }

    #[test]
    fn password_hash_roundtrip() {
        let auth = service();
        let hash = auth.hash_password("hunter2").unwrap();
        assert!(auth.verify_password("hunter2", &hash).unwrap());
        assert!(!auth.verify_password("hunter3", &hash).unwrap());
    }

    #[test]
    fn verifies_hashes_from_argon2_0_5() {
        // Produced by argon2 0.5 (`Argon2::default()`); stored user passwords look like this.
        let hash = "$argon2id$v=19$m=19456,t=2,p=1$9z1k3bVftUl57uONSs6lrw$nViMG0o4j+qwiT1TtSE9J1dZDjbwCHj/xKYxa6Fkau0";
        assert!(service().verify_password("hunter2", hash).unwrap());
    }

    #[test]
    fn api_key_hash_is_lowercase_hex_sha256() {
        assert_eq!(
            service().hash_api_key("vk_test"),
            "84457fe090033a7aed800e1ba59fda7df2774e7217917cfe17225e37655dd08f"
        );
    }

    #[test]
    fn token_roundtrip() {
        let auth = service();
        let token = auth
            .generate_token("u1", "alice", vec!["read".into()])
            .unwrap();
        let claims = auth.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "u1");
        assert_eq!(claims.permissions, vec!["read".to_string()]);
    }
}
