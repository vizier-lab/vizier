use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::ToSql;

use crate::{
    schema::{IntegrationEvent, JobKind, JobState, TaskRunState},
    storage::{
        feed::{CycleEvent, FeedStorage, FeedWindow, JobEvent, ReplyEvent, TaskRunEvent},
        sqlite::{SqliteStorage, integration::event_from_row},
    },
};

const HOUR_MS: i64 = 3_600_000;

fn millis(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

/// Parameters `?1…?4` are shared by every source, the agent ids follow as `?5…`:
/// `since`, the cursor's time, the cursor's id and the limit. `time` renders a time as the
/// column holds it (milliseconds, or text).
fn params(w: &FeedWindow, time: impl Fn(DateTime<Utc>) -> Box<dyn ToSql>) -> (Vec<Box<dyn ToSql>>, String) {
    let mut p: Vec<Box<dyn ToSql>> = vec![
        time(w.since),
        match &w.before {
            Some((at, _)) => time(*at),
            None => Box::new(Option::<i64>::None),
        },
        Box::new(w.before.as_ref().map(|(_, id)| id.clone())),
        Box::new(w.limit as i64),
    ];
    p.extend(w.agents.iter().map(|a| Box::new(a.clone()) as Box<dyn ToSql>));
    let marks = (0..w.agents.len()).map(|i| format!("?{}", i + 5)).collect::<Vec<_>>().join(", ");
    (p, marks)
}

fn ms(t: DateTime<Utc>) -> Box<dyn ToSql> {
    Box::new(t.timestamp_millis())
}

fn text(t: DateTime<Utc>) -> Box<dyn ToSql> {
    Box::new(t.to_rfc3339())
}

/// The cursor test for a source whose event ids are `id_expr` and times `at_expr`.
fn after_cursor(at_expr: &str, id_expr: &str) -> String {
    format!("(?2 IS NULL OR {at_expr} < ?2 OR ({at_expr} = ?2 AND {id_expr} < ?3))")
}

#[async_trait::async_trait]
impl FeedStorage for SqliteStorage {
    async fn feed_task_runs(&self, w: &FeedWindow) -> Result<Vec<TaskRunEvent>> {
        if w.agents.is_empty() {
            return Ok(vec![]);
        }
        let (p, marks) = params(w, ms);
        let at = "COALESCE(finished_at, ran_at)";
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, agent_id, task_slug, state, {at} FROM task_run \
             WHERE state != 'running' AND agent_id IN ({marks}) AND {at} >= ?1 AND {} \
             ORDER BY {at} DESC, 'task_run:' || id DESC LIMIT ?4",
            after_cursor(at, "'task_run:' || id"),
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(p.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, i64>(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, agent_id, task_slug, state, at)| {
                Some(TaskRunEvent { id, agent_id, task_slug, state: TaskRunState::from_str(&state)?, at: millis(at) })
            })
            .collect())
    }

    async fn feed_background_jobs(&self, w: &FeedWindow) -> Result<Vec<JobEvent>> {
        if w.agents.is_empty() {
            return Ok(vec![]);
        }
        let (p, marks) = params(w, ms);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, origin_agent, kind, state, reason, finished_at FROM background_job \
             WHERE finished_at IS NOT NULL AND state NOT IN ('running', 'reporting') \
             AND origin_agent IN ({marks}) AND finished_at >= ?1 AND {} \
             ORDER BY finished_at DESC, 'job:' || id DESC LIMIT ?4",
            after_cursor("finished_at", "'job:' || id"),
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(p.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, agent_id, kind, state, reason, at)| {
                Some(JobEvent { id, agent_id, kind: JobKind::from_str(&kind)?, state: JobState::from_str(&state)?, reason, at: millis(at) })
            })
            .collect())
    }

    async fn feed_dream_cycles(&self, w: &FeedWindow) -> Result<Vec<CycleEvent>> {
        if w.agents.is_empty() {
            return Ok(vec![]);
        }
        let (p, marks) = params(w, ms);
        let conn = self.conn.lock();
        // A cycle is as recent as its last journal entry.
        let mut stmt = conn.prepare(&format!(
            "SELECT dream_cycle_id, agent_id, COUNT(*), MAX(timestamp) AS at FROM dream_journal \
             WHERE dream_cycle_id IS NOT NULL AND agent_id IN ({marks}) \
             GROUP BY dream_cycle_id, agent_id \
             HAVING at >= ?1 AND {} \
             ORDER BY at DESC, 'dream:' || dream_cycle_id DESC LIMIT ?4",
            after_cursor("at", "'dream:' || dream_cycle_id"),
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(p.iter()), |row| {
                Ok(CycleEvent {
                    cycle_id: row.get(0)?,
                    agent_id: row.get(1)?,
                    entries: row.get::<_, i64>(2)?.max(0) as u64,
                    at: millis(row.get(3)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    async fn feed_integration_events(&self, w: &FeedWindow) -> Result<Vec<IntegrationEvent>> {
        if w.agents.is_empty() {
            return Ok(vec![]);
        }
        let (p, marks) = params(w, text);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, agent_id, integration_id, integration_name, family, actor, action, fields, \
             session, at FROM integration_event \
             WHERE agent_id IN ({marks}) AND at >= ?1 AND {} \
             ORDER BY at DESC, 'integration:' || id DESC LIMIT ?4",
            after_cursor("at", "'integration:' || id"),
        ))?;
        let events = stmt
            .query_map(rusqlite::params_from_iter(p.iter()), event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(events.into_iter().flatten().collect())
    }

    async fn feed_channel_replies(&self, w: &FeedWindow) -> Result<Vec<ReplyEvent>> {
        if w.agents.is_empty() {
            return Ok(vec![]);
        }
        let (p, marks) = params(w, ms);
        let conn = self.conn.lock();
        // One event per conversation per hour, however many replies it held.
        let id = format!("'reply:' || channel || ':' || (timestamp / {HOUR_MS})");
        let mut stmt = conn.prepare(&format!(
            "SELECT agent_id, channel, timestamp / {HOUR_MS} AS hour, COUNT(*), MAX(timestamp) AS at \
             FROM session_history \
             WHERE content_type = 'Response' AND (channel LIKE 'discord\\_\\_%' ESCAPE '\\' OR channel LIKE 'telegram\\_\\_%' ESCAPE '\\') \
             AND agent_id IN ({marks}) AND timestamp >= ?1 \
             GROUP BY agent_id, channel, hour \
             HAVING {} \
             ORDER BY at DESC, {id} DESC LIMIT ?4",
            after_cursor("at", &id),
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(p.iter()), |row| {
                Ok(ReplyEvent {
                    agent_id: row.get(0)?,
                    channel: row.get(1)?,
                    hour: row.get(2)?,
                    replies: row.get::<_, i64>(3)?.max(0) as u64,
                    at: millis(row.get(4)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
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
        crate::storage::sqlite::init_task_run_schema(&conn).unwrap();
        crate::storage::sqlite::init_background_job_schema(&conn).unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        crate::storage::sqlite::init_integration_schema(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE dream_journal (id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, dream_cycle_id TEXT, \
             stage TEXT NOT NULL, timestamp INTEGER NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn window(agents: &[&str], since: i64, before: Option<(i64, &str)>, limit: usize) -> FeedWindow {
        FeedWindow {
            agents: agents.iter().map(|a| a.to_string()).collect(),
            since: millis(since),
            before: before.map(|(at, id)| (millis(at), id.to_string())),
            limit,
        }
    }

    fn run(conn: &Connection, id: i64, agent: &str, ran: i64, finished: Option<i64>, state: &str) {
        conn.execute(
            "INSERT INTO task_run (id, agent_id, task_slug, ran_at, finished_at, session_key, state) VALUES (?1, ?2, 't', ?3, ?4, 'k', ?5)",
            rusqlite::params![id, agent, ran, finished, state],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn task_runs_are_those_that_ended_for_the_asked_agents_after_since() {
        let (s, _d) = setup();
        {
            let conn = s.conn.lock();
            run(&conn, 1, "a", 100, Some(200), "answered");
            run(&conn, 2, "a", 300, None, "running");
            run(&conn, 3, "b", 100, Some(250), "answered");
            run(&conn, 4, "a", 10, Some(20), "interrupted");
            run(&conn, 5, "c", 100, Some(260), "answered");
        }
        let got = s.feed_task_runs(&window(&["a", "b"], 150, None, 10)).await.unwrap();
        let ids: Vec<i64> = got.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![3, 1], "newest first; running, too old and other agents' runs are left out");
    }

    #[tokio::test]
    async fn the_cursor_continues_after_the_last_event_even_at_the_same_instant() {
        let (s, _d) = setup();
        {
            let conn = s.conn.lock();
            for id in 1..=4 {
                run(&conn, id, "a", 100, Some(500), "answered");
            }
        }
        let first = s.feed_task_runs(&window(&["a"], 0, None, 2)).await.unwrap();
        assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 3]);
        let last = first.last().unwrap();
        let second = s
            .feed_task_runs(&window(&["a"], 0, Some((last.at.timestamp_millis(), &format!("task_run:{}", last.id))), 2))
            .await
            .unwrap();
        assert_eq!(second.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[tokio::test]
    async fn channel_replies_collapse_per_conversation_hour_and_skip_other_channels() {
        let (s, _d) = setup();
        let hour = 3_600_000;
        {
            let conn = s.conn.lock();
            let mut n = 0;
            let mut put = |channel: &str, ts: i64, kind: &str| {
                n += 1;
                conn.execute(
                    "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) VALUES (?1, 'a', ?2, NULL, ?3, ?4, '{}', ?1)",
                    rusqlite::params![n, channel, ts, kind],
                )
                .unwrap();
            };
            put("discord__1", 10 * hour + 5, "Response");
            put("discord__1", 10 * hour + 9, "Response");
            put("discord__1", 11 * hour + 1, "Response");
            put("discord__1", 10 * hour + 7, "Request");
            put("http__ana__x", 10 * hour + 7, "Response");
            put("telegram__5", 10 * hour + 2, "Response");
        }
        let got = s.feed_channel_replies(&window(&["a"], 0, None, 10)).await.unwrap();
        let summary: Vec<(String, i64, u64)> = got.into_iter().map(|r| (r.channel, r.hour, r.replies)).collect();
        assert_eq!(
            summary,
            vec![("discord__1".into(), 11, 1), ("discord__1".into(), 10, 2), ("telegram__5".into(), 10, 1)]
        );
    }

    #[tokio::test]
    async fn a_dream_cycle_is_one_event_at_its_last_entry() {
        let (s, _d) = setup();
        {
            let conn = s.conn.lock();
            for (id, cycle, ts) in [("1", "c1", 100), ("2", "c1", 300), ("3", "c2", 50)] {
                conn.execute(
                    "INSERT INTO dream_journal (id, agent_id, dream_cycle_id, stage, timestamp, data) VALUES (?1, 'a', ?2, 'x', ?3, '{}')",
                    rusqlite::params![id, cycle, ts],
                )
                .unwrap();
            }
        }
        let got = s.feed_dream_cycles(&window(&["a"], 60, None, 10)).await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].cycle_id.as_str(), got[0].entries, got[0].at), ("c1", 2, millis(300)));
    }

    #[tokio::test]
    async fn no_agents_means_no_events() {
        let (s, _d) = setup();
        assert!(s.feed_task_runs(&window(&[], 0, None, 10)).await.unwrap().is_empty());
        assert!(s.feed_channel_replies(&window(&[], 0, None, 10)).await.unwrap().is_empty());
    }
}
