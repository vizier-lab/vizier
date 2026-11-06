//! The two halves of a Bedrock SSO session: the non-secret view and the encrypted payload
//! (`specs/044-bedrock-sso-inference-profiles/data-model.md` "SSO session secret", research
//! D3 and D8).
//!
//! [`SsoSessionSecret`] is what `bedrock_sso_session.data` holds, wrapped in a
//! [`SecretCipher`](crate::encryption::SecretCipher) envelope whose AAD binds it to its profile.
//! It deliberately does **not** derive `Serialize` (research D8): the only way to turn it into
//! bytes is [`SsoSessionSecret::to_json`], called from exactly one place
//! (`encryption::fields::protect_sso_session`), so no generic serialiser — no API response, no
//! log line, no span — can emit a token by accident. [`SsoSession`] is the projection that *is*
//! serialised, and it carries no secret.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Where the connection stands, as the settings row renders it (data-model "the connection
/// state machine"). Derived from the stored rows on every read, never stored itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "kebab-case")]
pub enum SsoSessionState {
    /// The five connection fields are saved and no session row exists.
    Configured,
    /// A session row exists, decrypted, and its access token has not expired.
    SignedIn,
    /// No session row (or no usable refresh token) although the connection says signed in.
    Expired,
    /// A session row exists but this workspace key cannot read it (FR-009f).
    Unreadable,
}

/// The session as an API response or the settings row may show it: profile, start URL, both
/// regions, account, role, expiry and state — never a token, a client secret or a device code
/// (FR-009b, FR-035).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoSession {
    /// The non-secret handle the stored row is keyed by (FR-009e).
    pub profile: String,
    pub start_url: String,
    /// The Identity Center instance's region.
    pub sso_region: String,
    /// The Bedrock region requests go to — a different region, usually.
    pub bedrock_region: String,
    pub account_id: String,
    pub role_name: String,
    /// When the access token expires; `None` while the connection is only configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Local>>,
    pub state: SsoSessionState,
}

/// The encrypted payload of one `bedrock_sso_session` row (data-model "SSO session secret").
///
/// `Deserialize` only: see the module comment. `Debug` is hand-written so a stray `{:?}` in a
/// log line prints `[redacted]` instead of a token (research D8.3, FR-035).
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct SsoSessionSecret {
    /// From the token endpoint; leaves the process only to reach the SSO portal.
    pub access_token: String,
    /// Present when the flow returns one; used to refresh an expired access token.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// When `access_token` expires.
    pub token_expires_at: DateTime<Local>,
    /// From the one-time client registration.
    pub client_id: String,
    /// From the one-time client registration.
    pub client_secret: String,
    /// When the registration's secret expires; past it, register again.
    #[serde(default)]
    pub client_secret_expires_at: Option<DateTime<Local>>,
    /// For diagnostics and for ordering the compare-and-swap write.
    pub issued_at: DateTime<Local>,
    /// Incremented by the store on every successful write; the compare-and-swap key
    /// (research D5, FR-014).
    #[serde(default)]
    pub revision: u64,
}

impl std::fmt::Debug for SsoSessionSecret {
    /// Prints which fields are set, never what they hold.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoSessionSecret")
            .field("access_token", &"[redacted]")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "[redacted]"))
            .field("token_expires_at", &self.token_expires_at)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .field("client_secret_expires_at", &self.client_secret_expires_at)
            .field("issued_at", &self.issued_at)
            .field("revision", &self.revision)
            .finish()
    }
}

impl SsoSessionSecret {
    /// The exact JSON that `encryption::fields::protect_sso_session` encrypts.
    ///
    /// Built field by field instead of derived, because `Serialize` must not exist on this type
    /// (research D8): a hand-written producer is the one place that can emit the payload, and
    /// the round-trip test in `storage::sqlite::sso_session` keeps it in step with the
    /// `Deserialize` side.
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "access_token": self.access_token,
            "refresh_token": self.refresh_token,
            "token_expires_at": self.token_expires_at,
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "client_secret_expires_at": self.client_secret_expires_at,
            "issued_at": self.issued_at,
            "revision": self.revision,
        })
        .to_string()
    }

    /// The revision this value is at; `0` for a session that has never been written.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> SsoSessionSecret {
        SsoSessionSecret {
            access_token: "access-token-value".into(),
            refresh_token: Some("refresh-token-value".into()),
            token_expires_at: crate::time::now(),
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            client_secret_expires_at: None,
            issued_at: crate::time::now(),
            revision: 3,
        }
    }

    #[test]
    fn the_written_json_reads_back_as_the_same_secret() {
        let written = secret();
        let read: SsoSessionSecret = serde_json::from_str(&written.to_json()).unwrap();
        assert_eq!(read, written);
    }

    #[test]
    fn the_json_names_every_field_the_deserialiser_expects() {
        let json: serde_json::Value = serde_json::from_str(&secret().to_json()).unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "access_token",
                "client_id",
                "client_secret",
                "client_secret_expires_at",
                "issued_at",
                "refresh_token",
                "revision",
                "token_expires_at",
            ]
        );
    }

    #[test]
    fn neither_debug_nor_the_view_can_print_a_secret() {
        let printed = format!("{:?}", secret());
        for secret_value in [
            "access-token-value",
            "refresh-token-value",
            "client-secret-value",
        ] {
            assert!(!printed.contains(secret_value), "{printed}");
        }
        assert!(printed.contains("[redacted]"));

        let view = SsoSession {
            profile: "acme-bedrock".into(),
            start_url: "https://acme.awsapps.com/start".into(),
            sso_region: "us-east-1".into(),
            bedrock_region: "eu-west-1".into(),
            account_id: "123456789012".into(),
            role_name: "BedrockOperator".into(),
            expires_at: None,
            state: SsoSessionState::Configured,
        };
        let printed = serde_json::to_string(&view).unwrap();
        assert!(!printed.contains("token"));
        assert!(!printed.contains("secret"));
        // The state is the kebab-case name the API contract uses.
        assert!(printed.contains("\"configured\""), "{printed}");
    }
}
