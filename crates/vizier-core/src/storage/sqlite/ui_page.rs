//! `UiPageStorage` over the shared SQL connection (`specs/063-html-generative-ui/data-model.md`):
//! one implementation for SQLite and PostgreSQL.

use anyhow::Result;
use serde_json::Value;

use crate::{
    schema::{VizierChannelId, VizierSession},
    storage::{
        fence::Scope,
        sql::{self, Connection, Row, params},
        sqlite::{SqlStorage, fence::check_fence},
        ui_page::{
            AddVersionOutcome, NewUiPageVersion, SaveStateOutcome, UiPageRecord, UiPageStorage,
            UiPageVersionRecord, VersionOrigin, pending_verification,
        },
    },
};

const PAGE_COLUMNS: &str = "id, agent_id, session_channel, session_topic, title, latest_version, \
     state, state_seq, created_at, updated_at";

const VERSION_COLUMNS: &str = "page_id, version, size, sha256, origin, base_version, edit_count, \
     verification, history_uid, created_at";

/// The session predicate, parameters `?1` agent, `?2` channel JSON, `?3` topic.
const SESSION: &str =
    "agent_id = ?1 AND session_channel = ?2 AND session_topic IS NOT DISTINCT FROM ?3";

fn now_ms() -> i64 {
    crate::time::now().timestamp_millis()
}

fn channel_column(channel: &VizierChannelId) -> Result<String> {
    Ok(serde_json::to_string(channel)?)
}

/// A row that no longer parses (a channel shape from a newer build, say) reads as missing.
fn page_from_row(row: &Row) -> sql::Result<Option<UiPageRecord>> {
    let id: String = row.get(0)?;
    let agent_id: String = row.get(1)?;
    let channel: String = row.get(2)?;
    let topic: Option<String> = row.get(3)?;
    let title: String = row.get(4)?;
    let latest_version: i64 = row.get(5)?;
    let state: Option<String> = row.get(6)?;
    let state_seq: i64 = row.get(7)?;
    let created_at: i64 = row.get(8)?;
    let updated_at: i64 = row.get(9)?;
    let Ok(channel) = serde_json::from_str::<VizierChannelId>(&channel) else {
        return Ok(None);
    };
    Ok(Some(UiPageRecord {
        id,
        session: VizierSession(agent_id.clone(), channel, topic),
        agent_id,
        title,
        latest_version: latest_version.max(0) as u32,
        state: state.and_then(|text| serde_json::from_str(&text).ok()),
        state_seq: state_seq.max(0) as u64,
        created_at,
        updated_at,
    }))
}

fn version_from_row(row: &Row) -> sql::Result<Option<UiPageVersionRecord>> {
    let page_id: String = row.get(0)?;
    let version: i64 = row.get(1)?;
    let size: i64 = row.get(2)?;
    let sha256: String = row.get(3)?;
    let origin: String = row.get(4)?;
    let base_version: Option<i64> = row.get(5)?;
    let edit_count: Option<i64> = row.get(6)?;
    let verification: String = row.get(7)?;
    let history_uid: Option<String> = row.get(8)?;
    let created_at: i64 = row.get(9)?;
    let Some(origin) = VersionOrigin::parse(&origin) else {
        return Ok(None);
    };
    Ok(Some(UiPageVersionRecord {
        page_id,
        version: version.max(0) as u32,
        size: size.max(0) as u64,
        sha256,
        origin,
        base_version: base_version.map(|v| v.max(0) as u32),
        edit_count: edit_count.map(|v| v.max(0) as u32),
        verification: serde_json::from_str(&verification).unwrap_or_else(|_| pending_verification()),
        history_uid,
        created_at,
    }))
}

/// The agent a page belongs to, read in the writer's transaction. A missing page has nothing
/// to protect.
fn agent_of(tx: &Connection, page_id: &str) -> Result<Option<String>> {
    Ok(tx
        .query_all("SELECT agent_id FROM ui_page WHERE id = ?1", params![page_id])?
        .first()
        .map(|row| row.get::<_, String>(0))
        .transpose()?)
}

