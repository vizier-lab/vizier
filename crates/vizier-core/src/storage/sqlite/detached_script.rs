use chrono::{Local, TimeZone};

use crate::storage::fence::Scope;
use crate::storage::sql::{self, Connection, OptionalExtension, Row, ToSql, params};
use crate::storage::sqlite::fence::check_fence;
use crate::time::Timestamp;
use anyhow::Result;

use crate::{
    schema::{
        AgentId, Canceller, DetachedScript, DetachedScriptId, DetachedScriptState,
        VizierChannelId, VizierSession,
    },
    storage::{
        detached_script::{CancelOutcome, DetachedScriptStorage},
        sqlite::SqlStorage,
    },
};

/// Every script read selects these columns in this order.
const COLUMNS: &str = "id, agent_id, session_channel, session_topic, parent_id, depth, intent, \
     state, timeout_secs, code, tool_calls, created_at, finished_at, outcome, delivered_at, \
     cancelled_by";

/// The session predicate, parameters `?1` agent, `?2` channel JSON, `?3` topic.
const SESSION: &str =
    "agent_id = ?1 AND session_channel = ?2 AND session_topic IS NOT DISTINCT FROM ?3";

/// Final, not cancelled, not delivered.
const DELIVERABLE: &str =
    "state NOT IN ('running', 'cancelled') AND delivered_at IS NULL";

fn millis(ms: i64) -> Option<Timestamp> {
    Local.timestamp_millis_opt(ms).single()
}

fn channel_column(channel: &VizierChannelId) -> Result<String> {
    Ok(serde_json::to_string(channel)?)
}

/// A row that no longer parses (an unknown state from a newer build, say) is skipped rather
/// than failing the whole read.
fn from_row(row: &Row) -> sql::Result<Option<DetachedScript>> {
    let id: String = row.get(0)?;
    let agent_id: String = row.get(1)?;
    let channel: String = row.get(2)?;
    let topic: Option<String> = row.get(3)?;
    let parent_id: Option<String> = row.get(4)?;
    let depth: i64 = row.get(5)?;
    let intent: String = row.get(6)?;
    let state: String = row.get(7)?;
    let timeout_secs: i64 = row.get(8)?;
    let code: String = row.get(9)?;
    let tool_calls: String = row.get(10)?;
    let created_at: i64 = row.get(11)?;
    let finished_at: Option<i64> = row.get(12)?;
    let outcome: Option<String> = row.get(13)?;
    let delivered_at: Option<i64> = row.get(14)?;
    let cancelled_by: Option<String> = row.get(15)?;

    let (Some(state), Some(created_at), Ok(channel)) = (
        DetachedScriptState::parse(&state),
        millis(created_at),
        serde_json::from_str::<VizierChannelId>(&channel),
    ) else {
        return Ok(None);
    };
    Ok(Some(DetachedScript {
        id,
        session: VizierSession(agent_id.clone(), channel, topic),
        agent_id,
        parent_id,
        depth: depth.clamp(0, u8::MAX as i64) as u8,
        intent,
        state,
        timeout_secs: timeout_secs.max(0) as u64,
        code,
        tool_calls: serde_json::from_str(&tool_calls).unwrap_or(serde_json::Value::Array(vec![])),
        created_at,
        finished_at: finished_at.and_then(millis),
        outcome: outcome.and_then(|text| serde_json::from_str(&text).ok()),
        delivered_at: delivered_at.and_then(millis),
        cancelled_by: cancelled_by.as_deref().and_then(Canceller::from_column),
    }))
}

