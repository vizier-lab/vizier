use anyhow::Result;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, ToSql};

use crate::{
    agents::agent::model::usage::TurnId,
    schema::{
        metrics::TokenCounts,
        usage::{UsageActivity, UsageEvent},
    },
    storage::{
        sqlite::SqliteStorage,
        usage::{ConversationRows, UsageEventRow, UsageGroup, UsageGrouping, UsageQuery, UsageStorage},
    },
};

const INSERT: &str = "INSERT INTO usage_event (agent_id, at, activity, channel, topic, place, turn_id, \
     round, provider, model, served_model, input_tokens, cached_input_tokens, \
     cache_creation_input_tokens, output_tokens, duration_ms, legacy) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)";

/// An event is part of a failed turn iff its turn is in `usage_failed_turn` (research R3).
const FAILED: &str = "(e.turn_id IS NOT NULL AND e.turn_id IN \
     (SELECT turn_id FROM usage_failed_turn WHERE agent_id = ?1))";

const EVENT_COLUMNS: &str = "e.agent_id, e.at, e.activity, e.channel, e.topic, e.place, e.turn_id, \
     e.round, e.provider, e.model, e.served_model, e.input_tokens, e.cached_input_tokens, \
     e.cache_creation_input_tokens, e.output_tokens, e.duration_ms, e.legacy";

fn millis_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

fn insert(conn: &Connection, e: &UsageEvent) -> rusqlite::Result<usize> {
    conn.execute(
        INSERT,
        rusqlite::params![
            e.agent_id,
            e.at.timestamp_millis(),
            e.activity.as_str(),
            e.channel,
            e.topic,
            e.place,
            e.turn_id,
            e.round,
            e.provider,
            e.model,
            e.served_model,
            e.tokens.input as i64,
            e.tokens.cached as i64,
            e.tokens.cache_write as i64,
            e.tokens.output as i64,
            e.duration_ms as i64,
            e.legacy,
        ],
    )
}

/// `WHERE` for a query, with its parameters. `?1` is always the agent id, which [`FAILED`]
/// relies on.
fn filter(q: &UsageQuery) -> (String, Vec<Box<dyn ToSql>>) {
    let mut sql = "e.agent_id = ?1 AND e.at >= ?2 AND e.at <= ?3".to_string();
    let mut params: Vec<Box<dyn ToSql>> = vec![
        Box::new(q.agent_id.clone()),
        Box::new(q.start.timestamp_millis()),
        Box::new(q.end.timestamp_millis()),
    ];
    if let Some(activity) = q.activity {
        params.push(Box::new(activity.as_str()));
        sql.push_str(&format!(" AND e.activity = ?{}", params.len()));
    }
    if let Some((channel, topic)) = &q.conversation {
        params.push(Box::new(channel.clone()));
        sql.push_str(&format!(" AND e.channel = ?{}", params.len()));
        params.push(Box::new(topic.clone()));
        sql.push_str(&format!(" AND e.topic IS ?{}", params.len()));
    }
    (sql, params)
}