fn insert_version(
    tx: &Connection,
    page_id: &str,
    number: u32,
    version: &NewUiPageVersion,
    owner: &(Option<String>, Option<i64>),
    at: i64,
) -> Result<()> {
    tx.execute(
        &format!(
            "INSERT INTO ui_page_version ({VERSION_COLUMNS}, owner_node, owner_token) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11)"
        ),
        params![
            page_id,
            number as i64,
            version.size as i64,
            version.sha256,
            version.origin.as_str(),
            version.base_version.map(|v| v as i64),
            version.edit_count.map(|v| v as i64),
            pending_verification().to_string(),
            at,
            owner.0,
            owner.1,
        ],
    )?;
    Ok(())
}

fn page_ids(conn: &Connection, filter: &str, params: &[&dyn sql::ToSql]) -> Result<Vec<String>> {
    let rows = conn.query_all(&format!("SELECT id FROM ui_page WHERE {filter}"), params)?;
    rows.iter()
        .map(|row| row.get::<_, String>(0).map_err(Into::into))
        .collect()
}

/// Delete the pages matching `filter` and their versions, in one statement each.
fn delete_pages(conn: &Connection, filter: &str, params: &[&dyn sql::ToSql]) -> Result<Vec<String>> {
    let ids = page_ids(conn, filter, params)?;
    conn.execute(
        &format!("DELETE FROM ui_page_version WHERE page_id IN (SELECT id FROM ui_page WHERE {filter})"),
        params,
    )?;
    conn.execute(&format!("DELETE FROM ui_page WHERE {filter}"), params)?;
    Ok(ids)
}

/// A conversation's pages, inside the caller's transaction (`delete_session`).
pub(super) fn delete_session_pages(conn: &Connection, session: &VizierSession) -> Result<Vec<String>> {
    let channel = channel_column(&session.1)?;
    delete_pages(conn, SESSION, params![session.0, channel, session.2])
}

/// An agent's pages, inside the caller's transaction (`delete_agent`).
pub(super) fn delete_agent_pages(conn: &Connection, agent_id: &str) -> Result<Vec<String>> {
    delete_pages(conn, "agent_id = ?1", params![agent_id])
}

#[async_trait::async_trait]
impl UiPageStorage for SqlStorage {
    async fn create_ui_page(&self, page: &UiPageRecord, first: &NewUiPageVersion) -> Result<()> {
        let scope = Scope::Agent(page.agent_id.clone());
        let owner = self.fences.owner_of(&scope);
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        tx.execute(
            &format!(
                "INSERT INTO ui_page ({PAGE_COLUMNS}, owner_node, owner_token) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, NULL, 0, ?6, ?6, ?7, ?8)"
            ),
            params![
                page.id,
                page.agent_id,
                channel_column(&page.session.1)?,
                page.session.2,
                page.title,
                page.created_at,
                owner.0,
                owner.1,
            ],
        )?;
        insert_version(&tx, &page.id, 1, first, &owner, page.created_at)?;
        tx.commit()?;
        Ok(())
    }

