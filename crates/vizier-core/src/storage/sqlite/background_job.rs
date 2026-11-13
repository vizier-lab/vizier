use crate::time::Timestamp;
use std::collections::HashMap;

use crate::storage::sql::{self, Connection, OptionalExtension, Row, ToSql, params};
use anyhow::Result;
use crate::storage::fence::{Fences, Scope};
use crate::storage::sqlite::fence::check_fence;
use chrono::{TimeZone, Local};

use crate::{
    schema::{
        AgentId, BackgroundJob, BackgroundPiece, Canceller, JobKind, JobState, PieceState, TopicId,
        VizierChannelId, VizierSession,
    },
    storage::{
        background_job::{BackgroundJobStorage, InterruptedJob},
        sqlite::SqlStorage,
    },
};

/// Every job read selects these columns in this order.
const JOB_COLUMNS: &str = "id, kind, origin_agent, origin_channel, origin_topic, depth, \
     timeout_secs, created_at, finished_at, state, cancelled_by, reason";

fn millis_to_utc(ms: i64) -> Option<Timestamp> {
    Local.timestamp_millis_opt(ms).single()
}

fn channel_to_column(channel: &VizierChannelId) -> Result<String> {
    Ok(serde_json::to_string(channel)?)
}

/// A job row without its pieces. Rows that no longer parse (an unknown state written by a
/// newer build, say) are skipped rather than failing the whole read.
fn job_from_row(row: &Row) -> sql::Result<Option<BackgroundJob>> {
    let id: String = row.get(0)?;
    let kind: String = row.get(1)?;
    let origin_agent: String = row.get(2)?;
    let origin_channel: String = row.get(3)?;
    let origin_topic: Option<String> = row.get(4)?;
    let depth: i64 = row.get(5)?;
    let timeout_secs: i64 = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let finished_at: Option<i64> = row.get(8)?;
    let state: String = row.get(9)?;
    let cancelled_by: Option<String> = row.get(10)?;
    let reason: Option<String> = row.get(11)?;

    let (Some(kind), Some(state), Some(created_at), Ok(channel)) = (
        JobKind::from_str(&kind),
        JobState::from_str(&state),
        millis_to_utc(created_at),
        serde_json::from_str::<VizierChannelId>(&origin_channel),
    ) else {
        return Ok(None);
    };

    Ok(Some(BackgroundJob {
        id,
        kind,
        origin: VizierSession(origin_agent, channel, origin_topic),
        depth: depth.clamp(0, u8::MAX as i64) as u8,
        timeout_secs: timeout_secs.max(0) as u64,
        created_at,
        finished_at: finished_at.and_then(millis_to_utc),
        state,
        cancelled_by: cancelled_by.as_deref().and_then(Canceller::from_column),
        reason,
        pieces: vec![],
    }))
}

fn load_pieces(conn: &Connection, job_id: &str) -> Result<Vec<BackgroundPiece>> {
    let mut stmt = conn.prepare(
        "SELECT ordinal, prompt, executor_agent, session_channel, session_topic, started_at, \
         finished_at, state, reason FROM background_piece WHERE job_id = ?1 ORDER BY ordinal",
    )?;
    let pieces = stmt
        .query_map([job_id], |row| {
            let ordinal: i64 = row.get(0)?;
            let prompt: String = row.get(1)?;
            let executor: String = row.get(2)?;
            let channel: String = row.get(3)?;
            let topic: String = row.get(4)?;
            let started_at: i64 = row.get(5)?;
            let finished_at: Option<i64> = row.get(6)?;
            let state: String = row.get(7)?;
            let reason: Option<String> = row.get(8)?;

            let (Some(state), Some(started_at), Ok(channel)) = (
                PieceState::from_str(&state),
                millis_to_utc(started_at),
                serde_json::from_str::<VizierChannelId>(&channel),
            ) else {
                return Ok(None);
            };

            Ok(Some(BackgroundPiece {
                ordinal: ordinal.max(0) as u32,
                prompt,
                session: VizierSession(executor, channel, Some(topic)),
                started_at,
                finished_at: finished_at.and_then(millis_to_utc),
                state,
                reason,
            }))
        })?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    Ok(pieces)
}

