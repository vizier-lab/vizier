//! The Workspace tables (`specs/053-workspace-channel/data-model.md`). New tables, so no
//! migration; PostgreSQL has the same shape in `0004_workspaces.sql`.

use crate::storage::sql::params;
use anyhow::Result;

use crate::{
    schema::{MemberKind, Workspace, WorkspaceMember, WorkspaceRole, WorkspaceTopic},
    storage::{sqlite::SqlStorage, workspace::WorkspaceStorage},
};

pub fn init_workspace_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS workspace (
            id             TEXT PRIMARY KEY,
            name           TEXT NOT NULL,
            name_key       TEXT NOT NULL UNIQUE,
            created_by     TEXT NOT NULL,
            stopped_at_seq INTEGER NOT NULL DEFAULT 0,
            created_at     INTEGER NOT NULL,
            updated_at     INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workspace_topic (
            id           TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
            name         TEXT NOT NULL,
            name_key     TEXT NOT NULL,
            created_by   TEXT NOT NULL,
            created_at   INTEGER NOT NULL,
            UNIQUE (workspace_id, name_key)
        );

        CREATE TABLE IF NOT EXISTS workspace_member (
            workspace_id TEXT NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
            kind         TEXT NOT NULL CHECK (kind IN ('person', 'agent')),
            handle       TEXT NOT NULL,
            role         TEXT NOT NULL CHECK (role IN ('owner', 'member')),
            added_by     TEXT NOT NULL,
            added_at     INTEGER NOT NULL,
            PRIMARY KEY (workspace_id, kind, handle),
            UNIQUE (workspace_id, handle)
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_member_handle
            ON workspace_member(kind, handle);

        CREATE TABLE IF NOT EXISTS workspace_entry (
            seq              INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id     TEXT NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
            topic_id         TEXT NOT NULL REFERENCES workspace_topic(id) ON DELETE CASCADE,
            kind             TEXT NOT NULL CHECK (kind IN ('person', 'agent', 'note')),
            author_kind      TEXT NOT NULL,
            author           TEXT NOT NULL,
            author_name      TEXT NOT NULL,
            body             TEXT NOT NULL,
            files            TEXT,
            history_uid      TEXT,
            source_session   TEXT,
            origin           TEXT,
            is_final         INTEGER NOT NULL DEFAULT 0,
            activated_by_seq INTEGER,
            at               INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_entry_topic
            ON workspace_entry(topic_id, seq);

        CREATE TABLE IF NOT EXISTS workspace_mention (
            entry_seq   INTEGER NOT NULL REFERENCES workspace_entry(seq) ON DELETE CASCADE,
            target_kind TEXT NOT NULL,
            target      TEXT NOT NULL,
            state       TEXT NOT NULL,
            updated_at  INTEGER NOT NULL,
            PRIMARY KEY (entry_seq, target)
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_mention_state
            ON workspace_mention(state);

        CREATE TABLE IF NOT EXISTS workspace_read (
            topic_id TEXT NOT NULL REFERENCES workspace_topic(id) ON DELETE CASCADE,
            agent_id TEXT NOT NULL,
            read_seq INTEGER NOT NULL,
            PRIMARY KEY (topic_id, agent_id)
        );

        -- The timeline read pages entries, then one query for the steps of the sessions in that
        -- span (research R3).
        CREATE INDEX IF NOT EXISTS idx_sh_channel_topic_time
            ON session_history(channel, topic, timestamp);
        ",
    )?;
    Ok(())
}

fn row_to_workspace(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        name: row.get(1)?,
        created_by: row.get(2)?,
        stopped_at_seq: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

const WORKSPACE_COLUMNS: &str = "id, name, created_by, stopped_at_seq, created_at, updated_at";

fn row_to_topic(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<WorkspaceTopic> {
    Ok(WorkspaceTopic {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        name: row.get(2)?,
        created_by: row.get(3)?,
        created_at: row.get(4)?,
    })
}

const TOPIC_COLUMNS: &str = "id, workspace_id, name, created_by, created_at";

fn row_to_member(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<WorkspaceMember> {
    let kind: String = row.get(1)?;
    let role: String = row.get(3)?;
    Ok(WorkspaceMember {
        workspace_id: row.get(0)?,
        kind: MemberKind::parse(&kind).unwrap_or(MemberKind::Person),
        handle: row.get(2)?,
        role: WorkspaceRole::parse(&role).unwrap_or(WorkspaceRole::Member),
        added_by: row.get(4)?,
        added_at: row.get(5)?,
    })
}

const MEMBER_COLUMNS: &str = "workspace_id, kind, handle, role, added_by, added_at";

fn insert_member(conn: &crate::storage::sql::Connection, m: &WorkspaceMember) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO workspace_member ({MEMBER_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
        params![
            m.workspace_id,
            m.kind.as_str(),
            m.handle,
            m.role.as_str(),
            m.added_by,
            m.added_at
        ],
    )?;
    Ok(())
}

fn insert_topic(conn: &crate::storage::sql::Connection, t: &WorkspaceTopic) -> Result<()> {
    conn.execute(
        "INSERT INTO workspace_topic (id, workspace_id, name, name_key, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            t.id,
            t.workspace_id,
            t.name,
            crate::workspaces::names::key(&t.name),
            t.created_by,
            t.created_at
        ],
    )?;
    Ok(())
}

#[async_trait::async_trait]
impl WorkspaceStorage for SqlStorage {
    async fn create_workspace(
        &self,
        workspace: &Workspace,
        topic: &WorkspaceTopic,
        owner: &WorkspaceMember,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO workspace \
             (id, name, name_key, created_by, stopped_at_seq, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workspace.id,
                workspace.name,
                crate::workspaces::names::key(&workspace.name),
                workspace.created_by,
                workspace.stopped_at_seq,
                workspace.created_at,
                workspace.updated_at
            ],
        )?;
        insert_topic(&tx, topic)?;
        insert_member(&tx, owner)?;
        tx.commit()?;
        Ok(())
    }

    async fn get_workspace(&self, id: &str) -> Result<Option<Workspace>> {
        let conn = self.conn.lock()?;
        let mut stmt =
            conn.prepare(&format!("SELECT {WORKSPACE_COLUMNS} FROM workspace WHERE id = ?1"))?;
        let mut rows = stmt.query_map(params![id], row_to_workspace)?;
        Ok(rows.next().transpose()?)
    }

    async fn find_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn
            .prepare(&format!("SELECT {WORKSPACE_COLUMNS} FROM workspace WHERE name_key = ?1"))?;
        let mut rows =
            stmt.query_map(params![crate::workspaces::names::key(name)], row_to_workspace)?;
        Ok(rows.next().transpose()?)
    }

    async fn workspaces_of_person(&self, username: &str) -> Result<Vec<(Workspace, WorkspaceRole)>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT w.id, w.name, w.created_by, w.stopped_at_seq, w.created_at, w.updated_at, m.role \
             FROM workspace w JOIN workspace_member m ON m.workspace_id = w.id \
             WHERE m.kind = 'person' AND m.handle = ?1 ORDER BY w.name_key",
        )?;
        let rows = stmt.query_map(params![username], |row| {
            let role: String = row.get(6)?;
            Ok((
                row_to_workspace(row)?,
                WorkspaceRole::parse(&role).unwrap_or(WorkspaceRole::Member),
            ))
        })?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn workspaces_of_agent(&self, agent_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT workspace_id FROM workspace_member WHERE kind = 'agent' AND handle = ?1",
        )?;
        let rows = stmt.query_map(params![agent_id], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn
            .prepare(&format!("SELECT {WORKSPACE_COLUMNS} FROM workspace ORDER BY name_key"))?;
        let rows = stmt.query_map([], row_to_workspace)?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn rename_workspace(&self, id: &str, name: &str, now: i64) -> Result<()> {
        self.conn.lock()?.execute(
            "UPDATE workspace SET name = ?1, name_key = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, crate::workspaces::names::key(name), now, id],
        )?;
        Ok(())
    }

    async fn delete_workspace(&self, id: &str) -> Result<()> {
        self.conn
            .lock()?
            .execute("DELETE FROM workspace WHERE id = ?1", params![id])?;
        Ok(())
    }

    async fn list_topics(&self, workspace_id: &str) -> Result<Vec<WorkspaceTopic>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TOPIC_COLUMNS} FROM workspace_topic WHERE workspace_id = ?1 \
             ORDER BY created_at, name_key"
        ))?;
        let rows = stmt.query_map(params![workspace_id], row_to_topic)?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn get_topic(&self, workspace_id: &str, topic_id: &str) -> Result<Option<WorkspaceTopic>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TOPIC_COLUMNS} FROM workspace_topic WHERE workspace_id = ?1 AND id = ?2"
        ))?;
        let mut rows = stmt.query_map(params![workspace_id, topic_id], row_to_topic)?;
        Ok(rows.next().transpose()?)
    }

    async fn find_topic_by_name(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<Option<WorkspaceTopic>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TOPIC_COLUMNS} FROM workspace_topic WHERE workspace_id = ?1 AND name_key = ?2"
        ))?;
        let mut rows = stmt.query_map(
            params![workspace_id, crate::workspaces::names::key(name)],
            row_to_topic,
        )?;
        Ok(rows.next().transpose()?)
    }

    async fn create_topic(&self, topic: &WorkspaceTopic) -> Result<()> {
        insert_topic(&self.conn.lock()?, topic)
    }

    async fn rename_topic(&self, workspace_id: &str, topic_id: &str, name: &str) -> Result<()> {
        self.conn.lock()?.execute(
            "UPDATE workspace_topic SET name = ?1, name_key = ?2 WHERE workspace_id = ?3 AND id = ?4",
            params![name, crate::workspaces::names::key(name), workspace_id, topic_id],
        )?;
        Ok(())
    }

    async fn delete_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()> {
        self.conn.lock()?.execute(
            "DELETE FROM workspace_topic WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, topic_id],
        )?;
        Ok(())
    }

    async fn list_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {MEMBER_COLUMNS} FROM workspace_member WHERE workspace_id = ?1 \
             ORDER BY added_at, handle"
        ))?;
        let rows = stmt.query_map(params![workspace_id], row_to_member)?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn get_member(&self, workspace_id: &str, handle: &str) -> Result<Option<WorkspaceMember>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {MEMBER_COLUMNS} FROM workspace_member WHERE workspace_id = ?1 AND handle = ?2"
        ))?;
        let mut rows = stmt.query_map(params![workspace_id, handle], row_to_member)?;
        Ok(rows.next().transpose()?)
    }

    async fn add_member(&self, member: &WorkspaceMember) -> Result<()> {
        insert_member(&self.conn.lock()?, member)
    }

    async fn set_member_role(
        &self,
        workspace_id: &str,
        handle: &str,
        role: WorkspaceRole,
    ) -> Result<()> {
        self.conn.lock()?.execute(
            "UPDATE workspace_member SET role = ?1 WHERE workspace_id = ?2 AND handle = ?3",
            params![role.as_str(), workspace_id, handle],
        )?;
        Ok(())
    }

    async fn remove_member(&self, workspace_id: &str, kind: MemberKind, handle: &str) -> Result<()> {
        self.conn.lock()?.execute(
            "DELETE FROM workspace_member WHERE workspace_id = ?1 AND kind = ?2 AND handle = ?3",
            params![workspace_id, kind.as_str(), handle],
        )?;
        Ok(())
    }

    async fn remove_agent_everywhere(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM workspace_member WHERE kind = 'agent' AND handle = ?1",
            params![agent_id],
        )?;
        tx.execute("DELETE FROM workspace_read WHERE agent_id = ?1", params![agent_id])?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn storage() -> SqlStorage {
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::storage::sqlite::init_history_schema(&conn).unwrap();
            init_workspace_schema(&conn).unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        });
        let dir = tempfile::tempdir().unwrap();
        SqlStorage::new(
            store,
            Arc::new(crate::storage::document::BlobDocumentStore::local(
                dir.path().to_path_buf(),
            )),
        )
    }

    fn parts(id: &str, name: &str) -> (Workspace, WorkspaceTopic, WorkspaceMember) {
        (
            Workspace {
                id: id.into(),
                name: name.into(),
                created_by: "rafi".into(),
                stopped_at_seq: 0,
                created_at: 1,
                updated_at: 1,
            },
            WorkspaceTopic {
                id: format!("{id}-general"),
                workspace_id: id.into(),
                name: "general".into(),
                created_by: "rafi".into(),
                created_at: 1,
            },
            WorkspaceMember {
                workspace_id: id.into(),
                kind: MemberKind::Person,
                handle: "rafi".into(),
                role: WorkspaceRole::Owner,
                added_by: "rafi".into(),
                added_at: 1,
            },
        )
    }

    fn member(ws: &str, kind: MemberKind, handle: &str) -> WorkspaceMember {
        WorkspaceMember {
            workspace_id: ws.into(),
            kind,
            handle: handle.into(),
            role: WorkspaceRole::Member,
            added_by: "rafi".into(),
            added_at: 2,
        }
    }

    #[tokio::test]
    async fn a_workspace_starts_with_its_topic_and_owner() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();

        assert_eq!(s.get_workspace("w1").await.unwrap(), Some(w));
        assert_eq!(s.list_topics("w1").await.unwrap(), vec![t]);
        assert_eq!(s.list_members("w1").await.unwrap(), vec![o]);
    }

    #[tokio::test]
    async fn names_differing_only_in_case_collide() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        let (w2, t2, o2) = parts("w2", "OPS");
        let err = s.create_workspace(&w2, &t2, &o2).await.unwrap_err();
        assert!(crate::storage::sql::is_constraint_violation(&err), "{err:#}");
        // The failed create left nothing behind.
        assert!(s.get_workspace("w2").await.unwrap().is_none());
        assert_eq!(s.find_workspace_by_name("Ops").await.unwrap().unwrap().id, "w1");
    }

    #[tokio::test]
    async fn a_handle_is_one_member_whatever_its_kind() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "alpha")).await.unwrap();

        let same_handle = s.add_member(&member("w1", MemberKind::Person, "alpha")).await.unwrap_err();
        assert!(crate::storage::sql::is_constraint_violation(&same_handle));
        assert_eq!(s.get_member("w1", "alpha").await.unwrap().unwrap().kind, MemberKind::Agent);
    }

    #[tokio::test]
    async fn topics_are_unique_per_workspace_not_across() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        let (w2, t2, o2) = parts("w2", "dev");
        s.create_workspace(&w2, &t2, &o2).await.unwrap();

        let clash = WorkspaceTopic { id: "x".into(), name: "General".into(), ..t.clone() };
        let err = s.create_topic(&clash).await.unwrap_err();
        assert!(crate::storage::sql::is_constraint_violation(&err));
        assert_eq!(s.find_topic_by_name("w2", "general").await.unwrap().unwrap().id, "w2-general");
    }

    #[tokio::test]
    async fn listing_follows_membership() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        let (w2, t2, mut o2) = parts("w2", "dev");
        o2.handle = "maya".into();
        s.create_workspace(&w2, &t2, &o2).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "alpha")).await.unwrap();
        s.add_member(&member("w2", MemberKind::Agent, "alpha")).await.unwrap();

        let rafi = s.workspaces_of_person("rafi").await.unwrap();
        assert_eq!(rafi.len(), 1);
        assert_eq!((rafi[0].0.id.as_str(), rafi[0].1), ("w1", WorkspaceRole::Owner));
        let mut agent_in = s.workspaces_of_agent("alpha").await.unwrap();
        agent_in.sort();
        assert_eq!(agent_in, ["w1", "w2"]);
        assert_eq!(s.list_workspaces().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rename_and_role_changes_stick() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        s.rename_workspace("w1", "platform", 9).await.unwrap();
        s.set_member_role("w1", "rafi", WorkspaceRole::Member).await.unwrap();
        s.rename_topic("w1", "w1-general", "main").await.unwrap();

        let renamed = s.get_workspace("w1").await.unwrap().unwrap();
        assert_eq!((renamed.name.as_str(), renamed.updated_at), ("platform", 9));
        assert!(s.find_workspace_by_name("ops").await.unwrap().is_none());
        assert_eq!(s.get_member("w1", "rafi").await.unwrap().unwrap().role, WorkspaceRole::Member);
        assert_eq!(s.get_topic("w1", "w1-general").await.unwrap().unwrap().name, "main");
    }

    #[tokio::test]
    async fn deleting_a_workspace_takes_its_rows_with_it() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "alpha")).await.unwrap();
        s.delete_workspace("w1").await.unwrap();

        assert!(s.get_workspace("w1").await.unwrap().is_none());
        assert!(s.list_topics("w1").await.unwrap().is_empty());
        assert!(s.list_members("w1").await.unwrap().is_empty());
        assert!(s.workspaces_of_agent("alpha").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn removing_an_agent_everywhere_leaves_people_and_other_agents() {
        let s = storage();
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "alpha")).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "beta")).await.unwrap();
        s.remove_agent_everywhere("alpha").await.unwrap();

        let handles: Vec<_> = s.list_members("w1").await.unwrap().into_iter().map(|m| m.handle).collect();
        assert_eq!(handles, ["rafi", "beta"]);
    }
}
