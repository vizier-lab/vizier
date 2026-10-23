//! What happened across a set of agents since a time, for Home's "Since you were away"
//! (`specs/024-webui-rewrite/contracts/http-api.md` § Overview). One read per source, each
//! newest first and bounded by `limit`, so merging them costs `sources × limit` rows however much
//! history there is.

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{IntegrationEvent, JobKind, JobState, TaskRunState},
    storage::VizierStorage,
};

/// A window onto the feed: only `agents`, only after `since`, and only before the cursor.
#[derive(Debug, Clone)]
pub struct FeedWindow {
    pub agents: Vec<String>,
    pub since: DateTime<Utc>,
    /// Events strictly older than this `(at, id)`, ordering ids as strings.
    pub before: Option<(DateTime<Utc>, String)>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunEvent {
    pub id: i64,
    pub agent_id: String,
    pub task_slug: String,
    pub state: TaskRunState,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEvent {
    pub id: String,
    pub agent_id: String,
    pub kind: JobKind,
    pub state: JobState,
    pub reason: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleEvent {
    pub cycle_id: String,
    pub agent_id: String,
    /// Journal entries the cycle wrote.
    pub entries: u64,
    pub at: DateTime<Utc>,
}

/// An agent's replies in one Discord or Telegram conversation within one hour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyEvent {
    pub agent_id: String,
    pub channel: String,
    /// The hour bucket (hours since the epoch).
    pub hour: i64,
    pub replies: u64,
    pub at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait FeedStorage {
    /// Runs that have ended, by when they ended.
    async fn feed_task_runs(&self, w: &FeedWindow) -> Result<Vec<TaskRunEvent>>;
    /// Jobs that are over, launched from `agents`.
    async fn feed_background_jobs(&self, w: &FeedWindow) -> Result<Vec<JobEvent>>;
    /// Dream cycles, by their last journal entry.
    async fn feed_dream_cycles(&self, w: &FeedWindow) -> Result<Vec<CycleEvent>>;
    async fn feed_integration_events(&self, w: &FeedWindow) -> Result<Vec<IntegrationEvent>>;
    async fn feed_channel_replies(&self, w: &FeedWindow) -> Result<Vec<ReplyEvent>>;
}

#[async_trait::async_trait]
impl FeedStorage for VizierStorage {
    async fn feed_task_runs(&self, w: &FeedWindow) -> Result<Vec<TaskRunEvent>> {
        self.0.feed_task_runs(w).await
    }

    async fn feed_background_jobs(&self, w: &FeedWindow) -> Result<Vec<JobEvent>> {
        self.0.feed_background_jobs(w).await
    }

    async fn feed_dream_cycles(&self, w: &FeedWindow) -> Result<Vec<CycleEvent>> {
        self.0.feed_dream_cycles(w).await
    }

    async fn feed_integration_events(&self, w: &FeedWindow) -> Result<Vec<IntegrationEvent>> {
        self.0.feed_integration_events(w).await
    }

    async fn feed_channel_replies(&self, w: &FeedWindow) -> Result<Vec<ReplyEvent>> {
        self.0.feed_channel_replies(w).await
    }
}