/// Scripts matching `filter` (a `WHERE` clause), ordered by `order`.
fn load(
    conn: &Connection,
    filter: &str,
    order: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<DetachedScript>> {
    let sql = format!("SELECT {COLUMNS} FROM detached_script WHERE {filter} ORDER BY {order}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params, from_row)?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    Ok(rows)
}

/// The agent a script belongs to, read in the writer's transaction. A missing row has nothing
/// to protect.
fn agent_of(tx: &Connection, id: &str) -> Result<Option<String>> {
    Ok(tx
        .query_all("SELECT agent_id FROM detached_script WHERE id = ?1", params![id])?
        .first()
        .map(|row| row.get::<_, String>(0))
        .transpose()?)
}

fn state_of(tx: &Connection, id: &str) -> Result<Option<DetachedScriptState>> {
    Ok(tx
        .query_all("SELECT state FROM detached_script WHERE id = ?1", params![id])?
        .first()
        .map(|row| row.get::<_, String>(0))
        .transpose()?
        .as_deref()
        .and_then(DetachedScriptState::parse))
}

#[async_trait::async_trait]
impl DetachedScriptStorage for SqlStorage {
    async fn open_detached_script(&self, script: DetachedScript) -> Result<()> {
        let scope = Scope::Agent(script.agent_id.clone());
        let (owner_node, owner_token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        tx.execute(
            "INSERT INTO detached_script (id, agent_id, session_channel, session_topic, \
             parent_id, depth, intent, state, timeout_secs, code, tool_calls, created_at, \
             finished_at, outcome, delivered_at, cancelled_by, owner_node, owner_token) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                script.id,
                script.agent_id,
                channel_column(&script.session.1)?,
                script.session.2,
                script.parent_id,
                script.depth as i64,
                script.intent,
                script.state.as_str(),
                script.timeout_secs as i64,
                script.code,
                serde_json::to_string(&script.tool_calls)?,
                script.created_at.timestamp_millis(),
                script.finished_at.map(|at| at.timestamp_millis()),
                script.outcome.as_ref().map(serde_json::to_string).transpose()?,
                script.delivered_at.map(|at| at.timestamp_millis()),
                script.cancelled_by.as_ref().map(Canceller::to_column),
                owner_node,
                owner_token,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn record_detached_progress(
        &self,
        id: &str,
        tool_calls: serde_json::Value,
    ) -> Result<Option<DetachedScriptState>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(None);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        tx.execute(
            "UPDATE detached_script SET tool_calls = ?1 WHERE id = ?2 AND state = 'running'",
            params![serde_json::to_string(&tool_calls)?, id],
        )?;
        let state = state_of(&tx, id)?;
        tx.commit()?;
        Ok(state)
    }

    async fn close_detached_script(
        &self,
        id: &str,
        state: DetachedScriptState,
        outcome: serde_json::Value,
        tool_calls: serde_json::Value,
        at: Timestamp,
    ) -> Result<bool> {
        anyhow::ensure!(
            state.is_deliverable(),
            "close_detached_script takes a final, non-cancelled state, not {}",
            state.as_str()
        );
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(false);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        // The `state = 'running'` predicate is the whole exactly-once guarantee: the task and
        // a cancel both move a script out of `running`, and only one of them can match.
        let changed = tx.execute(
            "UPDATE detached_script SET state = ?1, outcome = ?2, tool_calls = ?3, \
             finished_at = ?4 WHERE id = ?5 AND state = 'running'",
            params![
                state.as_str(),
                serde_json::to_string(&outcome)?,
                serde_json::to_string(&tool_calls)?,
                at.timestamp_millis(),
                id,
            ],
        )?;
        tx.commit()?;
        Ok(changed == 1)
    }

    async fn cancel_detached_script(
        &self,
        id: &str,
        by: Canceller,
        at: Timestamp,
    ) -> Result<CancelOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent) = agent_of(&tx, id)? else {
            return Ok(CancelOutcome::NotFound);
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent))?;
        let by = by.to_column();
        let at = at.timestamp_millis();
        let cancel = |id: &str| -> Result<bool> {
            Ok(tx.execute(
                "UPDATE detached_script SET state = 'cancelled', cancelled_by = ?1, \
                 finished_at = ?2 WHERE id = ?3 AND state = 'running'",
                params![by, at, id],
            )? == 1)
        };
        if !cancel(id)? {
            let state = state_of(&tx, id)?.unwrap_or(DetachedScriptState::Cancelled);
            tx.commit()?;
            return Ok(CancelOutcome::AlreadyFinished(state));
        }

        // Walk the launch tree breadth first. A child whose agent another process owns is
        // still marked here; its task sees `cancelled` at its next progress write.
        let mut cancelled = vec![id.to_string()];
        let mut frontier = vec![id.to_string()];
        while let Some(parent) = frontier.pop() {
            let children: Vec<String> = tx
                .query_all(
                    "SELECT id FROM detached_script WHERE parent_id = ?1 ORDER BY created_at, id",
                    params![parent],
                )?
                .iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<sql::Result<_>>()?;
            for child in children {
                if cancel(&child)? {
                    cancelled.push(child.clone());
                }
                // A finished child may still have running grandchildren.
                frontier.push(child);
            }
        }
        tx.commit()?;
        Ok(CancelOutcome::Cancelled(cancelled))
    }

    async fn claim_deliverable_scripts(
        &self,
        session: &VizierSession,
    ) -> Result<Vec<DetachedScript>> {
        let channel = channel_column(&session.1)?;
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &Scope::Agent(session.0.clone()))?;
        let candidates = load(
            &tx,
            &format!("{SESSION} AND {DELIVERABLE}"),
            "finished_at, id",
            &[&session.0, &channel, &session.2],
        )?;
        let now = crate::time::now();
        let mut claimed = Vec::with_capacity(candidates.len());
        for mut script in candidates {
            // Re-checked per row so a concurrent claimer (another connection on PostgreSQL)
            // that took it first leaves it out: the update waits on its row lock and then
            // sees `delivered_at` set.
            let changed = tx.execute(
                "UPDATE detached_script SET delivered_at = ?1 WHERE id = ?2 AND delivered_at IS NULL",
                params![now.timestamp_millis(), script.id],
            )?;
            if changed == 1 {
                script.delivered_at = Some(now);
                claimed.push(script);
            }
        }
        tx.commit()?;
        Ok(claimed)
    }

    async fn list_detached_scripts(
        &self,
        session: &VizierSession,
        final_limit: usize,
    ) -> Result<Vec<DetachedScript>> {
        let channel = channel_column(&session.1)?;
        let limit = final_limit as i64;
        let conn = self.conn.lock()?;
        let mut rows = load(
            &conn,
            &format!(
                "{SESSION} AND (state = 'running' OR id IN (SELECT id FROM detached_script \
                 WHERE {SESSION} AND state <> 'running' ORDER BY finished_at DESC, id DESC \
                 LIMIT ?4))"
            ),
            "created_at, id",
            &[&session.0, &channel, &session.2, &limit],
        )?;
        rows.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
        Ok(rows)
    }

    async fn get_detached_script(&self, id: &str) -> Result<Option<DetachedScript>> {
        let conn = self.conn.lock()?;
        let sql = format!("SELECT {COLUMNS} FROM detached_script WHERE id = ?1");
        Ok(conn.query_row(&sql, [id], from_row).optional()?.flatten())
    }

    async fn interrupt_running_scripts(
        &self,
        agent_id: &str,
        below: Option<u64>,
        ids: Option<&[DetachedScriptId]>,
    ) -> Result<Vec<DetachedScript>> {
        let scope = Scope::Agent(agent_id.to_owned());
        let (node, token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        let agent = agent_id.to_owned();
        let running = match below {
            Some(below) => load(
                &tx,
                "agent_id = ?1 AND state = 'running' AND (owner_token IS NULL OR owner_token < ?2)",
                "created_at, id",
                &[&agent, &(below as i64)],
            )?,
            None => load(&tx, "agent_id = ?1 AND state = 'running'", "created_at, id", &[&agent])?,
        };
        let now = crate::time::now();
        let mut moved = vec![];
        for mut script in running {
            if ids.is_some_and(|ids| !ids.contains(&script.id)) {
                continue;
            }
            let changed = tx.execute(
                "UPDATE detached_script SET state = 'interrupted', finished_at = ?1, \
                 outcome = ?2, owner_node = ?3, owner_token = ?4 \
                 WHERE id = ?5 AND state = 'running'",
                params![
                    now.timestamp_millis(),
                    serde_json::to_string(&serde_json::json!(INTERRUPTED))?,
                    node,
                    token,
                    script.id,
                ],
            )?;
            if changed == 1 {
                script.state = DetachedScriptState::Interrupted;
                script.finished_at = Some(now);
                script.outcome = Some(serde_json::json!(INTERRUPTED));
                moved.push(script);
            }
        }
        tx.commit()?;
        Ok(moved)
    }

    async fn delete_session_scripts(
        &self,
        session: &VizierSession,
    ) -> Result<Vec<DetachedScriptId>> {
        let channel = channel_column(&session.1)?;
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let running: Vec<String> = tx
            .query_all(
                &format!("SELECT id FROM detached_script WHERE {SESSION} AND state = 'running'"),
                params![session.0, channel, session.2],
            )?
            .iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<sql::Result<_>>()?;
        tx.execute(
            &format!("DELETE FROM detached_script WHERE {SESSION}"),
            params![session.0, channel, session.2],
        )?;
        tx.commit()?;
        Ok(running)
    }

    async fn delete_agent_scripts(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute("DELETE FROM detached_script WHERE agent_id = ?1", params![agent_id])?;
        Ok(())
    }

    async fn deliverable_script_sessions(&self, agent_id: &AgentId) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            &format!(
                "SELECT DISTINCT session_channel, session_topic FROM detached_script \
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

    async fn count_running_scripts(
        &self,
        agent_id: &str,
        channel: &VizierChannelId,
    ) -> Result<std::collections::HashMap<Option<String>, usize>> {
        self.running_by_topic(agent_id, channel)
    }
}

impl SqlStorage {
    fn running_by_topic(
        &self,
        agent_id: &str,
        channel: &VizierChannelId,
    ) -> Result<std::collections::HashMap<Option<String>, usize>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            "SELECT session_topic, COUNT(*) FROM detached_script              WHERE agent_id = ?1 AND session_channel = ?2 AND state = 'running' GROUP BY session_topic",
            params![agent_id, channel_column(channel)?],
        )?;
        let mut out = std::collections::HashMap::new();
        for row in rows {
            let topic: Option<String> = row.get(0)?;
            let count: i64 = row.get(1)?;
            out.insert(topic, count.max(0) as usize);
        }
        Ok(out)
    }
}

