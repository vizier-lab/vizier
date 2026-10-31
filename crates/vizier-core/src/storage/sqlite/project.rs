//! The `project` table (`specs/040-projects-mode/data-model.md` §1): one additive SQLite table
//! with a soft-removed state, so reopening a folder restores the id and every session under it.

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::{
    schema::{
        Project, ProjectMode, ProjectSessionRow, VizierSessionDetail, Worktree, WorktreeRef,
    },
    storage::{project::ProjectStorage, sqlite::SqliteStorage},
};

/// The project table and its lookup index. New tables only, so no migration is needed.
pub fn init_project_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS project (
            id            TEXT PRIMARY KEY,
            agent_id      TEXT NOT NULL,
            name          TEXT NOT NULL,
            folder        TEXT NOT NULL,
            owner_id      TEXT NOT NULL,
            shell_enabled INTEGER NOT NULL DEFAULT 1,
            created_at    INTEGER NOT NULL,
            removed_at    INTEGER,
            UNIQUE(agent_id, folder)
        );
        CREATE INDEX IF NOT EXISTS idx_project_agent
            ON project(agent_id, removed_at);

        -- One row per project chat that was switched away from the default Plan mode
        -- (`specs/041-project-coding-workspace` data-model §2).
        CREATE TABLE IF NOT EXISTS project_chat (
            agent_id   TEXT    NOT NULL,
            channel    TEXT    NOT NULL,
            topic      TEXT    NOT NULL,
            mode       TEXT    NOT NULL CHECK (mode IN ('plan', 'build')),
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (agent_id, channel, topic)
        );

        -- One isolated checkout bound to exactly one chat topic (data-model §3).
        CREATE TABLE IF NOT EXISTS project_worktree (
            id          TEXT    PRIMARY KEY,
            project_id  TEXT    NOT NULL REFERENCES project(id) ON DELETE CASCADE,
            agent_id    TEXT    NOT NULL,
            owner_id    TEXT    NOT NULL,
            owner_name  TEXT    NOT NULL,
            topic       TEXT    NOT NULL,
            name        TEXT    NOT NULL,
            branch      TEXT    NOT NULL,
            base_ref    TEXT    NOT NULL,
            path        TEXT    NOT NULL,
            created_at  INTEGER NOT NULL,
            UNIQUE (project_id, name),
            UNIQUE (project_id, topic)
        );
        CREATE INDEX IF NOT EXISTS idx_worktree_project ON project_worktree(project_id);
        ",
    )?;
    Ok(())
}

fn row_to_worktree(row: &rusqlite::Row<'_>) -> rusqlite::Result<Worktree> {
    let path: String = row.get(9)?;
    Ok(Worktree {
        id: row.get(0)?,
        project_id: row.get(1)?,
        agent_id: row.get(2)?,
        owner_id: row.get(3)?,
        owner_name: row.get(4)?,
        topic: row.get(5)?,
        name: row.get(6)?,
        branch: row.get(7)?,
        base_ref: row.get(8)?,
        path: std::path::PathBuf::from(path),
        created_at: row.get(10)?,
    })
}

const WORKTREE_COLUMNS: &str = "id, project_id, agent_id, owner_id, owner_name, topic, name, \
                                branch, base_ref, path, created_at";

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let folder: String = row.get(3)?;
    Ok(Project {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        name: row.get(2)?,
        folder: std::path::PathBuf::from(folder),
        owner_id: row.get(4)?,
        shell_enabled: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        removed_at: row.get(7)?,
    })
}

fn read_project(conn: &Connection, agent_id: &str, project_id: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, name, folder, owner_id, shell_enabled, created_at, removed_at \
         FROM project WHERE agent_id = ?1 AND id = ?2",
    )?;
    let mut rows = stmt.query_map(params![agent_id, project_id], row_to_project)?;
    Ok(rows.next().transpose()?)
}

