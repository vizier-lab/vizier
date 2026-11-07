//! The stored session the flow leaves behind: persist, load, expiry and the compare-and-swap write
//! (FR-009, FR-014, `contracts/bedrock-sso-wire.md` §5).
//!
//! The encryption is not re-implemented here: [`SsoSessionStorage`] takes and returns the plain
//! [`SsoSessionSecret`] and owns the envelope through
//! [`protect_sso_session`](crate::encryption::fields::protect_sso_session), so the AAD is bound to
//! the profile in exactly one place (FR-009a). This layer adds what the flow needs on top of that
//! port: one value type for "what is now stored", the conflict rule of §5, the five-minute renewal
//! threshold, and [`SsoSessions::renew`] — the one definition of what a renewal costs on the wire.



use crate::time::Timestamp;
use super::client::{ClientRegistration, SsoClient, TokenPoll, TokenSet};
use super::error::SsoError;
use super::portal::{RoleCredentials, SsoPortal};
use crate::schema::{SsoSessionSecret, SsoSessionState};
use crate::storage::sso_session::{SsoSessionStorage, SsoWrite};

/// What a write left in storage.
///
/// A second process may have renewed the same session between our read and our write; §5 says the
/// winner's session is used, never overwritten. So every write answers with the session that is
/// actually stored, and whether that was ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persisted {
    /// The session now stored: ours at its new revision, or the one the winner wrote.
    pub session: SsoSessionSecret,
    /// `false` when another writer got there first.
    pub written: bool,
}

impl Persisted {
    /// When the stored session's access token expires — what the connection row shows.
    pub fn expires_at(&self) -> Timestamp {
        self.session.token_expires_at
    }
}

/// One connection's stored session, over the storage port (data-model "SSO session secret").
#[derive(Debug)]
pub struct SsoSessions<'a, S: SsoSessionStorage + ?Sized> {
    storage: &'a S,
}

impl<'a, S: SsoSessionStorage + ?Sized> SsoSessions<'a, S> {
    pub fn new(storage: &'a S) -> Self {
        Self { storage }
    }

    /// The port, for a test that asserts what was (or was not) written.
    pub fn storage(&self) -> &S {
        self.storage
    }

    /// The session stored for `profile`, or `None` when there is no row.
    ///
    /// A row that exists but cannot be opened is [`SsoError::SessionUnreadable`], never `None`:
    /// reporting "signed out" for a key problem would hide the one action that fixes it, and the
    /// row is left where it is (FR-009f, SC-019).
    pub async fn load(&self, profile: &str) -> Result<Option<SsoSessionSecret>, SsoError> {
        match self.storage.get_sso_session(profile).await {
            Ok(session) => Ok(session),
            Err(error) => {
                // The cause is a cipher or storage fault about an *envelope*; the payload never
                // reaches a log line (FR-035).
                tracing::warn!(profile, error = %error, "the stored sso session cannot be read");
                Err(SsoError::SessionUnreadable {
                    profile: profile.to_string(),
                })
            }
        }
    }

    /// The session a completed sign-in produces: the tokens plus the registration that minted
    /// them (§2, §3). The store owns the revision number.
    ///
    /// A re-sign-in replaces the row a previous session left (FR-008: an expired session asks
    /// for a code and nothing else). A row this workspace key cannot open is replaced too: the
    /// person just completed a new sign-in, so the new session is the one that can be used, and
    /// the connection itself is untouched (FR-009f).
    pub async fn store_sign_in(
        &self,
        profile: &str,
        registration: &ClientRegistration,
        tokens: TokenSet,
    ) -> Result<Persisted, SsoError> {
        let secret = SsoSessionSecret {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_expires_at: tokens.expires_at,
            client_id: registration.client_id.clone(),
            client_secret: registration.client_secret.clone(),
            client_secret_expires_at: registration.expires_at,
            issued_at: crate::time::now(),
            revision: 0,
        };
        let expected = match self.load(profile).await {
            Ok(session) => session.map(|session| session.revision),
            Err(_) => {
                self.delete(profile).await?;
                None
            }
        };
        self.write(profile, secret, expected).await
    }

