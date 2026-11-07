//! The session failures a surface reports until a sign-in clears them (FR-018).
//!
//! An ended session is not an agent crash. The turn that meets it fails with the session's own
//! wording (FR-016), the agent stays available, and its health keeps naming the cause until a new
//! sign-in succeeds (FR-018). What is kept here is that one sentence per connection, in memory:
//! it describes what this process just saw, never a stored state, and it carries no token — only
//! the message [`SsoError`](super::SsoError) already renders without one (FR-009b, FR-035).

use std::collections::HashMap;

use parking_lot::Mutex;

use super::error::is_session_failure_message;
use crate::config::provider::ProviderId;

/// The last session failure each connection reported, by provider id.
///
/// Nothing is ever written to disk: the stored session row and its `sso_state` are the durable
/// truth, and this is the extra fact a row cannot show — that a request was actually refused
/// (a session revoked in AWS, for instance, while the token still looks unexpired).
#[derive(Debug, Default)]
pub struct SessionFailures {
    failures: Mutex<HashMap<ProviderId, String>>,
}

impl SessionFailures {
    /// Record `message` as what `provider` is currently in, and say whether it was recorded.
    ///
    /// Only a session failure is recorded (FR-016): a wrong key, a wrong region or a wrong model
    /// must never make an agent look like a session problem, so the caller cannot store one here
    /// by accident.
    pub fn record(&self, provider: &ProviderId, message: &str) -> bool {
        if !is_session_failure_message(message) {
            return false;
        }
        self.failures
            .lock()
            .insert(provider.clone(), message.to_string());
        true
    }

    /// Forget `provider`'s failure: a sign-in succeeded, or a renewal proved the session works.
    pub fn clear(&self, provider: &ProviderId) {
        self.failures.lock().remove(provider);
    }

    /// What `provider` last failed with, while it is still in that failure.
    pub fn get(&self, provider: &ProviderId) -> Option<String> {
        self.failures.lock().get(provider).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::sso::{SsoError, SESSION_ENDED};

    const OTHER: ProviderId = ProviderId::OPENAI;

    fn sso() -> ProviderId {
        ProviderId::AMAZON_BEDROCK_SSO
    }

    #[test]
    fn a_session_failure_is_held_until_it_is_cleared() {
        let failures = SessionFailures::default();
        assert!(failures.record(&sso(), &SsoError::SessionExpired.to_string()));
        assert_eq!(
            failures.get(&sso()).as_deref(),
            Some(SsoError::SessionExpired.to_string().as_str())
        );

        failures.clear(&sso());
        assert_eq!(failures.get(&sso()), None);
        // Clearing one that already cleared is not an error.
        failures.clear(&sso());
    }

    #[test]
    fn a_failure_that_is_not_about_the_session_is_never_recorded() {
        let failures = SessionFailures::default();
        assert!(!failures.record(
            &sso(),
            "HTTP 401 Unauthorized: Incorrect API key provided"
        ));
        assert!(!failures.record(&sso(), "invalid AWS region \"eu-west\""));
        assert_eq!(failures.get(&sso()), None);
    }

    #[test]
    fn each_connection_holds_its_own_failure() {
        let failures = SessionFailures::default();
        failures.record(&sso(), &SsoError::SessionExpired.to_string());
        assert!(failures
            .get(&OTHER)
            .is_none_or(|message| !message.contains(SESSION_ENDED)));

        failures.record(&OTHER, &SsoError::SessionExpired.to_string());
        failures.clear(&OTHER);
        assert!(
            failures.get(&sso()).is_some(),
            "clearing one connection leaves the other alone"
        );
    }
}
