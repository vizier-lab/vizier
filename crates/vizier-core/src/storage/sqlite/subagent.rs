use chrono::{Local, TimeZone};

use crate::storage::fence::Scope;
use crate::storage::sql::{self, Connection, OptionalExtension, Row, ToSql, params};
use crate::storage::sqlite::fence::check_fence;
use crate::time::Timestamp;
use anyhow::Result;

use crate::{
    schema::{
        AgentId, Subagent, SubagentId, SubagentState, SubagentStopReason, VizierChannelId,
        VizierSession,
    },
    storage::{
        sqlite::SqlStorage,
        subagent::{Settled, StopOutcome, SubagentStorage},
    },
};

/// Every worker read selects these columns in this order.
const COLUMNS: &str = "id, agent_id, hiring_channel, hiring_topic, parent_detached_id, group_id, \
     position, group_size, worker_topic, task, timeout_secs, state, stop_reason, question, result, \
     result_truncated, error, outcome_seq, seen_seq, delivered_seq, deadline_at, remaining_ms, \
     created_at, updated_at, ended_at";

/// The hiring-conversation predicate, parameters `?1` agent, `?2` channel JSON, `?3` topic.
const HIRING: &str =
    "agent_id = ?1 AND hiring_channel = ?2 AND hiring_topic IS NOT DISTINCT FROM ?3";

/// A newest outcome nobody has taken yet (data-model.md "Deliverability"). Written without
/// `MAX`/`GREATEST` so it reads the same on both backends.
const DELIVERABLE: &str = "state IN ('asking', 'answered', 'failed', 'timed_out') \
     AND outcome_seq > seen_seq AND outcome_seq > delivered_seq";

/// Not final.
const UNFINISHED: &str = "state IN ('running', 'asking')";

fn millis(ms: i64) -> Option<Timestamp> {
    Local.timestamp_millis_opt(ms).single()
}

fn channel_column(channel: &VizierChannelId) -> Result<String> {
    Ok(serde_json::to_string(channel)?)
}

/// A row that no longer parses (an unknown state from a newer build, say) is skipped rather than
/// failing the whole read.
fn from_row(row: &Row) -> sql::Result<Option<Subagent>> {
    let id: String = row.get(0)?;
    let agent_id: String = row.get(1)?;
    let channel: String = row.get(2)?;
    let topic: Option<String> = row.get(3)?;
    let parent_detached_id: Option<String> = row.get(4)?;
    let group_id: String = row.get(5)?;
    let position: i64 = row.get(6)?;
    let group_size: i64 = row.get(7)?;
    let worker_topic: String = row.get(8)?;
    let task: String = row.get(9)?;
    let timeout_secs: i64 = row.get(10)?;
    let state: String = row.get(11)?;
    let stop_reason: Option<String> = row.get(12)?;
    let question: Option<String> = row.get(13)?;
    let result: Option<String> = row.get(14)?;
    let result_truncated: i64 = row.get(15)?;
    let error: Option<String> = row.get(16)?;
    let outcome_seq: i64 = row.get(17)?;
    let seen_seq: i64 = row.get(18)?;
    let delivered_seq: i64 = row.get(19)?;
    let deadline_at: Option<i64> = row.get(20)?;
    let remaining_ms: Option<i64> = row.get(21)?;
    let created_at: i64 = row.get(22)?;
    let updated_at: i64 = row.get(23)?;
    let ended_at: Option<i64> = row.get(24)?;

    let (Some(state), Some(created_at), Some(updated_at), Ok(channel)) = (
        SubagentState::parse(&state),
        millis(created_at),
        millis(updated_at),
        serde_json::from_str::<VizierChannelId>(&channel),
    ) else {
        return Ok(None);
    };
    Ok(Some(Subagent {
        id,
        hiring_session: VizierSession(agent_id.clone(), channel, topic),
        agent_id,
        parent_detached_id,
        group_id,
        position: position.clamp(0, u32::MAX as i64) as u32,
        group_size: group_size.clamp(0, u32::MAX as i64) as u32,
        worker_topic,
        task,
        timeout_secs: timeout_secs.max(0) as u64,
        state,
        stop_reason: stop_reason.as_deref().and_then(SubagentStopReason::parse),
        question,
        result,
        result_truncated: result_truncated != 0,
        error,
        outcome_seq,
        seen_seq,
        delivered_seq,
        deadline_at: deadline_at.and_then(millis),
        remaining_ms,
        created_at,
        updated_at,
        ended_at: ended_at.and_then(millis),
    }))
}

