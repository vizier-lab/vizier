//! Shared chat snapshots (`specs/063-shared-chat/data-model.md`): a frozen, read-only copy of one
//! conversation, who may open it, and the revoked state. Storage is unaware of permissions; the
//! service layer owns them. The snapshot text is stored and returned byte for byte.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{auth::Principal, storage::VizierStorage};

/// Who may open a share. Fixed at creation (there is no update).
///
/// Nothing outside [`Audience::allows`] and [`Audience::cache_policy`] branches on the variant
/// (constitution II); the name used on the wire and in storage comes from one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum Audience {
    Specific,
    SignedIn,
    Public,
}

/// What a viewer may do with a share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allowed,
    /// There is no session; signing in may change the answer.
    NeedSignIn,
    /// Signed in and not allowed.
    Denied,
}

/// How a response may be cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Anyone gets the same body: a shared cache may keep it for five minutes (R7).
    Shared,
    /// Depends on who asks: never kept.
    Private,
}

impl CachePolicy {
    /// The `Cache-Control` value of a successful read.
    pub fn cache_control(self) -> &'static str {
        match self {
            Self::Shared => "public, max-age=300, s-maxage=300",
            Self::Private => "private, no-store",
        }
    }

    /// Whether the response varies by credentials.
    pub fn varies_by_credentials(self) -> bool {
        self == Self::Private
    }
}

const NAMES: [(Audience, &str); 3] = [
    (Audience::Specific, "specific"),
    (Audience::SignedIn, "signed_in"),
    (Audience::Public, "public"),
];

impl Audience {
    /// The name stored and sent.
    pub fn name(self) -> &'static str {
        NAMES
            .iter()
            .find(|(audience, _)| *audience == self)
            .map_or("public", |(_, name)| name)
    }

    /// The audience called `name`, if it is one.
    pub fn from_name(name: &str) -> Option<Self> {
        NAMES.iter().find(|(_, n)| *n == name).map(|(a, _)| *a)
    }

    /// Whether `viewer` may open a share naming `named` (user ids; the service includes the
    /// author). A person who no longer exists cannot be a `viewer`, and is never matched by
    /// someone else's id, so a deleted or removed named person is refused like anyone unnamed.
    pub fn allows(&self, viewer: Option<&Principal>, named: &[String]) -> Access {
        match (self, viewer) {
            (Self::Public, _) => Access::Allowed,
            (_, None) => Access::NeedSignIn,
            (Self::SignedIn, Some(_)) => Access::Allowed,
            (Self::Specific, Some(who)) => {
                if named.iter().any(|id| *id == who.user_id) {
                    Access::Allowed
                } else {
                    Access::Denied
                }
            }
        }
    }

    /// Whether a share of this audience names people (and must then list 1 to 50 of them).
    pub fn takes_people(&self) -> bool {
        matches!(self, Self::Specific)
    }

    /// Only a public body is the same for everyone, so only it may be cached.
    pub fn cache_policy(&self) -> CachePolicy {
        match self {
            Self::Public => CachePolicy::Shared,
            Self::Specific | Self::SignedIn => CachePolicy::Private,
        }
    }
}

/// Everything about a share except its content and its named people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRecord {
    pub share_id: String,
    pub creator_id: String,
    pub agent_id: String,
    pub topic_id: String,
    pub title: String,
    pub audience: Audience,
    pub include_trail: bool,
    pub format_version: i64,
    pub size_bytes: i64,
    /// Unix milliseconds.
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    pub revoked_by: Option<String>,
    /// The removal report as sent (counts by kind), shown in the author's list.
    pub removals_json: String,
}

/// Composite keyset pagination cursor combining `(created_at, share_id)` to avoid skipping items
/// with identical timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ShareCursor {
    pub created_at: i64,
    pub share_id: String,
}

impl ShareCursor {
    pub fn new(created_at: i64, share_id: impl Into<String>) -> Self {
        Self {
            created_at,
            share_id: share_id.into(),
        }
    }

    pub fn encode(&self) -> String {
        format!("{}:{}", self.created_at, self.share_id)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if let Some((ts_str, id)) = s.split_once(':') {
            if let Ok(ts) = ts_str.parse::<i64>() {
                return Some(Self::new(ts, id));
            }
        }
        if let Ok(cursor) = serde_json::from_str::<Self>(s) {
            return Some(cursor);
        }
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(s) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                if let Some(cursor) = Self::parse(&decoded) {
                    return Some(cursor);
                }
            }
        }
        if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                if let Some(cursor) = Self::parse(&decoded) {
                    return Some(cursor);
                }
            }
        }
        if let Ok(ts) = s.parse::<i64>() {
            return Some(Self::new(ts, String::new()));
        }
        None
    }
}

