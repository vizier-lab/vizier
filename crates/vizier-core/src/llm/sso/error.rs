//! The act-on-able failures of a Bedrock SSO sign-in (FR-041), in one enum (research D7).
//!
//! Every variant carries only the non-secret facts a person can act on — a profile name, an
//! account and role, a model id, an ARN, a region — and nothing else. No variant has anywhere to
//! put a device code, an access or refresh token, a client secret or a role credential, so a token
//! cannot reach a message through this type by construction; the unit test at the bottom renders
//! every variant and asserts the text carries none of them (FR-035, contracts §7).
//!
//! FR-041 names ten failures. Two further variants live here because they are acted on in the
//! same place and must not collapse into one of those ten: [`SsoError::SessionUnreadable`] is the
//! cipher's failure (FR-009f) and [`SsoError::AlreadyConfirmed`] is a device code confirmed twice
//! (contracts/bedrock-sso-wire.md §3), which is neither a denial nor an expiry.
//!
//! One cause is special-cased by the spec, and by this module: an **ended session** (FR-016).
//! [`SsoError::SessionExpired`] and [`SsoError::SessionUnreadable`] read as the session and the
//! re-sign-in action, never as a wrong key, a wrong region or a wrong model, and
//! [`SsoError::is_session_failure`] names them without reading the text. A path that only has the
//! rendered message — a turn's error frame — still recognises one through
//! [`is_session_failure_message`], whose stems are the messages' own first words.
//!
//! The two conversions below are the whole boundary: [`SsoError`] never reaches a caller as
//! itself. `to_llm_error` is for the request path (a credential could not be produced),
//! `to_service_error` for the settings surface (a person has an action to take). One message per
//! cause, and no two messages share wording (SC-015).

use crate::config::provider::AMAZON_BEDROCK_SSO_NAME;
use crate::llm::error::LlmError;
use crate::service::ServiceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsoError {
    /// The person refused the request in the browser (`access_denied`).
    SignInDenied,
    /// The code died before it was confirmed (`expired_token`, or the poll deadline passed).
    CodeExpired,
    /// The same code was confirmed twice: already used on the wire (`invalid_grant`) or already
    /// confirmed by this flow. Its own outcome, because re-polling cannot fix it.
    AlreadyConfirmed,
    /// The session ended on AWS, or its refresh token was refused (FR-016).
    SessionExpired,
    /// A session row exists but this workspace key cannot open it (FR-009f, SC-019).
    SessionUnreadable { profile: String },
    /// The signed-in identity is not assigned the chosen role in the chosen account.
    RoleNotAssigned { account_id: String, role_name: String },
    /// The role exists but may not perform the action the request needs.
    PermissionMissing { action: String },
    /// Bedrock model access for this model is not enabled in the account.
    ModelAccessNotEnabled { model: String },
    /// The ARN parses but is not one of the connection's saved profiles (FR-030).
    ProfileNotFound { arn: String },
    /// An ARN's region and the connection's Bedrock region disagree (FR-024).
    RegionMismatch {
        arn_region: String,
        connection_region: String,
    },
    /// AWS answered 429.
    Throttled,
    /// The endpoint, the network or a response shape (FR-006, §2). The region is named because
    /// that is the thing a person usually got wrong (FR-010).
    Unavailable { region: String, reason: String },
}

