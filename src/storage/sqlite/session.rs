use anyhow::Result;
use chrono::DateTime;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::{
        history::is_non_user_channel,
        session::{CONVERSATION_SEARCH_ROWS, ConversationRow, SessionStorage},
        sqlite::SqliteStorage,
    },
};

fn session_detail_id(agent_id: &str, channel: &VizierChannelId, topic: &Option<TopicId>) -> String {
    format!(
        "{}#{}#{}",
        agent_id,
        channel.to_slug(),
        topic.clone().unwrap_or("DEFAULT".into())
    )
}

#[async_trait::async_trait]
impl SessionStorage for SqliteStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let id = session_detail_id(&session.agent_id, &session.channel, &session.topic);
        let data = serde_json::to_string(&session)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO session_detail (id, agent_id, channel, topic, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, session.agent_id, session.channel.to_slug(), session.topic, data],
        )?;
        Ok(())
    }

    async fn update_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let data = serde_json::to_string(&session)?;
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE session_detail SET data = ?1 WHERE agent_id = ?2 AND channel = ?3 AND topic = ?4",
            rusqlite::params![data, session.agent_id, session.channel.to_slug(), session.topic],
        )?;
        Ok(())
    }

    async fn conversation_rows(
        &self,
        agent_id: &str,
        q: Option<&str>,
    ) -> Result<Vec<ConversationRow>> {
        use std::collections::{BTreeMap, HashMap, HashSet};
        let conn = self.conn.lock();

        // Every (channel, topic) with when it began and last moved.
        let mut stmt = conn.prepare(
            "SELECT channel, COALESCE(topic, ''), MIN(timestamp), MAX(timestamp) FROM session_history \
             WHERE agent_id = ?1 GROUP BY channel, topic",
        )?;
        let spans = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut titles: HashMap<(String, String), String> = HashMap::new();
        let mut stmt = conn.prepare("SELECT data FROM session_detail WHERE agent_id = ?1")?;
        for data in stmt
            .query_map(rusqlite::params![agent_id], |row| row.get::<_, String>(0))?
            .flatten()
        {
            if let Ok(detail) = serde_json::from_str::<VizierSessionDetail>(&data) {
                titles.insert(
                    (detail.channel.to_slug(), detail.topic.unwrap_or_default()),
                    detail.title,
                );
            }
        }

        let needle = q.map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
        // Where the text was found: (channel, topic) pairs among the newest messages.
        let mut by_message: HashSet<(String, String)> = HashSet::new();
        if let Some(needle) = &needle {
            // The text lives in the entry's JSON; the bound keeps this a bounded scan, newest first.
            let mut stmt = conn.prepare(
                "SELECT DISTINCT channel, COALESCE(topic, '') FROM (SELECT channel, topic, data FROM session_history \
                 WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2) WHERE data LIKE ?3 ESCAPE '\\'",
            )?;
            let like = format!(
                "%{}%",
                needle
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            by_message.extend(
                stmt.query_map(
                    rusqlite::params![agent_id, CONVERSATION_SEARCH_ROWS as i64, like],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .flatten(),
            );
        }
        let title_has = |channel: &str, topic: &str| {
            needle.as_ref().is_some_and(|n| {
                titles
                    .get(&(channel.to_string(), topic.to_string()))
                    .is_some_and(|t| t.to_lowercase().contains(n))
            })
        };

        // Per channel: topics by when they began.
        let mut channels: BTreeMap<String, Vec<(i64, i64, String)>> = BTreeMap::new();
        for (channel, topic, first, last) in spans {
            if is_non_user_channel(&channel) {
                continue;
            }
            channels
                .entry(channel)
                .or_default()
                .push((first, last, topic));
        }

        let mut rows: Vec<ConversationRow> = vec![];
        for (channel, mut topics) in channels {
            topics.sort();
            let count = topics.len() as u64;
            if channel.starts_with("http__") {
                // A WebUI conversation is a topic: each is its own row.
                for (i, (_, last, topic)) in topics.iter().enumerate() {
                    if needle.is_some()
                        && !title_has(&channel, topic)
                        && !by_message.contains(&(channel.clone(), topic.clone()))
                    {
                        continue;
                    }
                    let Some(last_activity) = DateTime::from_timestamp_millis(*last) else {
                        continue;
                    };
                    rows.push(ConversationRow {
                        title: titles.get(&(channel.clone(), topic.clone())).cloned(),
                        channel: channel.clone(),
                        topics: count,
                        last_activity,
                        latest_topic: topic.clone(),
                        latest_ordinal: i as u64 + 1,
                    });
                }
            } else {
                // Anywhere else the channel is the conversation.
                let Some(latest) = topics.iter().max_by_key(|t| (t.1, t.0)).cloned() else {
                    continue;
                };
                let matched = needle.is_none()
                    || topics.iter().any(|t| {
                        title_has(&channel, &t.2)
                            || by_message.contains(&(channel.clone(), t.2.clone()))
                    });
                if !matched {
                    continue;
                }
                let Some(last_activity) = DateTime::from_timestamp_millis(latest.1) else {
                    continue;
                };
                let ordinal = topics.iter().position(|t| t.2 == latest.2).unwrap_or(0) as u64 + 1;
                rows.push(ConversationRow {
                    title: titles.get(&(channel.clone(), latest.2.clone())).cloned(),
                    channel,
                    topics: count,
                    last_activity,
                    latest_topic: latest.2,
                    latest_ordinal: ordinal,
                });
            }
        }
        rows.sort_by(|a, b| {
            b.last_activity
                .cmp(&a.last_activity)
                .then_with(|| b.key().cmp(&a.key()))
        });
        Ok(rows)
    }

    async fn thinking_agents(&self) -> Result<std::collections::HashSet<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT agent_id FROM session_detail WHERE json_extract(data, '$.is_thinking') = 1",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    async fn reset_thinking_except(&self, keep: &[crate::schema::VizierSession]) -> Result<usize> {
        let conn = self.conn.lock();
        let rows: Vec<(String, String, Option<String>, String)> = {
            let mut stmt =
                conn.prepare("SELECT agent_id, channel, topic, data FROM session_detail")?;
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let kept: std::collections::HashSet<(String, String, String)> = keep
            .iter()
            .map(|s| (s.0.clone(), s.1.to_slug(), s.2.clone().unwrap_or_default()))
            .collect();
        let mut reset = 0;
        for (agent, channel, topic, data) in rows {
            if kept.contains(&(
                agent.clone(),
                channel.clone(),
                topic.clone().unwrap_or_default(),
            )) {
                continue;
            }
            let Ok(mut detail) = serde_json::from_str::<VizierSessionDetail>(&data) else {
                continue;
            };
            if !detail.is_thinking {
                continue;
            }
            detail.is_thinking = false;
            conn.execute(
                "UPDATE session_detail SET data = ?1 WHERE agent_id = ?2 AND channel = ?3 AND topic IS ?4",
                rusqlite::params![serde_json::to_string(&detail)?, agent, channel, topic],
            )?;
            reset += 1;
        }
        Ok(reset)
    }

    async fn update_thinking_state(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
        is_thinking: bool,
    ) -> Result<()> {
        let conn = self.conn.lock();
        // Read existing
        let data: String = {
            let mut stmt = conn.prepare(
                "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            )?;
            let mut rows = stmt.query_map(
                rusqlite::params![agent_id, channel.to_slug(), topic],
                |row| {
                    let data: String = row.get(0)?;
                    Ok(data)
                },
            )?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Ok(()), // Not found, nothing to update
            }
        };

        let mut detail: VizierSessionDetail = serde_json::from_str(&data)?;
        detail.is_thinking = is_thinking;

        let new_data = serde_json::to_string(&detail)?;
        conn.execute(
            "UPDATE session_detail SET data = ?1 WHERE agent_id = ?2 AND channel = ?3 AND topic = ?4",
            rusqlite::params![new_data, agent_id, channel.to_slug(), topic],
        )?;
        Ok(())
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![agent_id, channel.to_slug(), topic],
            |row| {
                let data: String = row.get(0)?;
                Ok(data)
            },
        )?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        channel: Option<VizierChannelId>,
    ) -> Result<Vec<VizierSessionDetail>> {
        let conn = self.conn.lock();
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match &channel {
            Some(ch) => (
                "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2",
                vec![
                    Box::new(agent_id.clone()) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(ch.to_slug()),
                ],
            ),
            None => (
                "SELECT data FROM session_detail WHERE agent_id = ?1",
                vec![Box::new(agent_id.clone()) as Box<dyn rusqlite::types::ToSql>],
            ),
        };

        let mut stmt = conn.prepare(sql)?;
        let details = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<VizierSessionDetail>(&data).ok())
            .collect();
        Ok(details)
    }

    async fn delete_session(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            rusqlite::params![agent_id, channel.to_slug(), topic],
        )?;

        // The jobs launched from this topic go with it, so no orphaned rows remain to
        // authorize a piece read against. Pieces are deleted explicitly rather than trusting
        // `ON DELETE CASCADE`, which only fires with `PRAGMA foreign_keys` on.
        let origin_channel = serde_json::to_string(&channel)?;
        conn.execute(
            "DELETE FROM background_piece WHERE job_id IN (SELECT id FROM background_job \
             WHERE origin_agent = ?1 AND origin_channel = ?2 AND origin_topic IS ?3)",
            rusqlite::params![agent_id, origin_channel, topic],
        )?;
        conn.execute(
            "DELETE FROM background_job \
             WHERE origin_agent = ?1 AND origin_channel = ?2 AND origin_topic IS ?3",
            rusqlite::params![agent_id, origin_channel, topic],
        )?;

        // The topic's client tool calls, and the state its pages saved, go with it.
        conn.execute(
            "DELETE FROM client_tool_call WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3",
            rusqlite::params![agent_id, channel.to_slug(), topic],
        )?;
        // So do its scratch files.
        conn.execute(
            "DELETE FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            rusqlite::params![agent_id, channel.to_slug(), topic],
        )?;
        // And its pending turns.
        conn.execute(
            "DELETE FROM session_turn WHERE agent_id = ?1 AND channel = ?2 AND topic = COALESCE(?3, '')",
            rusqlite::params![agent_id, channel.to_slug(), topic],
        )?;

        // Its transient session-file records and message artifact references go with it; the
        // upload blobs are removed by `SessionFiles` after this commit (`specs/035-add-artifacts`).
        let session_slug = format!("{}__{}__{}", agent_id, channel.to_slug(), topic);
        conn.execute(
            "DELETE FROM session_file WHERE agent_id = ?1 AND session_slug = ?2",
            rusqlite::params![agent_id, session_slug],
        )?;
        conn.execute(
            "DELETE FROM artifact_reference WHERE kind = 'message' AND agent_id = ?1 AND session_slug = ?2",
            rusqlite::params![agent_id, session_slug],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use rusqlite::Connection;

    use super::*;
    use crate::storage::document::LocalDocumentStore;

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_artifact_schema(&conn).unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE session_detail (id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, channel TEXT NOT NULL, \
             topic TEXT, data TEXT NOT NULL);",
        )
        .unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn say(
        s: &SqliteStorage,
        n: &mut i64,
        agent: &str,
        channel: &str,
        topic: Option<&str>,
        ts: i64,
        text: &str,
    ) {
        *n += 1;
        s.conn
            .lock()
            .execute(
                "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'Request', ?6, ?1)",
                rusqlite::params![n.to_string(), agent, channel, topic, ts, serde_json::json!({ "text": text }).to_string()],
            )
            .unwrap();
    }

    fn title(s: &SqliteStorage, agent: &str, channel: VizierChannelId, topic: &str, title: &str) {
        let detail = VizierSessionDetail {
            agent_id: agent.into(),
            channel: channel.clone(),
            topic: Some(topic.into()),
            title: title.into(),
            is_thinking: false,
        };
        s.conn
            .lock()
            .execute(
                "INSERT INTO session_detail (id, agent_id, channel, topic, data) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![format!("{agent}#{}#{topic}", channel.to_slug()), agent, channel.to_slug(), topic, serde_json::to_string(&detail).unwrap()],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn conversations_are_channels_newest_first_without_the_ones_nobody_talks_on() {
        let (s, _d) = setup();
        let mut n = 0;
        let web = "http__ana__vizier-webui";
        say(&s, &mut n, "a", "discord__1", None, 100, "hello");
        say(&s, &mut n, "a", web, Some("t1"), 50, "first");
        say(&s, &mut n, "a", web, Some("t2"), 300, "second");
        say(&s, &mut n, "a", web, Some("t1"), 200, "again");
        say(&s, &mut n, "a", "task__digest__1", None, 999, "scheduled");
        say(
            &s,
            &mut n,
            "a",
            "DREAM__x__extraction",
            None,
            999,
            "dreaming",
        );
        say(&s, &mut n, "b", "telegram__9", None, 999, "someone else's");
        title(
            &s,
            "a",
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            "t2",
            "vector-db-review",
        );

        let rows = s.conversation_rows("a", None).await.unwrap();
        let shown: Vec<(&str, u64, &str, u64)> = rows
            .iter()
            .map(|r| {
                (
                    r.channel.as_str(),
                    r.topics,
                    r.latest_topic.as_str(),
                    r.latest_ordinal,
                )
            })
            .collect();
        // A WebUI topic is a row of its own (topic 2 moved last, then Discord, then topic 1 at 200).
        assert_eq!(
            shown,
            vec![
                (web, 2, "t2", 2),
                (web, 2, "t1", 1),
                ("discord__1", 1, "", 1)
            ]
        );
        assert_eq!(rows[0].title.as_deref(), Some("vector-db-review"));
        assert_eq!(rows[1].title, None);
    }

    #[tokio::test]
    async fn q_finds_a_title_or_a_message_and_treats_wildcards_as_text() {
        let (s, _d) = setup();
        let mut n = 0;
        say(
            &s,
            &mut n,
            "a",
            "discord__1",
            None,
            100,
            "the quarterly budget",
        );
        say(&s, &mut n, "a", "discord__2", None, 200, "50% off");
        say(
            &s,
            &mut n,
            "a",
            "http__ana__vizier-webui",
            Some("t1"),
            300,
            "hi",
        );
        title(
            &s,
            "a",
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            "t1",
            "Budget planning",
        );

        let ids =
            |rows: Vec<ConversationRow>| rows.into_iter().map(|r| r.channel).collect::<Vec<_>>();
        assert_eq!(
            ids(s.conversation_rows("a", Some("BUDGET")).await.unwrap()),
            vec!["http__ana__vizier-webui", "discord__1"]
        );
        // Only the topic that has the word, not its sibling.
        say(
            &s,
            &mut n,
            "a",
            "http__ana__vizier-webui",
            Some("t2"),
            400,
            "unrelated",
        );
        let both = s.conversation_rows("a", Some("budget")).await.unwrap();
        assert_eq!(
            both.iter()
                .filter(|r| r.channel.starts_with("http__"))
                .count(),
            1
        );
        assert_eq!(
            ids(s.conversation_rows("a", Some("50%")).await.unwrap()),
            vec!["discord__2"]
        );
        assert_eq!(
            ids(s.conversation_rows("a", Some("%")).await.unwrap()),
            vec!["discord__2"]
        );
        assert!(
            s.conversation_rows("a", Some("nothing like this"))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