/// A share with its stored document and named people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareContent {
    pub record: ShareRecord,
    /// The document exactly as received.
    pub snapshot_json: String,
    /// Named people (`Audience::Specific`); empty otherwise.
    pub people: Vec<String>,
}

/// A listed share: metadata only, never the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareSummary {
    pub record: ShareRecord,
    pub people_count: usize,
    /// Whether the person the list was made for is among the named people. Only filled by
    /// [`ShareStorage::list_all_shares`]; the names themselves are never returned.
    pub viewer_named: bool,
}

/// What an insert did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    /// The id exists already; the caller makes a new one.
    IdTaken,
    /// The author already has the maximum of active shares.
    CapReached,
    /// The author's live snapshots plus this one would exceed the byte budget (FR-047).
    QuotaExceeded,
}

#[async_trait::async_trait]
pub trait ShareStorage {
    /// Insert the row and its audience rows in one transaction, checking the author's active
    /// count and live byte total inside it so two parallel creates cannot both pass either cap.
    async fn insert_share(
        &self,
        share: &ShareContent,
        max_active: usize,
        max_live_bytes: i64,
    ) -> Result<InsertOutcome>;

    /// The summed `size_bytes` of an author's live (not revoked) shares: the FR-047 total.
    async fn live_bytes_by_creator(&self, creator_id: &str) -> Result<i64>;

    /// One share, revoked or not, with its content and named people.
    async fn get_share(&self, share_id: &str) -> Result<Option<ShareContent>>;

    /// An author's shares, newest first, keyset-paged by `before` (composite `created_at, share_id`). No content.
    async fn list_shares_by_creator(
        &self,
        creator_id: &str,
        before: Option<ShareCursor>,
        limit: usize,
        active_only: bool,
    ) -> Result<Vec<ShareSummary>>;

    /// Every share's metadata, newest first. No content, no names; `viewer_id` only decides
    /// `ShareSummary::viewer_named`.
    async fn list_all_shares(
        &self,
        viewer_id: &str,
        before: Option<ShareCursor>,
        limit: usize,
        active_only: bool,
    ) -> Result<Vec<ShareSummary>>;

    /// Revoke an active share: set `revoked_at` and delete the snapshot text (the row stays so
    /// that the id keeps answering like any unavailable one). `true` only when this call
    /// revoked it.
    async fn revoke_share(&self, share_id: &str, by: &str, at: i64) -> Result<bool>;

    async fn count_active_by_creator(&self, creator_id: &str) -> Result<usize>;

    /// Enabled people whose username starts with `prefix` (case-insensitive), by username,
    /// as `(user_id, username)`. Nothing else about them is read.
    async fn search_people(&self, prefix: &str, limit: usize) -> Result<Vec<(String, String)>>;

    /// Delete an author's shares, and the person's place in other shares' audiences (the
    /// account is gone). Returns how many shares were deleted.
    async fn delete_shares_by_creator(&self, creator_id: &str) -> Result<usize>;

    /// Revoke all active shares belonging to a topic/session (cascading revoke on chat deletion).
    async fn revoke_shares_by_topic(&self, agent_id: &str, topic_id: &str, by: &str, at: i64) -> Result<usize>;

    /// Revoke all active shares belonging to an agent (cascading revoke on agent deletion).
    async fn revoke_shares_by_agent(&self, agent_id: &str, by: &str, at: i64) -> Result<usize>;
}

#[async_trait::async_trait]
impl ShareStorage for VizierStorage {
    async fn insert_share(
        &self,
        share: &ShareContent,
        max_active: usize,
        max_live_bytes: i64,
    ) -> Result<InsertOutcome> {
        self.0.insert_share(share, max_active, max_live_bytes).await
    }

    async fn live_bytes_by_creator(&self, creator_id: &str) -> Result<i64> {
        self.0.live_bytes_by_creator(creator_id).await
    }

    async fn get_share(&self, share_id: &str) -> Result<Option<ShareContent>> {
        self.0.get_share(share_id).await
    }

