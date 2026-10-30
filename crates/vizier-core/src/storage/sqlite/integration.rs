use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::{
    encryption::{
        IntegrationSecretPaths, ProtectMode, SecretCipher, fields::decrypt_integration_settings,
    },
    schema::{
        AgentId, EventAction, EventDraft, Integration, IntegrationEvent, IntegrationFamily,
        IntegrationGrant, Requester,
    },
    storage::{
        integration::{IntegrationConflict, IntegrationStorage},
        sqlite::SqliteStorage,
    },
};

const COLUMNS: &str =
    "id, kind, family, name, settings, enabled, created_by, creator, created_at, updated_at";

fn parse_time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// `creator` as stored, or the 016 `created_by` user id it replaces.
fn read_creator(creator: Option<String>, created_by: Option<String>) -> Option<Requester> {
    creator
        .and_then(|c| serde_json::from_str(&c).ok())
        .or(created_by.map(Requester::User))
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawIntegration> {
    Ok(RawIntegration {
        id: row.get(0)?,
        kind: row.get(1)?,
        family: row.get(2)?,
        name: row.get(3)?,
        settings: row.get(4)?,
        enabled: row.get(5)?,
        created_by: row.get(6)?,
        creator: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// One `integration` row before its settings are decrypted. Decryption needs the cipher and the
/// kind's secret paths, which the row-mapping closure cannot carry.
struct RawIntegration {
    id: String,
    kind: String,
    family: String,
    name: String,
    settings: String,
    enabled: i64,
    created_by: Option<String>,
    creator: Option<String>,
    created_at: String,
    updated_at: String,
}

/// Integrations matching `filter` (a SQL condition over `integration`), with their agents.
fn load(
    conn: &Connection,
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
    filter: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<Integration>> {
    let sql = format!("SELECT {COLUMNS} FROM integration WHERE {filter} ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params, from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let mut integrations: Vec<Integration> = Vec::with_capacity(rows.len());
    for row in rows {
        let (Some(family), Ok(settings)) = (
            IntegrationFamily::parse(&row.family),
            serde_json::from_str::<serde_json::Value>(&row.settings),
        ) else {
            continue;
        };
        let settings =
            decrypt_integration_settings(cipher, paths, &row.id, &row.kind, &settings)
                .map_err(|e| anyhow!("integration '{}': {e}", row.id))?;
        integrations.push(Integration {
            id: row.id,
            kind: row.kind,
            family,
            name: row.name,
            settings,
            enabled: row.enabled != 0,
            creator: read_creator(row.creator, row.created_by),
            agents: vec![],
            created_at: parse_time(&row.created_at),
            updated_at: parse_time(&row.updated_at),
        });
    }

    let mut attachments: HashMap<String, Vec<AgentId>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT integration_id, agent_id FROM integration_attachment ORDER BY created_at, agent_id",
    )?;
    for row in stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))? {
        let (id, agent) = row?;
        attachments.entry(id).or_default().push(agent);
    }
    for integration in &mut integrations {
        integration.agents = attachments.remove(&integration.id).unwrap_or_default();
    }
    Ok(integrations)
}

fn get(
    conn: &Connection,
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
    id: &str,
) -> Result<Option<Integration>> {
    Ok(load(conn, cipher, paths, "id = ?1", &[&id])?.into_iter().next())
}

fn attached_agents(conn: &Connection, id: &str) -> Result<Vec<AgentId>> {
    let mut stmt =
        conn.prepare("SELECT agent_id FROM integration_attachment WHERE integration_id = ?1")?;
    let agents = stmt
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(agents)
}

/// Another integration linked to `agent_id` that matches `condition`, as `(id, name)`.
fn attached_other(
    conn: &Connection,
    agent_id: &str,
    exclude: &str,
    condition: &str,
    value: &str,
) -> Result<Option<(String, String)>> {
    let sql = format!(
        "SELECT i.id, i.name FROM integration i \
         JOIN integration_attachment a ON a.integration_id = i.id \
         WHERE a.agent_id = ?1 AND i.id != ?2 AND {condition} LIMIT 1"
    );
    Ok(conn
        .query_row(&sql, rusqlite::params![agent_id, exclude, value], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?)
}

/// An MCP server named like `integration` already on `agent_id`.
fn check_mcp_name(conn: &Connection, integration: &Integration, agent_id: &str) -> Result<()> {
    if let Some((integration_id, name)) = attached_other(
        conn,
        agent_id,
        &integration.id,
        "i.family = 'mcp' AND i.name = ?3",
        &integration.name,
    )? {
        return Err(IntegrationConflict::NameInUseOnAgent { integration_id, name }.into());
    }
    Ok(())
}

/// Link a new integration to its agent, under the family's rule.
fn link(conn: &Connection, integration: &Integration, agent_id: &str) -> Result<()> {
    match integration.family {
        IntegrationFamily::Channel => {
            if let Some((integration_id, name)) =
                attached_other(conn, agent_id, &integration.id, "i.kind = ?3", &integration.kind)?
            {
                return Err(IntegrationConflict::PlatformInUseOnAgent { integration_id, name }.into());
            }
        }
        IntegrationFamily::Mcp => check_mcp_name(conn, integration, agent_id)?,
        // One configuration per addon per agent, or two would give the same tool names.
        IntegrationFamily::Addon => {
            if let Some((integration_id, name)) =
                attached_other(conn, agent_id, &integration.id, "i.kind = ?3", &integration.kind)?
            {
                return Err(IntegrationConflict::AddonInstalled { integration_id, name }.into());
            }
        }
    }
    conn.execute(
        "INSERT INTO integration_attachment (integration_id, agent_id, created_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![integration.id, agent_id, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Record `event` about `integration` on each of `agents`.
fn record(
    conn: &Connection,
    agents: &[AgentId],
    integration_id: Option<&str>,
    integration_name: &str,
    family: IntegrationFamily,
    event: &EventDraft,
) -> Result<()> {
    let actor = serde_json::to_string(&event.actor)?;
    let fields = serde_json::to_string(&event.fields)?;
    let at = Utc::now().to_rfc3339();
    for agent_id in agents {
        conn.execute(
            "INSERT INTO integration_event (id, agent_id, integration_id, integration_name, family, \
             actor, action, fields, session, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                agent_id,
                integration_id,
                integration_name,
                family.as_str(),
                actor,
                event.action.as_str(),
                fields,
                event.session,
                at,
            ],
        )?;
    }
    Ok(())
}

/// Delete what only `agent_id` uses, unlink it from legacy rows, and drop its grants and
/// events; the ids deleted. Shared with `AgentStorage::delete_agent`, which already holds the
/// connection.
pub(crate) fn delete_agent_integrations(conn: &Connection, agent_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT a.integration_id FROM integration_attachment a WHERE a.agent_id = ?1 \
         AND (SELECT COUNT(*) FROM integration_attachment b \
              WHERE b.integration_id = a.integration_id) = 1",
    )?;
    let owned = stmt
        .query_map([agent_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    for id in &owned {
        conn.execute("DELETE FROM integration_attachment WHERE integration_id = ?1", [id])?;
        conn.execute("DELETE FROM integration WHERE id = ?1", [id])?;
    }
    conn.execute("DELETE FROM integration_attachment WHERE agent_id = ?1", [agent_id])?;
    conn.execute("DELETE FROM integration_grant WHERE agent_id = ?1", [agent_id])?;
    conn.execute("DELETE FROM integration_event WHERE agent_id = ?1", [agent_id])?;
    Ok(owned)
}

fn channel_token(settings: &serde_json::Value) -> Option<&str> {
    settings
        .get("token")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
}

fn union(lists: impl IntoIterator<Item = Vec<AgentId>>) -> Vec<AgentId> {
    lists
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<IntegrationEvent>> {
    let family: String = row.get(4)?;
    let actor: String = row.get(5)?;
    let action: String = row.get(6)?;
    let fields: String = row.get(7)?;
    let at: String = row.get(9)?;
    let (Some(family), Ok(actor), Some(action)) = (
        IntegrationFamily::parse(&family),
        serde_json::from_str(&actor),
        EventAction::parse(&action),
    ) else {
        return Ok(None);
    };
    Ok(Some(IntegrationEvent {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        integration_id: row.get(2)?,
        integration_name: row.get(3)?,
        family,
        actor,
        action,
        fields: serde_json::from_str(&fields).unwrap_or_default(),
        session: row.get(8)?,
        at: parse_time(&at),
    }))
}

#[async_trait::async_trait]
impl IntegrationStorage for SqliteStorage {
    async fn list_integrations(&self) -> Result<Vec<Integration>> {
        load(
            &self.conn.lock(),
            &self.cipher,
            &*self.secret_paths,
            "1 = 1",
            &[],
        )
    }

    async fn list_agent_integrations(&self, agent_id: &str) -> Result<Vec<Integration>> {
        load(
            &self.conn.lock(),
            &self.cipher,
            &*self.secret_paths,
            "id IN (SELECT integration_id FROM integration_attachment WHERE agent_id = ?1)",
            &[&agent_id],
        )
    }

    async fn get_integration(&self, id: &str) -> Result<Option<Integration>> {
        get(&self.conn.lock(), &self.cipher, &*self.secret_paths, id)
    }

    async fn save_integration(
        &self,
        integration: &Integration,
        attach_to: Option<&str>,
        events: &[EventDraft],
    ) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        if attach_to.is_some()
            && get(&tx, &self.cipher, &*self.secret_paths, &integration.id)?.is_some()
        {
            return Err(anyhow!("integration '{}' already exists", integration.id));
        }

        if integration.family == IntegrationFamily::Channel
            && let Some(token) = channel_token(&integration.settings)
        {
            let same_kind = load(
                &tx,
                &self.cipher,
                &*self.secret_paths,
                "kind = ?1 AND id != ?2",
                &[&integration.kind, &integration.id],
            )?;
            if let Some(other) = same_kind
                .iter()
                .find(|other| channel_token(&other.settings) == Some(token))
            {
                return Err(IntegrationConflict::TokenInUse {
                    integration_id: other.id.clone(),
                    name: other.name.clone(),
                }
                .into());
            }
        }

        let (protected_settings, _) = crate::encryption::fields::protect_integration_settings(
            &self.cipher,
            &*self.secret_paths,
            &integration.id,
            &integration.kind,
            &integration.settings,
            ProtectMode::Write,
        )?;

        let before = attached_agents(&tx, &integration.id)?;
        let creator = integration.creator.as_ref().map(serde_json::to_string).transpose()?;
        // `scope` is a 016 column kept for the DDL (no migrations); nothing reads it.
        tx.execute(
            "INSERT INTO integration (id, kind, family, name, settings, enabled, scope, creator, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'owned', ?7, ?8, ?9) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, settings = excluded.settings, \
             enabled = excluded.enabled, updated_at = excluded.updated_at",
            rusqlite::params![
                integration.id,
                integration.kind,
                integration.family.as_str(),
                integration.name,
                serde_json::to_string(&protected_settings)?,
                integration.enabled as i64,
                creator,
                integration.created_at.to_rfc3339(),
                integration.updated_at.to_rfc3339(),
            ],
        )?;

        // A rename must not collide on an agent it is already on.
        if integration.family == IntegrationFamily::Mcp {
            for agent in &before {
                check_mcp_name(&tx, integration, agent)?;
            }
        }

        if let Some(agent_id) = attach_to {
            link(&tx, integration, agent_id)?;
        }
        let after = attached_agents(&tx, &integration.id)?;
        for event in events {
            record(&tx, &after, Some(&integration.id), &integration.name, integration.family, event)?;
        }
        tx.commit()?;
        Ok(union([before, after]))
    }

    async fn delete_integration(&self, id: &str, event: &EventDraft) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let Some(integration) = get(&tx, &self.cipher, &*self.secret_paths, id)? else {
            return Ok(vec![]);
        };
        record(&tx, &integration.agents, Some(id), &integration.name, integration.family, event)?;
        tx.execute("DELETE FROM integration_attachment WHERE integration_id = ?1", [id])?;
        tx.execute("DELETE FROM integration WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(integration.agents)
    }

    async fn unlink(&self, id: &str, agent_id: &str, event: &EventDraft) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let integration = get(&tx, &self.cipher, &*self.secret_paths, id)?
            .ok_or_else(|| anyhow!("integration '{id}' not found"))?;
        if integration.agents.len() < 2 || !integration.agents.iter().any(|a| a == agent_id) {
            return Err(anyhow!(
                "'{}' is not linked to several agents including '{agent_id}'; delete it instead",
                integration.name
            ));
        }
        let agents = vec![agent_id.to_string()];
        record(&tx, &agents, Some(id), &integration.name, integration.family, event)?;
        tx.execute(
            "DELETE FROM integration_attachment WHERE integration_id = ?1 AND agent_id = ?2",
            rusqlite::params![id, agent_id],
        )?;
        tx.commit()?;
        Ok(agents)
    }

    async fn delete_agent_integrations(&self, agent_id: &str) -> Result<Vec<String>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let deleted = delete_agent_integrations(&tx, agent_id)?;
        tx.commit()?;
        Ok(deleted)
    }

    async fn list_grants(&self, agent_id: &str) -> Result<Vec<IntegrationGrant>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT family, granted_by, granted_at FROM integration_grant WHERE agent_id = ?1 \
             ORDER BY family",
        )?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(family, granted_by, granted_at)| {
                Some(IntegrationGrant {
                    agent_id: agent_id.to_string(),
                    family: IntegrationFamily::parse(&family)?,
                    granted_by,
                    granted_at: parse_time(&granted_at),
                })
            })
            .collect())
    }

    async fn set_grant(
        &self,
        agent_id: &str,
        family: IntegrationFamily,
        on: bool,
        granted_by: &str,
    ) -> Result<bool> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let changed = if on {
            tx.execute(
                "INSERT OR IGNORE INTO integration_grant (agent_id, family, granted_by, granted_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![agent_id, family.as_str(), granted_by, Utc::now().to_rfc3339()],
            )?
        } else {
            tx.execute(
                "DELETE FROM integration_grant WHERE agent_id = ?1 AND family = ?2",
                rusqlite::params![agent_id, family.as_str()],
            )?
        } > 0;
        if changed {
            let action = if on { EventAction::GrantOn } else { EventAction::GrantOff };
            let event = EventDraft::new(Requester::User(granted_by.to_string()), action);
            record(&tx, &[agent_id.to_string()], None, family.as_str(), family, &event)?;
        }
        tx.commit()?;
        Ok(changed)
    }

    async fn list_events(
        &self,
        agent_id: &str,
        integration_id: Option<&str>,
        before: Option<(DateTime<Utc>, String)>,
        limit: usize,
    ) -> Result<Vec<IntegrationEvent>> {
        let conn = self.conn.lock();
        let (before_at, before_id) = match &before {
            Some((at, id)) => (Some(at.to_rfc3339()), Some(id.clone())),
            None => (None, None),
        };
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, integration_id, integration_name, family, actor, action, fields, \
             session, at FROM integration_event WHERE agent_id = ?1 \
             AND (?2 IS NULL OR integration_id = ?2) \
             AND (?3 IS NULL OR at < ?3 OR (at = ?3 AND id < ?4)) \
             ORDER BY at DESC, id DESC LIMIT ?5",
        )?;
        let events = stmt
            .query_map(
                rusqlite::params![agent_id, integration_id, before_at, before_id, limit as i64],
                event_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(events.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use serde_json::json;

    use super::*;
    use crate::storage::{document::LocalDocumentStore, sqlite::init_integration_schema};

    fn storage() -> SqliteStorage {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        init_integration_schema(&conn).unwrap();
        SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(std::env::temp_dir())),
        )
    }

    fn ev(action: EventAction) -> EventDraft {
        EventDraft::new(Requester::User("admin".into()), action)
    }

    fn created() -> EventDraft {
        ev(EventAction::Created)
    }

    fn integration(id: &str, kind: &str, family: IntegrationFamily, name: &str, settings: serde_json::Value) -> Integration {
        Integration {
            id: id.into(),
            kind: kind.into(),
            family,
            name: name.into(),
            settings,
            enabled: true,
            creator: Some(Requester::Agent("a".into())),
            agents: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn mcp(id: &str, name: &str) -> Integration {
        integration(id, "mcp", IntegrationFamily::Mcp, name, json!({"transport": "http", "uri": "http://x"}))
    }

    fn bot(id: &str, kind: &str, token: &str) -> Integration {
        integration(id, kind, IntegrationFamily::Channel, id, json!({ "token": token }))
    }

    fn brave(id: &str) -> Integration {
        integration(id, "brave_search", IntegrationFamily::Addon, id, json!({"api_key": "k"}))
    }

    fn conflict(err: anyhow::Error) -> IntegrationConflict {
        err.downcast::<IntegrationConflict>().expect("a conflict")
    }

    /// A 016-style second link, which nothing can create any more.
    fn legacy_link(s: &SqliteStorage, id: &str, agent_id: &str) {
        s.conn
            .lock()
            .execute(
                "INSERT INTO integration_attachment (integration_id, agent_id, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, agent_id, Utc::now().to_rfc3339()],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn round_trip_on_one_agent() {
        let s = storage();
        assert_eq!(s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap(), ["a"]);

        let got = s.get_integration("1").await.unwrap().unwrap();
        assert_eq!(got.name, "fs");
        assert_eq!(got.family, IntegrationFamily::Mcp);
        assert_eq!(got.settings["uri"], "http://x");
        assert_eq!(got.agents, ["a"]);
        assert_eq!(got.creator, Some(Requester::Agent("a".into())));
        assert_eq!(got.owner_agent().map(String::as_str), Some("a"));

        let mut renamed = got.clone();
        renamed.name = "files".into();
        renamed.enabled = false;
        assert_eq!(s.save_integration(&renamed, None, &[ev(EventAction::Edited)]).await.unwrap(), ["a"]);
        let got = s.get_integration("1").await.unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.enabled), ("files", false));
        assert_eq!(got.agents, ["a"]);
    }

    #[tokio::test]
    async fn creating_an_existing_id_is_refused() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        assert!(s.save_integration(&mcp("1", "fs"), Some("b"), &[created()]).await.is_err());
        assert_eq!(s.get_integration("1").await.unwrap().unwrap().agents, ["a"]);
    }

    #[tokio::test]
    async fn a_legacy_creator_reads_as_a_user() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        s.conn
            .lock()
            .execute("UPDATE integration SET creator = NULL, created_by = 'alice' WHERE id = '1'", [])
            .unwrap();
        let got = s.get_integration("1").await.unwrap().unwrap();
        assert_eq!(got.creator, Some(Requester::User("alice".into())));
    }

    #[tokio::test]
    async fn delete_reports_agents_and_records_it() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        assert_eq!(s.delete_integration("1", &ev(EventAction::Deleted)).await.unwrap(), ["a"]);
        assert!(s.get_integration("1").await.unwrap().is_none());
        assert!(s.list_agent_integrations("a").await.unwrap().is_empty());
        let actions: Vec<_> = s
            .list_events("a", Some("1"), None, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect();
        assert_eq!(actions, [EventAction::Deleted, EventAction::Created]);
    }

    #[tokio::test]
    async fn a_deleted_agent_takes_its_integrations_and_only_unlinks_legacy_ones() {
        let s = storage();
        s.save_integration(&mcp("m", "fs"), Some("a"), &[created()]).await.unwrap();
        s.save_integration(&bot("t", "telegram", "tok"), Some("a"), &[created()]).await.unwrap();
        s.save_integration(&mcp("shared", "git"), Some("a"), &[created()]).await.unwrap();
        legacy_link(&s, "shared", "b");
        s.set_grant("a", IntegrationFamily::Mcp, true, "admin").await.unwrap();

        let mut deleted = s.delete_agent_integrations("a").await.unwrap();
        deleted.sort();
        assert_eq!(deleted, ["m", "t"]);
        assert!(s.get_integration("m").await.unwrap().is_none());
        assert_eq!(s.get_integration("shared").await.unwrap().unwrap().agents, ["b"]);
        assert!(s.list_grants("a").await.unwrap().is_empty());
        assert!(s.list_events("a", None, None, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn unlink_is_only_for_legacy_rows() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        assert!(s.unlink("1", "a", &ev(EventAction::Unlinked)).await.is_err());
        legacy_link(&s, "1", "b");
        let got = s.get_integration("1").await.unwrap().unwrap();
        assert!(got.is_legacy());
        assert_eq!(s.unlink("1", "a", &ev(EventAction::Unlinked)).await.unwrap(), ["a"]);
        assert_eq!(s.get_integration("1").await.unwrap().unwrap().agents, ["b"]);
        assert_eq!(s.list_events("a", Some("1"), None, 1).await.unwrap()[0].action, EventAction::Unlinked);
    }

    #[tokio::test]
    async fn a_token_is_used_once_per_platform() {
        let s = storage();
        s.save_integration(&bot("t1", "telegram", "tok"), Some("a"), &[created()]).await.unwrap();
        let err = s.save_integration(&bot("t2", "telegram", "tok"), Some("b"), &[created()]).await.unwrap_err();
        assert_eq!(conflict(err).code(), "token_in_use");
        // Another platform may hold the same string, and re-saving the holder is fine.
        s.save_integration(&bot("d1", "discord", "tok"), Some("c"), &[created()]).await.unwrap();
        s.save_integration(&bot("t1", "telegram", "tok"), None, &[ev(EventAction::Edited)]).await.unwrap();
    }

    #[tokio::test]
    async fn one_connection_per_platform_per_agent() {
        let s = storage();
        s.save_integration(&bot("t1", "telegram", "a"), Some("x"), &[created()]).await.unwrap();
        let err = s.save_integration(&bot("t2", "telegram", "b"), Some("x"), &[created()]).await.unwrap_err();
        assert_eq!(
            conflict(err),
            IntegrationConflict::PlatformInUseOnAgent { integration_id: "t1".into(), name: "t1".into() }
        );
        s.save_integration(&bot("d1", "discord", "c"), Some("x"), &[created()]).await.unwrap();
    }

    #[tokio::test]
    async fn mcp_names_are_unique_per_agent_only() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        // Same name elsewhere is fine.
        s.save_integration(&mcp("2", "fs"), Some("b"), &[created()]).await.unwrap();
        let err = s.save_integration(&mcp("4", "fs"), Some("a"), &[created()]).await.unwrap_err();
        assert_eq!(conflict(err).code(), "name_in_use_on_agent");

        // Renaming into a name already on its agent is refused too.
        s.save_integration(&mcp("3", "git"), Some("a"), &[created()]).await.unwrap();
        let err = s.save_integration(&mcp("3", "fs"), None, &[ev(EventAction::Edited)]).await.unwrap_err();
        assert_eq!(conflict(err).code(), "name_in_use_on_agent");
    }

    #[tokio::test]
    async fn a_second_configuration_of_an_addon_is_refused() {
        let s = storage();
        s.save_integration(&brave("b1"), Some("a"), &[created()]).await.unwrap();
        let err = s.save_integration(&brave("b2"), Some("a"), &[created()]).await.unwrap_err();
        assert_eq!(
            conflict(err),
            IntegrationConflict::AddonInstalled { integration_id: "b1".into(), name: "b1".into() }
        );
        // Another agent gets its own.
        s.save_integration(&brave("b3"), Some("c"), &[created()]).await.unwrap();
    }

    #[tokio::test]
    async fn grants_switch_once_and_record_it() {
        let s = storage();
        assert!(s.set_grant("a", IntegrationFamily::Mcp, true, "alice").await.unwrap());
        assert!(!s.set_grant("a", IntegrationFamily::Mcp, true, "bob").await.unwrap());
        let grants = s.list_grants("a").await.unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!((grants[0].family, grants[0].granted_by.as_str()), (IntegrationFamily::Mcp, "alice"));

        assert!(s.set_grant("a", IntegrationFamily::Mcp, false, "bob").await.unwrap());
        assert!(!s.set_grant("a", IntegrationFamily::Mcp, false, "bob").await.unwrap());
        assert!(s.list_grants("a").await.unwrap().is_empty());

        let events = s.list_events("a", None, None, 10).await.unwrap();
        let actions: Vec<_> = events.iter().map(|e| e.action).collect();
        assert_eq!(actions, [EventAction::GrantOff, EventAction::GrantOn]);
        assert_eq!(events[0].actor, Requester::User("bob".into()));
    }

    #[tokio::test]
    async fn events_page_newest_first() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a"), &[created()]).await.unwrap();
        for _ in 0..3 {
            s.save_integration(&mcp("1", "fs"), None, &[ev(EventAction::Edited)]).await.unwrap();
        }
        let first = s.list_events("a", None, None, 2).await.unwrap();
        assert_eq!(first.len(), 2);
        let last = &first[1];
        let rest = s.list_events("a", None, Some((last.at, last.id.clone())), 10).await.unwrap();
        assert_eq!(rest.len(), 2);
        assert_eq!(rest.last().unwrap().action, EventAction::Created);
    }

    /// With the production cipher, a `Secret`/`SecretMap` path is an envelope while transport,
    /// command, args and safe-search stay readable; a plaintext secret after migration is an
    /// error, not silently accepted.
    #[tokio::test]
    async fn settings_are_protected_at_rest_and_tampering_errors() {
        use crate::encryption::storage_tests::{file_backed, raw_value, vizier_storage};

        let h = file_backed();
        let v = vizier_storage(&h);
        v.save_integration(&bot("t1", "telegram", "telegram-secret-abc123"), Some("a"), &[created()])
            .await
            .unwrap();
        v.save_integration(
            &integration(
                "b1",
                "brave_search",
                IntegrationFamily::Addon,
                "brave",
                json!({"api_key": "brave-secret-abc123", "safesearch": true}),
            ),
            Some("a"),
            &[created()],
        )
        .await
        .unwrap();
        v.save_integration(
            &integration(
                "m1",
                "mcp",
                IntegrationFamily::Mcp,
                "crypto",
                json!({"transport": "local", "command": "cat", "args": ["--verbose"],
                       "env": {"GITHUB_TOKEN": "mcp-secret-abc123"}}),
            ),
            Some("a"),
            &[created()],
        )
        .await
        .unwrap();
        v.save_integration(
            &integration("legacy", "retired_kind", IntegrationFamily::Mcp, "legacy",
                         json!({"token": "legacy-secret-abc123"})),
            Some("a"),
            &[created()],
        )
        .await
        .unwrap();

        let telegram = raw_value(&h.storage, "SELECT settings FROM integration WHERE id = 't1'", &[]);
        assert!(telegram.contains("enc:v1:") && !telegram.contains("telegram-secret-abc123"));
        let brave_raw = raw_value(&h.storage, "SELECT settings FROM integration WHERE id = 'b1'", &[]);
        assert!(brave_raw.contains("enc:v1:"));
        assert!(brave_raw.contains("\"safesearch\":true"));
        let mcp_raw = raw_value(&h.storage, "SELECT settings FROM integration WHERE id = 'm1'", &[]);
        assert!(mcp_raw.contains("enc:v1:") && !mcp_raw.contains("mcp-secret-abc123"));
        assert!(mcp_raw.contains("\"command\":\"cat\""));
        assert!(mcp_raw.contains("\"args\":[\"--verbose\"]"));
        let legacy_raw = raw_value(&h.storage, "SELECT settings FROM integration WHERE id = 'legacy'", &[]);
        assert!(legacy_raw.starts_with('"'));
        assert!(!legacy_raw.contains("legacy-secret-abc123"));

        // Reads decrypt for their normal consumers.
        assert_eq!(
            v.get_integration("t1").await.unwrap().unwrap().settings["token"],
            "telegram-secret-abc123"
        );
        assert_eq!(
            v.get_integration("legacy").await.unwrap().unwrap().settings["token"],
            "legacy-secret-abc123"
        );

        h.storage
            .conn
            .lock()
            .execute("UPDATE integration SET settings = '{\"token\":\"plaintext\"}' WHERE id = 't1'", [])
            .unwrap();
        assert!(v.get_integration("t1").await.is_err());
        assert!(v.list_integrations().await.is_err());
    }
}
