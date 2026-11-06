use crate::time::Timestamp;
use crate::storage::sql::{self, Connection, OptionalExtension, ToSql, params, params_from_iter};
use anyhow::Result;
use chrono::{TimeZone, Local};

use crate::{
    schema::{
        InsertedNotification, NotificationCategory, NotificationCounts, NotificationDraft,
        NotificationFilter, NotificationItem, NotificationTone,
    },
    storage::{
        notification::NotificationStorage, notification::RETENTION_DAYS, sqlite::SqlStorage,
    },
};

fn millis(ms: i64) -> Timestamp {
    Local.timestamp_millis_opt(ms).single().unwrap_or_default()
}

/// The oldest instant still retained.
fn cutoff() -> i64 {
    (crate::time::now() - chrono::Duration::days(RETENTION_DAYS)).timestamp_millis()
}

/// `IN (?, ?, …)` for `agents`, which is never empty at a call site (the caller returns early).
fn marks(agents: &[String]) -> String {
    vec!["?"; agents.len()].join(", ")
}

/// Delete an agent's notifications; their read rows cascade.
pub(super) fn delete_agent_notifications(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM notification WHERE agent_id = ?1",
        params![agent_id],
    )?;
    Ok(())
}

#[async_trait::async_trait]
impl NotificationStorage for SqlStorage {
    async fn insert_notifications(
        &self,
        drafts: &[NotificationDraft],
    ) -> Result<Vec<InsertedNotification>> {
        if drafts.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let mut inserted = Vec::new();
        for draft in drafts {
            // A programmer error, not user input: refuse the whole batch rather than storing a
            // row no client can render.
            if draft.event_key.trim().is_empty()
                || draft.summary.trim().is_empty()
                || draft.href.trim().is_empty()
            {
                return Err(anyhow::anyhow!(
                    "notification draft needs a non-empty event_key, summary and href"
                ));
            }
            // The delete/insert race: the service checks too, this is the storage-side net.
            let exists: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM agent_config WHERE agent_id = ?1",
                    params![draft.agent_id],
                    |row| row.get(0),
                )
                .ok();
            if exists.is_none() {
                continue;
            }
            let source_session = draft
                .source_session
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            // `RETURNING` yields a row only when this call inserted one, so a duplicate
            // `event_key` announces nothing.
            let id: Option<i64> = tx
                .query_row(
                    "INSERT INTO notification
                        (event_key, agent_id, audience_user_id, category, tone, summary, href,
                         created_at, source_session, source_title, excerpt)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT DO NOTHING
                     RETURNING id",
                    params![
                        draft.event_key.as_str(),
                        draft.agent_id.as_str(),
                        draft.audience_user_id.as_deref(),
                        draft.category.as_str(),
                        draft.tone.as_str(),
                        draft.summary.as_str(),
                        draft.href.as_str(),
                        draft.created_at.timestamp_millis(),
                        source_session.as_deref(),
                        draft.source_title.as_deref(),
                        draft.excerpt.as_deref(),
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = id {
                inserted.push(InsertedNotification {
                    item: NotificationItem {
                        id,
                        at: draft.created_at,
                        agent_id: draft.agent_id.clone(),
                        category: draft.category,
                        tone: draft.tone,
                        text: draft.summary.clone(),
                        href: draft.href.clone(),
                        read: false,
                        title: draft.source_title.clone(),
                        excerpt: draft.excerpt.clone(),
                        source_session: draft.source_session.clone(),
                    },
                    audience: draft.audience_user_id.clone(),
                });
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        agents: &[String],
        filter: NotificationFilter,
        before: Option<(Timestamp, i64)>,
        limit: usize,
    ) -> Result<Vec<NotificationItem>> {
        if agents.is_empty() {
            return Ok(vec![]);
        }
        let mut sql = format!(
            "SELECT n.id, n.agent_id, n.category, n.tone, n.summary, n.href, n.created_at, \
                    (r.notification_id IS NOT NULL) AS read, \
                    n.source_session, n.source_title, n.excerpt \
             FROM notification n \
             LEFT JOIN notification_read r \
               ON r.notification_id = n.id AND r.user_id = ? \
             WHERE n.agent_id IN ({}) AND n.created_at >= ? \
               AND (n.audience_user_id IS NULL OR n.audience_user_id = ?)",
            marks(agents),
        );
        match filter {
            NotificationFilter::All => {}
            NotificationFilter::Unread => sql.push_str(" AND r.notification_id IS NULL"),
            NotificationFilter::Read => sql.push_str(" AND r.notification_id IS NOT NULL"),
        }
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(user_id.to_string())];
        params.extend(agents.iter().map(|a| Box::new(a.clone()) as Box<dyn ToSql>));
        params.push(Box::new(cutoff()));
        params.push(Box::new(user_id.to_string()));
        if let Some((at, id)) = before {
            sql.push_str(" AND (n.created_at < ? OR (n.created_at = ? AND n.id < ?))");
            params.push(Box::new(at.timestamp_millis()));
            params.push(Box::new(at.timestamp_millis()));
            params.push(Box::new(id));
        }
        sql.push_str(" ORDER BY n.created_at DESC, n.id DESC LIMIT ?");
        params.push(Box::new(limit as i64));

        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            })?
            .collect::<sql::Result<Vec<_>>>()?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    id,
                    agent_id,
                    category,
                    tone,
                    summary,
                    href,
                    created_at,
                    read,
                    source_session,
                    source_title,
                    excerpt,
                )| {
                    Some(NotificationItem {
                        id,
                        at: millis(created_at),
                        agent_id,
                        category: NotificationCategory::parse(&category)?,
                        tone: NotificationTone::parse(&tone)?,
                        text: summary,
                        href,
                        read: read != 0,
                        title: source_title,
                        excerpt,
                        // A row whose session no longer parses just loses its title.
                        source_session: source_session
                            .and_then(|json| serde_json::from_str(&json).ok()),
                    })
                },
            )
            .collect())
    }

    async fn notification_counts(
        &self,
        user_id: &str,
        agents: &[String],
    ) -> Result<NotificationCounts> {
        if agents.is_empty() {
            return Ok(NotificationCounts::default());
        }
        let sql = format!(
            "SELECT COUNT(*), \
                    COALESCE(SUM(CASE WHEN r.notification_id IS NULL THEN 1 ELSE 0 END), 0) \
             FROM notification n \
             LEFT JOIN notification_read r \
               ON r.notification_id = n.id AND r.user_id = ? \
             WHERE n.agent_id IN ({}) AND n.created_at >= ? \
               AND (n.audience_user_id IS NULL OR n.audience_user_id = ?)",
            marks(agents),
        );
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(user_id.to_string())];
        params.extend(agents.iter().map(|a| Box::new(a.clone()) as Box<dyn ToSql>));
        params.push(Box::new(cutoff()));
        params.push(Box::new(user_id.to_string()));
        let conn = self.conn.lock()?;
        let (all, unread): (i64, i64) =
            conn.query_row(&sql, params_from_iter(params.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        let all = all.max(0) as u64;
        let unread = (unread.max(0) as u64).min(all);
        Ok(NotificationCounts {
            all,
            unread,
            read: all - unread,
        })
    }

    async fn latest_notification_id(&self, user_id: &str, agents: &[String]) -> Result<i64> {
        if agents.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "SELECT COALESCE(MAX(n.id), 0) FROM notification n \
             WHERE n.agent_id IN ({}) AND n.created_at >= ? \
               AND (n.audience_user_id IS NULL OR n.audience_user_id = ?)",
            marks(agents),
        );
        let mut params: Vec<Box<dyn ToSql>> = agents
            .iter()
            .map(|a| Box::new(a.clone()) as Box<dyn ToSql>)
            .collect();
        params.push(Box::new(cutoff()));
        params.push(Box::new(user_id.to_string()));
        let conn = self.conn.lock()?;
        Ok(conn.query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))?)
    }

    async fn set_notification_read(
        &self,
        user_id: &str,
        notification_id: i64,
        read: bool,
        at: Timestamp,
        agents: &[String],
    ) -> Result<bool> {
        if agents.is_empty() {
            return Ok(false);
        }
        let conn = self.conn.lock()?;
        let sql = format!(
            "SELECT 1 FROM notification \
             WHERE id = ? AND agent_id IN ({}) AND created_at >= ? \
               AND (audience_user_id IS NULL OR audience_user_id = ?)",
            marks(agents)
        );
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(notification_id)];
        params.extend(agents.iter().map(|a| Box::new(a.clone()) as Box<dyn ToSql>));
        params.push(Box::new(cutoff()));
        params.push(Box::new(user_id.to_string()));
        let visible: Option<i64> = conn
            .query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))
            .ok();
        if visible.is_none() {
            return Ok(false);
        }
        if read {
            conn.execute(
                "INSERT INTO notification_read (user_id, notification_id, read_at) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT(user_id, notification_id) DO UPDATE SET read_at = excluded.read_at",
                params![user_id, notification_id, at.timestamp_millis()],
            )?;
        } else {
            conn.execute(
                "DELETE FROM notification_read WHERE user_id = ?1 AND notification_id = ?2",
                params![user_id, notification_id],
            )?;
        }
        Ok(true)
    }

    async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        at: Timestamp,
        agents: &[String],
    ) -> Result<u64> {
        if agents.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "INSERT INTO notification_read (user_id, notification_id, read_at) \
             SELECT CAST(? AS TEXT), n.id, CAST(? AS BIGINT) FROM notification n \
             WHERE n.agent_id IN ({}) AND n.created_at >= ? \
               AND (n.audience_user_id IS NULL OR n.audience_user_id = ?) \
             ON CONFLICT DO NOTHING",
            marks(agents)
        );
        let mut params: Vec<Box<dyn ToSql>> = vec![
            Box::new(user_id.to_string()),
            Box::new(at.timestamp_millis()),
        ];
        params.extend(agents.iter().map(|a| Box::new(a.clone()) as Box<dyn ToSql>));
        params.push(Box::new(cutoff()));
        params.push(Box::new(user_id.to_string()));
        let conn = self.conn.lock()?;
        let inserted = conn.execute(&sql, params_from_iter(params.iter()))?;
        Ok(inserted as u64)
    }

    async fn purge_notifications_before(&self, at: Timestamp) -> Result<usize> {
        let conn = self.conn.lock()?;
        let deleted = conn.execute(
            "DELETE FROM notification WHERE created_at < ?1",
            params![at.timestamp_millis()],
        )?;
        Ok(deleted)
    }

    async fn purge_orphaned_notifications(&self) -> Result<usize> {
        let conn = self.conn.lock()?;
        let deleted = conn.execute(
            "DELETE FROM notification \
             WHERE agent_id NOT IN (SELECT agent_id FROM agent_config)",
            [],
        )?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::storage::sql::SqlStore;
    use rusqlite::Connection;

    use super::*;
    use crate::storage::document::BlobDocumentStore;

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_notification_schema(&conn).unwrap();
            crate::storage::sqlite::init_history_schema(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE agent_config (agent_id TEXT PRIMARY KEY, data TEXT NOT NULL);",
            )
            .unwrap();
            SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn agent(s: &SqlStorage, id: &str) {
        s.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_config (agent_id, data) VALUES (?1, '{}') ON CONFLICT DO NOTHING",
                params![id],
            )
            .unwrap();
    }

    fn draft(key: &str, agent_id: &str, at: Timestamp) -> NotificationDraft {
        NotificationDraft {
            event_key: key.into(),
            agent_id: agent_id.into(),
            audience_user_id: None,
            category: NotificationCategory::TaskRun,
            tone: NotificationTone::Default,
            summary: format!("Task “{key}” answered."),
            href: format!("/{agent_id}/tasks"),
            created_at: at,
            source_session: None,
            source_title: None,
            excerpt: None,
        }
    }

    fn ids(items: &[NotificationItem]) -> Vec<i64> {
        items.iter().map(|i| i.id).collect()
    }

    #[test]
    fn the_schema_adds_the_audience_to_a_table_created_before_it() {
        let conn = Connection::open_in_memory().unwrap();
        // The notification table as the first 029 build created it.
        conn.execute_batch(
            "CREATE TABLE notification (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                event_key   TEXT NOT NULL UNIQUE,
                agent_id    TEXT NOT NULL,
                category    TEXT NOT NULL,
                tone        TEXT NOT NULL,
                summary     TEXT NOT NULL,
                href        TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );",
        )
        .unwrap();
        crate::storage::sqlite::init_notification_schema(&conn).unwrap();
        let has = conn
            .prepare("PRAGMA table_info(notification)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .any(|name| name == "audience_user_id");
        assert!(has, "the audience column is added to an existing table");
    }

    #[tokio::test]
    async fn a_duplicate_event_key_inserts_once_and_never_updates() {
        let (s, _d) = setup();
        agent(&s, "a");
        let at = crate::time::now();
        let first = s
            .insert_notifications(&[draft("task_run:1", "a", at)])
            .await
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].item.text, "Task “task_run:1” answered.");
        assert!(!first[0].item.read);
        let mut second = draft("task_run:1", "a", at);
        second.summary = "something else".into();
        assert!(
            s.insert_notifications(&[second]).await.unwrap().is_empty(),
            "a duplicate event_key returns nothing to announce"
        );
        let items = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "Task “task_run:1” answered.");
    }

    #[tokio::test]
    async fn the_source_and_excerpt_columns_round_trip() {
        use crate::schema::{VizierChannelId, VizierSession};
        let (s, _d) = setup();
        agent(&s, "a");
        let session = VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some("topic-1".into()),
        );
        let mut with = draft("chat:1", "a", crate::time::now());
        with.source_session = Some(session.clone());
        with.excerpt = Some("The middle plan is underpriced.".into());
        let mut task = draft("task:1", "a", crate::time::now());
        task.source_title = Some("Weekly brief".into());
        let plain = draft("plain:1", "a", crate::time::now());
        let inserted = s.insert_notifications(&[with, task, plain]).await.unwrap();
        assert_eq!(inserted.len(), 3);
        assert_eq!(inserted[0].item.source_session.as_ref(), Some(&session));

        let items = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        let by = |text: &str| items.iter().find(|i| i.text.contains(text)).unwrap();
        let chat = by("chat:1");
        assert_eq!(chat.source_session.as_ref(), Some(&session));
        assert_eq!(
            chat.excerpt.as_deref(),
            Some("The middle plan is underpriced.")
        );
        assert_eq!(by("task:1").title.as_deref(), Some("Weekly brief"));
        let plain = by("plain:1");
        assert!(plain.title.is_none() && plain.excerpt.is_none() && plain.source_session.is_none());
    }

    #[test]
    fn the_schema_drops_old_channel_reply_rows_and_their_read_rows() {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_notification_schema(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO notification (id, event_key, agent_id, category, tone, summary, href, created_at)
                VALUES (1, 'reply:a:discord__1:9', 'a', 'channel_replies', 'default', 'Replied once.', '/a/chat', 1),
                       (2, 'task_run:1', 'a', 'task_run', 'default', 'Task answered.', '/a/tasks', 1);
             INSERT INTO notification_read (user_id, notification_id, read_at) VALUES ('u', 1, 1), ('u', 2, 1);",
        )
        .unwrap();
        // The next start runs the schema again.
        crate::storage::sqlite::init_notification_schema(&conn).unwrap();
        let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("SELECT COUNT(*) FROM notification"), 1);
        assert_eq!(count("SELECT COUNT(*) FROM notification WHERE category = 'channel_replies'"), 0);
    }

    #[tokio::test]
    async fn the_latest_id_is_the_highest_one_this_person_may_see() {
        let (s, _d) = setup();
        agent(&s, "a");
        agent(&s, "b");
        assert_eq!(s.latest_notification_id("u", &["a".into()]).await.unwrap(), 0);
        let mut addressed = draft("chat", "a", crate::time::now());
        addressed.audience_user_id = Some("ana".into());
        s.insert_notifications(&[draft("1", "a", crate::time::now()), draft("2", "b", crate::time::now()), addressed])
            .await
            .unwrap();
        assert_eq!(s.latest_notification_id("ana", &["a".into(), "b".into()]).await.unwrap(), 3);
        assert_eq!(
            s.latest_notification_id("bob", &["a".into(), "b".into()]).await.unwrap(),
            2,
            "another person's row does not move the mark"
        );
        assert_eq!(s.latest_notification_id("ana", &["a".into()]).await.unwrap(), 3);
        assert_eq!(s.latest_notification_id("ana", &[]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_draft_for_a_missing_agent_is_skipped() {
        let (s, _d) = setup();
        agent(&s, "a");
        assert!(
            s.insert_notifications(&[draft("k", "gone", crate::time::now())])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            s.notification_counts("u", &["gone".into()])
                .await
                .unwrap()
                .all,
            0
        );
    }

    #[tokio::test]
    async fn list_and_counts_honour_visibility_filter_and_ties() {
        let (s, _d) = setup();
        agent(&s, "a");
        agent(&s, "b");
        let at = crate::time::now();
        s.insert_notifications(&[
            draft("1", "a", at),
            draft("2", "a", at),
            draft("3", "b", at),
        ])
        .await
        .unwrap();
        let all = s
            .list_notifications(
                "u",
                &["a".into(), "b".into()],
                NotificationFilter::All,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(ids(&all), vec![3, 2, 1], "newest first, id breaks the tie");

        let only_a = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(ids(&only_a), vec![2, 1]);

        let empty = s
            .list_notifications("u", &[], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert!(empty.is_empty());
        assert_eq!(
            s.notification_counts("u", &[]).await.unwrap(),
            NotificationCounts::default()
        );

        s.set_notification_read("u", 2, true, crate::time::now(), &["a".into()])
            .await
            .unwrap();
        let unread = s
            .list_notifications("u", &["a".into()], NotificationFilter::Unread, None, 10)
            .await
            .unwrap();
        assert_eq!(ids(&unread), vec![1]);
        let read = s
            .list_notifications("u", &["a".into()], NotificationFilter::Read, None, 10)
            .await
            .unwrap();
        assert_eq!(ids(&read), vec![2]);
        let counts = s
            .notification_counts("u", &["a".into(), "b".into()])
            .await
            .unwrap();
        assert_eq!((counts.all, counts.unread, counts.read), (3, 2, 1));
    }

    #[tokio::test]
    async fn pages_use_the_cursor_without_gaps_or_repeats() {
        let (s, _d) = setup();
        agent(&s, "a");
        let at = crate::time::now();
        s.insert_notifications(&[
            draft("1", "a", at),
            draft("2", "a", at),
            draft("3", "a", at),
        ])
        .await
        .unwrap();
        let first = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 2)
            .await
            .unwrap();
        assert_eq!(ids(&first), vec![3, 2]);
        let last = first.last().unwrap();
        let second = s
            .list_notifications(
                "u",
                &["a".into()],
                NotificationFilter::All,
                Some((last.at, last.id)),
                2,
            )
            .await
            .unwrap();
        assert_eq!(ids(&second), vec![1]);
    }

    #[tokio::test]
    async fn read_state_is_per_person_and_idempotent() {
        let (s, _d) = setup();
        agent(&s, "a");
        s.insert_notifications(&[draft("1", "a", crate::time::now())])
            .await
            .unwrap();
        let item = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        let id = item[0].id;

        assert!(
            s.set_notification_read("u", id, true, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );
        assert!(
            s.set_notification_read("u", id, true, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );
        assert!(
            !s.set_notification_read("u", 9999, true, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );
        assert!(
            !s.set_notification_read("u", id, true, crate::time::now(), &["b".into()])
                .await
                .unwrap()
        );

        let mine = s.notification_counts("u", &["a".into()]).await.unwrap();
        assert_eq!((mine.all, mine.unread, mine.read), (1, 0, 1));
        let theirs = s.notification_counts("v", &["a".into()]).await.unwrap();
        assert_eq!((theirs.all, theirs.unread, theirs.read), (1, 1, 0));

        assert!(
            s.set_notification_read("u", id, false, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );
        assert_eq!(
            s.notification_counts("u", &["a".into()])
                .await
                .unwrap()
                .unread,
            1
        );
    }

    #[tokio::test]
    async fn mark_all_covers_unread_rows_beyond_the_page() {
        let (s, _d) = setup();
        agent(&s, "a");
        let at = crate::time::now();
        s.insert_notifications(&[
            draft("1", "a", at),
            draft("2", "a", at),
            draft("3", "a", at),
        ])
        .await
        .unwrap();
        let marked = s
            .mark_all_notifications_read("u", crate::time::now(), &["a".into()])
            .await
            .unwrap();
        assert_eq!(marked, 3);
        assert_eq!(
            s.notification_counts("u", &["a".into()]).await.unwrap(),
            NotificationCounts {
                all: 3,
                unread: 0,
                read: 3
            }
        );
        // Idempotent: nothing new is inserted.
        assert_eq!(
            s.mark_all_notifications_read("u", crate::time::now(), &["a".into()])
                .await
                .unwrap(),
            0
        );
        // A row created after the snapshot stays unread.
        s.insert_notifications(&[draft("4", "a", crate::time::now())])
            .await
            .unwrap();
        assert_eq!(
            s.notification_counts("u", &["a".into()])
                .await
                .unwrap()
                .unread,
            1
        );
    }

    #[tokio::test]
    async fn rows_outside_the_retention_window_are_neither_listed_nor_counted_and_are_purged() {
        let (s, _d) = setup();
        agent(&s, "a");
        let old = crate::time::now() - chrono::Duration::days(RETENTION_DAYS + 1);
        let recent = crate::time::now() - chrono::Duration::days(RETENTION_DAYS - 1);
        s.insert_notifications(&[draft("old", "a", old), draft("new", "a", recent)])
            .await
            .unwrap();
        let items = s
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            s.notification_counts("u", &["a".into()]).await.unwrap().all,
            1
        );

        let purged = s
            .purge_notifications_before(crate::time::now() - chrono::Duration::days(RETENTION_DAYS))
            .await
            .unwrap();
        assert_eq!(purged, 1);
        assert_eq!(
            s.conn
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM notification", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn deleting_an_agent_removes_its_notifications_and_read_rows_only() {
        let (s, _d) = setup();
        agent(&s, "a");
        agent(&s, "b");
        s.insert_notifications(&[draft("1", "a", crate::time::now()), draft("2", "b", crate::time::now())])
            .await
            .unwrap();
        let items = s
            .list_notifications(
                "u",
                &["a".into(), "b".into()],
                NotificationFilter::All,
                None,
                10,
            )
            .await
            .unwrap();
        let a_id = items.iter().find(|i| i.agent_id == "a").unwrap().id;
        s.set_notification_read("u", a_id, true, crate::time::now(), &["a".into()])
            .await
            .unwrap();

        delete_agent_notifications(&s.conn.lock().unwrap(), "a").unwrap();

        let left = s
            .list_notifications(
                "u",
                &["a".into(), "b".into()],
                NotificationFilter::All,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].agent_id, "b");
        assert_eq!(
            s.conn
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM notification_read", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "the read row cascaded"
        );
    }

    #[tokio::test]
    async fn an_addressed_row_is_listed_counted_and_read_only_by_its_person() {
        let (s, _d) = setup();
        agent(&s, "a");
        let at = crate::time::now();
        let mut addressed = draft("chat:1", "a", at);
        addressed.audience_user_id = Some("ana".into());
        addressed.category = NotificationCategory::ChatReply;
        s.insert_notifications(&[draft("shared", "a", at), addressed])
            .await
            .unwrap();

        let mine = s
            .list_notifications("ana", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(mine.len(), 2);
        let chat = mine
            .iter()
            .find(|i| i.category == NotificationCategory::ChatReply)
            .unwrap();
        let theirs = s
            .list_notifications("bob", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(theirs.len(), 1, "bob does not see ana's conversation");
        assert_eq!(
            s.notification_counts("ana", &["a".into()])
                .await
                .unwrap()
                .all,
            2
        );
        assert_eq!(
            s.notification_counts("bob", &["a".into()])
                .await
                .unwrap()
                .all,
            1
        );

        // Another person cannot even tell the addressed row exists.
        assert!(
            !s.set_notification_read("bob", chat.id, true, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );
        assert!(
            s.set_notification_read("ana", chat.id, true, crate::time::now(), &["a".into()])
                .await
                .unwrap()
        );

        // Mark-all covers each person's own visible set only: bob reads the shared row,
        // ana read only her conversation, so the shared row is still unread for her.
        s.mark_all_notifications_read("bob", crate::time::now(), &["a".into()])
            .await
            .unwrap();
        let counts = s.notification_counts("ana", &["a".into()]).await.unwrap();
        assert_eq!((counts.unread, counts.read), (1, 1));
        assert_eq!(
            s.notification_counts("bob", &["a".into()])
                .await
                .unwrap()
                .unread,
            0
        );
    }

    #[tokio::test]
    async fn orphan_cleanup_drops_rows_for_agents_that_no_longer_exist() {
        let (s, _d) = setup();
        agent(&s, "a");
        s.insert_notifications(&[draft("1", "a", crate::time::now())])
            .await
            .unwrap();
        s.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM agent_config", [])
            .unwrap();
        assert_eq!(s.purge_orphaned_notifications().await.unwrap(), 1);
    }

    crate::postgres_tests!(
        the_latest_id_is_the_highest_one_this_person_may_see,
        the_source_and_excerpt_columns_round_trip,
        a_duplicate_event_key_inserts_once_and_never_updates,
        a_draft_for_a_missing_agent_is_skipped,
        list_and_counts_honour_visibility_filter_and_ties,
        pages_use_the_cursor_without_gaps_or_repeats,
        read_state_is_per_person_and_idempotent,
        mark_all_covers_unread_rows_beyond_the_page,
        rows_outside_the_retention_window_are_neither_listed_nor_counted_and_are_purged,
        deleting_an_agent_removes_its_notifications_and_read_rows_only,
        an_addressed_row_is_listed_counted_and_read_only_by_its_person,
        orphan_cleanup_drops_rows_for_agents_that_no_longer_exist,
    );
}