    async fn add_ui_page_version(
        &self,
        page_id: &str,
        base: u32,
        version: &NewUiPageVersion,
        title: Option<&str>,
    ) -> Result<AddVersionOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent_id) = agent_of(&tx, page_id)? else {
            return Ok(AddVersionOutcome::NotFound);
        };
        let scope = Scope::Agent(agent_id);
        check_fence(&tx, &self.fences, &scope)?;
        let owner = self.fences.owner_of(&scope);
        let at = now_ms();
        let next = base + 1;
        // The guard: only the caller that saw `base` as the latest moves it on.
        let changed = tx.execute(
            "UPDATE ui_page SET latest_version = ?1, title = COALESCE(?2, title), updated_at = ?3, \
             owner_node = ?4, owner_token = ?5 WHERE id = ?6 AND latest_version = ?7",
            params![next as i64, title, at, owner.0, owner.1, page_id, base as i64],
        )?;
        if changed == 0 {
            let latest: i64 = tx.query_row(
                "SELECT latest_version FROM ui_page WHERE id = ?1",
                params![page_id],
                |row| row.get(0),
            )?;
            return Ok(AddVersionOutcome::Stale {
                latest: latest.max(0) as u32,
            });
        }
        insert_version(&tx, page_id, next, version, &owner, at)?;
        tx.commit()?;
        Ok(AddVersionOutcome::Added(next))
    }

    async fn get_ui_page(&self, page_id: &str) -> Result<Option<UiPageRecord>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            &format!("SELECT {PAGE_COLUMNS} FROM ui_page WHERE id = ?1"),
            params![page_id],
        )?;
        Ok(match rows.first() {
            Some(row) => page_from_row(row)?,
            None => None,
        })
    }

    async fn get_ui_page_version(
        &self,
        page_id: &str,
        version: u32,
    ) -> Result<Option<UiPageVersionRecord>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            &format!("SELECT {VERSION_COLUMNS} FROM ui_page_version WHERE page_id = ?1 AND version = ?2"),
            params![page_id, version as i64],
        )?;
        Ok(match rows.first() {
            Some(row) => version_from_row(row)?,
            None => None,
        })
    }

    async fn list_ui_page_versions(&self, page_id: &str) -> Result<Vec<UiPageVersionRecord>> {
        let conn = self.conn.lock()?;
        let rows = conn.query_all(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM ui_page_version WHERE page_id = ?1 ORDER BY version"
            ),
            params![page_id],
        )?;
        let mut versions = Vec::with_capacity(rows.len());
        for row in &rows {
            if let Some(version) = version_from_row(row)? {
                versions.push(version);
            }
        }
        Ok(versions)
    }

    async fn set_ui_page_verification(
        &self,
        page_id: &str,
        version: u32,
        verification: &Value,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent_id) = agent_of(&tx, page_id)? else {
            return Ok(());
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent_id))?;
        tx.execute(
            "UPDATE ui_page_version SET verification = ?1 WHERE page_id = ?2 AND version = ?3",
            params![verification.to_string(), page_id, version as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn set_ui_page_history_uid(
        &self,
        page_id: &str,
        version: u32,
        uid: &str,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let Some(agent_id) = agent_of(&tx, page_id)? else {
            return Ok(());
        };
        check_fence(&tx, &self.fences, &Scope::Agent(agent_id))?;
        tx.execute(
            "UPDATE ui_page_version SET history_uid = ?1 WHERE page_id = ?2 AND version = ?3",
            params![uid, page_id, version as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn save_ui_page_state(
        &self,
        page_id: &str,
        state: &Value,
        expected_seq: Option<u64>,
    ) -> Result<SaveStateOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let stored = tx
            .query_all("SELECT state_seq FROM ui_page WHERE id = ?1", params![page_id])?
            .first()
            .map(|row| row.get::<_, i64>(0))
            .transpose()?;
        let Some(stored) = stored.map(|seq| seq.max(0) as u64) else {
            return Ok(SaveStateOutcome::NotFound);
        };
        if expected_seq.is_some_and(|expected| expected != stored) {
            return Ok(SaveStateOutcome::Stale(stored));
        }
        let next = stored + 1;
        tx.execute(
            "UPDATE ui_page SET state = ?1, state_seq = ?2, updated_at = ?3 WHERE id = ?4",
            params![state.to_string(), next as i64, now_ms(), page_id],
        )?;
        tx.commit()?;
        Ok(SaveStateOutcome::Saved(next))
    }

    async fn delete_ui_pages_for_session(&self, session: &VizierSession) -> Result<Vec<String>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let ids = delete_session_pages(&tx, session)?;
        tx.commit()?;
        Ok(ids)
    }

    async fn delete_ui_pages_for_agent(&self, agent_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        let ids = delete_agent_pages(&tx, agent_id)?;
        tx.commit()?;
        Ok(ids)
    }

    async fn sweep_pending_ui_verifications(&self, agent_id: &str, below: u64) -> Result<u64> {
        let scope = Scope::Agent(agent_id.to_string());
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        check_fence(&tx, &self.fences, &scope)?;
        let closed = serde_json::json!({ "state": "unverified", "reason": "closed" }).to_string();
        let changed = tx.execute(
            "UPDATE ui_page_version SET verification = ?1 \
             WHERE verification = ?2 AND (owner_token IS NULL OR owner_token < ?3) \
             AND page_id IN (SELECT id FROM ui_page WHERE agent_id = ?4)",
            params![closed, pending_verification().to_string(), below as i64, agent_id],
        )?;
        tx.commit()?;
        Ok(changed as u64)
    }
}

#[cfg(test)]
#[path = "ui_page_tests.rs"]
mod tests;
