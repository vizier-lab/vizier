use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};

use crate::{
    schema::VizierSession,
    storage::{
        scratch::{MAX_SCRATCH_FILE_BYTES, MAX_SCRATCH_FILES, ScratchStorage, ScratchWrite},
        sqlite::SqliteStorage,
    },
};

/// `(agent_id, channel, topic)`, with a missing topic stored as `''` (research R2).
fn key(session: &VizierSession) -> (&str, String, &str) {
    (&session.0, session.1.to_slug(), session.2.as_deref().unwrap_or(""))
}

fn names(conn: &Connection, session: &VizierSession) -> rusqlite::Result<Vec<String>> {
    let (agent_id, channel, topic) = key(session);
    let mut stmt = conn.prepare(
        "SELECT name FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3 ORDER BY name",
    )?;
    stmt.query_map(rusqlite::params![agent_id, channel, topic], |row| row.get(0))?
        .collect()
}

/// Write or append in one transaction, refusing before any change when a limit would be
/// passed (research R3). An append extends the stored content in SQL, never reading it here.
fn save(conn: &Connection, session: &VizierSession, name: &str, content: &str, append: bool) -> Result<ScratchWrite> {
    let (agent_id, channel, topic) = key(session);
    let tx = conn.unchecked_transaction()?;

    let current: Option<u64> = tx
        .query_row(
            "SELECT size FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3 AND name = ?4",
            rusqlite::params![agent_id, channel, topic, name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .map(|size| size as u64);

    let added = content.len() as u64;
    let size = if append { current.unwrap_or(0) + added } else { added };
    if size > MAX_SCRATCH_FILE_BYTES {
        return Ok(ScratchWrite::TooLarge { limit: MAX_SCRATCH_FILE_BYTES, current: current.unwrap_or(0) });
    }

    if current.is_none() {
        let existing = names(&tx, session)?;
        if existing.len() >= MAX_SCRATCH_FILES {
            return Ok(ScratchWrite::TooMany { limit: MAX_SCRATCH_FILES, names: existing });
        }
    }

    let update = if append {
        "content = scratch_file.content || excluded.content, size = scratch_file.size + excluded.size"
    } else {
        "content = excluded.content, size = excluded.size"
    };
    tx.execute(
        &format!(
            "INSERT INTO scratch_file (agent_id, channel, topic, name, content, size, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(agent_id, channel, topic, name) DO UPDATE SET {update}, updated_at = excluded.updated_at"
        ),
        rusqlite::params![agent_id, channel, topic, name, content, added as i64, Utc::now().to_rfc3339()],
    )?;
    tx.commit()?;

    Ok(ScratchWrite::Written { size })
}

#[async_trait::async_trait]
impl ScratchStorage for SqliteStorage {
    async fn write_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite> {
        save(&self.conn.lock(), session, name, content, false)
    }

    async fn append_scratch(&self, session: &VizierSession, name: &str, content: &str) -> Result<ScratchWrite> {
        save(&self.conn.lock(), session, name, content, true)
    }

    async fn read_scratch(&self, session: &VizierSession, name: &str) -> Result<Option<String>> {
        let (agent_id, channel, topic) = key(session);
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT content FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3 AND name = ?4",
                rusqlite::params![agent_id, channel, topic, name],
                |row| row.get(0),
            )
            .optional()?)
    }

    async fn scratch_names(&self, session: &VizierSession) -> Result<Vec<String>> {
        Ok(names(&self.conn.lock(), session)?)
    }

    async fn clear_scratch(&self, session: &VizierSession) -> Result<usize> {
        let (agent_id, channel, topic) = key(session);
        let conn = self.conn.lock();
        Ok(conn.execute(
            "DELETE FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            rusqlite::params![agent_id, channel, topic],
        )?)
    }

    async fn clear_scratch_except(&self, keep: &[VizierSession]) -> Result<usize> {
        let keep: std::collections::HashSet<(String, String, String)> = keep
            .iter()
            .map(|s| {
                let (agent, channel, topic) = key(s);
                (agent.to_string(), channel, topic.to_string())
            })
            .collect();
        let conn = self.conn.lock();
        let rows: Vec<(String, String, String)> = {
            let mut stmt = conn.prepare("SELECT DISTINCT agent_id, channel, topic FROM scratch_file")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut cleared = 0;
        for (agent, channel, topic) in rows {
            if keep.contains(&(agent.clone(), channel.clone(), topic.clone())) {
                continue;
            }
            cleared += conn.execute(
                "DELETE FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
                rusqlite::params![agent, channel, topic],
            )?;
        }
        Ok(cleared)
    }

    async fn clear_all_scratch(&self) -> Result<usize> {
        Ok(self.conn.lock().execute("DELETE FROM scratch_file", [])?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{schema::VizierChannelId, storage::document::LocalDocumentStore};

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_scratch_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session_of(agent: &str, user: &str, topic: Option<&str>) -> VizierSession {
        VizierSession(
            agent.into(),
            VizierChannelId::HTTP(user.into(), "webui".into()),
            topic.map(Into::into),
        )
    }

    fn session() -> VizierSession {
        session_of("agent-1", "alice", Some("t"))
    }

    fn written(size: u64) -> ScratchWrite {
        ScratchWrite::Written { size }
    }

    async fn read(storage: &SqliteStorage, session: &VizierSession, name: &str) -> Option<String> {
        storage.read_scratch(session, name).await.unwrap()
    }

    async fn fill(storage: &SqliteStorage, session: &VizierSession) {
        for i in 0..MAX_SCRATCH_FILES {
            storage.write_scratch(session, &format!("f{i:02}"), "x").await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_write_replaces_the_content() {
        let (storage, _dir) = setup();
        assert_eq!(storage.write_scratch(&session(), "plan", "one").await.unwrap(), written(3));
        assert_eq!(storage.write_scratch(&session(), "plan", "second").await.unwrap(), written(6));
        assert_eq!(read(&storage, &session(), "plan").await.as_deref(), Some("second"));
        assert_eq!(read(&storage, &session(), "other").await, None);
    }

    #[tokio::test]
    async fn a_missing_topic_upserts_one_row() {
        let (storage, _dir) = setup();
        let s = session_of("agent-1", "alice", None);
        storage.write_scratch(&s, "plan", "a").await.unwrap();
        storage.write_scratch(&s, "plan", "b").await.unwrap();
        assert_eq!(storage.scratch_names(&s).await.unwrap(), ["plan"]);
        assert_eq!(read(&storage, &s, "plan").await.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn an_oversized_write_changes_nothing() {
        let (storage, _dir) = setup();
        storage.write_scratch(&session(), "log", "keep").await.unwrap();
        let big = "x".repeat(MAX_SCRATCH_FILE_BYTES as usize + 1);
        assert_eq!(
            storage.write_scratch(&session(), "log", &big).await.unwrap(),
            ScratchWrite::TooLarge { limit: MAX_SCRATCH_FILE_BYTES, current: 4 }
        );
        assert_eq!(read(&storage, &session(), "log").await.as_deref(), Some("keep"));
        assert_eq!(
            storage.write_scratch(&session(), "new", &big).await.unwrap(),
            ScratchWrite::TooLarge { limit: MAX_SCRATCH_FILE_BYTES, current: 0 }
        );
        // Exactly the limit fits.
        let max = "x".repeat(MAX_SCRATCH_FILE_BYTES as usize);
        assert_eq!(storage.write_scratch(&session(), "log", &max).await.unwrap(), written(MAX_SCRATCH_FILE_BYTES));
    }

    #[tokio::test]
    async fn a_new_name_past_the_count_is_refused_but_overwrites_work() {
        let (storage, _dir) = setup();
        fill(&storage, &session()).await;
        match storage.write_scratch(&session(), "extra", "x").await.unwrap() {
            ScratchWrite::TooMany { limit, names } => {
                assert_eq!(limit, MAX_SCRATCH_FILES);
                assert_eq!(names.len(), MAX_SCRATCH_FILES);
                assert_eq!(names[0], "f00");
            }
            other => panic!("expected TooMany, got {other:?}"),
        }
        assert_eq!(read(&storage, &session(), "extra").await, None);
        assert_eq!(storage.write_scratch(&session(), "f00", "yy").await.unwrap(), written(2));
    }

    #[tokio::test]
    async fn sizes_are_utf8_bytes() {
        let (storage, _dir) = setup();
        assert_eq!(storage.write_scratch(&session(), "u", "é漢").await.unwrap(), written(5));
        assert_eq!(storage.append_scratch(&session(), "u", "é").await.unwrap(), written(7));
    }

    #[tokio::test]
    async fn sessions_and_agents_never_share_files() {
        let (storage, _dir) = setup();
        let sessions = [
            session(),
            session_of("agent-1", "alice", Some("other-topic")),
            session_of("agent-1", "bob", Some("t")),
            session_of("agent-2", "alice", Some("t")),
            session_of("agent-1", "alice", None),
        ];
        for (i, s) in sessions.iter().enumerate() {
            storage.write_scratch(s, "plan", &i.to_string()).await.unwrap();
        }
        storage.write_scratch(&session(), "only-mine", "x").await.unwrap();

        for (i, s) in sessions.iter().enumerate() {
            assert_eq!(read(&storage, s, "plan").await, Some(i.to_string()));
        }
        assert_eq!(storage.scratch_names(&session()).await.unwrap(), ["only-mine", "plan"]);
        assert_eq!(storage.scratch_names(&sessions[1]).await.unwrap(), ["plan"]);

        assert_eq!(storage.clear_scratch(&session()).await.unwrap(), 2);
        for s in &sessions[1..] {
            assert_eq!(storage.scratch_names(s).await.unwrap(), ["plan"]);
        }
    }

    #[tokio::test]
    async fn appends_join_exactly() {
        let (storage, _dir) = setup();
        assert_eq!(storage.append_scratch(&session(), "log", "a").await.unwrap(), written(1));
        storage.append_scratch(&session(), "log", "b").await.unwrap();
        assert_eq!(storage.append_scratch(&session(), "log", "c").await.unwrap(), written(3));
        assert_eq!(read(&storage, &session(), "log").await.as_deref(), Some("abc"));

        assert_eq!(storage.append_scratch(&session(), "log", "").await.unwrap(), written(3));
        assert_eq!(read(&storage, &session(), "log").await.as_deref(), Some("abc"));
        assert_eq!(storage.append_scratch(&session(), "empty", "").await.unwrap(), written(0));
        assert_eq!(read(&storage, &session(), "empty").await.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn an_append_past_the_limits_changes_nothing() {
        let (storage, _dir) = setup();
        let most = "x".repeat(MAX_SCRATCH_FILE_BYTES as usize - 1);
        storage.write_scratch(&session(), "log", &most).await.unwrap();
        assert_eq!(
            storage.append_scratch(&session(), "log", "yz").await.unwrap(),
            ScratchWrite::TooLarge { limit: MAX_SCRATCH_FILE_BYTES, current: MAX_SCRATCH_FILE_BYTES - 1 }
        );
        assert_eq!(read(&storage, &session(), "log").await.map(|c| c.len()), Some(most.len()));

        let (storage, _dir) = setup();
        fill(&storage, &session()).await;
        assert!(matches!(
            storage.append_scratch(&session(), "extra", "x").await.unwrap(),
            ScratchWrite::TooMany { .. }
        ));
        assert_eq!(storage.append_scratch(&session(), "f01", "y").await.unwrap(), written(2));
    }

    #[tokio::test]
    async fn clears_empty_sessions_and_reset_the_count() {
        let (storage, _dir) = setup();
        let other = session_of("agent-2", "bob", None);
        fill(&storage, &session()).await;
        storage.write_scratch(&other, "a", "x").await.unwrap();

        assert_eq!(storage.clear_scratch(&session()).await.unwrap(), MAX_SCRATCH_FILES);
        assert!(storage.scratch_names(&session()).await.unwrap().is_empty());
        assert_eq!(storage.write_scratch(&session(), "fresh", "x").await.unwrap(), written(1));

        assert_eq!(storage.clear_all_scratch().await.unwrap(), 2);
        assert!(storage.scratch_names(&session()).await.unwrap().is_empty());
        assert!(storage.scratch_names(&other).await.unwrap().is_empty());
    }
}
