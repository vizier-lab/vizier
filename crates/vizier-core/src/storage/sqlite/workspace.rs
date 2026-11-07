//! The Workspace tables (`specs/053-workspace-channel/data-model.md`). New tables, so no
//! migration; PostgreSQL has the same shape in `0004_workspaces.sql`.

use crate::storage::sql::{OptionalExtension, params};
use anyhow::Result;

use crate::{
    schema::{
        CatchUp, EntryFile, EntryKind, EntryMention, MemberKind, MentionState, NewEntry, SessionHistory,
        SessionHistoryContent, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierResponseContent, VizierSession, Workspace, WorkspaceEntry, WorkspaceMember,
        WorkspaceRole, WorkspaceTopic,
    },
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

const ENTRY_COLUMNS: &str = "seq, workspace_id, topic_id, kind, author_kind, author, author_name, \
                             body, files, history_uid, source_session, origin, is_final, \
                             activated_by_seq, at";

fn row_to_entry(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<WorkspaceEntry> {
    let kind: String = row.get(3)?;
    let author_kind: String = row.get(4)?;
    let files: Option<String> = row.get(8)?;
    let origin: Option<String> = row.get(11)?;
    Ok(WorkspaceEntry {
        seq: row.get(0)?,
        workspace_id: row.get(1)?,
        topic_id: row.get(2)?,
        kind: EntryKind::parse(&kind).unwrap_or(EntryKind::Note),
        author_kind: MemberKind::parse(&author_kind).unwrap_or(MemberKind::Person),
        author: row.get(5)?,
        author_name: row.get(6)?,
        body: row.get(7)?,
        files: files.and_then(|f| serde_json::from_str(&f).ok()).unwrap_or_default(),
        history_uid: row.get(9)?,
        source_session: row.get(10)?,
        origin: origin.and_then(|o| serde_json::from_str(&o).ok()),
        is_final: row.get::<_, i64>(12)? != 0,
        activated_by_seq: row.get(13)?,
        at: row.get(14)?,
    })
}

/// The one insert of an entry and its mentions, shared by the storage method and by the history
/// save, which writes an agent's shown messages in its own transaction (research R3).
pub(super) fn insert_entry_row(
    conn: &crate::storage::sql::Connection,
    entry: &NewEntry,
    mentions: &[(MemberKind, String, MentionState)],
) -> Result<i64> {
    let files = (!entry.files.is_empty())
        .then(|| serde_json::to_string(&entry.files))
        .transpose()?;
    let origin = entry.origin.as_ref().map(|o| o.to_string());
    let seq: i64 = conn.query_row(
        "INSERT INTO workspace_entry \
         (workspace_id, topic_id, kind, author_kind, author, author_name, body, files, \
          history_uid, source_session, origin, is_final, activated_by_seq, at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) RETURNING seq",
        params![
            entry.workspace_id,
            entry.topic_id,
            entry.kind.as_str(),
            entry.author_kind.as_str(),
            entry.author,
            entry.author_name,
            entry.body,
            files,
            entry.history_uid,
            entry.source_session,
            origin,
            entry.is_final as i64,
            entry.activated_by_seq,
            entry.at
        ],
        |row| row.get::<_, i64>(0),
    )?;
    for (kind, target, state) in mentions {
        conn.execute(
            "INSERT INTO workspace_mention (entry_seq, target_kind, target, state, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT DO NOTHING",
            params![seq, kind.as_str(), target, state.as_str(), entry.at],
        )?;
    }
    Ok(seq)
}

/// The entry that started an agent's current turn in a topic: the newest request in its record
/// that is not a copied read. `None` when the turn was not started from the Workspace.
pub(super) fn activation_seq(
    conn: &crate::storage::sql::Connection,
    agent_id: &str,
    channel_slug: &str,
    topic_id: &str,
) -> Result<Option<i64>> {
    let mut stmt = conn.prepare(
        "SELECT data FROM session_history \
         WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3 AND content_type = 'Request' \
         ORDER BY seq DESC LIMIT 8",
    )?;
    let rows = stmt.query_map(params![agent_id, channel_slug, topic_id], |row| row.get::<_, String>(0))?;
    for data in rows {
        let Ok(entry) = serde_json::from_str::<SessionHistory>(&data?) else { continue };
        let SessionHistoryContent::Request(request) = entry.content else { continue };
        if matches!(request.content, VizierRequestContent::SilentRead(_)) {
            continue;
        }
        return Ok(request.metadata.get("workspace_entry").and_then(|v| v.as_i64()));
    }
    Ok(None)
}

/// Writes the shown part of a saved history row into the shared timeline, inside the history
/// save's own transaction. Only an agent's text in a `Workspace` session counts: an
/// `AssistantMessage`, or a final reply with a message. Everything else (tool calls, results,
/// thoughts, requests) stays in the agent's own record.
pub(super) fn index_history_entry(
    conn: &crate::storage::sql::Connection,
    session: &VizierSession,
    uid: &str,
    content: &SessionHistoryContent,
    at: i64,
) -> Result<()> {
    let VizierChannelId::Workspace(workspace_id) = &session.1 else {
        return Ok(());
    };
    let Some(topic_id) = session.2.as_deref() else {
        return Ok(());
    };
    let (body, is_final) = match content {
        SessionHistoryContent::AssistantMessage(text) => (text.clone(), false),
        SessionHistoryContent::Response(response) => match &response.content {
            VizierResponseContent::Message { content, .. } => (content.clone(), true),
            VizierResponseContent::AudioReply(_, Some(transcript), _) => (transcript.clone(), true),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    if body.trim().is_empty() {
        return Ok(());
    }

    // A topic deleted while a turn was running has nowhere to show the text.
    let topic_exists = conn
        .query_row(
            "SELECT 1 FROM workspace_topic WHERE id = ?1 AND workspace_id = ?2",
            params![topic_id, workspace_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !topic_exists {
        return Ok(());
    }

    let agent_id = session.0.as_str();
    let author_name = conn
        .query_row(
            "SELECT data FROM agent_config WHERE agent_id = ?1",
            params![agent_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|data| serde_json::from_str::<serde_json::Value>(&data).ok())
        .and_then(|config| config.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .unwrap_or_else(|| agent_id.to_string());

    let activated_by_seq = activation_seq(conn, agent_id, &session.1.to_slug(), topic_id)?;

    // Members, to resolve `@handle`. An agent does not wake itself by naming itself.
    let mut handles: Vec<(String, MemberKind)> = Vec::new();
    {
        let mut stmt =
            conn.prepare("SELECT handle, kind FROM workspace_member WHERE workspace_id = ?1")?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (handle, kind) = row?;
            if let Some(kind) = MemberKind::parse(&kind)
                && !(kind == MemberKind::Agent && handle == agent_id)
            {
                handles.push((handle, kind));
            }
        }
    }
    let names: Vec<&str> = handles.iter().map(|(h, _)| h.as_str()).collect();
    let mentions: Vec<(MemberKind, String, MentionState)> =
        crate::workspaces::mentions::parse(&body, &names)
            .into_iter()
            .filter_map(|m| {
                let kind = handles.iter().find(|(h, _)| *h == m.handle)?.1;
                let state = match kind {
                    MemberKind::Agent => MentionState::Pending,
                    MemberKind::Person => MentionState::Resolved,
                };
                Some((kind, m.handle, state))
            })
            .collect();

    let entry = NewEntry {
        workspace_id: workspace_id.clone(),
        topic_id: topic_id.to_string(),
        kind: EntryKind::Agent,
        author_kind: MemberKind::Agent,
        author: agent_id.to_string(),
        author_name,
        body,
        files: vec![],
        history_uid: Some(uid.to_string()),
        source_session: Some(session.to_slug()),
        origin: None,
        is_final,
        activated_by_seq,
        at,
    };
    insert_entry_row(conn, &entry, &mentions)?;
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

    async fn insert_entry(
        &self,
        entry: &NewEntry,
        mentions: &[(MemberKind, String, MentionState)],
    ) -> Result<WorkspaceEntry> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let seq = insert_entry_row(&tx, entry, mentions)?;
        tx.commit()?;
        Ok(WorkspaceEntry {
            seq,
            workspace_id: entry.workspace_id.clone(),
            topic_id: entry.topic_id.clone(),
            kind: entry.kind,
            author_kind: entry.author_kind,
            author: entry.author.clone(),
            author_name: entry.author_name.clone(),
            body: entry.body.clone(),
            files: entry.files.clone(),
            history_uid: entry.history_uid.clone(),
            source_session: entry.source_session.clone(),
            origin: entry.origin.clone(),
            is_final: entry.is_final,
            activated_by_seq: entry.activated_by_seq,
            at: entry.at,
        })
    }

    async fn get_entry(&self, seq: i64) -> Result<Option<WorkspaceEntry>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn
            .prepare(&format!("SELECT {ENTRY_COLUMNS} FROM workspace_entry WHERE seq = ?1"))?;
        let mut rows = stmt.query_map(params![seq], row_to_entry)?;
        Ok(rows.next().transpose()?)
    }

    async fn entry_by_history_uid(&self, uid: &str) -> Result<Option<WorkspaceEntry>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM workspace_entry WHERE history_uid = ?1"
        ))?;
        let mut rows = stmt.query_map(params![uid], row_to_entry)?;
        Ok(rows.next().transpose()?)
    }

    async fn list_entries(
        &self,
        topic_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<WorkspaceEntry>> {
        let conn = self.conn.lock()?;
        let limit = limit as i64;
        let mut entries = match before {
            Some(before) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {ENTRY_COLUMNS} FROM workspace_entry \
                     WHERE topic_id = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3"
                ))?;
                let rows = stmt.query_map(params![topic_id, before, limit], row_to_entry)?;
                rows.collect::<crate::storage::sql::Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {ENTRY_COLUMNS} FROM workspace_entry \
                     WHERE topic_id = ?1 ORDER BY seq DESC LIMIT ?2"
                ))?;
                let rows = stmt.query_map(params![topic_id, limit], row_to_entry)?;
                rows.collect::<crate::storage::sql::Result<Vec<_>>>()?
            }
        };
        entries.reverse();
        Ok(entries)
    }

    async fn head_seq(&self, workspace_id: &str) -> Result<i64> {
        let conn = self.conn.lock()?;
        Ok(conn.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM workspace_entry WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get::<_, i64>(0),
        )?)
    }

    async fn mentions_of(&self, seqs: &[i64]) -> Result<Vec<EntryMention>> {
        if seqs.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock()?;
        let mut all = Vec::new();
        for chunk in seqs.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let mut stmt = conn.prepare(&format!(
                "SELECT entry_seq, target_kind, target, state FROM workspace_mention \
                 WHERE entry_seq IN ({placeholders}) ORDER BY entry_seq, target"
            ))?;
            let rows = stmt.query_map(
                crate::storage::sql::params_from_iter(chunk.iter()),
                row_to_mention,
            )?;
            for row in rows {
                all.push(row?);
            }
        }
        Ok(all)
    }

    async fn pending_mentions(&self) -> Result<Vec<EntryMention>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT entry_seq, target_kind, target, state FROM workspace_mention \
             WHERE state = 'pending' AND target_kind = 'agent' ORDER BY entry_seq, target",
        )?;
        let rows = stmt.query_map([], row_to_mention)?;
        Ok(rows.collect::<crate::storage::sql::Result<Vec<_>>>()?)
    }

    async fn set_mention_state(
        &self,
        entry_seq: i64,
        target: &str,
        state: MentionState,
        now: i64,
    ) -> Result<()> {
        self.conn.lock()?.execute(
            "UPDATE workspace_mention SET state = ?1, updated_at = ?2 \
             WHERE entry_seq = ?3 AND target = ?4",
            params![state.as_str(), now, entry_seq, target],
        )?;
        Ok(())
    }

    async fn stop_all(&self, workspace_id: &str, now: i64) -> Result<(usize, i64)> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let head: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM workspace_entry WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get::<_, i64>(0),
        )?;
        let dropped = tx.execute(
            "UPDATE workspace_mention SET state = 'dropped', updated_at = ?1 \
             WHERE state = 'pending' \
               AND entry_seq IN (SELECT seq FROM workspace_entry WHERE workspace_id = ?2)",
            params![now, workspace_id],
        )?;
        tx.execute(
            "UPDATE workspace SET stopped_at_seq = ?1 WHERE id = ?2",
            params![head, workspace_id],
        )?;
        tx.commit()?;
        Ok((dropped, head))
    }

    async fn current_activation(&self, session: &VizierSession) -> Result<Option<i64>> {
        let Some(topic) = session.2.as_deref() else { return Ok(None) };
        let conn = self.conn.lock()?;
        activation_seq(&conn, &session.0, &session.1.to_slug(), topic)
    }

    async fn steps_between(
        &self,
        workspace_id: &str,
        topic_id: &str,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<Vec<SessionHistory>> {
        use super::history::{fill_artifact_availability, fill_client_calls, fill_question_sets, fill_reactions, history_order_key, history_row, parse_history_row};
        let conn = self.conn.lock()?;
        let slug = VizierChannelId::Workspace(workspace_id.to_string()).to_slug();
        let mut stmt = conn.prepare(
            "SELECT data, seq FROM session_history \
             WHERE channel = ?1 AND topic = ?2 AND timestamp >= ?3 AND timestamp <= ?4 \
               AND content_type <> 'Request' \
             ORDER BY timestamp, seq",
        )?;
        let mut list: Vec<SessionHistory> = stmt
            .query_map(params![slug, topic_id, from_ms, to_ms], history_row)?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
            .collect();
        list.sort_by_key(history_order_key);
        fill_reactions(&conn, &mut list)?;
        fill_artifact_availability(&conn, &mut list)?;
        fill_client_calls(&conn, &mut list)?;
        fill_question_sets(&conn, &mut list)?;
        Ok(list)
    }

    async fn begin_reading_agent(&self, workspace_id: &str, agent_id: &str) -> Result<()> {
        self.conn.lock()?.execute(
            "INSERT INTO workspace_read (topic_id, agent_id, read_seq) \
             SELECT t.id, ?2, \
                    (SELECT COALESCE(MAX(seq), 0) FROM workspace_entry WHERE workspace_id = ?1) \
             FROM workspace_topic t WHERE t.workspace_id = ?1 \
             ON CONFLICT DO NOTHING",
            params![workspace_id, agent_id],
        )?;
        Ok(())
    }

    async fn begin_reading_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()> {
        self.conn.lock()?.execute(
            "INSERT INTO workspace_read (topic_id, agent_id, read_seq) \
             SELECT ?2, m.handle, \
                    (SELECT COALESCE(MAX(seq), 0) FROM workspace_entry WHERE workspace_id = ?1) \
             FROM workspace_member m WHERE m.workspace_id = ?1 AND m.kind = 'agent' \
             ON CONFLICT DO NOTHING",
            params![workspace_id, topic_id],
        )?;
        Ok(())
    }

    async fn read_position(&self, topic_id: &str, agent_id: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT read_seq FROM workspace_read WHERE topic_id = ?1 AND agent_id = ?2",
        )?;
        let mut rows = stmt.query_map(params![topic_id, agent_id], |row| row.get::<_, i64>(0))?;
        Ok(rows.next().transpose()?)
    }

    async fn catch_up(
        &self,
        session: &VizierSession,
        topic_id: &str,
        copy_before: i64,
        position: i64,
        cap: usize,
    ) -> Result<CatchUp> {
        let agent_id = session.0.as_str();
        let slug = session.to_slug();
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;

        let Some(read) = tx
            .query_row(
                "SELECT read_seq FROM workspace_read WHERE topic_id = ?1 AND agent_id = ?2",
                params![topic_id, agent_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        else {
            return Ok(CatchUp::default());
        };

        const RANGE: &str = "topic_id = ?1 AND seq > ?2 AND seq < ?3 \
                             AND kind IN ('person', 'agent') \
                             AND (source_session IS NULL OR source_session <> ?4)";
        let unread: i64 = tx.query_row(
            &format!("SELECT COUNT(*) FROM workspace_entry WHERE {RANGE}"),
            params![topic_id, read, copy_before, slug],
            |row| row.get::<_, i64>(0),
        )?;
        let mut rows = {
            let mut stmt = tx.prepare(&format!(
                "SELECT {ENTRY_COLUMNS} FROM workspace_entry WHERE {RANGE} \
                 ORDER BY seq DESC LIMIT ?5"
            ))?;
            let found = stmt.query_map(
                params![topic_id, read, copy_before, slug, cap as i64],
                row_to_entry,
            )?;
            found.collect::<crate::storage::sql::Result<Vec<_>>>()?
        };
        rows.reverse();

        for entry in &rows {
            let at = crate::time::from_millis(entry.at);
            let request = VizierRequest {
                timestamp: at,
                user: entry.author.clone(),
                content: VizierRequestContent::SilentRead(with_file_note(&entry.body, &entry.files)),
                metadata: serde_json::json!({
                    "workspace_entry": entry.seq,
                    "workspace_author_kind": entry.author_kind.as_str(),
                    "workspace_read": true,
                }),
                ..Default::default()
            };
            let history = SessionHistory {
                uid: uuid::Uuid::new_v4().to_string(),
                vizier_session: session.clone(),
                content: SessionHistoryContent::Request(request),
                timestamp: at,
                reactions: vec![],
                seq: None,
            };
            super::history::insert_history_entry(&tx, session, &history)?;
        }
        if position > read {
            tx.execute(
                "UPDATE workspace_read SET read_seq = ?1 WHERE topic_id = ?2 AND agent_id = ?3",
                params![position, topic_id, agent_id],
            )?;
        }
        tx.commit()?;
        Ok(CatchUp {
            copied: rows.len(),
            skipped: (unread as usize).saturating_sub(rows.len()),
        })
    }
}

fn row_to_mention(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<EntryMention> {
    let kind: String = row.get(1)?;
    let state: String = row.get(3)?;
    Ok(EntryMention {
        entry_seq: row.get(0)?,
        target_kind: MemberKind::parse(&kind).unwrap_or(MemberKind::Agent),
        target: row.get(2)?,
        state: MentionState::parse(&state).unwrap_or(MentionState::Pending),
    })
}

/// What a silent reader sees of a message's files: their names, which it can open from its own
/// session files.
fn with_file_note(body: &str, files: &[EntryFile]) -> String {
    if files.is_empty() {
        return body.to_string();
    }
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    format!("{body}\n[attached: {}]", names.join(", "))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::schema::{VizierResponse, VizierResponseContent};
    use crate::storage::history::HistoryStorage;

    fn storage() -> SqlStorage {
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::storage::sqlite::init_history_schema(&conn).unwrap();
            crate::storage::sqlite::init_reaction_schema(&conn).unwrap();
            init_workspace_schema(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE agent_config (agent_id TEXT PRIMARY KEY, data TEXT NOT NULL); \
                 CREATE TABLE artifact_reference (artifact_id TEXT, agent_id TEXT, kind TEXT, \
                 target TEXT, session_slug TEXT, history_uid TEXT, channel TEXT, topic TEXT, \
                 bundle TEXT, path TEXT, label TEXT, created_at INTEGER, \
                 UNIQUE (artifact_id, kind, target)); \
                 INSERT INTO agent_config VALUES ('alpha', '{\"name\": \"Alpha\"}');",
            )
            .unwrap();
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

    // --- the shared timeline ---------------------------------------------------------------

    fn session(agent: &str) -> VizierSession {
        VizierSession(agent.into(), VizierChannelId::Workspace("w1".into()), Some("w1-general".into()))
    }

    /// `ops` with rafi, and agents alpha and beta.
    async fn room(s: &SqlStorage) {
        let (w, t, o) = parts("w1", "ops");
        s.create_workspace(&w, &t, &o).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "alpha")).await.unwrap();
        s.add_member(&member("w1", MemberKind::Agent, "beta")).await.unwrap();
    }

    fn person_entry(body: &str, at: i64) -> NewEntry {
        NewEntry {
            workspace_id: "w1".into(),
            topic_id: "w1-general".into(),
            kind: EntryKind::Person,
            author_kind: MemberKind::Person,
            author: "rafi".into(),
            author_name: "rafi".into(),
            body: body.into(),
            files: vec![],
            history_uid: None,
            source_session: None,
            origin: None,
            is_final: false,
            activated_by_seq: None,
            at,
        }
    }

    fn reply(text: &str) -> SessionHistoryContent {
        SessionHistoryContent::Response(VizierResponse {
            timestamp: crate::time::now(),
            content: VizierResponseContent::Message { content: text.into(), stats: None },
            attachments: vec![],
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn an_agents_shown_text_joins_the_timeline_and_nothing_else_does() {
        let s = storage();
        room(&s).await;
        let alpha = session("alpha");
        let request = VizierRequest {
            timestamp: crate::time::now(),
            user: "rafi".into(),
            content: VizierRequestContent::Chat("@alpha look".into()),
            metadata: serde_json::json!({ "workspace_entry": 7 }),
            ..Default::default()
        };
        s.save_session_history(alpha.clone(), SessionHistoryContent::Request(request)).await.unwrap();
        // A copied read is a request too, and must not be taken for the activation.
        let read = VizierRequest {
            timestamp: crate::time::now(),
            user: "beta".into(),
            content: VizierRequestContent::SilentRead("noise".into()),
            metadata: serde_json::json!({ "workspace_entry": 99 }),
            ..Default::default()
        };
        s.save_session_history(alpha.clone(), SessionHistoryContent::Request(read)).await.unwrap();

        s.save_session_history(
            alpha.clone(),
            SessionHistoryContent::ToolCall { call_id: "c".into(), name: "think".into(), arguments: serde_json::json!({}) },
        )
        .await
        .unwrap();
        let said = s
            .save_session_history(alpha.clone(), SessionHistoryContent::AssistantMessage("checking, @beta and @rafi and @alpha".into()))
            .await
            .unwrap();
        let done = s.save_session_history(alpha.clone(), reply("done")).await.unwrap();

        let entries = s.list_entries("w1-general", None, 50).await.unwrap();
        assert_eq!(entries.len(), 2, "{entries:#?}");
        let (mid, last) = (&entries[0], &entries[1]);
        assert_eq!((mid.kind, mid.author.as_str(), mid.author_name.as_str()), (EntryKind::Agent, "alpha", "Alpha"));
        assert_eq!((mid.history_uid.as_deref(), mid.is_final, mid.activated_by_seq), (Some(said.as_str()), false, Some(7)));
        assert_eq!(mid.source_session.as_deref(), Some(alpha.to_slug().as_str()));
        assert_eq!((last.history_uid.as_deref(), last.is_final, last.body.as_str()), (Some(done.as_str()), true, "done"));
        assert_eq!(s.entry_by_history_uid(&said).await.unwrap().unwrap().seq, mid.seq);

        // Beta wakes later; rafi is only highlighted; alpha naming itself wakes nobody.
        let mentions = s.mentions_of(&[mid.seq, last.seq]).await.unwrap();
        let shape: Vec<_> = mentions.iter().map(|m| (m.target.as_str(), m.state)).collect();
        assert_eq!(shape, [("beta", MentionState::Pending), ("rafi", MentionState::Resolved)]);
        assert_eq!(s.pending_mentions().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_session_outside_a_workspace_or_without_a_topic_writes_no_entry() {
        let s = storage();
        room(&s).await;
        let elsewhere = VizierSession("alpha".into(), VizierChannelId::HTTP("rafi".into(), "x".into()), Some("t".into()));
        s.save_session_history(elsewhere, reply("hello")).await.unwrap();
        let gone = VizierSession("alpha".into(), VizierChannelId::Workspace("w1".into()), Some("deleted-topic".into()));
        s.save_session_history(gone, reply("hello")).await.unwrap();
        assert!(s.list_entries("w1-general", None, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn catch_up_copies_what_was_missed_once_and_moves_the_position() {
        let s = storage();
        room(&s).await;
        s.begin_reading_agent("w1", "beta").await.unwrap();
        for (i, text) in ["one", "two", "three"].iter().enumerate() {
            s.insert_entry(&person_entry(text, 100 + i as i64), &[]).await.unwrap();
        }
        let activating = s.insert_entry(&person_entry("@beta now", 200), &[]).await.unwrap();

        let beta = session("beta");
        let result = s.catch_up(&beta, "w1-general", activating.seq, activating.seq, 500).await.unwrap();
        assert_eq!(result, CatchUp { copied: 3, skipped: 0 });
        assert_eq!(s.read_position("w1-general", "beta").await.unwrap(), Some(activating.seq));

        let history = s.list_session_history(beta.clone(), None, None, None).await.unwrap();
        let texts: Vec<_> = history
            .iter()
            .filter_map(|h| match &h.content {
                SessionHistoryContent::Request(r) => match &r.content {
                    VizierRequestContent::SilentRead(t) => Some((r.user.clone(), t.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(texts, [("rafi".into(), "one".into()), ("rafi".into(), "two".into()), ("rafi".into(), "three".into())]);

        // The same call again copies nothing: the position moved with the copy.
        let again = s.catch_up(&beta, "w1-general", activating.seq, activating.seq, 500).await.unwrap();
        assert_eq!(again, CatchUp::default());
        assert_eq!(s.list_session_history(beta, None, None, None).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn catch_up_leaves_out_an_agents_own_text_notes_and_the_activating_entry() {
        let s = storage();
        room(&s).await;
        s.begin_reading_agent("w1", "beta").await.unwrap();
        let beta = session("beta");
        let mut own = person_entry("my own words", 100);
        (own.kind, own.author_kind, own.author, own.source_session) =
            (EntryKind::Agent, MemberKind::Agent, "beta".into(), Some(beta.to_slug()));
        s.insert_entry(&own, &[]).await.unwrap();
        let mut note = person_entry("Rafi stopped Alpha", 101);
        note.kind = EntryKind::Note;
        s.insert_entry(&note, &[]).await.unwrap();
        let mut by_alpha = person_entry("from alpha", 102);
        (by_alpha.kind, by_alpha.author_kind, by_alpha.author) = (EntryKind::Agent, MemberKind::Agent, "alpha".into());
        by_alpha.source_session = Some(session("alpha").to_slug());
        s.insert_entry(&by_alpha, &[]).await.unwrap();
        let activating = s.insert_entry(&person_entry("@beta", 103), &[]).await.unwrap();

        let result = s.catch_up(&beta, "w1-general", activating.seq, activating.seq, 500).await.unwrap();
        assert_eq!(result.copied, 1);
        let history = s.list_session_history(beta, None, None, None).await.unwrap();
        assert_eq!(history.len(), 1);
        // Tool calls and thoughts of the other agent never entered: only its shown text did.
        assert!(matches!(&history[0].content, SessionHistoryContent::Request(r) if r.user == "alpha"));
    }

    #[tokio::test]
    async fn catch_up_copies_the_newest_few_and_counts_the_rest() {
        let s = storage();
        room(&s).await;
        s.begin_reading_agent("w1", "beta").await.unwrap();
        for i in 0..7 {
            s.insert_entry(&person_entry(&format!("m{i}"), 100 + i), &[]).await.unwrap();
        }
        let head = s.head_seq("w1").await.unwrap();
        let result = s.catch_up(&session("beta"), "w1-general", head + 1, head, 3).await.unwrap();
        assert_eq!(result, CatchUp { copied: 3, skipped: 4 });
        let bodies: Vec<_> = s
            .list_session_history(session("beta"), None, None, None)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|h| match h.content {
                SessionHistoryContent::Request(r) => match r.content {
                    VizierRequestContent::SilentRead(t) => Some(t),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(bodies, ["m4", "m5", "m6"]);
    }

    #[tokio::test]
    async fn an_agent_that_never_started_reading_copies_nothing_and_a_new_one_starts_at_the_head() {
        let s = storage();
        room(&s).await;
        s.insert_entry(&person_entry("before", 100), &[]).await.unwrap();
        let head = s.head_seq("w1").await.unwrap();
        // No position row: nothing to catch up from.
        assert_eq!(s.catch_up(&session("beta"), "w1-general", head + 1, head, 500).await.unwrap(), CatchUp::default());

        s.begin_reading_agent("w1", "beta").await.unwrap();
        assert_eq!(s.read_position("w1-general", "beta").await.unwrap(), Some(head));
        // Joining again does not rewind it.
        s.insert_entry(&person_entry("after", 101), &[]).await.unwrap();
        s.begin_reading_agent("w1", "beta").await.unwrap();
        assert_eq!(s.read_position("w1-general", "beta").await.unwrap(), Some(head));
    }

    #[tokio::test]
    async fn stop_all_drops_pending_mentions_and_records_the_head() {
        let s = storage();
        room(&s).await;
        let e = s
            .insert_entry(&person_entry("@alpha @beta", 100), &[
                (MemberKind::Agent, "alpha".into(), MentionState::Pending),
                (MemberKind::Agent, "beta".into(), MentionState::Woken),
            ])
            .await
            .unwrap();
        let (dropped, head) = s.stop_all("w1", 500).await.unwrap();
        assert_eq!((dropped, head), (1, e.seq));
        assert_eq!(s.get_workspace("w1").await.unwrap().unwrap().stopped_at_seq, e.seq);
        let states: Vec<_> = s.mentions_of(&[e.seq]).await.unwrap().into_iter().map(|m| m.state).collect();
        assert_eq!(states, [MentionState::Dropped, MentionState::Woken]);
        assert!(s.pending_mentions().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn entries_page_back_from_the_newest() {
        let s = storage();
        room(&s).await;
        for i in 0..5 {
            s.insert_entry(&person_entry(&format!("m{i}"), 100 + i), &[]).await.unwrap();
        }
        let newest = s.list_entries("w1-general", None, 2).await.unwrap();
        assert_eq!(newest.iter().map(|e| e.body.as_str()).collect::<Vec<_>>(), ["m3", "m4"]);
        let older = s.list_entries("w1-general", Some(newest[0].seq), 2).await.unwrap();
        assert_eq!(older.iter().map(|e| e.body.as_str()).collect::<Vec<_>>(), ["m1", "m2"]);
    }
}