    async fn list_shares_by_creator(
        &self,
        creator_id: &str,
        before: Option<ShareCursor>,
        limit: usize,
        active_only: bool,
    ) -> Result<Vec<ShareSummary>> {
        self.0.list_shares_by_creator(creator_id, before, limit, active_only).await
    }

    async fn list_all_shares(
        &self,
        viewer_id: &str,
        before: Option<ShareCursor>,
        limit: usize,
        active_only: bool,
    ) -> Result<Vec<ShareSummary>> {
        self.0.list_all_shares(viewer_id, before, limit, active_only).await
    }

    async fn revoke_share(&self, share_id: &str, by: &str, at: i64) -> Result<bool> {
        self.0.revoke_share(share_id, by, at).await
    }

    async fn count_active_by_creator(&self, creator_id: &str) -> Result<usize> {
        self.0.count_active_by_creator(creator_id).await
    }

    async fn search_people(&self, prefix: &str, limit: usize) -> Result<Vec<(String, String)>> {
        self.0.search_people(prefix, limit).await
    }

    async fn delete_shares_by_creator(&self, creator_id: &str) -> Result<usize> {
        self.0.delete_shares_by_creator(creator_id).await
    }

    async fn revoke_shares_by_topic(&self, agent_id: &str, topic_id: &str, by: &str, at: i64) -> Result<usize> {
        self.0.revoke_shares_by_topic(agent_id, topic_id, by, at).await
    }

    async fn revoke_shares_by_agent(&self, agent_id: &str, by: &str, at: i64) -> Result<usize> {
        self.0.revoke_shares_by_agent(agent_id, by, at).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AuthMethod, storage::user::Role};

    fn who(id: &str) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: Role {
                role_id: "r".into(),
                name: "r".into(),
                permissions: vec![],
                is_system: false,
                created_at: crate::time::now(),
            },
            permissions: vec![],
            method: AuthMethod::Jwt,
        }
    }

    /// Audience × viewer: the author (always in the list passed in), a named person, another
    /// signed-in person, nobody.
    #[test]
    fn allows_over_the_whole_table() {
        let named = vec!["author".to_string(), "bea".to_string()];
        let author = who("author");
        let bea = who("bea");
        let other = who("other");
        use Access::*;
        let table = [
            (Audience::Specific, [Allowed, Allowed, Denied, NeedSignIn]),
            (Audience::SignedIn, [Allowed, Allowed, Allowed, NeedSignIn]),
            (Audience::Public, [Allowed, Allowed, Allowed, Allowed]),
        ];
        for (audience, expect) in table {
            let got = [
                audience.allows(Some(&author), &named),
                audience.allows(Some(&bea), &named),
                audience.allows(Some(&other), &named),
                audience.allows(None, &named),
            ];
            assert_eq!(got, expect, "{audience:?}");
        }
    }

    /// A named person who is later disabled or deleted can no longer sign in, so they never
    /// arrive as a viewer; the id left in the list opens nothing for anyone else.
    #[test]
    fn a_removed_named_person_opens_nothing() {
        let named = vec!["author".to_string(), "gone".to_string()];
        assert_eq!(Audience::Specific.allows(None, &named), Access::NeedSignIn);
        assert_eq!(Audience::Specific.allows(Some(&who("gone-2")), &named), Access::Denied);
        assert_eq!(Audience::Specific.allows(Some(&who("other")), &[]), Access::Denied);
    }

    #[test]
    fn only_public_is_cached() {
        assert_eq!(Audience::Public.cache_policy(), CachePolicy::Shared);
        assert_eq!(Audience::SignedIn.cache_policy(), CachePolicy::Private);
        assert_eq!(Audience::Specific.cache_policy(), CachePolicy::Private);
        assert_eq!(CachePolicy::Shared.cache_control(), "public, max-age=300, s-maxage=300");
        assert_eq!(CachePolicy::Private.cache_control(), "private, no-store");
        assert!(!CachePolicy::Shared.cache_control().contains("immutable"));
    }

    #[test]
    fn names_round_trip_and_match_serde() {
        for audience in [Audience::Specific, Audience::SignedIn, Audience::Public] {
            assert_eq!(Audience::from_name(audience.name()), Some(audience));
            assert_eq!(serde_json::to_value(audience).unwrap(), audience.name());
        }
        assert_eq!(Audience::from_name("friends"), None);
        assert_eq!(Audience::from_name(""), None);
    }
}
