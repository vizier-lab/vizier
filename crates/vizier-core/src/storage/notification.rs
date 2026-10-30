//! The notification inbox's storage concern (`specs/029-webui-notifications/
//! contracts/notification-storage.md`): two tables, the exactly-once insert behind `event_key`,
//! per-person read state, keyset paging and the retention window.

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{NotificationCounts, NotificationDraft, NotificationFilter, NotificationItem, ReplyBatch},
    storage::VizierStorage,
};

/// The retained window: older rows are neither listed nor counted, whatever the purge has done.
pub const RETENTION_DAYS: i64 = 30;

/// One notification's inbox read.
#[async_trait::async_trait]
pub trait NotificationStorage {
    /// Idempotent batch insert on `event_key`; returns how many were new.
    async fn insert_notifications(&self, drafts: &[NotificationDraft]) -> Result<usize>;

    /// Newest first, filtered by state and the visible agent whitelist.
    async fn list_notifications(
        &self,
        user_id: &str,
        agents: &[String],
        filter: NotificationFilter,
        before: Option<(DateTime<Utc>, i64)>,
        limit: usize,
    ) -> Result<Vec<NotificationItem>>;

    /// Over the visible agents and the retention window, whatever the selected filter.
    async fn notification_counts(
        &self,
        user_id: &str,
        agents: &[String],
    ) -> Result<NotificationCounts>;

    /// Read/unread for one visible notification; false when missing, invisible or expired.
    async fn set_notification_read(
        &self,
        user_id: &str,
        notification_id: i64,
        read: bool,
        at: DateTime<Utc>,
        agents: &[String],
    ) -> Result<bool>;

    /// Marks every visible, retained unread notification read; returns rows inserted.
    async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        at: DateTime<Utc>,
        agents: &[String],
    ) -> Result<u64>;

    /// Closed-hour reply aggregates for all agents, used by the projection.
    async fn channel_reply_batches(&self, from_hour: i64, to_hour: i64)
    -> Result<Vec<ReplyBatch>>;

    async fn purge_notifications_before(&self, at: DateTime<Utc>) -> Result<usize>;

    /// Startup orphan cleanup: drop rows whose agent is gone, closing a delete/insert race.
    async fn purge_orphaned_notifications(&self) -> Result<usize>;
}

#[async_trait::async_trait]
impl NotificationStorage for VizierStorage {
    async fn insert_notifications(&self, drafts: &[NotificationDraft]) -> Result<usize> {
        self.0.insert_notifications(drafts).await
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        agents: &[String],
        filter: NotificationFilter,
        before: Option<(DateTime<Utc>, i64)>,
        limit: usize,
    ) -> Result<Vec<NotificationItem>> {
        self.0
            .list_notifications(user_id, agents, filter, before, limit)
            .await
    }

    async fn notification_counts(
        &self,
        user_id: &str,
        agents: &[String],
    ) -> Result<NotificationCounts> {
        self.0.notification_counts(user_id, agents).await
    }

    async fn set_notification_read(
        &self,
        user_id: &str,
        notification_id: i64,
        read: bool,
        at: DateTime<Utc>,
        agents: &[String],
    ) -> Result<bool> {
        self.0
            .set_notification_read(user_id, notification_id, read, at, agents)
            .await
    }

    async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        at: DateTime<Utc>,
        agents: &[String],
    ) -> Result<u64> {
        self.0.mark_all_notifications_read(user_id, at, agents).await
    }

    async fn channel_reply_batches(
        &self,
        from_hour: i64,
        to_hour: i64,
    ) -> Result<Vec<ReplyBatch>> {
        self.0.channel_reply_batches(from_hour, to_hour).await
    }

    async fn purge_notifications_before(&self, at: DateTime<Utc>) -> Result<usize> {
        self.0.purge_notifications_before(at).await
    }

    async fn purge_orphaned_notifications(&self) -> Result<usize> {
        self.0.purge_orphaned_notifications().await
    }
}