/// The outcome of a script whose process or agent instance stopped under it.
pub const INTERRUPTED: &str = "the server restarted or the agent was restarted while the script ran";

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::document::BlobDocumentStore;
    use crate::storage::sql::SqlStore;

    pub(crate) fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_detached_script_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage =
            SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
        (storage, dir)
    }

    pub(crate) fn session(topic: &str) -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("dani".into(), "chat".into()),
            Some(topic.into()),
        )
    }

    pub(crate) fn script(id: &str, session: VizierSession, parent: Option<&str>) -> DetachedScript {
        DetachedScript {
            id: id.into(),
            agent_id: session.0.clone(),
            session,
            parent_id: parent.map(str::to_string),
            depth: 0,
            intent: format!("intent of {id}"),
            state: DetachedScriptState::Running,
            timeout_secs: 600,
            code: "1".into(),
            tool_calls: serde_json::json!([]),
            created_at: crate::time::now(),
            finished_at: None,
            outcome: None,
            delivered_at: None,
            cancelled_by: None,
        }
    }

    async fn close(storage: &SqlStorage, id: &str) -> bool {
        storage
            .close_detached_script(
                id,
                DetachedScriptState::Succeeded,
                serde_json::json!({"ok": true}),
                serde_json::json!([{"seq": 1, "name": "shell_exec"}]),
                crate::time::now(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_script_round_trips() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        let got = storage.get_detached_script("d_1").await.unwrap().unwrap();
        assert_eq!(got.state, DetachedScriptState::Running);
        assert_eq!(got.session, session("t"));
        assert_eq!(got.intent, "intent of d_1");
        assert!(storage.get_detached_script("d_nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn progress_reports_the_current_state() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        let state = storage
            .record_detached_progress("d_1", serde_json::json!([{"seq": 1}]))
            .await
            .unwrap();
        assert_eq!(state, Some(DetachedScriptState::Running));
        assert_eq!(storage.get_detached_script("d_1").await.unwrap().unwrap().tool_call_count(), 1);

        storage
            .cancel_detached_script("d_1", Canceller::Agent("agent-1".into()), crate::time::now())
            .await
            .unwrap();
        let state = storage.record_detached_progress("d_1", serde_json::json!([])).await.unwrap();
        assert_eq!(state, Some(DetachedScriptState::Cancelled));
        // A cancelled row keeps the list it had.
        assert_eq!(storage.get_detached_script("d_1").await.unwrap().unwrap().tool_call_count(), 1);
        assert_eq!(storage.record_detached_progress("d_gone", serde_json::json!([])).await.unwrap(), None);
    }

    #[tokio::test]
    async fn cancel_and_close_race_and_one_wins() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        assert!(close(&storage, "d_1").await);
        let outcome = storage
            .cancel_detached_script("d_1", Canceller::Agent("agent-1".into()), crate::time::now())
            .await
            .unwrap();
        assert_eq!(outcome, CancelOutcome::AlreadyFinished(DetachedScriptState::Succeeded));

        storage.open_detached_script(script("d_2", session("t"), None)).await.unwrap();
        let outcome = storage
            .cancel_detached_script("d_2", Canceller::Agent("agent-1".into()), crate::time::now())
            .await
            .unwrap();
        assert_eq!(outcome, CancelOutcome::Cancelled(vec!["d_2".into()]));
        assert!(!close(&storage, "d_2").await, "a cancelled script cannot also finish");
        assert_eq!(
            storage
                .cancel_detached_script("d_x", Canceller::Agent("agent-1".into()), crate::time::now())
                .await
                .unwrap(),
            CancelOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn cancel_cascades_down_the_launch_tree() {
        let (storage, _dir) = setup();
        for (id, parent) in [("d_a", None), ("d_b", Some("d_a")), ("d_c", Some("d_b")), ("d_d", Some("d_a"))] {
            storage.open_detached_script(script(id, session("t"), parent)).await.unwrap();
        }
        // d_d finished on its own; its own children would still be walked.
        assert!(close(&storage, "d_d").await);
        let outcome = storage
            .cancel_detached_script("d_a", Canceller::Agent("agent-1".into()), crate::time::now())
            .await
            .unwrap();
        let CancelOutcome::Cancelled(mut ids) = outcome else { panic!("{outcome:?}") };
        ids.sort();
        assert_eq!(ids, vec!["d_a", "d_b", "d_c"]);
        assert_eq!(
            storage.get_detached_script("d_d").await.unwrap().unwrap().state,
            DetachedScriptState::Succeeded
        );
    }

    #[tokio::test]
    async fn a_claim_takes_each_script_once() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        storage.open_detached_script(script("d_2", session("t"), None)).await.unwrap();
        storage.open_detached_script(script("d_3", session("other"), None)).await.unwrap();
        assert!(storage.claim_deliverable_scripts(&session("t")).await.unwrap().is_empty());

        assert!(close(&storage, "d_1").await);
        assert!(close(&storage, "d_3").await);
        storage
            .cancel_detached_script("d_2", Canceller::Agent("agent-1".into()), crate::time::now())
            .await
            .unwrap();

        let claimed = storage.claim_deliverable_scripts(&session("t")).await.unwrap();
        assert_eq!(claimed.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["d_1"]);
        assert!(claimed[0].delivered_at.is_some());
        assert!(storage.claim_deliverable_scripts(&session("t")).await.unwrap().is_empty());
        assert_eq!(
            storage.deliverable_script_sessions(&"agent-1".to_string()).await.unwrap(),
            vec![session("other")]
        );
    }

    #[tokio::test]
    async fn racing_claimers_split_the_rows() {
        let (storage, _dir) = setup();
        let storage = Arc::new(storage);
        for i in 0..20 {
            let id = format!("d_{i}");
            storage.open_detached_script(script(&id, session("t"), None)).await.unwrap();
            assert!(close(&storage, &id).await);
        }
        let claims = (0..4).map(|_| {
            let storage = storage.clone();
            tokio::spawn(async move { storage.claim_deliverable_scripts(&session("t")).await.unwrap() })
        });
        let mut ids: Vec<String> = vec![];
        for claim in claims {
            ids.extend(claim.await.unwrap().into_iter().map(|s| s.id));
        }
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, 20, "every script claimed");
        assert_eq!(ids.len(), 20, "none claimed twice");
    }

    #[tokio::test]
    async fn the_list_is_running_plus_the_last_final_ones() {
        let (storage, _dir) = setup();
        for i in 0..5 {
            let id = format!("d_{i}");
            storage.open_detached_script(script(&id, session("t"), None)).await.unwrap();
            if i < 4 {
                assert!(close(&storage, &id).await);
            }
        }
        let listed = storage.list_detached_scripts(&session("t"), 2).await.unwrap();
        let ids: Vec<_> = listed.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), 3, "{ids:?}");
        assert!(ids.contains(&"d_4"), "the running one");
    }

    #[tokio::test]
    async fn interrupt_moves_only_running_rows_and_they_become_deliverable() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        storage.open_detached_script(script("d_2", session("t"), None)).await.unwrap();
        assert!(close(&storage, "d_2").await);
        let moved = storage.interrupt_running_scripts("agent-1", None, None).await.unwrap();
        assert_eq!(moved.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["d_1"]);
        assert_eq!(moved[0].state, DetachedScriptState::Interrupted);
        assert!(storage.interrupt_running_scripts("agent-1", None, None).await.unwrap().is_empty());
        assert_eq!(storage.claim_deliverable_scripts(&session("t")).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn deletes_remove_rows_and_name_the_running_ones() {
        let (storage, _dir) = setup();
        storage.open_detached_script(script("d_1", session("t"), None)).await.unwrap();
        storage.open_detached_script(script("d_2", session("t"), None)).await.unwrap();
        storage.open_detached_script(script("d_3", session("u"), None)).await.unwrap();
        assert!(close(&storage, "d_2").await);
        assert_eq!(storage.delete_session_scripts(&session("t")).await.unwrap(), vec!["d_1".to_string()]);
        assert!(storage.get_detached_script("d_2").await.unwrap().is_none());
        storage.delete_agent_scripts("agent-1").await.unwrap();
        assert!(storage.get_detached_script("d_3").await.unwrap().is_none());
    }
}