/// Workers matching `filter` (a `WHERE` clause), ordered by `order`.
fn load(
    conn: &Connection,
    filter: &str,
    order: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<Subagent>> {
    let sql = format!("SELECT {COLUMNS} FROM subagent WHERE {filter} ORDER BY {order}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params, from_row)?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    Ok(rows)
}

fn one(conn: &Connection, id: &str) -> Result<Option<Subagent>> {
    let sql = format!("SELECT {COLUMNS} FROM subagent WHERE id = ?1");
    Ok(conn.query_row(&sql, [id], from_row).optional()?.flatten())
}

/// The agent a worker belongs to, read in the writer's transaction. A missing row has nothing
/// to protect.
fn agent_of(tx: &Connection, id: &str) -> Result<Option<String>> {
    Ok(tx
        .query_all("SELECT agent_id FROM subagent WHERE id = ?1", params![id])?
        .first()
        .map(|row| row.get::<_, String>(0))
        .transpose()?)
}

fn hiring_params(session: &VizierSession) -> Result<(String, String, Option<String>)> {
    Ok((session.0.clone(), channel_column(&session.1)?, session.2.clone()))
}

#[async_trait::async_trait]
impl SubagentStorage for SqlStorage {
    async fn open_subagents(&self, rows: Vec<Subagent>) -> Result<()> {
        let Some(first) = rows.first() else {
            return Ok(());
        };
        let scope = Scope::Agent(first.agent_id.clone());
        let (owner_node, owner_token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        for row in &rows {
            anyhow::ensure!(
                row.agent_id == first.agent_id,
                "a spawned group belongs to one agent"
            );
            tx.execute(
                &format!(
                    "INSERT INTO subagent ({COLUMNS}, owner_node, owner_token) VALUES \
                     (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                     ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)"
                ),
                params![
                    row.id,
                    row.agent_id,
                    channel_column(&row.hiring_session.1)?,
                    row.hiring_session.2,
                    row.parent_detached_id,
                    row.group_id,
                    row.position as i64,
                    row.group_size as i64,
                    row.worker_topic,
                    row.task,
                    row.timeout_secs as i64,
                    row.state.as_str(),
                    row.stop_reason.map(|r| r.as_str()),
                    row.question,
                    row.result,
                    row.result_truncated as i64,
                    row.error,
                    row.outcome_seq,
                    row.seen_seq,
                    row.delivered_seq,
                    row.deadline_at.map(|at| at.timestamp_millis()),
                    row.remaining_ms,
                    row.created_at.timestamp_millis(),
                    row.updated_at.timestamp_millis(),
                    row.ended_at.map(|at| at.timestamp_millis()),
                    owner_node,
                    owner_token,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn ask_subagent(
        &self,
        id: &str,
        question: &str,
        remaining_ms: i64,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(None);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        let changed = tx.execute(
            "UPDATE subagent SET state = 'asking', question = ?1, remaining_ms = ?2, \
             deadline_at = NULL, outcome_seq = outcome_seq + 1, updated_at = ?3 \
             WHERE id = ?4 AND state = 'running'",
            params![question, remaining_ms.max(0), at.timestamp_millis(), id],
        )?;
        let row = if changed == 1 { one(&tx, id)? } else { None };
        tx.commit()?;
        Ok(row)
    }

    async fn reply_subagent(
        &self,
        id: &str,
        deadline_at: Timestamp,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(None);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        let changed = tx.execute(
            "UPDATE subagent SET state = 'running', deadline_at = ?1, remaining_ms = NULL, \
             updated_at = ?2 WHERE id = ?3 AND state = 'asking'",
            params![deadline_at.timestamp_millis(), at.timestamp_millis(), id],
        )?;
        let row = if changed == 1 { one(&tx, id)? } else { None };
        tx.commit()?;
        Ok(row)
    }

    async fn settle_subagent(
        &self,
        id: &str,
        settled: Settled,
        at: Timestamp,
    ) -> Result<Option<Subagent>> {
        let (state, result, truncated, error) = match settled {
            Settled::Answered { result, truncated } => {
                (SubagentState::Answered, Some(result), truncated, None)
            }
            Settled::Failed(error) => (SubagentState::Failed, None, false, Some(error)),
            Settled::TimedOut(error) => (SubagentState::TimedOut, None, false, Some(error)),
        };
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(None);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        // The `state = 'running'` predicate is the exactly-once guarantee: the turn ending, the
        // timer and a stop all move a worker out of `running`, and only one can match.
        let changed = tx.execute(
            "UPDATE subagent SET state = ?1, result = ?2, result_truncated = ?3, error = ?4, \
             deadline_at = NULL, outcome_seq = outcome_seq + 1, updated_at = ?5, ended_at = ?5 \
             WHERE id = ?6 AND state = 'running'",
            params![
                state.as_str(),
                result,
                truncated as i64,
                error,
                at.timestamp_millis(),
                id
            ],
        )?;
        let row = if changed == 1 { one(&tx, id)? } else { None };
        tx.commit()?;
        Ok(row)
    }

    async fn stop_subagent(
        &self,
        id: &str,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<StopOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(StopOutcome::NotFound);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        let changed = tx.execute(
            &format!(
                "UPDATE subagent SET state = 'stopped', stop_reason = ?1, deadline_at = NULL, \
                 updated_at = ?2, ended_at = ?2 WHERE id = ?3 AND {UNFINISHED}"
            ),
            params![reason.as_str(), at.timestamp_millis(), id],
        )?;
        let row = one(&tx, id)?;
        tx.commit()?;
        Ok(match (changed, row) {
            (_, None) => StopOutcome::NotFound,
            (1, Some(row)) => StopOutcome::Stopped(row),
            (_, Some(row)) => StopOutcome::AlreadyFinished(row),
        })
    }

    async fn mark_subagents_seen(&self, ids: &[SubagentId]) -> Result<Vec<SubagentId>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let mut marked = vec![];
        let mut checked: Vec<String> = vec![];
        for id in ids {
            let Some(agent) = agent_of(&tx, id)? else {
                continue;
            };
            if !checked.contains(&agent) {
                check_fence(&tx, &self.fences, &Scope::Agent(agent.clone()))?;
                checked.push(agent);
            }
            let changed = tx.execute(
                &format!("UPDATE subagent SET seen_seq = outcome_seq WHERE id = ?1 AND {DELIVERABLE}"),
                params![id],
            )?;
            if changed == 1 {
                marked.push(id.clone());
            }
        }
        tx.commit()?;
        Ok(marked)
    }

    async fn claim_subagent_deliveries(&self, session: &VizierSession) -> Result<Vec<Subagent>> {
        let (agent, channel, topic) = hiring_params(session)?;
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &Scope::Agent(agent.clone()))?;
        let candidates = load(
            &tx,
            &format!("{HIRING} AND {DELIVERABLE}"),
            "updated_at, id",
            &[&agent, &channel, &topic],
        )?;
        let mut claimed = Vec::with_capacity(candidates.len());
        for mut worker in candidates {
            // Re-checked per row, so a wait (or another claimer) that took the outcome first
            // leaves it out.
            let changed = tx.execute(
                &format!(
                    "UPDATE subagent SET delivered_seq = outcome_seq WHERE id = ?1 AND {DELIVERABLE}"
                ),
                params![worker.id],
            )?;
            if changed == 1 {
                worker.delivered_seq = worker.outcome_seq;
                claimed.push(worker);
            }
        }
        tx.commit()?;
        Ok(claimed)
    }

    async fn subagent_by_worker(&self, worker_topic: &str) -> Result<Option<Subagent>> {
        let conn = self.conn.lock()?;
        let sql = format!("SELECT {COLUMNS} FROM subagent WHERE worker_topic = ?1");
        Ok(conn
            .query_row(&sql, [worker_topic], from_row)
            .optional()?
            .flatten())
    }

    async fn get_subagent(&self, id: &str) -> Result<Option<Subagent>> {
        let conn = self.conn.lock()?;
        one(&conn, id)
    }

    async fn list_subagents(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<Subagent>> {
        let (agent, channel, topic) = hiring_params(session)?;
        let limit = final_limit as i64;
        let conn = self.conn.lock()?;
        let mut rows = load(
            &conn,
            &format!(
                "{HIRING} AND ({UNFINISHED} OR id IN (SELECT id FROM subagent WHERE {HIRING} \
                 AND NOT ({UNFINISHED}) ORDER BY ended_at DESC, id DESC LIMIT ?4))"
            ),
            "created_at, id",
            &[&agent, &channel, &topic, &limit],
        )?;
        // Unfinished first, then the final ones; each oldest first.
        rows.sort_by(|a, b| {
            a.state
                .is_final()
                .cmp(&b.state.is_final())
                .then_with(|| a.created_at.cmp(&b.created_at))
                .then_with(|| a.position.cmp(&b.position))
        });
        Ok(rows)
    }

    async fn reown_running_subagents(
        &self,
        agent_id: &AgentId,
        below: u64,
    ) -> Result<Vec<Subagent>> {
        let scope = Scope::Agent(agent_id.clone());
        let (node, token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        let rows = load(
            &tx,
            "agent_id = ?1 AND state = 'running' AND (owner_token IS NULL OR owner_token < ?2)",
            "created_at, id",
            &[agent_id, &(below as i64)],
        )?;
        let mut owned = vec![];
        for row in rows {
            let changed = tx.execute(
                "UPDATE subagent SET owner_node = ?1, owner_token = ?2 \
                 WHERE id = ?3 AND state = 'running'",
                params![node, token, row.id],
            )?;
            if changed == 1 {
                owned.push(row);
            }
        }
        tx.commit()?;
        Ok(owned)
    }

    async fn subagent_delivery_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            &format!(
                "SELECT DISTINCT hiring_channel, hiring_topic FROM subagent \
                 WHERE agent_id = ?1 AND {DELIVERABLE}"
            ),
            params![agent_id],
        )?;
        let mut sessions = vec![];
        for row in rows {
            let channel: String = row.get(0)?;
            let topic: Option<String> = row.get(1)?;
            if let Ok(channel) = serde_json::from_str::<VizierChannelId>(&channel) {
                sessions.push(VizierSession(agent_id.clone(), channel, topic));
            }
        }
        Ok(sessions)
    }

    async fn stop_session_subagents(
        &self,
        session: &VizierSession,
        reason: SubagentStopReason,
        at: Timestamp,
    ) -> Result<Vec<Subagent>> {
        let (agent, channel, topic) = hiring_params(session)?;
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &Scope::Agent(agent.clone()))?;
        let unfinished = load(
            &tx,
            &format!("{HIRING} AND {UNFINISHED}"),
            "created_at, id",
            &[&agent, &channel, &topic],
        )?;
        let mut stopped = vec![];
        for row in unfinished {
            let changed = tx.execute(
                &format!(
                    "UPDATE subagent SET state = 'stopped', stop_reason = ?1, deadline_at = NULL, \
                     updated_at = ?2, ended_at = ?2 WHERE id = ?3 AND {UNFINISHED}"
                ),
                params![reason.as_str(), at.timestamp_millis(), row.id],
            )?;
            if changed == 1
                && let Some(row) = one(&tx, &row.id)?
            {
                stopped.push(row);
            }
        }
        tx.commit()?;
        Ok(stopped)
    }

    async fn delete_session_subagents(&self, session: &VizierSession) -> Result<Vec<Subagent>> {
        let (agent, channel, topic) = hiring_params(session)?;
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let rows = load(&tx, HIRING, "created_at, id", &[&agent, &channel, &topic])?;
        tx.execute(
            &format!("DELETE FROM subagent WHERE {HIRING}"),
            params![agent, channel, topic],
        )?;
        tx.commit()?;
        Ok(rows)
    }

    async fn delete_agent_subagents(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute("DELETE FROM subagent WHERE agent_id = ?1", params![agent_id])?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::document::BlobDocumentStore;
    use crate::storage::sql::SqlStore;

    pub(crate) fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_subagent_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage =
            SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
        (storage, dir)
    }

    pub(crate) fn hiring(topic: &str) -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("dani".into(), "chat".into()),
            Some(topic.into()),
        )
    }

    pub(crate) fn worker(id: &str, session: VizierSession, position: u32) -> Subagent {
        let now = crate::time::now();
        Subagent {
            id: id.into(),
            agent_id: session.0.clone(),
            hiring_session: session,
            parent_detached_id: None,
            group_id: "g_1".into(),
            position,
            group_size: 3,
            worker_topic: format!("topic-{id}"),
            task: format!("task of {id}"),
            timeout_secs: 600,
            state: SubagentState::Running,
            stop_reason: None,
            question: None,
            result: None,
            result_truncated: false,
            error: None,
            outcome_seq: 0,
            seen_seq: 0,
            delivered_seq: 0,
            deadline_at: Some(now + chrono::Duration::seconds(600)),
            remaining_ms: None,
            created_at: now,
            updated_at: now,
            ended_at: None,
        }
    }

    fn answered() -> Settled {
        Settled::Answered { result: "done".into(), truncated: false }
    }

    async fn open(storage: &SqlStorage, ids: &[&str], session: VizierSession) {
        let rows = ids
            .iter()
            .enumerate()
            .map(|(i, id)| worker(id, session.clone(), i as u32 + 1))
            .collect();
        storage.open_subagents(rows).await.unwrap();
    }

    #[tokio::test]
    async fn a_group_round_trips_and_is_found_by_its_worker_topic() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1", "s_2"], hiring("t")).await;
        let got = storage.get_subagent("s_2").await.unwrap().unwrap();
        assert_eq!(got.position, 2);
        assert_eq!(got.hiring_session, hiring("t"));
        assert_eq!(got.state, SubagentState::Running);
        let by_worker = storage.subagent_by_worker("topic-s_1").await.unwrap().unwrap();
        assert_eq!(by_worker.id, "s_1");
        assert!(storage.get_subagent("s_nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ask_reply_and_settle_follow_the_state_machine() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1"], hiring("t")).await;
        let now = crate::time::now();

        assert!(storage.reply_subagent("s_1", now, now).await.unwrap().is_none(), "not asking");
        let asked = storage.ask_subagent("s_1", "which?", 5_000, now).await.unwrap().unwrap();
        assert_eq!(asked.state, SubagentState::Asking);
        assert_eq!(asked.question.as_deref(), Some("which?"));
        assert_eq!(asked.remaining_ms, Some(5_000));
        assert_eq!(asked.deadline_at, None);
        assert_eq!(asked.outcome_seq, 1);
        assert!(storage.settle_subagent("s_1", answered(), now).await.unwrap().is_none(), "an asking worker is not settled by its turn ending");

        let replied = storage.reply_subagent("s_1", now, now).await.unwrap().unwrap();
        assert_eq!(replied.state, SubagentState::Running);
        assert_eq!(replied.remaining_ms, None);
        assert!(replied.deadline_at.is_some());

        let second = storage.ask_subagent("s_1", "and?", 1_000, now).await.unwrap().unwrap();
        assert_eq!(second.outcome_seq, 2, "a second question is a new outcome");
        storage.reply_subagent("s_1", now, now).await.unwrap().unwrap();

        let done = storage.settle_subagent("s_1", answered(), now).await.unwrap().unwrap();
        assert_eq!(done.state, SubagentState::Answered);
        assert_eq!(done.result.as_deref(), Some("done"));
        assert_eq!(done.outcome_seq, 3);
        assert!(done.ended_at.is_some());
        assert!(storage.settle_subagent("s_1", Settled::Failed("x".into()), now).await.unwrap().is_none(), "final stays final");
        assert!(storage.ask_subagent("s_1", "late?", 0, now).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stop_wins_over_a_later_settle_and_reports_a_final_row() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1", "s_2"], hiring("t")).await;
        let now = crate::time::now();
        let StopOutcome::Stopped(row) =
            storage.stop_subagent("s_1", SubagentStopReason::ByAgent, now).await.unwrap()
        else {
            panic!("expected a stop");
        };
        assert_eq!(row.state, SubagentState::Stopped);
        assert_eq!(row.stop_reason, Some(SubagentStopReason::ByAgent));
        assert!(storage.settle_subagent("s_1", answered(), now).await.unwrap().is_none());

        storage.settle_subagent("s_2", answered(), now).await.unwrap().unwrap();
        assert!(matches!(
            storage.stop_subagent("s_2", SubagentStopReason::ByAgent, now).await.unwrap(),
            StopOutcome::AlreadyFinished(row) if row.state == SubagentState::Answered
        ));
        assert_eq!(
            storage.stop_subagent("s_x", SubagentStopReason::ByAgent, now).await.unwrap(),
            StopOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn an_outcome_is_handed_out_once_by_a_read_or_a_claim() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1", "s_2", "s_3"], hiring("t")).await;
        open(&storage, &["s_9"], hiring("other")).await;
        let now = crate::time::now();
        assert!(storage.claim_subagent_deliveries(&hiring("t")).await.unwrap().is_empty());

        storage.settle_subagent("s_1", answered(), now).await.unwrap();
        storage.ask_subagent("s_2", "which?", 1_000, now).await.unwrap();
        storage.settle_subagent("s_3", answered(), now).await.unwrap();
        storage.settle_subagent("s_9", answered(), now).await.unwrap();

        // A wait already returned s_3's answer to the main agent.
        assert_eq!(storage.mark_subagents_seen(&["s_3".into()]).await.unwrap(), vec!["s_3".to_string()]);
        let claimed = storage.claim_subagent_deliveries(&hiring("t")).await.unwrap();
        let mut ids: Vec<_> = claimed.iter().map(|w| w.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["s_1", "s_2"]);
        assert!(storage.claim_subagent_deliveries(&hiring("t")).await.unwrap().is_empty());
        assert!(storage.mark_subagents_seen(&["s_1".into()]).await.unwrap().is_empty(), "delivered is not seen again");
        assert_eq!(
            storage.subagent_delivery_sessions(&"agent-1".to_string()).await.unwrap(),
            vec![hiring("other")]
        );

        // The reply, then a second question: deliverable again.
        storage.reply_subagent("s_2", now, now).await.unwrap();
        storage.ask_subagent("s_2", "and?", 1_000, now).await.unwrap();
        let again = storage.claim_subagent_deliveries(&hiring("t")).await.unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].question.as_deref(), Some("and?"));
    }

    #[tokio::test]
    async fn a_stopped_worker_is_never_delivered() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1"], hiring("t")).await;
        let now = crate::time::now();
        storage.ask_subagent("s_1", "which?", 1_000, now).await.unwrap();
        storage.stop_subagent("s_1", SubagentStopReason::ByAgent, now).await.unwrap();
        assert!(storage.claim_subagent_deliveries(&hiring("t")).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn racing_claimers_split_the_outcomes() {
        let (storage, _dir) = setup();
        let storage = Arc::new(storage);
        let ids: Vec<String> = (0..20).map(|i| format!("s_{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        open(&storage, &refs, hiring("t")).await;
        for id in &ids {
            storage.settle_subagent(id, answered(), crate::time::now()).await.unwrap();
        }
        let claims = (0..4).map(|_| {
            let storage = storage.clone();
            tokio::spawn(async move { storage.claim_subagent_deliveries(&hiring("t")).await.unwrap() })
        });
        let mut got: Vec<String> = vec![];
        for claim in claims {
            got.extend(claim.await.unwrap().into_iter().map(|w| w.id));
        }
        got.sort();
        let before = got.len();
        got.dedup();
        assert_eq!(before, 20);
        assert_eq!(got.len(), 20);
    }

    #[tokio::test]
    async fn the_list_is_unfinished_first_then_the_last_final_ones() {
        let (storage, _dir) = setup();
        open(&storage, &["s_0", "s_1", "s_2", "s_3", "s_4"], hiring("t")).await;
        let now = crate::time::now();
        for id in ["s_0", "s_1", "s_2"] {
            storage.settle_subagent(id, answered(), now).await.unwrap();
        }
        storage.ask_subagent("s_4", "which?", 0, now).await.unwrap();
        let listed = storage.list_subagents(&hiring("t"), 2).await.unwrap();
        let ids: Vec<_> = listed.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids.len(), 4, "{ids:?}");
        assert_eq!(&ids[..2], &["s_3", "s_4"], "running and asking first");
    }

    #[tokio::test]
    async fn conversation_stop_and_delete() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1", "s_2"], hiring("t")).await;
        open(&storage, &["s_3"], hiring("u")).await;
        let now = crate::time::now();
        storage.settle_subagent("s_2", answered(), now).await.unwrap();
        let stopped = storage
            .stop_session_subagents(&hiring("t"), SubagentStopReason::ConversationDeleted, now)
            .await
            .unwrap();
        assert_eq!(stopped.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), vec!["s_1"]);
        assert_eq!(stopped[0].stop_reason, Some(SubagentStopReason::ConversationDeleted));
        assert_eq!(storage.delete_session_subagents(&hiring("t")).await.unwrap().len(), 2);
        assert!(storage.get_subagent("s_1").await.unwrap().is_none());
        storage.delete_agent_subagents("agent-1").await.unwrap();
        assert!(storage.get_subagent("s_3").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn recovery_reowns_only_running_rows() {
        let (storage, _dir) = setup();
        open(&storage, &["s_1", "s_2"], hiring("t")).await;
        storage.settle_subagent("s_2", answered(), crate::time::now()).await.unwrap();
        let owned = storage.reown_running_subagents(&"agent-1".to_string(), 1).await.unwrap();
        assert_eq!(owned.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), vec!["s_1"]);
    }
}