impl std::fmt::Display for SsoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignInDenied => write!(
                f,
                "the sign-in was denied in the browser, so nothing was saved; start it again to retry"
            ),
            Self::CodeExpired => write!(
                f,
                "the sign-in code expired before it was confirmed; start the sign-in again for a new code"
            ),
            Self::AlreadyConfirmed => write!(
                f,
                "this sign-in code was already confirmed; start a new sign-in for a fresh code"
            ),
            // FR-016, contracts §6: the session is the cause and signing in again is the
            // action. The connection is named so the person knows which form to open.
            Self::SessionExpired => write!(
                f,
                "{SESSION_ENDED}; sign in again to reconnect {AMAZON_BEDROCK_SSO_NAME:?}"
            ),
            Self::SessionUnreadable { profile } => write!(
                f,
                "the stored session for profile {profile:?} {SESSION_UNREADABLE}; sign in again \
                 (nothing was deleted)"
            ),
            Self::RoleNotAssigned {
                account_id,
                role_name,
            } => write!(
                f,
                "this identity is not assigned role {role_name:?} in account {account_id}; assign it \
                 in IAM Identity Center and sign in again"
            ),
            Self::PermissionMissing { action } => write!(
                f,
                "the chosen role is not allowed to {action}; add that permission to the role's policy"
            ),
            Self::ModelAccessNotEnabled { model } => write!(
                f,
                "model access for {model:?} is not enabled in this account; enable it in the Bedrock \
                 console"
            ),
            Self::ProfileNotFound { arn } => write!(
                f,
                "this connection has no inference profile {arn}; add it to the connection first"
            ),
            Self::RegionMismatch {
                arn_region,
                connection_region,
            } => write!(
                f,
                "the profile is in region {arn_region:?} but the connection's Bedrock region is \
                 {connection_region:?}; the two must match"
            ),
            Self::Throttled => write!(
                f,
                "AWS throttled the request; it can be retried in a moment"
            ),
            Self::Unavailable { region, reason } => write!(
                f,
                "AWS Identity Center in region {region:?} did not answer: {reason}"
            ),
        }
    }
}

impl std::error::Error for SsoError {}

/// The first words of the ended-session message (contracts §6). A caller that only has the
/// rendered text recognises the cause by this stem rather than by guessing (FR-016, FR-017).
pub const SESSION_ENDED: &str = "the AWS SSO session has ended";

/// The stem every unreadable-session message carries: the envelope, not the credentials, is what
/// failed (FR-009f, FR-016).
pub const SESSION_UNREADABLE: &str = "cannot be read with this workspace key";

/// Whether a rendered message is a session failure — the ended session or the unreadable one
/// (FR-016). The one way a path without the typed error (a turn's `Error` frame) can tell a
/// session cause from a key, a region or a model one.
pub fn is_session_failure_message(message: &str) -> bool {
    message.contains(SESSION_ENDED) || message.contains(SESSION_UNREADABLE)
}

impl SsoError {
    /// Whether this failure is about the **session** itself rather than a key, a region or a
    /// model (FR-016): the causes a person fixes by signing in again, and the ones a background
    /// path must report rather than retry (FR-017, FR-018).
    pub fn is_session_failure(&self) -> bool {
        matches!(self, Self::SessionExpired | Self::SessionUnreadable { .. })
    }

    /// A stable machine-readable code for the settings surface, one per cause (SC-015). The
    /// WebUI branches on this, never on the message.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SignInDenied => "sso_sign_in_denied",
            Self::CodeExpired => "sso_code_expired",
            Self::AlreadyConfirmed => "sso_code_already_confirmed",
            Self::SessionExpired => "sso_session_expired",
            Self::SessionUnreadable { .. } => "sso_session_unreadable",
            Self::RoleNotAssigned { .. } => "sso_role_not_assigned",
            Self::PermissionMissing { .. } => "sso_permission_missing",
            Self::ModelAccessNotEnabled { .. } => "sso_model_access_not_enabled",
            Self::ProfileNotFound { .. } => "sso_profile_not_found",
            Self::RegionMismatch { .. } => "sso_region_mismatch",
            Self::Throttled => "sso_throttled",
            Self::Unavailable { .. } => "sso_unavailable",
        }
    }

    /// The request path's view: a credential could not be produced, so the turn cannot run.
    /// Throttling keeps its status, an unreachable endpoint is a transport failure, and every
    /// other cause is a configuration/credential refusal (`LlmError::Config`).
    pub fn to_llm_error(&self) -> LlmError {
        match self {
            Self::Throttled => LlmError::Http {
                status: 429,
                retry_after: None,
                body: self.to_string(),
            },
            Self::Unavailable { .. } => LlmError::Transport(self.to_string()),
            _ => LlmError::Config(self.to_string()),
        }
    }

    /// The settings surface's view, with the status each cause deserves: a denial or a missing
    /// permission is `403`, an ended or unreadable session is `401`, a missing profile is `404`,
    /// throttling is `429`, an unreachable region is `503`, and a malformed input (a code past its
    /// life, an unassigned role, a region disagreement) is `400`.
    pub fn to_service_error(&self) -> ServiceError {
        match self {
            Self::SignInDenied | Self::PermissionMissing { .. } | Self::ModelAccessNotEnabled { .. } => {
                ServiceError::Forbidden(self.to_string())
            }
            Self::SessionExpired | Self::SessionUnreadable { .. } => {
                ServiceError::Unauthorized(self.to_string())
            }
            Self::ProfileNotFound { .. } => ServiceError::NotFound(self.to_string()),
            Self::Throttled => ServiceError::RateLimited(self.to_string()),
            Self::Unavailable { .. } => ServiceError::Unavailable(self.to_string()),
            Self::AlreadyConfirmed => ServiceError::Conflict {
                message: self.to_string(),
                code: Some(self.code().to_string()),
                data: None,
            },
            Self::CodeExpired | Self::RoleNotAssigned { .. } | Self::RegionMismatch { .. } => {
                ServiceError::Invalid {
                    status: 400,
                    message: self.to_string(),
                    code: Some(self.code().to_string()),
                }
            }
        }
    }
}

