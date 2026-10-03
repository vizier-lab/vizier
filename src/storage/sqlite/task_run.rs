use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};

use crate::{
    schema::{AgentId, TaskRun, TaskRunState},
    storage::{sqlite::SqliteStorage, task_run::TaskRunStorage},
};

/// Every ordered read selects these columns in this order.
const RUN_COLUMNS: &str = "id, agent_id, task_slug, ran_at, finished_at, session_key, state";

fn millis_to_utc(ms: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(ms).single()
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<TaskRun>> {
    let id: i64 = row.get(0)?;
    let agent_id: String = row.get(1)?;
    let task_slug: String = row.get(2)?;
    let ran_at_ms: i64 = row.get(3)?;
    let finished_at_ms: Option<i64> = row.get(4)?;
    let session_key: String = row.get(5)?;
    let state: String = row.get(6)?;

    let Some(ran_at) = millis_to_utc(ran_at_ms) else {
        return Ok(None);
    };
    let Some(state) = TaskRunState::from_str(&state) else {
        return Ok(None);
    };

    Ok(Some(TaskRun {
        id,
        agent_id,
        task_slug,
        ran_at,
        finished_at: finished_at_ms.and_then(millis_to_utc),
        session_key,
        state,
    }))
}

#[async_trait::async_trait]
impl TaskRunStorage for SqliteStorage {
    async fn open_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
        session_key: String,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO task_run (agent_id, task_slug, ran_at, finished_at, session_key, state) \
             VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            rusqlite::params![
                agent_id,
                task_slug,
                ran_at.timestamp_millis(),
                session_key,
                TaskRunState::Running.as_str(),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    async fn close_task_run(
        &self,
        id: i64,
        state: TaskRunState,
        finished_at: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        // `state = 'running'` in the predicate is what makes a terminal state terminal:
        // a late close cannot overwrite one already recorded.
        conn.execute(
            "UPDATE task_run SET state = ?1, finished_at = ?2 WHERE id = ?3 AND state = ?4",
            rusqlite::params![
                state.as_str(),
                finished_at.timestamp_millis(),
                id,
                TaskRunState::Running.as_str(),
            ],
        )?;
        Ok(())
    }

    async fn running_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
    ) -> Result<Option<TaskRun>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT {RUN_COLUMNS} FROM task_run \
             WHERE agent_id = ?1 AND task_slug = ?2 AND state = ?3 \
             ORDER BY ran_at DESC, id DESC LIMIT 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query_map(
            rusqlite::params![agent_id, task_slug, TaskRunState::Running.as_str()],
            run_from_row,
        )?;

        match rows.next() {
            Some(Ok(run)) => Ok(run),
            Some(Err(err)) => Err(err.into()),
            None => Ok(None),
        }
    }

    async fn list_task_runs(
        &self,
        agent_id: AgentId,
        task_slug: String,
        before: Option<DateTime<Utc>>,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<TaskRun>> {
        let conn = self.conn.lock();

        // Key-set cursor on the strictly-ordered pair `(ran_at, id)`. `before_id` is what
        // keeps runs sharing a millisecond from straddling a page boundary; an offset
        // cursor would also shift under a new run landing at the head mid-paging.
        let mut sql = format!(
            "SELECT {RUN_COLUMNS} FROM task_run WHERE agent_id = ?1 AND task_slug = ?2"
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(agent_id), Box::new(task_slug)];

        if let Some(before) = before {
            let before_ms = before.timestamp_millis();
            match before_id {
                Some(before_id) => {
                    sql.push_str(" AND (ran_at < ?3 OR (ran_at = ?3 AND id < ?4))");
                    params.push(Box::new(before_ms));
                    params.push(Box::new(before_id));
                }
                None => {
                    sql.push_str(" AND ran_at < ?3");
                    params.push(Box::new(before_ms));
                }
            }
        }

        sql.push_str(" ORDER BY ran_at DESC, id DESC LIMIT ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;
        let runs: Vec<TaskRun> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), run_from_row)?
            .filter_map(|r| r.ok())
            .flatten()
            .collect();

        Ok(runs)
    }

    async fn get_task_run(
        &self,
        agent_id: AgentId,
        task_slug: String,
        ran_at: DateTime<Utc>,
    ) -> Result<Option<TaskRun>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT {RUN_COLUMNS} FROM task_run \
             WHERE agent_id = ?1 AND task_slug = ?2 AND ran_at = ?3 \
             ORDER BY id DESC LIMIT 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query_map(
            rusqlite::params![agent_id, task_slug, ran_at.timestamp_millis()],
            run_from_row,
        )?;

        match rows.next() {
            Some(Ok(run)) => Ok(run),
            Some(Err(err)) => Err(err.into()),
            None => Ok(None),
        }
    }

    async fn interrupt_open_task_runs(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let swept = conn.execute(
            "UPDATE task_run SET state = ?1, finished_at = ?2 WHERE state = ?3",
            rusqlite::params![
                TaskRunState::Interrupted.as_str(),
                Utc::now().timestamp_millis(),
                TaskRunState::Running.as_str(),
            ],
        )?;

        Ok(swept)
    }

    async fn delete_task_runs(&self, agent_id: AgentId, task_slug: String) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM task_run WHERE agent_id = ?1 AND task_slug = ?2",
            rusqlite::params![agent_id, task_slug],
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
        crate::storage::sqlite::init_task_run_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn at(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(ms).single().unwrap()
    }

    /// Open a run and close it `Answered`, so a test can build a history cheaply.
    async fn answered_run(storage: &SqliteStorage, slug: &str, ran_at: DateTime<Utc>) -> i64 {
        let id = storage
            .open_task_run(
                "agent-1".into(),
                slug.into(),
                ran_at,
                format!("agent-1__task__{slug}__{}", ran_at.timestamp_millis()),
            )
            .await
            .unwrap();
        storage
            .close_task_run(id, TaskRunState::Answered, ran_at)
            .await
            .unwrap();
        id
    }

    /// T019: the whole reason the cursor carries `before_id`. Five runs share one
    /// millisecond, so a cursor on `ran_at` alone would either re-serve the whole tie
    /// group or skip past it.
    #[tokio::test]
    async fn page_cursor_over_runs_sharing_one_millisecond_neither_duplicates_nor_skips() {
        let (storage, _dir) = setup();
        let same_ms = at(1_700_000_000_000);
        for _ in 0..5 {
            answered_run(&storage, "tick", same_ms).await;
        }

        let mut seen: Vec<i64> = Vec::new();
        let mut cursor: Option<(DateTime<Utc>, i64)> = None;
        loop {
            let page = storage
                .list_task_runs(
                    "agent-1".into(),
                    "tick".into(),
                    cursor.map(|(ran_at, _)| ran_at),
                    cursor.map(|(_, id)| id),
                    2,
                )
                .await
                .unwrap();
            if page.is_empty() {
                break;
            }
            let last = page.last().unwrap();
            cursor = Some((last.ran_at, last.id));
            seen.extend(page.iter().map(|run| run.id));
        }

        assert_eq!(seen.len(), 5, "every run is served exactly once: {seen:?}");
        let mut unique = seen.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 5, "no run is served twice: {seen:?}");
    }

    /// T019: `has_more` is derived from asking for one past the page, so it must be false
    /// only once the page is the last one.
    #[tokio::test]
    async fn has_more_is_false_only_on_the_last_page() {
        let (storage, _dir) = setup();
        for i in 0..5 {
            answered_run(&storage, "tick", at(1_700_000_000_000 + i * 1_000)).await;
        }

        // The route derives `has_more` by requesting `limit + 1` and trimming.
        let limit = 2usize;
        let mut cursor: Option<(DateTime<Utc>, i64)> = None;
        let mut has_more_flags: Vec<bool> = Vec::new();
        loop {
            let mut page = storage
                .list_task_runs(
                    "agent-1".into(),
                    "tick".into(),
                    cursor.map(|(ran_at, _)| ran_at),
                    cursor.map(|(_, id)| id),
                    limit + 1,
                )
                .await
                .unwrap();
            let has_more = page.len() > limit;
            page.truncate(limit);
            if page.is_empty() {
                break;
            }
            has_more_flags.push(has_more);
            let last = page.last().unwrap();
            cursor = Some((last.ran_at, last.id));
            if !has_more {
                break;
            }
        }

        assert_eq!(
            has_more_flags,
            vec![true, true, false],
            "five runs at a limit of two is three pages, the last one short"
        );
    }

    /// T019: newest first is the only ordering anything asks for.
    #[tokio::test]
    async fn runs_come_back_newest_first() {
        let (storage, _dir) = setup();
        for i in 0..3 {
            answered_run(&storage, "tick", at(1_700_000_000_000 + i * 1_000)).await;
        }

        let runs = storage
            .list_task_runs("agent-1".into(), "tick".into(), None, None, 10)
            .await
            .unwrap();

        let times: Vec<i64> = runs.iter().map(|r| r.ran_at.timestamp_millis()).collect();
        assert_eq!(
            times,
            vec![1_700_000_002_000, 1_700_000_001_000, 1_700_000_000_000]
        );
    }

    /// T020: the `running` row *is* the overlap lock, so closing a run has to clear it and
    /// a second open while one is in flight has to be visible as such.
    #[tokio::test]
    async fn at_most_one_running_row_per_task() {
        let (storage, _dir) = setup();
        let ran_at = at(1_700_000_000_000);

        assert!(
            storage
                .running_task_run("agent-1".into(), "tick".into())
                .await
                .unwrap()
                .is_none(),
            "nothing is in flight before the first firing"
        );

        let id = storage
            .open_task_run("agent-1".into(), "tick".into(), ran_at, "key".into())
            .await
            .unwrap();

        let running = storage
            .running_task_run("agent-1".into(), "tick".into())
            .await
            .unwrap()
            .expect("the open run is the lock");
        assert_eq!(running.id, id);
        assert_eq!(running.state, TaskRunState::Running);
        assert!(running.finished_at.is_none(), "running means unfinished");

        storage
            .close_task_run(id, TaskRunState::Answered, at(1_700_000_001_000))
            .await
            .unwrap();

        assert!(
            storage
                .running_task_run("agent-1".into(), "tick".into())
                .await
                .unwrap()
                .is_none(),
            "closing the run releases the lock"
        );

        // Another task's run is not this task's lock.
        storage
            .open_task_run("agent-1".into(), "other".into(), ran_at, "key".into())
            .await
            .unwrap();
        assert!(
            storage
                .running_task_run("agent-1".into(), "tick".into())
                .await
                .unwrap()
                .is_none()
        );
    }

    /// T020: a process that stops mid-run leaves a row claiming to be in flight, which
    /// would both display as perpetually running and permanently block the task.
    #[tokio::test]
    async fn the_sweep_turns_every_running_row_into_interrupted() {
        let (storage, _dir) = setup();
        let ran_at = at(1_700_000_000_000);
        let open_a = storage
            .open_task_run("agent-1".into(), "a".into(), ran_at, "key-a".into())
            .await
            .unwrap();
        storage
            .open_task_run("agent-2".into(), "b".into(), ran_at, "key-b".into())
            .await
            .unwrap();
        let closed = answered_run(&storage, "c", ran_at).await;

        let swept = storage.interrupt_open_task_runs().await.unwrap();
        assert_eq!(swept, 2, "only the two open rows are swept");

        let a = storage
            .get_task_run("agent-1".into(), "a".into(), ran_at)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.id, open_a);
        assert_eq!(a.state, TaskRunState::Interrupted);
        assert!(a.finished_at.is_some(), "an interrupted run is finished");

        let c = storage
            .get_task_run("agent-1".into(), "c".into(), ran_at)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.id, closed);
        assert_eq!(c.state, TaskRunState::Answered, "a closed run is untouched");

        assert!(
            storage
                .running_task_run("agent-1".into(), "a".into())
                .await
                .unwrap()
                .is_none(),
            "the sweep releases the lock, so the task is not permanently blocked"
        );
    }

    /// T020: a terminal state is terminal — a late close cannot overwrite one already recorded.
    #[tokio::test]
    async fn a_closed_run_is_not_reopened_by_a_late_close() {
        let (storage, _dir) = setup();
        let ran_at = at(1_700_000_000_000);
        let id = storage
            .open_task_run("agent-1".into(), "tick".into(), ran_at, "key".into())
            .await
            .unwrap();
        storage
            .close_task_run(id, TaskRunState::Interrupted, ran_at)
            .await
            .unwrap();
        storage
            .close_task_run(id, TaskRunState::Answered, ran_at)
            .await
            .unwrap();

        let run = storage
            .get_task_run("agent-1".into(), "tick".into(), ran_at)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.state, TaskRunState::Interrupted);
    }

    /// Deleting a task takes its runs with it, so a new task reusing a freed slug finds none.
    #[tokio::test]
    async fn deleting_a_tasks_runs_leaves_other_tasks_alone() {
        let (storage, _dir) = setup();
        let ran_at = at(1_700_000_000_000);
        answered_run(&storage, "tick", ran_at).await;
        answered_run(&storage, "other", ran_at).await;

        storage
            .delete_task_runs("agent-1".into(), "tick".into())
            .await
            .unwrap();

        assert!(
            storage
                .list_task_runs("agent-1".into(), "tick".into(), None, None, 10)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            storage
                .list_task_runs("agent-1".into(), "other".into(), None, None, 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