    /// The session with an access token the portal will accept: refresh it once through the same
    /// `/token` when it has expired, and write the renewal back with the compare-and-swap of §5.
    ///
    /// No refresh token, or a refresh the endpoint refuses, is [`SsoError::SessionExpired`]
    /// (FR-016) — the one cause a person acts on by signing in again.
    pub async fn access_token(
        &self,
        client: &SsoClient,
        profile: &str,
        mut session: SsoSessionSecret,
    ) -> Result<SsoSessionSecret, SsoError> {
        if !has_expired(&session, crate::time::now()) {
            return Ok(session);
        }
        let Some(refresh_token) = session.refresh_token.clone() else {
            return Err(SsoError::SessionExpired);
        };
        tracing::debug!(profile, "the stored access token has expired; refreshing it");
        let registration = ClientRegistration {
            client_id: session.client_id.clone(),
            client_secret: session.client_secret.clone(),
            expires_at: session.client_secret_expires_at,
        };
        match client
            .refresh_access_token(&registration, &refresh_token)
            .await?
        {
            TokenPoll::Approved(tokens) => {
                session.access_token = tokens.access_token;
                if tokens.refresh_token.is_some() {
                    session.refresh_token = tokens.refresh_token;
                }
                session.token_expires_at = tokens.expires_at;
                session.issued_at = crate::time::now();
                let revision = session.revision;
                let persisted = self.store_renewal(profile, session, revision).await?;
                tracing::debug!(
                    profile,
                    revision = persisted.session.revision,
                    written = persisted.written,
                    "the refreshed access token was kept"
                );
                Ok(persisted.session)
            }
            // `authorization_pending`/`slow_down` on a refresh grant, a denial, or a token
            // already used: none of them leaves a usable session behind.
            _ => Err(SsoError::SessionExpired),
        }
    }

    /// One renewal for one connection (FR-013, contract `bedrock-sso-wire.md` §5): a
    /// `CreateToken` **only** when the stored access token is past its expiry, then exactly one
    /// `GetRoleCredentials` — never two of either, whatever made the caller renew.
    ///
    /// The token half is [`SsoSessions::access_token`], so a refreshed access token is written back
    /// with the compare-and-swap of §5 rather than kept in memory. Whether a renewal is needed at
    /// all is [`needs_renewal`], and the serialization of concurrent callers is the connection's
    /// credential (FR-014), not this layer.
    pub async fn renew(
        &self,
        client: &SsoClient,
        profile: &str,
        session: SsoSessionSecret,
        account_id: &str,
        role_name: &str,
    ) -> Result<RoleCredentials, SsoError> {
        let session = self.access_token(client, profile, session).await?;
        SsoPortal::new(client.clone())
            .role_credentials(&session.access_token, account_id, role_name)
            .await
    }

    /// Write a renewed `secret`: only while the row is still at `expected_revision` (§5).
    pub async fn store_renewal(
        &self,
        profile: &str,
        secret: SsoSessionSecret,
        expected_revision: u64,
    ) -> Result<Persisted, SsoError> {
        self.write(profile, secret, Some(expected_revision)).await
    }

    /// Remove the row. Signing out of a session that is already gone is not an error (FR-009c).
    pub async fn delete(&self, profile: &str) -> Result<(), SsoError> {
        self.storage
            .delete_sso_session(profile)
            .await
            .map_err(|error| {
                tracing::warn!(profile, error = %error, "the stored sso session cannot be deleted");
                SsoError::SessionUnreadable {
                    profile: profile.to_string(),
                }
            })
    }

    /// The one compare-and-swap (§5). `expected` is `None` for a create.
    async fn write(
        &self,
        profile: &str,
        secret: SsoSessionSecret,
        expected: Option<u64>,
    ) -> Result<Persisted, SsoError> {
        let outcome = self
            .storage
            .put_sso_session(profile, &secret, expected)
            .await
            .map_err(|error| {
                tracing::warn!(profile, error = %error, "the sso session cannot be written");
                SsoError::SessionUnreadable {
                    profile: profile.to_string(),
                }
            })?;
        match outcome {
            SsoWrite::Written(revision) => {
                tracing::debug!(profile, revision, "the sso session was written");
                let mut stored = secret;
                stored.revision = revision;
                Ok(Persisted {
                    session: stored,
                    written: true,
                })
            }
            SsoWrite::Conflict => {
                tracing::debug!(
                    profile,
                    "another writer stored the sso session first; using theirs"
                );
                match self.load(profile).await? {
                    // The winner's session is the one to use; ours is discarded, not forced in.
                    Some(winner) => Ok(Persisted {
                        session: winner,
                        written: false,
                    }),
                    // The row was deleted between our read and our write: the session is gone,
                    // which is what re-signing in fixes.
                    None => Err(SsoError::SessionExpired),
                }
            }
        }
    }
}

/// When the stored session's access token expires; `None` when there is no session.
pub fn expires_at(session: Option<&SsoSessionSecret>) -> Option<Timestamp> {
    session.map(|session| session.token_expires_at)
}

/// Whether the access token is past its expiry at `now`.
pub fn has_expired(session: &SsoSessionSecret, now: Timestamp) -> bool {
    session.token_expires_at <= now
}