fn read_by_folder(conn: &Connection, agent_id: &str, folder: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, name, folder, owner_id, shell_enabled, created_at, removed_at \
         FROM project WHERE agent_id = ?1 AND folder = ?2",
    )?;
    let mut rows = stmt.query_map(params![agent_id, folder], row_to_project)?;
    Ok(rows.next().transpose()?)
}

fn is_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

/// Every project of one agent, in the same transaction as the rest of its rows.
pub fn delete_agent_projects(conn: &Connection, agent_id: &str) -> Result<usize> {
    // Explicit rather than trusting `ON DELETE CASCADE`, which only fires with foreign keys on.
    conn.execute("DELETE FROM project_chat WHERE agent_id = ?1", params![agent_id])?;
    conn.execute("DELETE FROM project_worktree WHERE agent_id = ?1", params![agent_id])?;
    Ok(conn.execute("DELETE FROM project WHERE agent_id = ?1", params![agent_id])?)
}

#[async_trait::async_trait]
impl ProjectStorage for SqliteStorage {
    async fn list_projects(&self, agent_id: &str, include_removed: bool) -> Result<Vec<Project>> {
        let conn = self.conn.lock();
        let sql = match include_removed {
            true => {
                "SELECT id, agent_id, name, folder, owner_id, shell_enabled, created_at, removed_at \
                 FROM project WHERE agent_id = ?1 ORDER BY created_at DESC, id"
            }
            false => {
                "SELECT id, agent_id, name, folder, owner_id, shell_enabled, created_at, removed_at \
                 FROM project WHERE agent_id = ?1 AND removed_at IS NULL \
                 ORDER BY created_at DESC, id"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![agent_id], row_to_project)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    async fn get_project(&self, agent_id: &str, project_id: &str) -> Result<Option<Project>> {
        read_project(&self.conn.lock(), agent_id, project_id)
    }

    async fn open_project(&self, project: Project) -> Result<(Project, bool)> {
        let conn = self.conn.lock();
        let folder = project.folder.to_string_lossy().into_owned();
        if let Some(existing) = read_by_folder(&conn, &project.agent_id, &folder)? {
            // Restoring keeps the id, so the project's sessions become addressable again.
            if existing.removed_at.is_some() {
                conn.execute(
                    "UPDATE project SET removed_at = NULL WHERE id = ?1",
                    params![existing.id],
                )?;
                return Ok((read_project(&conn, &project.agent_id, &existing.id)?.unwrap_or(existing), false));
            }
            return Ok((existing, false));
        }

        let inserted = conn.execute(
            "INSERT INTO project (id, agent_id, name, folder, owner_id, shell_enabled, created_at, removed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.id,
                project.agent_id,
                project.name,
                folder,
                project.owner_id,
                project.shell_enabled as i64,
                project.created_at,
                project.removed_at,
            ],
        );
        match inserted {
            Ok(_) => Ok((project, true)),
            // A concurrent open won the unique constraint: return its row instead of failing.
            Err(err) if is_constraint_violation(&err) => match read_by_folder(&conn, &project.agent_id, &folder)? {
                Some(existing) => Ok((existing, false)),
                None => Err(err.into()),
            },
            Err(err) => Err(err.into()),
        }
    }

    async fn rename_project(&self, project_id: &str, name: &str) -> Result<()> {
        self.conn
            .lock()
            .execute("UPDATE project SET name = ?1 WHERE id = ?2", params![name, project_id])?;
        Ok(())
    }

    async fn set_project_shell(&self, project_id: &str, enabled: bool) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE project SET shell_enabled = ?1 WHERE id = ?2",
            params![enabled as i64, project_id],
        )?;
        Ok(())
    }

    async fn remove_project(&self, project_id: &str, removed_at: i64) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE project SET removed_at = ?1 WHERE id = ?2",
            params![removed_at, project_id],
        )?;
        Ok(())
    }

    async fn project_sessions(
        &self,
        agent_id: &str,
        channel_slug: &str,
    ) -> Result<Vec<ProjectSessionRow>> {
        // `project__<user>__<id>`; the id addresses the project's worktrees.
        let project_id = channel_slug.rsplit_once("__").map_or("", |(_, id)| id);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT sd.topic, \
                    COALESCE(json_extract(sd.data, '$.title'), sd.topic), \
                    COALESCE(json_extract(sd.data, '$.is_thinking'), 0), \
                    MAX(sh.timestamp), \
                    MAX(COALESCE(pc.mode, 'plan')), \
                    MAX(pw.id), MAX(pw.name), MAX(pw.branch) \
             FROM session_detail sd \
             LEFT JOIN session_history sh \
               ON sh.agent_id = sd.agent_id \
              AND sh.channel = sd.channel \
              AND COALESCE(sh.topic, '') = COALESCE(sd.topic, '') \
             LEFT JOIN project_chat pc \
               ON pc.agent_id = sd.agent_id AND pc.channel = sd.channel \
              AND pc.topic = sd.topic \
             LEFT JOIN project_worktree pw \
               ON pw.project_id = ?3 AND pw.topic = sd.topic \
             WHERE sd.agent_id = ?1 AND sd.channel = ?2 AND sd.topic IS NOT NULL \
             GROUP BY sd.topic \
             ORDER BY MAX(sh.timestamp) IS NULL, MAX(sh.timestamp) DESC, sd.topic",
        )?;
        let rows = stmt.query_map(params![agent_id, channel_slug, project_id], |row| {
            let mode: String = row.get(4)?;
            let worktree_id: Option<String> = row.get(5)?;
            Ok(ProjectSessionRow {
                topic: row.get(0)?,
                title: row.get(1)?,
                is_thinking: row.get::<_, i64>(2)? != 0,
                last_activity: row.get(3)?,
                mode: mode.parse().unwrap_or_default(),
                worktree: match worktree_id {
                    Some(id) => Some(WorktreeRef {
                        id,
                        name: row.get(6)?,
                        branch: row.get(7)?,
                    }),
                    None => None,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    async fn project_session_count(&self, agent_id: &str, project_id: &str) -> Result<usize> {
        // `project__<user>__<id>`: one project id, whichever person owns the chat.
        let pattern = format!("project\\_\\_%\\_\\_{project_id}");
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM (SELECT DISTINCT topic FROM session_detail \
             WHERE agent_id = ?1 AND topic IS NOT NULL AND channel LIKE ?2 ESCAPE '\\')",
            params![agent_id, pattern],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    async fn project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
    ) -> Result<ProjectMode> {
        let mode: Option<String> = self
            .conn
            .lock()
            .query_row(
                "SELECT mode FROM project_chat WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
                params![agent_id, channel, topic],
                |row| row.get(0),
            )
            .ok();
        Ok(mode.and_then(|mode| mode.parse().ok()).unwrap_or_default())
    }

    async fn set_project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
        mode: ProjectMode,
    ) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO project_chat (agent_id, channel, topic, mode, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (agent_id, channel, topic) \
             DO UPDATE SET mode = excluded.mode, updated_at = excluded.updated_at",
            params![
                agent_id,
                channel,
                topic,
                mode.as_str(),
                chrono::Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    async fn insert_worktree(
        &self,
        worktree: &Worktree,
        detail: VizierSessionDetail,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO project_worktree ({WORKTREE_COLUMNS}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                worktree.id,
                worktree.project_id,
                worktree.agent_id,
                worktree.owner_id,
                worktree.owner_name,
                worktree.topic,
                worktree.name,
                worktree.branch,
                worktree.base_ref,
                worktree.path.to_string_lossy().into_owned(),
                worktree.created_at,
            ],
        )?;
        // The chat row is inserted on the same connection/transaction, so a worktree never
        // exists without it.
        let id = format!(
            "{}#{}#{}",
            detail.agent_id,
            detail.channel.to_slug(),
            detail.topic.clone().unwrap_or("DEFAULT".into())
        );
        tx.execute(
            "INSERT OR REPLACE INTO session_detail (id, agent_id, channel, topic, data) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                detail.agent_id,
                detail.channel.to_slug(),
                detail.topic,
                serde_json::to_string(&detail)?
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn worktree(&self, project_id: &str, id: &str) -> Result<Option<Worktree>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {WORKTREE_COLUMNS} FROM project_worktree WHERE project_id = ?1 AND id = ?2"
        ))?;
        let mut rows = stmt.query_map(params![project_id, id], row_to_worktree)?;
        Ok(rows.next().transpose()?)
    }

    async fn worktree_by_topic(
        &self,
        project_id: &str,
        topic: &str,
    ) -> Result<Option<Worktree>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {WORKTREE_COLUMNS} FROM project_worktree WHERE project_id = ?1 AND topic = ?2"
        ))?;
        let mut rows = stmt.query_map(params![project_id, topic], row_to_worktree)?;
        Ok(rows.next().transpose()?)
    }

    async fn worktrees(&self, project_id: &str) -> Result<Vec<Worktree>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {WORKTREE_COLUMNS} FROM project_worktree WHERE project_id = ?1 \
             ORDER BY created_at, name"
        ))?;
        let rows = stmt.query_map(params![project_id], row_to_worktree)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    async fn delete_worktree(&self, id: &str) -> Result<()> {
        self.conn
            .lock()
            .execute("DELETE FROM project_worktree WHERE id = ?1", params![id])?;
        Ok(())
    }

    async fn project_has_main_turn(&self, agent_id: &str, project_id: &str) -> Result<bool> {
        let pattern = format!("project\\_\\_%\\_\\_{project_id}");
        let pending: i64 = self.conn.lock().query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM session_turn st \
                 WHERE st.agent_id = ?1 AND st.channel LIKE ?2 ESCAPE '\\' \
                   AND st.topic NOT IN ( \
                       SELECT topic FROM project_worktree WHERE project_id = ?3))",
            params![agent_id, pattern, project_id],
            |row| row.get(0),
        )?;
        Ok(pending != 0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::schema::{VizierChannelId, VizierSessionDetail};
    use crate::storage::session::SessionStorage;

    fn storage() -> SqliteStorage {
        let conn = Connection::open_in_memory().unwrap();
        init_project_schema(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE session_detail (id TEXT PRIMARY KEY, agent_id TEXT NOT NULL, \
             channel TEXT NOT NULL, topic TEXT, data TEXT NOT NULL); \
             CREATE TABLE session_history (uid TEXT PRIMARY KEY, agent_id TEXT NOT NULL, \
             channel TEXT NOT NULL, topic TEXT, timestamp INTEGER NOT NULL, \
             content_type TEXT NOT NULL, data TEXT NOT NULL, seq INTEGER); \
             CREATE TABLE session_turn (id INTEGER PRIMARY KEY, agent_id TEXT NOT NULL, \
             channel TEXT NOT NULL, topic TEXT NOT NULL DEFAULT '');",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        SqliteStorage::new(
            Arc::new(parking_lot::Mutex::new(conn)),
            Arc::new(crate::storage::document::LocalDocumentStore::new(dir.path().to_path_buf())),
        )
    }

    fn project(folder: &std::path::Path) -> Project {
        Project {
            id: "p3xY9aQ2mZ".into(),
            agent_id: "dp-a".into(),
            name: "alpha".into(),
            folder: folder.to_path_buf(),
            owner_id: "u1".into(),
            shell_enabled: true,
            created_at: 1,
            removed_at: None,
        }
    }

    fn detail(topic: &str, title: &str) -> VizierSessionDetail {
        VizierSessionDetail {
            agent_id: "dp-a".into(),
            channel: VizierChannelId::Project("alice".into(), "p3xY9aQ2mZ".into()),
            topic: Some(topic.into()),
            title: title.into(),
            is_thinking: false,
        }
    }

    fn history(conn: &Connection, topic: &str, timestamp: i64) {
        conn.execute(
            "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) \
             VALUES (?1, 'dp-a', 'project__alice__p3xY9aQ2mZ', ?2, ?3, 'request', '{}', 1)",
            params![format!("u-{topic}-{timestamp}"), topic, timestamp],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn opening_the_same_folder_twice_reuses_the_row() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        let (first, created) = storage.open_project(project(dir.path())).await.unwrap();
        assert!(created);
        let (second, created) = storage.open_project(project(dir.path())).await.unwrap();
        assert!(!created);
        assert_eq!(first.id, second.id);
        assert_eq!(storage.list_projects("dp-a", false).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn removal_is_soft_and_reopening_restores_the_same_id() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        let (project, _) = storage.open_project(project(dir.path())).await.unwrap();
        storage.remove_project(&project.id, 99).await.unwrap();
        assert!(storage.list_projects("dp-a", false).await.unwrap().is_empty());
        assert!(storage.get_project("dp-a", &project.id).await.unwrap().unwrap().removed_at.is_some());

        let (restored, created) = storage.open_project(self::project(dir.path())).await.unwrap();
        assert!(!created);
        assert_eq!(restored.id, project.id);
        assert!(restored.removed_at.is_none());
        assert_eq!(storage.list_projects("dp-a", false).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn another_agent_gets_its_own_project_for_the_same_folder() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        let mut other = project(dir.path());
        other.id = "other12345".into();
        other.agent_id = "dp-b".into();
        assert!(storage.open_project(other).await.unwrap().1);
        assert_eq!(storage.list_projects("dp-b", false).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rename_and_the_shell_switch_persist() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        let (project, _) = storage.open_project(project(dir.path())).await.unwrap();
        storage.rename_project(&project.id, "renamed").await.unwrap();
        storage.set_project_shell(&project.id, false).await.unwrap();
        let stored = storage.get_project("dp-a", &project.id).await.unwrap().unwrap();
        assert_eq!(stored.name, "renamed");
        assert!(!stored.shell_enabled);
    }

    #[tokio::test]
    async fn sessions_are_newest_first_and_an_empty_one_is_last() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        for detail in [detail("old", "Old"), detail("new", "New"), detail("empty", "New session")] {
            storage.save_session_detail(detail).await.unwrap();
        }
        {
            let conn = storage.conn.lock();
            history(&conn, "old", 10);
            history(&conn, "new", 20);
        }
        let rows = storage
            .project_sessions("dp-a", "project__alice__p3xY9aQ2mZ")
            .await
            .unwrap();
        assert_eq!(rows.iter().map(|r| r.topic.as_str()).collect::<Vec<_>>(), vec!["new", "old", "empty"]);
        assert_eq!(rows[0].last_activity, Some(20));
        assert_eq!(rows[2].last_activity, None);
    }

    #[tokio::test]
    async fn deleting_an_agent_leaves_no_project_rows() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        {
            let conn = storage.conn.lock();
            delete_agent_projects(&conn, "dp-a").unwrap();
        }
        assert!(storage.list_projects("dp-a", true).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_session_count_covers_every_owner() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        storage.save_session_detail(detail("t1", "One")).await.unwrap();
        let mut bob = detail("t2", "Two");
        bob.channel = VizierChannelId::Project("bob".into(), "p3xY9aQ2mZ".into());
        storage.save_session_detail(bob).await.unwrap();
        assert_eq!(storage.project_session_count("dp-a", "p3xY9aQ2mZ").await.unwrap(), 2);
    }

    fn worktree(topic: &str) -> Worktree {
        Worktree {
            id: format!("w-{topic}"),
            project_id: "p3xY9aQ2mZ".into(),
            agent_id: "dp-a".into(),
            owner_id: "u1".into(),
            owner_name: "alice".into(),
            topic: topic.into(),
            name: format!("wt-{topic}"),
            branch: format!("wt-{topic}"),
            base_ref: "master".into(),
            path: std::path::PathBuf::from(format!("/tmp/.vizier/dp-a/wt-{topic}")),
            created_at: 2,
        }
    }

    #[tokio::test]
    async fn a_chat_starts_in_plan_and_the_mode_upserts() {
        let storage = storage();
        let channel = "project__alice__p3xY9aQ2mZ";
        assert_eq!(
            storage.project_chat_mode("dp-a", channel, "t1").await.unwrap(),
            ProjectMode::Plan
        );
        storage
            .set_project_chat_mode("dp-a", channel, "t1", ProjectMode::Build)
            .await
            .unwrap();
        assert_eq!(
            storage.project_chat_mode("dp-a", channel, "t1").await.unwrap(),
            ProjectMode::Build
        );
        // Another chat keeps its own mode.
        assert_eq!(
            storage.project_chat_mode("dp-a", channel, "t2").await.unwrap(),
            ProjectMode::Plan
        );
    }

    #[tokio::test]
    async fn a_worktree_and_its_chat_are_written_together_and_listed_with_their_mode() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        let wt = worktree("t1");
        storage.insert_worktree(&wt, detail("t1", "wt-t1")).await.unwrap();
        storage
            .set_project_chat_mode(
                "dp-a",
                "project__alice__p3xY9aQ2mZ",
                "t1",
                ProjectMode::Build,
            )
            .await
            .unwrap();

        assert_eq!(storage.worktree("p3xY9aQ2mZ", "w-t1").await.unwrap(), Some(wt.clone()));
        assert_eq!(
            storage.worktree_by_topic("p3xY9aQ2mZ", "t1").await.unwrap(),
            Some(wt.clone())
        );
        assert_eq!(storage.worktrees("p3xY9aQ2mZ").await.unwrap().len(), 1);

        let rows = storage
            .project_sessions("dp-a", "project__alice__p3xY9aQ2mZ")
            .await
            .unwrap();
        let row = rows.iter().find(|row| row.topic == "t1").unwrap();
        assert_eq!(row.mode, ProjectMode::Build);
        let summary = row.worktree.as_ref().unwrap();
        assert_eq!((summary.id.as_str(), summary.name.as_str()), ("w-t1", "wt-t1"));

        storage.delete_worktree("w-t1").await.unwrap();
        assert!(storage.worktrees("p3xY9aQ2mZ").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_an_agent_leaves_no_chat_or_worktree_rows() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        storage
            .set_project_chat_mode("dp-a", "project__alice__p3xY9aQ2mZ", "t1", ProjectMode::Build)
            .await
            .unwrap();
        storage
            .insert_worktree(&worktree("t1"), detail("t1", "wt-t1"))
            .await
            .unwrap();
        {
            let conn = storage.conn.lock();
            delete_agent_projects(&conn, "dp-a").unwrap();
            let chats: i64 = conn
                .query_row("SELECT COUNT(*) FROM project_chat", [], |r| r.get(0))
                .unwrap();
            let worktrees: i64 = conn
                .query_row("SELECT COUNT(*) FROM project_worktree", [], |r| r.get(0))
                .unwrap();
            assert_eq!((chats, worktrees), (0, 0));
        }
    }

    #[tokio::test]
    async fn a_main_turn_ignores_worktree_chats() {
        let storage = storage();
        let dir = tempfile::tempdir().unwrap();
        storage.open_project(project(dir.path())).await.unwrap();
        storage
            .insert_worktree(&worktree("wt1"), detail("wt1", "wt-wt1"))
            .await
            .unwrap();
        {
            let conn = storage.conn.lock();
            conn.execute(
                "INSERT INTO session_turn (agent_id, channel, topic) \
                 VALUES ('dp-a', 'project__alice__p3xY9aQ2mZ', 'wt1')",
                [],
            )
            .unwrap();
        }
        assert!(!storage.project_has_main_turn("dp-a", "p3xY9aQ2mZ").await.unwrap());
        {
            let conn = storage.conn.lock();
            conn.execute(
                "INSERT INTO session_turn (agent_id, channel, topic) \
                 VALUES ('dp-a', 'project__alice__p3xY9aQ2mZ', 'main')",
                [],
            )
            .unwrap();
        }
        assert!(storage.project_has_main_turn("dp-a", "p3xY9aQ2mZ").await.unwrap());
    }
}
