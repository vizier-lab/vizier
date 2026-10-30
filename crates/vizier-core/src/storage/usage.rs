//! Usage accounting storage (`specs/018-usage-accounting/data-model.md` §3): one row per answered
//! model call, plus one row per failed turn, joined at query time.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};

use crate::{
    agents::agent::model::usage::TurnId,
    schema::{
        metrics::TokenCounts,
        usage::{UsageActivity, UsageEvent},
    },
    storage::VizierStorage,
};

/// What a usage read covers: one agent, a time range, optionally one activity and optionally one
/// conversation (`channel` slug and topic).
#[derive(Debug, Clone)]
pub struct UsageQuery {
    pub agent_id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub activity: Option<UsageActivity>,
    pub conversation: Option<(String, Option<String>)>,
}

/// How [`UsageStorage::usage_groups`] groups rows. Every grouping is also split by
/// `(provider, model)` and by failed/not failed, so the caller can price and split each group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageGrouping {
    /// UTC date × activity.
    Day,
    Activity,
    /// Also by served model.
    Model,
    Place,
    /// `(channel, topic)` × activity.
    Conversation,
}

/// One group's sums. Only the fields of its grouping are set.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageGroup {
    pub day: Option<NaiveDate>,
    pub activity: Option<UsageActivity>,
    pub served_model: Option<String>,
    pub place: Option<String>,
    pub channel: Option<String>,
    pub topic: Option<String>,
    pub provider: String,
    pub model: String,
    pub failed: bool,
    pub tokens: TokenCounts,
    pub calls: u64,
    /// Distinct turns among the rows (0 for rows without one).
    pub turns: u64,
    pub last_at: DateTime<Utc>,
}

/// One recorded call, as returned for a conversation's details.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEventRow {
    pub event: UsageEvent,
    pub failed: bool,
}

/// A conversation's calls in range: the heaviest `rows` (by tokens; the caller re-sorts by
/// cost), how many there are in all, and the most rounds any of its failed turns reached.
#[derive(Debug, Clone, Default)]
pub struct ConversationRows {
    pub rows: Vec<UsageEventRow>,
    pub total: u64,
    pub longest_failed_turn: Option<u32>,
}

/// Turns one agent started in one UTC hour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HourlyTurns {
    pub agent_id: String,
    /// The start of the hour.
    pub hour: DateTime<Utc>,
    pub turns: u64,
}

#[async_trait::async_trait]
pub trait UsageStorage {
    /// Turns per agent per UTC hour since `since`, a turn counted in the hour of its first
    /// recorded call (one that began before `since` counts from its first call after it).
    async fn hourly_turns(&self, agents: &[String], since: DateTime<Utc>) -> Result<Vec<HourlyTurns>>;

    /// Turns since `since` by activity, across `agents`.
    async fn turns_by_activity(
        &self,
        agents: &[String],
        since: DateTime<Utc>,
    ) -> Result<Vec<(UsageActivity, u64)>>;

    /// Token sums since `since` across `agents`, split by `(provider, model)` and by
    /// failed/not failed, for the caller to price. Only the provider, model, failed, tokens and
    /// counts of each group are set.
    async fn usage_groups_since(&self, agents: &[String], since: DateTime<Utc>) -> Result<Vec<UsageGroup>>;

    async fn record_usage(&self, event: UsageEvent) -> Result<()>;

    /// Idempotent: a turn is failed once.
    async fn record_failed_turn(&self, turn: &TurnId, agent_id: &str, at: DateTime<Utc>)
    -> Result<()>;

    async fn usage_groups(&self, q: &UsageQuery, grouping: UsageGrouping) -> Result<Vec<UsageGroup>>;

    /// `q.conversation` must be set. `limit` bounds `rows`.
    async fn conversation_usage(&self, q: &UsageQuery, limit: usize) -> Result<ConversationRows>;

    /// Whether any history is left for a conversation.
    async fn conversation_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool>;

    /// Whether a conversation still has its `session_detail` row: what deleting a WebUI topic
    /// removes (its history may stay behind).
    async fn session_detail_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool>;

    /// A conversation's saved title (`session_detail`), for readable names on the usage page.
    async fn session_title(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<Option<String>>;

    /// The one-time carry-over of reply stats (research R6), one transaction per call.
    async fn insert_legacy_usage(&self, events: Vec<UsageEvent>) -> Result<()>;
}

#[async_trait::async_trait]
impl UsageStorage for VizierStorage {
    async fn hourly_turns(&self, agents: &[String], since: DateTime<Utc>) -> Result<Vec<HourlyTurns>> {
        self.0.hourly_turns(agents, since).await
    }

    async fn turns_by_activity(
        &self,
        agents: &[String],
        since: DateTime<Utc>,
    ) -> Result<Vec<(UsageActivity, u64)>> {
        self.0.turns_by_activity(agents, since).await
    }

    async fn usage_groups_since(&self, agents: &[String], since: DateTime<Utc>) -> Result<Vec<UsageGroup>> {
        self.0.usage_groups_since(agents, since).await
    }

    async fn record_usage(&self, event: UsageEvent) -> Result<()> {
        self.0.record_usage(event).await
    }

    async fn record_failed_turn(
        &self,
        turn: &TurnId,
        agent_id: &str,
        at: DateTime<Utc>,
    ) -> Result<()> {
        self.0.record_failed_turn(turn, agent_id, at).await
    }

    async fn usage_groups(&self, q: &UsageQuery, grouping: UsageGrouping) -> Result<Vec<UsageGroup>> {
        self.0.usage_groups(q, grouping).await
    }

    async fn conversation_usage(&self, q: &UsageQuery, limit: usize) -> Result<ConversationRows> {
        self.0.conversation_usage(q, limit).await
    }

    async fn conversation_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool> {
        self.0.conversation_exists(agent_id, channel, topic).await
    }

    async fn session_detail_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool> {
        self.0.session_detail_exists(agent_id, channel, topic).await
    }

    async fn session_title(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<Option<String>> {
        self.0.session_title(agent_id, channel, topic).await
    }

    async fn insert_legacy_usage(&self, events: Vec<UsageEvent>) -> Result<()> {
        self.0.insert_legacy_usage(events).await
    }
}
