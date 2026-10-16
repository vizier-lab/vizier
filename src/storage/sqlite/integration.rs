use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::{
    schema::{AgentId, Integration, IntegrationFamily, IntegrationScope},
    storage::{
        integration::{IntegrationConflict, IntegrationStorage},
        sqlite::SqliteStorage,
    },
};

const COLUMNS: &str =
    "id, kind, family, name, settings, enabled, scope, created_by, created_at, updated_at";

fn parse_time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<Integration>> {
    let family: String = row.get(2)?;
    let settings: String = row.get(4)?;
    let scope: String = row.get(6)?;
    let created_at: String = row.get(8)?;
    let updated_at: String = row.get(9)?;
    let (Some(family), Some(scope), Ok(settings)) = (
        IntegrationFamily::parse(&family),
        IntegrationScope::parse(&scope),
        serde_json::from_str(&settings),
    ) else {
        return Ok(None);
    };
    Ok(Some(Integration {
        id: row.get(0)?,
        kind: row.get(1)?,
        family,
        name: row.get(3)?,
        settings,
        enabled: row.get::<_, i64>(5)? != 0,
        scope,
        created_by: row.get(7)?,
        agents: vec![],
        created_at: parse_time(&created_at),
        updated_at: parse_time(&updated_at),
    }))
}

