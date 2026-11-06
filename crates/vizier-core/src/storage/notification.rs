//! The notification inbox's storage concern (`specs/029-webui-notifications/
//! contracts/notification-storage.md`): two tables, the exactly-once insert behind `event_key`,
//! per-person read state, keyset paging and the retention window.

use crate::time::Timestamp;
use anyhow::Result;


use crate::{
    schema::{
        InsertedNotification, NotificationCounts, NotificationDraft, NotificationFilter,
        NotificationItem,
    },
    storage::VizierStorage,
};

/// The retained window: older rows are neither listed nor counted, whatever the purge has done.
pub const RETENTION_DAYS: i64 = 30;

/// One notification's inbox read.
#[async_trait::async_trait]
pub trait NotificationStorage {
    /// Idempotent batch insert on `event_key`; returns the rows this call actually inserted,
    /// so a duplicate announces nothing.
    async fn insert_notifications(
        &self,
        drafts: &[NotificationDraft],
    ) -> Result<Vec<InsertedNotification>>;

    /// Newest first, filtered by state and the visible agent whitelist.
    async fn list_notifications(
        &self,
        user_id: &str,
        agents: &[String],
        filter: NotificationFilter,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<NotificationItem>>;

    /// Over the visible agents and the retention window, whatever the selected filter.
    async fn notification_counts(
        &self,
        user_id: &str,
        agents: &[String],
    ) -> Result<NotificationCounts>;

    /// The highest id this person may see over the visible agents and the retention window,
    /// or `0` when there is none. Ids only grow, so a client that holds this id has seen
    /// everything up to it.
    async fn latest_notification_id(&self, user_id: &str, agents: &[String]) -> Result<i64>;

    /// Read/unread for one visible notification; false when missing, invisible or expired.
    async fn set_notification_read(
        &self,
        user_id: &str,
        notification_id: i64,
        read: bool,
        at: Timestamp,
        agents: &[String],
    ) -> Result<bool>;

    /// Marks every visible, retained unread notification read; returns rows inserted.
    async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        at: Timestamp,
        agents: &[String],
    ) -> Result<u64>;

    async fn purge_notifications_before(&self, at: Timestamp) -> Result<usize>;

    /// Startup orphan cleanup: drop rows whose agent is gone, closing a delete/insert race.
    async fn purge_orphaned_notifications(&self) -> Result<usize>;

    /// How many Workspace messages `user_id` has not read, by Workspace and by topic, leaving out
    /// what they wrote themselves. Each row is `(workspace_id, topic_id, unread)`.
    async fn workspace_unread(&self, user_id: &str, workspace_ids: &[String]) -> Result<Vec<(String, String, u64)>>;

    /// Marks every unread message of one topic read for `user_id`; returns how many.
    async fn mark_topic_read(&self, user_id: &str, workspace_id: &str, topic_id: &str, at: Timestamp) -> Result<u64>;

    /// Deletes a Workspace's notifications, or one topic's, with it (`topic_id` set).
    async fn delete_workspace_notifications(&self, workspace_id: &str, topic_id: Option<&str>) -> Result<u64>;
}

#[async_trait::async_trait]
impl NotificationStorage for VizierStorage {
    async fn insert_notifications(
        &self,
        drafts: &[NotificationDraft],
    ) -> Result<Vec<InsertedNotification>> {
        self.0.insert_notifications(drafts).await
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        agents: &[String],
        filter: NotificationFilter,
        before: Option<(Timestamp, i64)>,
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

    async fn latest_notification_id(&self, user_id: &str, agents: &[String]) -> Result<i64> {
        self.0.latest_notification_id(user_id, agents).await
    }

    async fn set_notification_read(
        &self,
        user_id: &str,
        notification_id: i64,
        read: bool,
        at: Timestamp,
        agents: &[String],
    ) -> Result<bool> {
        self.0
            .set_notification_read(user_id, notification_id, read, at, agents)
            .await
    }

    async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        at: Timestamp,
        agents: &[String],
    ) -> Result<u64> {
        self.0.mark_all_notifications_read(user_id, at, agents).await
    }

    async fn purge_notifications_before(&self, at: Timestamp) -> Result<usize> {
        self.0.purge_notifications_before(at).await
    }

    async fn purge_orphaned_notifications(&self) -> Result<usize> {
        self.0.purge_orphaned_notifications().await
    }

    async fn workspace_unread(&self, user_id: &str, workspace_ids: &[String]) -> Result<Vec<(String, String, u64)>> {
        self.0.workspace_unread(user_id, workspace_ids).await
    }

    async fn mark_topic_read(&self, user_id: &str, workspace_id: &str, topic_id: &str, at: Timestamp) -> Result<u64> {
        self.0.mark_topic_read(user_id, workspace_id, topic_id, at).await
    }

    async fn delete_workspace_notifications(&self, workspace_id: &str, topic_id: Option<&str>) -> Result<u64> {
        self.0.delete_workspace_notifications(workspace_id, topic_id).await
    }
}
