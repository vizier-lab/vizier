//! Proven links between a platform account (Discord, Telegram) and a Vizier user, the codes that
//! prove them and the history of changes (spec 069, data-model.md).

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{platform_links::PlatformPerson, schema::ChannelPlatform, time::Timestamp};

/// A platform account proven to belong to a Vizier user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformLink {
    pub platform: ChannelPlatform,
    pub platform_user_id: String,
    pub user_id: String,
    /// The Vizier username of `user_id`, read with the link.
    pub username: String,
    /// The platform handle as last seen. Display only; identity is `platform_user_id`.
    pub platform_username: Option<String>,
    /// The platform display name as last seen. Display only.
    pub platform_display_name: Option<String>,
    pub linked_at: Timestamp,
}

/// What changed about a person's links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum LinkAction {
    Linked,
    Replaced,
    Unlinked,
}

impl LinkAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linked => "linked",
            Self::Replaced => "replaced",
            Self::Unlinked => "unlinked",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "linked" => Some(Self::Linked),
            "replaced" => Some(Self::Replaced),
            "unlinked" => Some(Self::Unlinked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformLinkEvent {
    pub id: String,
    pub user_id: String,
    pub platform: ChannelPlatform,
    pub platform_user_id: String,
    pub action: LinkAction,
    pub at: Timestamp,
}

/// The outcome of redeeming a code.
#[derive(Debug, Clone, PartialEq)]
pub enum RedeemOutcome {
    Linked(PlatformLink),
    /// The person's other account on this platform was unlinked in the same step.
    Replaced {
        link: PlatformLink,
        previous: PlatformLink,
    },
    /// Unknown, expired or already used. One answer for all three (FR-008).
    Unavailable,
    /// The platform account belongs to another Vizier user. Nothing changed, and the code stays.
    LinkedElsewhere,
}

#[async_trait::async_trait]
pub trait PlatformLinkStorage {
    /// Store the hash of a new code for `person`, voiding any earlier one (FR-004).
    async fn issue_connect_code(
        &self,
        person: &PlatformPerson,
        code_hash: &str,
        now_ms: i64,
        expires_ms: i64,
    ) -> Result<()>;
    /// The account a live code was issued to. Consumes nothing.
    async fn peek_connect_code(
        &self,
        code_hash: &str,
        now_ms: i64,
    ) -> Result<Option<PlatformPerson>>;
    /// In one transaction: take the code, refuse an account linked to another user, replace the
    /// person's link for that platform, clear their hand-typed values for it, record the change.
    async fn redeem_connect_code(
        &self,
        code_hash: &str,
        user_id: &str,
        now_ms: i64,
    ) -> Result<RedeemOutcome>;
    async fn find_link(
        &self,
        platform: ChannelPlatform,
        platform_user_id: &str,
    ) -> Result<Option<PlatformLink>>;
    /// The person's links, by platform name.
    async fn list_links(&self, user_id: &str) -> Result<Vec<PlatformLink>>;
    /// `false` when there was nothing to unlink.
    async fn unlink_platform(&self, user_id: &str, platform: ChannelPlatform) -> Result<bool>;
    /// Remember the handle and display name last seen on a linked account.
    async fn touch_link_names(&self, person: &PlatformPerson) -> Result<()>;
    /// Newest first.
    async fn list_link_events(&self, user_id: &str, limit: u32) -> Result<Vec<PlatformLinkEvent>>;
    /// Delete codes that can no longer be redeemed; returns how many.
    async fn purge_expired_connect_codes(&self, now_ms: i64) -> Result<u64>;
}
