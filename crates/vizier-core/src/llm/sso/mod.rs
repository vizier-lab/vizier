//! Signing in to AWS IAM Identity Center and turning that into Bedrock role credentials
//! (`specs/044-bedrock-sso-inference-profiles`, research D3–D9).
//!
//! The flow lives under `llm/` because everything it produces is a credential for the provider
//! layer: [`client`] speaks the device flow to the SSO region's OIDC endpoints, [`portal`] reads
//! the accounts and roles the identity was assigned and exchanges them for role credentials,
//! [`session`] keeps the session in Vizier's own encrypted storage, and [`error`] names each
//! failure a person can act on. Nothing here touches a key file, a shell or the AWS CLI.
//!
//! # Entry points
//!
//! - [`begin_sign_in`] — register a client once and start a device flow (the pending panel).
//! - [`SsoClient`] — the wire calls on their own, for a re-sign-in with a stored registration.
//! - [`SsoSessions`] — load, persist and delete the session behind one profile.
//! - [`SsoPortal`] — accounts, roles and role credentials for a signed-in session.
//!
//! The regions are never interchangeable: every client is built from the connection's **SSO**
//! region, and the Bedrock region appears only where a request goes to Bedrock (FR-010).

use chrono::Utc;

pub mod client;
pub mod error;
pub mod failures;
pub mod portal;
pub mod session;

#[cfg(test)]
pub(crate) mod tests_support;

pub use client::{
    ClientRegistration, DeviceAuthorization, DeviceFlow, SsoClient, SsoEndpoints, TokenPoll,
    TokenSet,
};
pub use error::{SESSION_ENDED, SsoError, is_session_failure_message};
pub use failures::SessionFailures;
pub use portal::{DiscoveredAccounts, DiscoveredRoles, RoleCredentials, SsoAccount, SsoPortal, SsoRole};
pub use session::{
    Persisted, RENEW_BEFORE, SsoSessions, expires_at, has_expired, needs_renewal,
    state as session_state,
};

/// Start a sign-in: reuse the stored registration while it is still good, register one otherwise
/// (§2), then ask for a device code (§3). The returned flow shows the person a code and a URL and
/// keeps the device code to itself.
pub async fn begin_sign_in(
    client: SsoClient,
    registration: Option<ClientRegistration>,
    start_url: &str,
) -> Result<DeviceFlow, SsoError> {
    let registration = match registration {
        Some(registration) if registration.is_reusable(Utc::now()) => registration,
        _ => client.register_client().await?,
    };
    DeviceFlow::start(client, registration, start_url).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::sso::tests_support::{Stub, approved};

    #[tokio::test]
    async fn a_reusable_registration_is_not_made_again() {
        let stub = Stub::start(vec![approved()]).await;
        let client = SsoClient::at(reqwest::Client::new(), stub.base());
        let stored = ClientRegistration {
            client_id: "stored-client-id".into(),
            client_secret: "client-secret-value".into(),
            expires_at: None,
        };

        let flow = begin_sign_in(client.clone(), Some(stored.clone()), "https://acme/start")
            .await
            .unwrap();

        assert_eq!(flow.registration().client_id, "stored-client-id");
        assert_eq!(stub.path_count("/client/register"), 0);
        assert_eq!(stub.path_count("/device_authorization"), 1);
    }

    #[tokio::test]
    async fn a_missing_registration_is_made_once() {
        let stub = Stub::start(vec![approved()]).await;
        let client = SsoClient::at(reqwest::Client::new(), stub.base());

        let flow = begin_sign_in(client, None, "https://acme/start").await.unwrap();

        assert_eq!(flow.registration().client_id, "registered-client-id");
        assert_eq!(stub.path_count("/client/register"), 1);
    }

    #[test]
    fn both_endpoints_come_from_the_sso_region_and_never_from_the_bedrock_one() {
        let endpoints = SsoEndpoints::for_sso_region("eu-west-1");
        assert_eq!(endpoints.oidc, "https://oidc.eu-west-1.amazonaws.com");
        assert_eq!(endpoints.portal, "https://portal.sso.eu-west-1.amazonaws.com");
    }

    #[tokio::test]
    async fn an_unreachable_endpoint_is_unavailable_rather_than_a_panic() {
        // A port nothing listens on: refused once, retried once, then reported.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let client = SsoClient::at(reqwest::Client::new(), &base);

        match client.register_client().await.unwrap_err() {
            SsoError::Unavailable { reason, .. } => {
                assert!(reason.contains("failed"), "{reason}")
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }
}
