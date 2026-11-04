//! The stored Bedrock SSO session: one row per profile, encrypted (spec 044, research D3).
//!
//! The trait exists so the flow in `llm/sso/` depends on a port rather than on `rusqlite`, and
//! so the compare-and-swap the two-process case needs (FR-014) has exactly one implementation.

use anyhow::Result;

use crate::schema::SsoSessionSecret;

/// The outcome of a compare-and-swap write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoWrite {
    /// The row was written and now carries this revision.
    Written(u64),
    /// Another writer stored a newer revision first; nothing was written. The caller re-reads
    /// and uses the winner's session (research D5, FR-014).
    Conflict,
}

/// One session row per profile (data-model "SSO session secret").
///
/// Every value crossing this boundary is the decrypted [`SsoSessionSecret`]: the envelope is the
/// storage's own business, produced and opened through
/// [`protect_sso_session`](crate::encryption::fields::protect_sso_session) so the AAD (and
/// therefore the binding to the profile) has one implementation.
#[async_trait::async_trait]
pub trait SsoSessionStorage {
    /// The session stored for `profile`, decrypted. `Ok(None)` when there is no row; an error
    /// when a row exists but this workspace key cannot read it (FR-009f) — never a silent
    /// `None`, which would report "signed out" for a key problem.
    async fn get_sso_session(&self, profile: &str) -> Result<Option<SsoSessionSecret>>;

    /// Write `secret` only if the stored row is still at `expected_revision` (`None` = "no row
    /// yet", so this is the create). The store owns the revision: the row is written one
    /// revision higher than `expected_revision` (`1` for a create), whatever `secret.revision`
    /// says, so two writers cannot both claim the same number.
    async fn put_sso_session(
        &self,
        profile: &str,
        secret: &SsoSessionSecret,
        expected_revision: Option<u64>,
    ) -> Result<SsoWrite>;

    /// Remove the row for `profile`. Signing out of a connection that is already signed out is
    /// not an error (FR-009c).
    async fn delete_sso_session(&self, profile: &str) -> Result<()>;
}