impl From<SsoError> for LlmError {
    fn from(value: SsoError) -> Self {
        value.to_llm_error()
    }
}

impl From<SsoError> for ServiceError {
    fn from(value: SsoError) -> Self {
        value.to_service_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every variant, built with values that would be readable if any Display printed its
    /// payload — the point of the test is that only the payload a person acts on appears.
    fn every_variant() -> Vec<SsoError> {
        vec![
            SsoError::SignInDenied,
            SsoError::CodeExpired,
            SsoError::AlreadyConfirmed,
            SsoError::SessionExpired,
            SsoError::SessionUnreadable {
                profile: "acme-bedrock".into(),
            },
            SsoError::RoleNotAssigned {
                account_id: "123456789012".into(),
                role_name: "BedrockOperator".into(),
            },
            SsoError::PermissionMissing {
                action: "invoke anthropic.claude-sonnet-4-5".into(),
            },
            SsoError::ModelAccessNotEnabled {
                model: "anthropic.claude-sonnet-4-5".into(),
            },
            SsoError::ProfileNotFound {
                arn: "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc".into(),
            },
            SsoError::RegionMismatch {
                arn_region: "us-east-1".into(),
                connection_region: "eu-west-1".into(),
            },
            SsoError::Throttled,
            SsoError::Unavailable {
                region: "us-east-1".into(),
                reason: "the endpoint answered HTTP 503".into(),
            },
        ]
    }

    #[test]
    fn no_variant_can_print_a_token_or_a_secret() {
        // Values a leaked message would carry if a variant had somewhere to put them.
        let poisons = [
            "access-token-value",
            "refresh-token-value",
            "client-secret-value",
            "device-code-value",
            "ASIAEXAMPLEKEYID",
            "session-token-value",
        ];
        for error in every_variant() {
            for rendered in [
                error.to_string(),
                format!("{error:?}"),
                error.to_llm_error().to_string(),
                error.to_service_error().message().to_string(),
            ] {
                for poison in poisons {
                    assert!(
                        !rendered.contains(poison),
                        "{error:?} rendered {poison:?}: {rendered}"
                    );
                }
                assert!(!rendered.is_empty());
            }
        }
    }

    #[test]
    fn every_cause_reads_differently_and_keeps_its_code() {
        let errors = every_variant();
        let mut seen: Vec<String> = Vec::new();
        for error in &errors {
            let text = error.to_string();
            assert!(
                !seen.iter().any(|other| other == &text),
                "two causes share a message: {text}"
            );
            seen.push(text);
        }

        let mut codes: Vec<&str> = errors.iter().map(SsoError::code).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), before, "two causes share a machine code");
    }

    #[test]
    fn each_cause_maps_to_the_status_a_person_can_act_on() {
        assert_eq!(SsoError::SignInDenied.to_service_error().status(), 403);
        assert_eq!(SsoError::CodeExpired.to_service_error().status(), 400);
        assert_eq!(SsoError::AlreadyConfirmed.to_service_error().status(), 409);
        assert_eq!(SsoError::SessionExpired.to_service_error().status(), 401);
        assert_eq!(
            SsoError::SessionUnreadable {
                profile: "acme-bedrock".into()
            }
            .to_service_error()
            .status(),
            401
        );
        assert_eq!(
            SsoError::ProfileNotFound {
                arn: "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc"
                    .into()
            }
            .to_service_error()
            .status(),
            404
        );
        assert_eq!(SsoError::Throttled.to_service_error().status(), 429);
        assert_eq!(
            SsoError::Unavailable {
                region: "us-east-1".into(),
                reason: "refused".into()
            }
            .to_service_error()
            .status(),
            503
        );
        // The request path keeps throttling retryable and everything else a config refusal.
        assert!(matches!(
            SsoError::Throttled.to_llm_error(),
            LlmError::Http { status: 429, .. }
        ));
        assert!(matches!(
            SsoError::SessionExpired.to_llm_error(),
            LlmError::Config(_)
        ));
        assert!(matches!(
            SsoError::Unavailable {
                region: "us-east-1".into(),
                reason: "refused".into()
            }
            .to_llm_error(),
            LlmError::Transport(_)
        ));
    }

    #[test]
    fn an_expired_session_points_at_signing_in_again() {
        // FR-016, contracts §6 — the exact wording, the connection by name so the person knows
        // which form to open.
        assert_eq!(
            SsoError::SessionExpired.to_string(),
            "the AWS SSO session has ended; sign in again to reconnect \"Amazon Bedrock (SSO)\""
        );
        let text = SsoError::SessionExpired.to_string();
        for wrong in ["key", "region", "model"] {
            assert!(!text.contains(wrong), "{text}");
        }
        assert!(SsoError::SessionExpired.is_session_failure());
        assert!(SsoError::SessionUnreadable {
            profile: "acme-bedrock".into()
        }
        .is_session_failure());
        // A wrong key, a wrong region and a wrong model keep their own variants (SC-015).
        for other in every_variant() {
            if !matches!(
                other,
                SsoError::SessionExpired | SsoError::SessionUnreadable { .. }
            ) {
                assert!(!other.is_session_failure(), "{other:?}");
            }
        }
    }

    #[test]
    fn every_session_failure_reads_as_a_session_and_offers_the_way_back() {
        let session_failures = [
            SsoError::SessionExpired,
            SsoError::SessionUnreadable {
                profile: "acme-bedrock".into(),
            },
        ];
        for error in &session_failures {
            // Every rendering a person or a log line can receive: the cause and the action are
            // there, and none of the three wrong causes is named.
            for rendered in [
                error.to_string(),
                error.to_llm_error().to_string(),
                error.to_service_error().message().to_string(),
            ] {
                assert!(rendered.contains("session"), "{error:?}: {rendered}");
                assert!(rendered.contains("sign in again"), "{error:?}: {rendered}");
                for wrong in ["API key", "region", "model"] {
                    assert!(
                        !rendered.contains(wrong),
                        "{error:?} is reported as a wrong {wrong}: {rendered}"
                    );
                }
                assert!(
                    is_session_failure_message(&rendered),
                    "{error:?} must be recognisable from its text: {rendered}"
                );
            }
        }

        // The text classifier names only a session failure: a refusal about a key, a region or a
        // model is not one, whatever the status it maps to.
        for other in every_variant() {
            if !other.is_session_failure() {
                assert!(
                    !is_session_failure_message(&other.to_string()),
                    "{other:?} was read as a session failure"
                );
            }
        }
        assert!(!is_session_failure_message("HTTP 401 Unauthorized: Incorrect API key"));
    }
}