/// Jobs matching `filter` (a `WHERE` clause over `background_job`), oldest first, with pieces.
fn load_jobs(conn: &Connection, filter: &str, params: &[&dyn ToSql]) -> Result<Vec<BackgroundJob>> {
    let sql =
        format!("SELECT {JOB_COLUMNS} FROM background_job WHERE {filter} ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let jobs: Vec<BackgroundJob> = stmt
        .query_map(params, job_from_row)?
        .filter_map(|r| r.ok())
        .flatten()
        .collect();

    jobs.into_iter()
        .map(|mut job| {
            job.pieces = load_pieces(conn, &job.id)?;
            Ok(job)
        })
        .collect()
}

const IN_FLIGHT: &str = "state IN ('running', 'reporting')";

/// Refuse a write of job `job_id` when a newer owner holds its launching agent's scope. The agent
/// is read from the row in the writer's transaction; a missing row has nothing to protect.
fn check_job_fence(tx: &Connection, fences: &Fences, job_id: &str) -> Result<()> {
    let agent = tx
        .query_all(
            "SELECT origin_agent FROM background_job WHERE id = ?1",
            params![job_id],
        )?
        .first()
        .map(|row| row.get::<_, String>(0))
        .transpose()?;
    match agent {
        Some(agent) => check_fence(tx, fences, &Scope::Agent(agent)),
        None => Ok(()),
    }
}

#[async_trait::async_trait]
impl BackgroundJobStorage for SqlStorage {
    async fn open_background_job(&self, job: BackgroundJob) -> Result<()> {
        // A job is owned by its launching agent's scope: its runner lives in that process.
        let scope = Scope::Agent(job.origin.0.clone());
        let (owner_node, owner_token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        tx.execute(
            "INSERT INTO background_job (id, kind, origin_agent, origin_channel, origin_topic, \
             depth, timeout_secs, created_at, finished_at, state, cancelled_by, reason, \
             owner_node, owner_token) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job.id,
                job.kind.as_str(),
                job.origin.0,
                channel_to_column(&job.origin.1)?,
                job.origin.2,
                job.depth as i64,
                job.timeout_secs as i64,
                job.created_at.timestamp_millis(),
                job.finished_at.map(|at| at.timestamp_millis()),
                job.state.as_str(),
                job.cancelled_by.as_ref().map(Canceller::to_column),
                job.reason,
                owner_node,
                owner_token,
            ],
        )?;
        for piece in &job.pieces {
            tx.execute(
                "INSERT INTO background_piece (job_id, ordinal, prompt, executor_agent, \
                 session_channel, session_topic, started_at, finished_at, state, reason) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    job.id,
                    piece.ordinal as i64,
                    piece.prompt,
                    piece.session.0,
                    channel_to_column(&piece.session.1)?,
                    piece.session.2.clone().unwrap_or_default(),
                    piece.started_at.timestamp_millis(),
                    piece.finished_at.map(|at| at.timestamp_millis()),
                    piece.state.as_str(),
                    piece.reason,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn close_background_piece(
        &self,
        job_id: &str,
        ordinal: u32,
        state: PieceState,
        reason: Option<String>,
        finished_at: Timestamp,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        check_job_fence(&tx, &self.fences, job_id)?;
        tx.execute(
            "UPDATE background_piece SET state = ?1, reason = ?2, finished_at = ?3 \
             WHERE job_id = ?4 AND ordinal = ?5 AND state = 'running'",
            params![
                state.as_str(),
                reason,
                finished_at.timestamp_millis(),
                job_id,
                ordinal as i64,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn transition_background_job(
        &self,
        job_id: &str,
        from: JobState,
        to: JobState,
        cancelled_by: Option<String>,
        reason: Option<String>,
        at: Timestamp,
    ) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_job_fence(&tx, &self.fences, job_id)?;
        // `reporting` is not terminal, so it leaves `finished_at` unset.
        let finished_at = (to != JobState::Reporting).then(|| at.timestamp_millis());
        // The `state = from` predicate is the whole exactly-once guarantee: the runner and a
        // cancel both move a job out of `running`, and only one of them can match.
        let changed = tx.execute(
            "UPDATE background_job SET state = ?1, finished_at = ?2, \
             cancelled_by = COALESCE(?3, cancelled_by), reason = COALESCE(?4, reason) \
             WHERE id = ?5 AND state = ?6",
            params![
                to.as_str(),
                finished_at,
                cancelled_by,
                reason,
                job_id,
                from.as_str(),
            ],
        )?;
        if changed == 1 && to == JobState::Cancelled {
            tx.execute(
                "UPDATE background_piece SET state = 'cancelled', finished_at = ?1 \
                 WHERE job_id = ?2 AND state = 'running'",
                params![at.timestamp_millis(), job_id],
            )?;
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    async fn background_job_by_piece(
        &self,
        executor_agent: &str,
        piece_topic: &str,
    ) -> Result<Option<BackgroundJob>> {
        let job_id: Option<String> = {
            let conn = self.conn.lock()?;
            conn.query_row(
                "SELECT job_id FROM background_piece WHERE executor_agent = ?1 AND session_topic = ?2 \
                 LIMIT 1",
                params![executor_agent, piece_topic],
                |row| row.get(0),
            )
            .optional()?
        };
        match job_id {
            Some(id) => self.get_background_job(&id).await,
            None => Ok(None),
        }
    }

    async fn get_background_job(&self, job_id: &str) -> Result<Option<BackgroundJob>> {
        let conn = self.conn.lock()?;
        let sql = format!("SELECT {JOB_COLUMNS} FROM background_job WHERE id = ?1");
        let job = conn
            .query_row(&sql, [job_id], job_from_row)
            .optional()?
            .flatten();
        match job {
            Some(mut job) => {
                job.pieces = load_pieces(&conn, &job.id)?;
                Ok(Some(job))
            }
            None => Ok(None),
        }
    }

    async fn list_running_background_jobs(
        &self,
        origin: VizierSession,
    ) -> Result<Vec<BackgroundJob>> {
        let conn = self.conn.lock()?;
        let channel = channel_to_column(&origin.1)?;
        load_jobs(
            &conn,
            &format!(
                "origin_agent = ?1 AND origin_channel = ?2 AND origin_topic IS NOT DISTINCT FROM ?3 \
                 AND {IN_FLIGHT}"
            ),
            &[&origin.0, &channel, &origin.2],
        )
    }

    async fn list_agent_running_background_jobs(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<BackgroundJob>> {
        let conn = self.conn.lock()?;
        load_jobs(
            &conn,
            &format!("origin_agent = ?1 AND {IN_FLIGHT}"),
            &[&agent_id],
        )
    }

    async fn count_running_background_jobs(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
    ) -> Result<HashMap<Option<TopicId>, usize>> {
        let conn = self.conn.lock()?;
        let sql = format!(
            "SELECT origin_topic, COUNT(*) FROM background_job \
             WHERE origin_agent = ?1 AND origin_channel = ?2 AND {IN_FLIGHT} \
             GROUP BY origin_topic"
        );
        let mut stmt = conn.prepare(&sql)?;
        let counts = stmt
            .query_map(params![agent_id, channel_to_column(&channel)?], |row| {
                let topic: Option<String> = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((topic, count.max(0) as usize))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(counts)
    }

    async fn open_background_jobs(&self) -> Result<Vec<BackgroundJob>> {
        let conn = self.conn.lock()?;
        load_jobs(&conn, IN_FLIGHT, &[])
    }

    async fn interrupt_open_background_jobs_except(
        &self,
        keep: &[String],
    ) -> Result<Vec<InterruptedJob>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let now = crate::time::now().timestamp_millis();
        let open: Vec<(String, String, String, String, Option<String>)> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT id, origin_agent, kind, origin_channel, origin_topic \
                 FROM background_job WHERE {IN_FLIGHT}"
            ))?;
            stmt.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<sql::Result<_>>()?
        };
        let mut swept = vec![];
        for (id, origin_agent, kind, channel, topic) in
            open.iter().filter(|(id, ..)| !keep.contains(id))
        {
            tx.execute(
                "UPDATE background_piece SET state = 'interrupted', finished_at = ?1 \
                 WHERE job_id = ?2 AND state = 'running'",
                params![now, id],
            )?;
            tx.execute(
                "UPDATE background_job SET state = 'interrupted', finished_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
            if let (Some(kind), Ok(channel)) = (
                crate::schema::JobKind::from_str(kind),
                serde_json::from_str::<VizierChannelId>(channel),
            ) {
                swept.push(InterruptedJob {
                    id: id.clone(),
                    agent_id: origin_agent.clone(),
                    kind,
                    origin: VizierSession(origin_agent.clone(), channel, topic.clone()),
                });
            }
        }
        tx.commit()?;
        Ok(swept)
    }

    async fn expired_background_jobs(&self, origin: &str, below: u64) -> Result<Vec<BackgroundJob>> {
        let conn = self.conn.lock()?;
        let (origin, below) = (origin.to_owned(), below as i64);
        load_jobs(
            &conn,
            &format!("{IN_FLIGHT} AND origin_agent = ?1 AND (owner_token IS NULL OR owner_token < ?2)"),
            &[&origin, &below],
        )
    }

    async fn interrupt_expired_background_jobs_except(
        &self,
        origin: &str,
        below: u64,
        keep: &[String],
    ) -> Result<Vec<InterruptedJob>> {
        let scope = Scope::Agent(origin.to_owned());
        let (node, token) = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        let now = crate::time::now().timestamp_millis();
        let open: Vec<(String, String, String, String, Option<String>)> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT id, origin_agent, kind, origin_channel, origin_topic \
                 FROM background_job WHERE {IN_FLIGHT} AND origin_agent = ?1 \
                 AND (owner_token IS NULL OR owner_token < ?2) ORDER BY created_at, id"
            ))?;
            stmt.query_map(params![origin, below as i64], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<sql::Result<_>>()?
        };
        let mut swept = vec![];
        for (id, origin_agent, kind, channel, topic) in &open {
            if keep.contains(id) {
                // Kept for `BackgroundJobs::recover_jobs`: still open, now ours.
                tx.execute(
                    "UPDATE background_job SET owner_node = ?2, owner_token = ?3 WHERE id = ?1",
                    params![id, node, token],
                )?;
                continue;
            }
            tx.execute(
                "UPDATE background_piece SET state = 'interrupted', finished_at = ?1 \
                 WHERE job_id = ?2 AND state = 'running'",
                params![now, id],
            )?;
            tx.execute(
                "UPDATE background_job SET state = 'interrupted', finished_at = ?1, \
                 owner_node = ?3, owner_token = ?4 WHERE id = ?2",
                params![now, id, node, token],
            )?;
            if let (Some(kind), Ok(channel)) = (
                crate::schema::JobKind::from_str(kind),
                serde_json::from_str::<VizierChannelId>(channel),
            ) {
                swept.push(InterruptedJob {
                    id: id.clone(),
                    agent_id: origin_agent.clone(),
                    kind,
                    origin: VizierSession(origin_agent.clone(), channel, topic.clone()),
                });
            }
        }
        tx.commit()?;
        Ok(swept)
    }

    async fn interrupt_open_background_jobs(&self) -> Result<usize> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let now = crate::time::now().timestamp_millis();
        tx.execute(
            "UPDATE background_piece SET state = 'interrupted', finished_at = ?1 \
             WHERE state = 'running'",
            [now],
        )?;
        let swept = tx.execute(
            &format!(
                "UPDATE background_job SET state = 'interrupted', finished_at = ?1 WHERE {IN_FLIGHT}"
            ),
            [now],
        )?;
        tx.commit()?;
        Ok(swept)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::storage::sql::SqlStore;

    use super::*;
    use crate::storage::document::BlobDocumentStore;

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::storage::sqlite::init_background_job_schema(&conn).unwrap();
            SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn origin() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("dani".into(), "chat".into()),
            Some("topic-1".into()),
        )
    }

    fn job(id: &str, pieces: u32) -> BackgroundJob {
        let now = crate::time::now();
        BackgroundJob {
            id: id.into(),
            kind: JobKind::Batch,
            origin: origin(),
            depth: 0,
            timeout_secs: 600,
            created_at: now,
            finished_at: None,
            state: JobState::Running,
            cancelled_by: None,
            reason: None,
            pieces: (0..pieces)
                .map(|ordinal| BackgroundPiece {
                    ordinal,
                    prompt: format!("task {ordinal}"),
                    session: VizierSession(
                        "agent-1".into(),
                        VizierChannelId::Subagent,
                        Some(format!("{id}-{ordinal}")),
                    ),
                    started_at: now,
                    finished_at: None,
                    state: PieceState::Running,
                    reason: None,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn open_then_get_round_trips_the_pieces_in_order() {
        let (storage, _dir) = setup();
        storage
            .open_background_job(job("b-000001", 3))
            .await
            .unwrap();

        let loaded = storage
            .get_background_job("b-000001")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.origin, origin());
        assert_eq!(loaded.state, JobState::Running);
        let ordinals: Vec<u32> = loaded.pieces.iter().map(|p| p.ordinal).collect();
        assert_eq!(ordinals, vec![0, 1, 2]);
        assert_eq!(loaded.pieces[1].prompt, "task 1");
        assert_eq!(
            loaded.pieces[2].session.2.as_deref(),
            Some("b-000001-2"),
            "the piece's own topic is kept"
        );

        let running = storage
            .list_running_background_jobs(origin())
            .await
            .unwrap();
        assert_eq!(running.len(), 1);
        let counts = storage
            .count_running_background_jobs("agent-1".into(), origin().1)
            .await
            .unwrap();
        assert_eq!(counts.get(&Some("topic-1".to_string())), Some(&1));
    }

    /// FR-029: the runner and a cancel both try to move the job out of `running`; exactly
    /// one of them wins.
    #[tokio::test]
    async fn the_running_guard_has_exactly_one_winner() {
        let (storage, _dir) = setup();
        storage
            .open_background_job(job("b-000002", 1))
            .await
            .unwrap();

        let now = crate::time::now();
        let (reporting, cancelled) = tokio::join!(
            storage.transition_background_job(
                "b-000002",
                JobState::Running,
                JobState::Reporting,
                None,
                None,
                now
            ),
            storage.transition_background_job(
                "b-000002",
                JobState::Running,
                JobState::Cancelled,
                Some("agent:agent-1".into()),
                Some("test".into()),
                now
            ),
        );
        assert!(
            reporting.unwrap() ^ cancelled.unwrap(),
            "exactly one transition out of running succeeds"
        );
    }

    #[tokio::test]
    async fn cancelling_closes_running_pieces_only() {
        let (storage, _dir) = setup();
        storage
            .open_background_job(job("b-000003", 2))
            .await
            .unwrap();
        storage
            .close_background_piece("b-000003", 0, PieceState::Answered, None, crate::time::now())
            .await
            .unwrap();

        let won = storage
            .transition_background_job(
                "b-000003",
                JobState::Running,
                JobState::Cancelled,
                Some("person:dani".into()),
                Some("no longer needed".into()),
                crate::time::now(),
            )
            .await
            .unwrap();
        assert!(won);

        let loaded = storage
            .get_background_job("b-000003")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.state, JobState::Cancelled);
        assert_eq!(loaded.cancelled_by, Some(Canceller::Person("dani".into())));
        assert_eq!(loaded.reason.as_deref(), Some("no longer needed"));
        assert_eq!(loaded.pieces[0].state, PieceState::Answered);
        assert_eq!(loaded.pieces[1].state, PieceState::Cancelled);
        assert!(
            storage
                .list_running_background_jobs(origin())
                .await
                .unwrap()
                .is_empty()
        );

        // A late close of a cancelled piece cannot overwrite it.
        storage
            .close_background_piece("b-000003", 1, PieceState::Answered, None, crate::time::now())
            .await
            .unwrap();
        let loaded = storage
            .get_background_job("b-000003")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.pieces[1].state, PieceState::Cancelled);
    }

    #[tokio::test]
    async fn the_sweep_interrupts_running_and_reporting_jobs_and_their_running_pieces() {
        let (storage, _dir) = setup();
        storage
            .open_background_job(job("b-00000a", 2))
            .await
            .unwrap();
        storage
            .open_background_job(job("b-00000b", 1))
            .await
            .unwrap();
        storage
            .open_background_job(job("b-00000c", 1))
            .await
            .unwrap();
        storage
            .close_background_piece("b-00000a", 0, PieceState::Answered, None, crate::time::now())
            .await
            .unwrap();
        // b: every piece done, moved to reporting.
        storage
            .close_background_piece("b-00000b", 0, PieceState::Answered, None, crate::time::now())
            .await
            .unwrap();
        storage
            .transition_background_job(
                "b-00000b",
                JobState::Running,
                JobState::Reporting,
                None,
                None,
                crate::time::now(),
            )
            .await
            .unwrap();
        // c: already reported, must be untouched.
        storage
            .close_background_piece("b-00000c", 0, PieceState::Answered, None, crate::time::now())
            .await
            .unwrap();
        for (from, to) in [
            (JobState::Running, JobState::Reporting),
            (JobState::Reporting, JobState::Reported),
        ] {
            storage
                .transition_background_job("b-00000c", from, to, None, None, crate::time::now())
                .await
                .unwrap();
        }

        let swept = storage.interrupt_open_background_jobs().await.unwrap();
        assert_eq!(swept, 2);

        let a = storage
            .get_background_job("b-00000a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.state, JobState::Interrupted);
        assert_eq!(a.pieces[0].state, PieceState::Answered);
        assert_eq!(a.pieces[1].state, PieceState::Interrupted);
        let b = storage
            .get_background_job("b-00000b")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b.state, JobState::Interrupted);
        let c = storage
            .get_background_job("b-00000c")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.state, JobState::Reported);
    }

    /// SC-004: 10,000 racing compare-and-set attempts still have exactly one winner.
    #[test]
    fn ten_thousand_postgres_transitions_have_one_winner() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let Some(store) = crate::tests_support::postgres_store() else {
            return;
        };
        const WRITERS: usize = 50;
        const EACH: usize = 200;
        let dir = tempfile::tempdir().unwrap();
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(storage.open_background_job(job("b-race-10000", 1)))
            .unwrap();

        let winners = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..WRITERS {
                let storage = storage.clone();
                let winners = &winners;
                scope.spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    runtime.block_on(async move {
                        for _ in 0..EACH {
                            if storage
                                .transition_background_job(
                                    "b-race-10000",
                                    JobState::Running,
                                    JobState::Reporting,
                                    None,
                                    None,
                                    crate::time::now(),
                                )
                                .await
                                .unwrap()
                            {
                                winners.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    });
                });
            }
        });
        assert_eq!(winners.load(Ordering::Relaxed), 1);
    }

    crate::postgres_tests!(
        open_then_get_round_trips_the_pieces_in_order,
        the_running_guard_has_exactly_one_winner,
        cancelling_closes_running_pieces_only,
        the_sweep_interrupts_running_and_reporting_jobs_and_their_running_pieces,
    );
}