/// How long before expiry a cached role credential is renewed: one renewal every five minutes at
/// most, and none at all while the cache is good (FR-013, data-model "role credentials").
pub const RENEW_BEFORE: chrono::TimeDelta = chrono::TimeDelta::minutes(5);

/// Whether cached role credentials must be renewed at `now` — the single definition of the
/// five-minute threshold (FR-013, FR-009d).
pub fn needs_renewal(credentials: &RoleCredentials, now: Timestamp) -> bool {
    credentials.expires_at - RENEW_BEFORE <= now
}

/// The state the settings row renders for a stored session (data-model state machine).
///
/// [`SsoSessionState::Unreadable`] is not produced here: it is the answer to a failed
/// [`SsoSessions::load`], which is a different call and a different message.
pub fn state(session: Option<&SsoSessionSecret>, now: Timestamp) -> SsoSessionState {
    match session {
        None => SsoSessionState::Configured,
        Some(session) if has_expired(session, now) => SsoSessionState::Expired,
        Some(_) => SsoSessionState::SignedIn,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;

    use super::*;
    use crate::llm::sso::tests_support::{MemSessions, Reply, Stub};

    const PROFILE: &str = "acme-bedrock";
    const ACCOUNT: &str = "123456789012";
    const ROLE: &str = "BedrockOperator";

    fn registration() -> ClientRegistration {
        ClientRegistration {
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            expires_at: None,
        }
    }

    fn tokens(access_token: &str) -> TokenSet {
        TokenSet {
            access_token: access_token.to_string(),
            refresh_token: Some("refresh-token-value".into()),
            expires_at: crate::time::now() + ChronoDuration::hours(1),
        }
    }

    fn tokens_expiring_in(seconds: i64) -> TokenSet {
        TokenSet {
            access_token: "access-token-value".to_string(),
            refresh_token: Some("refresh-token-value".into()),
            expires_at: crate::time::now() + ChronoDuration::seconds(seconds),
        }
    }

    /// §4.3's answer, with the key and the life the test wants.
    fn credentials_reply(access_key_id: &str, expires_in_seconds: i64) -> Reply {
        Reply::ok(serde_json::json!({
            "roleCredentials": {
                "accessKeyId": access_key_id,
                "secretAccessKey": "secret-access-value",
                "sessionToken": "session-token-value",
                "expiration": (crate::time::now() + ChronoDuration::seconds(expires_in_seconds))
                    .timestamp_millis(),
            }
        }))
    }

    #[tokio::test]
    async fn a_signed_in_session_reads_back_at_its_first_revision() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        let persisted = sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();

        assert!(persisted.written);
        assert_eq!(persisted.session.revision, 1);
        let loaded = sessions.load(PROFILE).await.unwrap().expect("a session");
        assert_eq!(loaded.access_token, "access-token-value");
        assert_eq!(loaded.client_secret, "client-secret-value");
        assert_eq!(loaded.token_expires_at, persisted.session.token_expires_at);
    }

    #[tokio::test]
    async fn a_lost_race_uses_the_winners_session_instead_of_overwriting_it() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        let first = sessions
            .store_sign_in(PROFILE, &registration(), tokens("winner-token"))
            .await
            .unwrap();

        // A second process renews from the revision it read before the winner wrote.
        let mut stale = first.session.clone();
        stale.access_token = "stale-token".into();
        let outcome = sessions.store_renewal(PROFILE, stale, 0).await.unwrap();

        assert!(!outcome.written);
        assert_eq!(outcome.session.access_token, "winner-token");
        assert_eq!(
            sessions.load(PROFILE).await.unwrap().unwrap().access_token,
            "winner-token"
        );
    }

    #[tokio::test]
    async fn a_renewal_at_the_current_revision_is_written_one_higher() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        let first = sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();

        let mut renewed = first.session.clone();
        renewed.access_token = "renewed-token".into();
        let outcome = sessions
            .store_renewal(PROFILE, renewed, first.session.revision)
            .await
            .unwrap();

        assert!(outcome.written);
        assert_eq!(outcome.session.revision, 2);
        assert_eq!(sessions.storage().len(), 1);
    }

    #[tokio::test]
    async fn an_unreadable_row_says_so_and_is_left_alone() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();
        storage.mark_unreadable(PROFILE);

        let error = sessions.load(PROFILE).await.unwrap_err();

        assert_eq!(
            error,
            SsoError::SessionUnreadable {
                profile: PROFILE.to_string()
            }
        );
        // Nothing was deleted (SC-019).
        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn signing_out_deletes_the_row_and_a_second_sign_out_is_not_an_error() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();

        sessions.delete(PROFILE).await.unwrap();
        sessions.delete(PROFILE).await.unwrap();

        assert_eq!(storage.len(), 0);
    }

    #[tokio::test]
    async fn expiry_and_state_follow_the_access_token() {
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        let now = crate::time::now();

        assert_eq!(state(None, now), SsoSessionState::Configured);
        assert_eq!(expires_at(None), None);

        sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();
        let session = sessions.load(PROFILE).await.unwrap().unwrap();
        assert_eq!(state(Some(&session), now), SsoSessionState::SignedIn);
        assert_eq!(
            expires_at(Some(&session)),
            Some(session.token_expires_at)
        );
        assert!(!has_expired(&session, now));
        assert_eq!(
            state(Some(&session), session.token_expires_at),
            SsoSessionState::Expired
        );
    }

    #[tokio::test]
    async fn a_renewal_under_a_live_access_token_is_one_credentials_call() {
        let stub = Stub::start_replies(vec![(
            "/federation/credentials",
            vec![credentials_reply("ASIAFIRSTKEY", 3600)],
        )])
        .await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        let signed_in = sessions
            .store_sign_in(PROFILE, &registration(), tokens("access-token-value"))
            .await
            .unwrap();
        let client = SsoClient::at(reqwest::Client::new(), stub.base());

        let credentials = sessions
            .renew(&client, PROFILE, signed_in.session, ACCOUNT, ROLE)
            .await
            .unwrap();

        assert_eq!(credentials.access_key_id, "ASIAFIRSTKEY");
        assert_eq!(
            stub.path_count("/token"),
            0,
            "a live access token is not refreshed (FR-013)"
        );
        assert_eq!(stub.path_count("/federation/credentials"), 1);
        // An unexpired token leaves the stored row exactly where it was.
        assert_eq!(sessions.load(PROFILE).await.unwrap().unwrap().revision, 1);
    }

    #[tokio::test]
    async fn an_expired_access_token_costs_one_create_token_before_the_credentials() {
        let stub = Stub::start_replies(vec![
            (
                "/token",
                vec![Reply::ok(serde_json::json!({
                    "accessToken": "renewed-access-token",
                    "refreshToken": "renewed-refresh-token",
                    "expiresIn": 3600,
                }))],
            ),
            (
                "/federation/credentials",
                vec![credentials_reply("ASIASECONDKEY", 3600)],
            ),
        ])
        .await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);
        let signed_in = sessions
            .store_sign_in(PROFILE, &registration(), tokens_expiring_in(-60))
            .await
            .unwrap();
        let client = SsoClient::at(reqwest::Client::new(), stub.base());

        let credentials = sessions
            .renew(&client, PROFILE, signed_in.session, ACCOUNT, ROLE)
            .await
            .unwrap();

        assert_eq!(credentials.access_key_id, "ASIASECONDKEY");
        assert_eq!(stub.path_count("/token"), 1, "one CreateToken, no more");
        // The token half is the refresh grant of §5, as JSON with camelCase keys (044).
        let token_request = stub
            .seen()
            .into_iter()
            .find(|(path, _)| path == "/token")
            .map(|(_, body)| body)
            .expect("one CreateToken");
        let sent: serde_json::Value =
            serde_json::from_str(&token_request).expect("the token request is JSON");
        assert_eq!(
            sent["grantType"], "refresh_token",
            "the token half is the refresh grant of §5"
        );
        assert_eq!(sent["refreshToken"], "refresh-token-value");
        assert!(sent.get("grant_type").is_none(), "{token_request}");
        assert_eq!(stub.path_count("/federation/credentials"), 1);
        // The refreshed token was written back through the compare-and-swap of §5.
        let stored = sessions.load(PROFILE).await.unwrap().unwrap();
        assert_eq!(stored.access_token, "renewed-access-token");
        assert_eq!(stored.revision, 2);
    }

    #[test]
    fn a_renewal_starts_five_minutes_before_expiry() {
        let now = crate::time::now();
        let mut credentials = RoleCredentials {
            access_key_id: "ASIAEXAMPLEKEYID".into(),
            secret_access_key: "secret-access-value".into(),
            session_token: "session-token-value".into(),
            expires_at: now + ChronoDuration::minutes(5) + ChronoDuration::seconds(1),
        };
        assert!(!needs_renewal(&credentials, now), "five minutes and a second left");

        credentials.expires_at = now + ChronoDuration::minutes(5);
        assert!(needs_renewal(&credentials, now), "the threshold itself renews");

        credentials.expires_at = now - ChronoDuration::seconds(1);
        assert!(needs_renewal(&credentials, now), "an expired credential renews");
    }
}