/// Integrations matching `filter` (a SQL condition over `integration`), with their agents.
fn load(
    conn: &Connection,
    filter: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<Integration>> {
    let sql = format!("SELECT {COLUMNS} FROM integration WHERE {filter} ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let mut integrations: Vec<Integration> = stmt
        .query_map(params, from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

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

fn get(conn: &Connection, id: &str) -> Result<Option<Integration>> {
    Ok(load(conn, "id = ?1", &[&id])?.into_iter().next())
}

fn attached_agents(conn: &Connection, id: &str) -> Result<Vec<AgentId>> {
    let mut stmt =
        conn.prepare("SELECT agent_id FROM integration_attachment WHERE integration_id = ?1")?;
    let agents = stmt
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(agents)
}

/// Another integration attached to `agent_id` that matches `condition`, as `(id, name)`.
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

/// An MCP server named like `integration` already attached to `agent_id`.
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

/// Attach under the family's rule. Returns the other agents the attachment affected (the
/// previous agent of a moved channel connection).
fn attach(conn: &Connection, integration: &Integration, agent_id: &str) -> Result<Vec<AgentId>> {
    let mut affected = vec![];
    match integration.family {
        IntegrationFamily::Channel => {
            if let Some((integration_id, name)) = attached_other(
                conn,
                agent_id,
                &integration.id,
                "i.kind = ?3",
                &integration.kind,
            )? {
                return Err(
                    IntegrationConflict::PlatformInUseOnAgent { integration_id, name }.into()
                );
            }
            // One agent per connection: attaching elsewhere is a move.
            affected = attached_agents(conn, &integration.id)?
                .into_iter()
                .filter(|a| a != agent_id)
                .collect();
            conn.execute(
                "DELETE FROM integration_attachment WHERE integration_id = ?1 AND agent_id != ?2",
                rusqlite::params![integration.id, agent_id],
            )?;
        }
        IntegrationFamily::Mcp => check_mcp_name(conn, integration, agent_id)?,
        // One configuration per addon per agent, or two would give the same tool names.
        IntegrationFamily::Addon => {
            conn.execute(
                "DELETE FROM integration_attachment WHERE agent_id = ?1 AND integration_id IN \
                 (SELECT id FROM integration WHERE kind = ?2 AND id != ?3)",
                rusqlite::params![agent_id, integration.kind, integration.id],
            )?;
        }
    }
    conn.execute(
        "INSERT OR IGNORE INTO integration_attachment (integration_id, agent_id, created_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![integration.id, agent_id, Utc::now().to_rfc3339()],
    )?;
    Ok(affected)
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

#[async_trait::async_trait]
impl IntegrationStorage for SqliteStorage {
    async fn list_integrations(&self) -> Result<Vec<Integration>> {
        load(&self.conn.lock(), "1 = 1", &[])
    }

    async fn list_agent_integrations(&self, agent_id: &str) -> Result<Vec<Integration>> {
        load(
            &self.conn.lock(),
            "id IN (SELECT integration_id FROM integration_attachment WHERE agent_id = ?1)",
            &[&agent_id],
        )
    }

    async fn get_integration(&self, id: &str) -> Result<Option<Integration>> {
        get(&self.conn.lock(), id)
    }

    async fn save_integration(
        &self,
        integration: &Integration,
        attach_to: Option<&str>,
    ) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        if integration.family == IntegrationFamily::Channel
            && let Some(token) = channel_token(&integration.settings)
        {
            let same_kind = load(&tx, "kind = ?1 AND id != ?2", &[&integration.kind, &integration.id])?;
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

        let before = attached_agents(&tx, &integration.id)?;
        tx.execute(
            "INSERT INTO integration (id, kind, family, name, settings, enabled, scope, created_by, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, settings = excluded.settings, \
             enabled = excluded.enabled, updated_at = excluded.updated_at",
            rusqlite::params![
                integration.id,
                integration.kind,
                integration.family.as_str(),
                integration.name,
                serde_json::to_string(&integration.settings)?,
                integration.enabled as i64,
                integration.scope.as_str(),
                integration.created_by,
                integration.created_at.to_rfc3339(),
                integration.updated_at.to_rfc3339(),
            ],
        )?;

        // A rename must not collide on an agent it is already attached to.
        if integration.family == IntegrationFamily::Mcp {
            for agent in &before {
                check_mcp_name(&tx, integration, agent)?;
            }
        }

        let moved = match attach_to {
            Some(agent_id) => attach(&tx, integration, agent_id)?,
            None => vec![],
        };
        let after = attached_agents(&tx, &integration.id)?;
        tx.commit()?;
        Ok(union([before, after, moved]))
    }

    async fn delete_integration(&self, id: &str) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let before = attached_agents(&tx, id)?;
        tx.execute("DELETE FROM integration_attachment WHERE integration_id = ?1", [id])?;
        tx.execute("DELETE FROM integration WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(before)
    }

    async fn set_attachment(
        &self,
        id: &str,
        agent_id: &str,
        attached: bool,
    ) -> Result<Vec<AgentId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let integration = get(&tx, id)?.ok_or_else(|| anyhow!("integration '{id}' not found"))?;
        let moved = if attached {
            attach(&tx, &integration, agent_id)?
        } else {
            tx.execute(
                "DELETE FROM integration_attachment WHERE integration_id = ?1 AND agent_id = ?2",
                rusqlite::params![id, agent_id],
            )?;
            vec![]
        };
        tx.commit()?;
        Ok(union([vec![agent_id.to_string()], moved]))
    }

    async fn delete_agent_attachments(&self, agent_id: &str) -> Result<()> {
        self.conn
            .lock()
            .execute("DELETE FROM integration_attachment WHERE agent_id = ?1", [agent_id])?;
        Ok(())
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

    fn integration(id: &str, kind: &str, family: IntegrationFamily, name: &str, settings: serde_json::Value) -> Integration {
        Integration {
            id: id.into(),
            kind: kind.into(),
            family,
            name: name.into(),
            settings,
            enabled: true,
            scope: IntegrationScope::Shared,
            created_by: Some("admin".into()),
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

    #[tokio::test]
    async fn round_trip_with_attachments() {
        let s = storage();
        let affected = s.save_integration(&mcp("1", "fs"), Some("a")).await.unwrap();
        assert_eq!(affected, ["a"]);
        s.set_attachment("1", "b", true).await.unwrap();

        let got = s.get_integration("1").await.unwrap().unwrap();
        assert_eq!(got.name, "fs");
        assert_eq!(got.family, IntegrationFamily::Mcp);
        assert_eq!(got.settings["uri"], "http://x");
        assert_eq!(got.agents, ["a", "b"]);
        assert_eq!(s.list_agent_integrations("b").await.unwrap().len(), 1);
        assert_eq!(s.list_integrations().await.unwrap().len(), 1);

        let mut renamed = got.clone();
        renamed.name = "files".into();
        renamed.enabled = false;
        assert_eq!(s.save_integration(&renamed, None).await.unwrap(), ["a", "b"]);
        let got = s.get_integration("1").await.unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.enabled), ("files", false));

        assert_eq!(s.set_attachment("1", "a", false).await.unwrap(), ["a"]);
        assert_eq!(s.get_integration("1").await.unwrap().unwrap().agents, ["b"]);
    }

    #[tokio::test]
    async fn delete_cascades_and_reports_agents() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a")).await.unwrap();
        s.set_attachment("1", "b", true).await.unwrap();
        assert_eq!(s.delete_integration("1").await.unwrap(), ["a", "b"]);
        assert!(s.get_integration("1").await.unwrap().is_none());
        assert!(s.list_agent_integrations("a").await.unwrap().is_empty());

        s.save_integration(&mcp("2", "fs"), Some("a")).await.unwrap();
        s.delete_agent_attachments("a").await.unwrap();
        assert!(s.get_integration("2").await.unwrap().unwrap().agents.is_empty());
    }

    #[tokio::test]
    async fn attaching_a_channel_moves_it() {
        let s = storage();
        s.save_integration(&bot("t", "telegram", "tok"), Some("a")).await.unwrap();
        assert_eq!(s.set_attachment("t", "b", true).await.unwrap(), ["a", "b"]);
        assert_eq!(s.get_integration("t").await.unwrap().unwrap().agents, ["b"]);
    }

    #[tokio::test]
    async fn a_token_is_used_once_per_platform() {
        let s = storage();
        s.save_integration(&bot("t1", "telegram", "tok"), None).await.unwrap();
        let err = s.save_integration(&bot("t2", "telegram", "tok"), None).await.unwrap_err();
        assert_eq!(conflict(err).code(), "token_in_use");
        // Another platform may hold the same string, and re-saving the holder is fine.
        s.save_integration(&bot("d1", "discord", "tok"), None).await.unwrap();
        s.save_integration(&bot("t1", "telegram", "tok"), None).await.unwrap();
    }

    #[tokio::test]
    async fn one_connection_per_platform_per_agent() {
        let s = storage();
        s.save_integration(&bot("t1", "telegram", "a"), Some("x")).await.unwrap();
        let err = s.save_integration(&bot("t2", "telegram", "b"), Some("x")).await.unwrap_err();
        assert_eq!(
            conflict(err),
            IntegrationConflict::PlatformInUseOnAgent { integration_id: "t1".into(), name: "t1".into() }
        );
        s.save_integration(&bot("d1", "discord", "c"), Some("x")).await.unwrap();
    }

    #[tokio::test]
    async fn mcp_names_are_unique_per_agent_only() {
        let s = storage();
        s.save_integration(&mcp("1", "fs"), Some("a")).await.unwrap();
        // Same name elsewhere is fine.
        s.save_integration(&mcp("2", "fs"), Some("b")).await.unwrap();
        let err = s.set_attachment("2", "a", true).await.unwrap_err();
        assert_eq!(conflict(err).code(), "name_in_use_on_agent");

        // Renaming into a name already attached on one of its agents is refused too.
        s.save_integration(&mcp("3", "git"), Some("a")).await.unwrap();
        let err = s.save_integration(&mcp("3", "fs"), None).await.unwrap_err();
        assert_eq!(conflict(err).code(), "name_in_use_on_agent");
    }

    #[tokio::test]
    async fn attaching_an_addon_replaces_the_agents_other_configuration() {
        let s = storage();
        s.save_integration(&brave("b1"), Some("a")).await.unwrap();
        s.save_integration(&brave("b2"), None).await.unwrap();
        s.set_attachment("b1", "c", true).await.unwrap();
        s.set_attachment("b2", "a", true).await.unwrap();
        let on_a: Vec<String> = s
            .list_agent_integrations("a")
            .await
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(on_a, ["b2"]);
        // Other agents keep theirs.
        assert_eq!(s.get_integration("b1").await.unwrap().unwrap().agents, ["c"]);
    }
}