fn event_from_row(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<UsageEvent> {
    let get_u64 = |i: usize| -> rusqlite::Result<u64> { Ok(row.get::<_, i64>(offset + i)?.max(0) as u64) };
    let activity: String = row.get(offset + 2)?;
    Ok(UsageEvent {
        agent_id: row.get(offset)?,
        at: millis_to_utc(row.get(offset + 1)?),
        activity: activity.parse().unwrap_or(UsageActivity::Other),
        channel: row.get(offset + 3)?,
        topic: row.get(offset + 4)?,
        place: row.get(offset + 5)?,
        turn_id: row.get(offset + 6)?,
        round: row.get(offset + 7)?,
        provider: row.get(offset + 8)?,
        model: row.get(offset + 9)?,
        served_model: row.get(offset + 10)?,
        tokens: TokenCounts {
            input: get_u64(11)?,
            cached: get_u64(12)?,
            cache_write: get_u64(13)?,
            output: get_u64(14)?,
        },
        duration_ms: get_u64(15)?,
        legacy: row.get(offset + 16)?,
    })
}

/// Called from `delete_agent` on the connection it already holds (FR-008).
pub fn delete_agent(conn: &Connection, agent_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM usage_event WHERE agent_id = ?1", [agent_id])?;
    conn.execute("DELETE FROM usage_failed_turn WHERE agent_id = ?1", [agent_id])?;
    Ok(())
}

#[async_trait::async_trait]
impl UsageStorage for SqliteStorage {
    async fn record_usage(&self, event: UsageEvent) -> Result<()> {
        let conn = self.conn.lock();
        insert(&conn, &event)?;
        Ok(())
    }

    async fn record_failed_turn(
        &self,
        turn: &TurnId,
        agent_id: &str,
        at: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO usage_failed_turn (turn_id, agent_id, at) VALUES (?1, ?2, ?3)",
            rusqlite::params![turn.as_str(), agent_id, at.timestamp_millis()],
        )?;
        Ok(())
    }

    async fn usage_groups(&self, q: &UsageQuery, grouping: UsageGrouping) -> Result<Vec<UsageGroup>> {
        // Every grouping selects the same six key columns, NULL where unused, so one row
        // reader serves them all.
        let day = "date(e.at / 1000, 'unixepoch')";
        let (keys, group_by): ([&str; 6], &str) = match grouping {
            UsageGrouping::Day => (
                [day, "e.activity", "NULL", "NULL", "NULL", "NULL"],
                "1, 2",
            ),
            UsageGrouping::Activity => (["NULL", "e.activity", "NULL", "NULL", "NULL", "NULL"], "2"),
            UsageGrouping::Model => (["NULL", "NULL", "e.served_model", "NULL", "NULL", "NULL"], "3"),
            UsageGrouping::Place => (["NULL", "NULL", "NULL", "e.place", "NULL", "NULL"], "4"),
            UsageGrouping::Conversation => (
                ["NULL", "e.activity", "NULL", "NULL", "e.channel", "e.topic"],
                "2, 5, 6",
            ),
        };
        let (where_sql, params) = filter(q);
        let sql = format!(
            "SELECT {}, {}, {}, {}, {}, {}, e.provider, e.model, {FAILED} AS failed, \
             SUM(e.input_tokens), SUM(e.cached_input_tokens), SUM(e.cache_creation_input_tokens), \
             SUM(e.output_tokens), COUNT(*), COUNT(DISTINCT e.turn_id), MAX(e.at) \
             FROM usage_event e WHERE {where_sql} \
             GROUP BY {group_by}, 7, 8, 9",
            keys[0], keys[1], keys[2], keys[3], keys[4], keys[5],
        );

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let u = |i: usize| -> rusqlite::Result<u64> { Ok(row.get::<_, i64>(i)?.max(0) as u64) };
                let day: Option<String> = row.get(0)?;
                let activity: Option<String> = row.get(1)?;
                Ok(UsageGroup {
                    day: day.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                    activity: activity.and_then(|a| a.parse().ok()),
                    served_model: row.get(2)?,
                    place: row.get(3)?,
                    channel: row.get(4)?,
                    topic: row.get(5)?,
                    provider: row.get(6)?,
                    model: row.get(7)?,
                    failed: row.get(8)?,
                    tokens: TokenCounts {
                        input: u(9)?,
                        cached: u(10)?,
                        cache_write: u(11)?,
                        output: u(12)?,
                    },
                    calls: u(13)?,
                    turns: u(14)?,
                    last_at: millis_to_utc(row.get(15)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    async fn conversation_usage(&self, q: &UsageQuery, limit: usize) -> Result<ConversationRows> {
        anyhow::ensure!(q.conversation.is_some(), "conversation_usage needs a conversation");
        let (where_sql, params) = filter(q);
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(&format!(
            "SELECT {FAILED} AS failed, {EVENT_COLUMNS} FROM usage_event e WHERE {where_sql} \
             ORDER BY (e.input_tokens + e.output_tokens) DESC, e.at DESC LIMIT {limit}"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                Ok(UsageEventRow {
                    failed: row.get(0)?,
                    event: event_from_row(row, 1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM usage_event e WHERE {where_sql}"),
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )?;
        let longest_failed_turn: Option<u32> = conn.query_row(
            &format!("SELECT MAX(e.round) FROM usage_event e WHERE {where_sql} AND {FAILED}"),
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )?;

        Ok(ConversationRows {
            rows,
            total: total.max(0) as u64,
            longest_failed_turn,
        })
    }

    async fn conversation_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_history \
             WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3)",
            rusqlite::params![agent_id, channel, topic],
            |row| row.get(0),
        )?)
    }

    async fn session_detail_exists(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_detail \
             WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3)",
            rusqlite::params![agent_id, channel, topic],
            |row| row.get(0),
        )?)
    }

    async fn session_title(
        &self,
        agent_id: &str,
        channel: &str,
        topic: Option<&str>,
    ) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let data: Option<String> = conn
            .query_row(
                "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3",
                rusqlite::params![agent_id, channel, topic],
                |row| row.get(0),
            )
            .optional()?;
        Ok(data
            .and_then(|d| serde_json::from_str::<crate::schema::VizierSessionDetail>(&d).ok())
            .map(|detail| detail.title)
            .filter(|title| !title.trim().is_empty()))
    }

    async fn insert_legacy_usage(&self, events: Vec<UsageEvent>) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for event in &events {
            insert(&tx, event)?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::storage::document::LocalDocumentStore;

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        crate::storage::sqlite::init_usage_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn at(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(ms).single().unwrap()
    }

    const DAY: i64 = 86_400_000;

    fn event(agent: &str, ms: i64, activity: UsageActivity, model: &str, turn: Option<&str>) -> UsageEvent {
        UsageEvent {
            agent_id: agent.into(),
            at: at(ms),
            activity,
            channel: Some("discord__1".into()),
            topic: None,
            place: "discord:1".into(),
            turn_id: turn.map(str::to_string),
            round: turn.map(|_| 1),
            provider: "anthropic".into(),
            model: model.into(),
            served_model: Some(format!("{model}-2025")),
            tokens: TokenCounts {
                input: 100,
                cached: 40,
                cache_write: 0,
                output: 10,
            },
            duration_ms: 5,
            legacy: false,
        }
    }

    fn query(agent: &str) -> UsageQuery {
        UsageQuery {
            agent_id: agent.into(),
            start: at(0),
            end: at(10 * DAY),
            activity: None,
            conversation: None,
        }
    }

    #[tokio::test]
    async fn groups_sum_by_their_dimension_and_split_by_model() {
        let (s, _d) = setup();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "sonnet", None)).await.unwrap();
        s.record_usage(event("a", DAY + 1, UsageActivity::Chat, "sonnet", None)).await.unwrap();
        s.record_usage(event("a", 2 * DAY, UsageActivity::Dreams, "haiku", None)).await.unwrap();
        s.record_usage(event("b", DAY, UsageActivity::Chat, "sonnet", None)).await.unwrap();

        let by_activity = s.usage_groups(&query("a"), UsageGrouping::Activity).await.unwrap();
        let chat = by_activity
            .iter()
            .find(|g| g.activity == Some(UsageActivity::Chat))
            .unwrap();
        assert_eq!(chat.calls, 2);
        assert_eq!(chat.tokens.input, 200);
        assert_eq!(by_activity.len(), 2, "agent b is not counted");

        let by_day = s.usage_groups(&query("a"), UsageGrouping::Day).await.unwrap();
        assert_eq!(by_day.len(), 2);
        assert!(by_day.iter().all(|g| g.day.is_some() && g.activity.is_some()));

        let by_model = s.usage_groups(&query("a"), UsageGrouping::Model).await.unwrap();
        assert_eq!(by_model.len(), 2);
        assert!(by_model.iter().any(|g| g.served_model.as_deref() == Some("haiku-2025")));

        let by_place = s.usage_groups(&query("a"), UsageGrouping::Place).await.unwrap();
        assert_eq!(by_place.len(), 2, "one place, two models");
        assert!(by_place.iter().all(|g| g.place.as_deref() == Some("discord:1")));

        let mut dreams = query("a");
        dreams.activity = Some(UsageActivity::Dreams);
        let only = s.usage_groups(&dreams, UsageGrouping::Activity).await.unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].model, "haiku");

        let mut early = query("a");
        early.end = at(DAY);
        assert_eq!(s.usage_groups(&early, UsageGrouping::Activity).await.unwrap()[0].calls, 1);
    }

    #[tokio::test]
    async fn a_failed_mark_counts_whether_it_lands_before_or_after_the_turns_rows() {
        let (s, _d) = setup();
        let early = TurnId::new();
        // The mark first, then the rounds: the order fire-and-forget writes can land in.
        s.record_failed_turn(&early, "a", at(DAY)).await.unwrap();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", Some(early.as_str()))).await.unwrap();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", Some(early.as_str()))).await.unwrap();
        let late = TurnId::new();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", Some(late.as_str()))).await.unwrap();
        s.record_failed_turn(&late, "a", at(DAY)).await.unwrap();
        s.record_failed_turn(&late, "a", at(DAY)).await.unwrap();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", Some("ok-turn"))).await.unwrap();

        let groups = s.usage_groups(&query("a"), UsageGrouping::Activity).await.unwrap();
        let failed = groups.iter().find(|g| g.failed).unwrap();
        assert_eq!(failed.calls, 3);
        assert_eq!(failed.turns, 2);
        assert_eq!(groups.iter().find(|g| !g.failed).unwrap().calls, 1);
    }

    #[tokio::test]
    async fn conversation_rows_are_bounded_and_counted() {
        let (s, _d) = setup();
        let turn = TurnId::new();
        for round in 1..=5u32 {
            let mut e = event("a", DAY, UsageActivity::Chat, "m", Some(turn.as_str()));
            e.round = Some(round);
            e.tokens.input = 100 * round as u64;
            s.record_usage(e).await.unwrap();
        }
        s.record_failed_turn(&turn, "a", at(DAY)).await.unwrap();

        let mut q = query("a");
        q.conversation = Some(("discord__1".into(), None));
        let rows = s.conversation_usage(&q, 2).await.unwrap();
        assert_eq!(rows.total, 5);
        assert_eq!(rows.rows.len(), 2);
        assert_eq!(rows.rows[0].event.tokens.input, 500, "heaviest first");
        assert!(rows.rows.iter().all(|r| r.failed));
        assert_eq!(rows.longest_failed_turn, Some(5));

        q.conversation = Some(("discord__2".into(), None));
        assert_eq!(s.conversation_usage(&q, 2).await.unwrap().total, 0);
    }

    #[tokio::test]
    async fn delete_agent_removes_only_that_agents_rows() {
        let (s, _d) = setup();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", Some("t"))).await.unwrap();
        s.record_failed_turn(&TurnId::new(), "a", at(DAY)).await.unwrap();
        s.record_usage(event("b", DAY, UsageActivity::Chat, "m", None)).await.unwrap();
        delete_agent(&s.conn.lock(), "a").unwrap();

        assert!(s.usage_groups(&query("a"), UsageGrouping::Activity).await.unwrap().is_empty());
        assert_eq!(s.usage_groups(&query("b"), UsageGrouping::Activity).await.unwrap().len(), 1);
        let left: i64 = s
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM usage_failed_turn", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[tokio::test]
    async fn an_old_http_place_is_rewritten_with_its_channel_slug() {
        let (s, _d) = setup();
        let mut e = event("a", DAY, UsageActivity::Chat, "m", None);
        e.channel = Some("http__alice__vizier-webui".into());
        e.topic = Some("t1".into());
        e.place = "http:alice:t1".into();
        s.record_usage(e).await.unwrap();
        s.record_usage(event("a", DAY, UsageActivity::Chat, "m", None)).await.unwrap();

        crate::storage::sqlite::init_usage_schema(&s.conn.lock()).unwrap();

        let places: Vec<String> = s
            .usage_groups(&query("a"), UsageGrouping::Place)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|g| g.place)
            .collect();
        assert!(places.contains(&"http:http__alice__vizier-webui:t1".to_string()), "{places:?}");
        assert!(places.contains(&"discord:1".to_string()), "other places untouched: {places:?}");
    }

    #[tokio::test]
    async fn legacy_rows_insert_in_one_batch() {
        let (s, _d) = setup();
        let mut e = event("a", DAY, UsageActivity::Chat, "", None);
        e.legacy = true;
        e.provider = String::new();
        s.insert_legacy_usage(vec![e.clone(), e]).await.unwrap();
        let groups = s.usage_groups(&query("a"), UsageGrouping::Model).await.unwrap();
        assert_eq!(groups[0].calls, 2);
        assert_eq!(groups[0].provider, "");
    }
}
