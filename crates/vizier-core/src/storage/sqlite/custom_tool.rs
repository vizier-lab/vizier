//! `CustomToolStorage` over the shared SQL connection (`specs/061-agent-custom-tools/data-model.md`):
//! one implementation for SQLite and PostgreSQL. A save is one transaction that re-checks every
//! stored-state rule; an emptied category is deleted in the transaction that empties it.

use anyhow::Result;
use uuid::Uuid;

use crate::storage::{
    custom_tool::{
        CategoryOutcome, CustomTool, CustomToolActor, CustomToolCategory, CustomToolDefinition,
        CustomToolEvent, CustomToolStorage, CustomToolVersion, NewVersion, SaveOutcome,
        VersionTrigger,
    },
    sql::{Connection, OptionalExtension, params},
    sqlite::SqlStorage,
};

fn now_ms() -> i64 {
    crate::time::now().timestamp_millis()
}

const VERSION_COLUMNS: &str = "tool_id, seq, name, category, description, input_schema, \
     output_description, script, calls, sample_input, actor_kind, actor_id, actor_name, \
     dream_cycle_id, trigger_kind, restored_from, reason, created_at";

const TOOL_COLUMNS: &str =
    "id, agent_id, name, category, current_seq, enabled, disabled_by, last_called_at, created_at, updated_at";

type ToolRow = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
    Option<i64>,
    i64,
    i64,
);

fn read_tool_row(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<ToolRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

type VersionRow = (
    String,
    i64,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<i64>,
    Option<String>,
    i64,
);

fn read_version_row(row: &crate::storage::sql::Row) -> crate::storage::sql::Result<VersionRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
    ))
}

fn version_from_row(row: VersionRow) -> Result<CustomToolVersion> {
    let (
        tool_id,
        seq,
        name,
        category,
        description,
        input_schema,
        output_description,
        script,
        calls,
        sample_input,
        actor_kind,
        actor_id,
        actor_name,
        dream_cycle_id,
        trigger,
        restored_from,
        reason,
        created_at,
    ) = row;
    Ok(CustomToolVersion {
        tool_id,
        seq,
        definition: CustomToolDefinition {
            name,
            category,
            description,
            input_schema: serde_json::from_str(&input_schema)?,
            output_description,
            script,
            calls: serde_json::from_str(&calls)?,
            sample_input: sample_input.map(|text| serde_json::from_str(&text)).transpose()?,
        },
        actor: CustomToolActor::from_columns(&actor_kind, actor_id, actor_name, dream_cycle_id),
        trigger: VersionTrigger::parse(&trigger),
        restored_from,
        reason,
        created_at,
    })
}

fn actor_json(actor: &CustomToolActor) -> Result<String> {
    Ok(serde_json::to_string(actor)?)
}

fn load_version(conn: &Connection, tool_id: &str, seq: i64) -> Result<Option<CustomToolVersion>> {
    let row = conn
        .query_row(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM custom_tool_version WHERE tool_id = ?1 AND seq = ?2"
            ),
            params![tool_id, seq],
            read_version_row,
        )
        .optional()?;
    row.map(version_from_row).transpose()
}

fn tool_from_row(conn: &Connection, row: ToolRow) -> Result<Option<CustomTool>> {
    let (id, agent_id, name, category, current_seq, enabled, disabled_by, last_called_at, created_at, updated_at) =
        row;
    let Some(current) = load_version(conn, &id, current_seq)? else {
        return Ok(None);
    };
    Ok(Some(CustomTool {
        id,
        agent_id,
        name,
        category,
        current_seq,
        enabled: enabled != 0,
        disabled_by: disabled_by.map(|text| serde_json::from_str(&text)).transpose()?,
        last_called_at,
        created_at,
        updated_at,
        current,
    }))
}

fn load_tool_by_id(conn: &Connection, agent_id: &str, tool_id: &str) -> Result<Option<CustomTool>> {
    let row = conn
        .query_row(
            &format!("SELECT {TOOL_COLUMNS} FROM custom_tool WHERE agent_id = ?1 AND id = ?2"),
            params![agent_id, tool_id],
            read_tool_row,
        )
        .optional()?;
    match row {
        Some(row) => tool_from_row(conn, row),
        None => Ok(None),
    }
}

fn event_row(
    conn: &Connection,
    agent_id: &str,
    tool_id: Option<&str>,
    tool_name: Option<&str>,
    kind: &str,
    detail: serde_json::Value,
    actor: &CustomToolActor,
    at: i64,
) -> Result<()> {
    let (actor_kind, actor_id, actor_name, cycle) = actor.to_columns();
    conn.execute(
        "INSERT INTO custom_tool_event
             (id, agent_id, tool_id, tool_name, kind, detail, actor_kind, actor_id, actor_name,
              dream_cycle_id, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            Uuid::new_v4().to_string(),
            agent_id,
            tool_id,
            tool_name,
            kind,
            detail.to_string(),
            actor_kind,
            actor_id,
            actor_name,
            cycle,
            at,
        ],
    )?;
    Ok(())
}

/// Delete the categories of `agent_id` no tool is in any more.
fn delete_empty_categories(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM custom_tool_category WHERE agent_id = ?1 AND name NOT IN
             (SELECT category FROM custom_tool WHERE agent_id = ?1)",
        params![agent_id],
    )?;
    Ok(())
}

fn category_exists(conn: &Connection, agent_id: &str, name: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT COUNT(*) FROM custom_tool_category WHERE agent_id = ?1 AND name = ?2",
            params![agent_id, name],
            |row| row.get::<_, i64>(0),
        )?
        > 0)
}

fn load_category(conn: &Connection, agent_id: &str, name: &str) -> Result<Option<CustomToolCategory>> {
    let row: Option<(String, String, Option<String>, i64)> = conn
        .query_row(
            "SELECT name, description, updated_by, updated_at FROM custom_tool_category
             WHERE agent_id = ?1 AND name = ?2",
            params![agent_id, name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((name, description, updated_by, updated_at)) = row else {
        return Ok(None);
    };
    let (tools, enabled_tools): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(enabled), 0) FROM custom_tool WHERE agent_id = ?1 AND category = ?2",
        params![agent_id, &name],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(Some(CustomToolCategory {
        name,
        description,
        updated_by: updated_by.map(|text| serde_json::from_str(&text)).transpose()?,
        updated_at,
        tools,
        enabled_tools,
    }))
}

/// Delete every custom tool row of an agent, on a connection the caller already holds.
pub(crate) fn delete_agent_custom_tools(conn: &Connection, agent_id: &str) -> Result<()> {
    for table in [
        "custom_tool_version",
        "custom_tool",
        "custom_tool_category",
        "custom_tool_event",
    ] {
        conn.execute(&format!("DELETE FROM {table} WHERE agent_id = ?1"), params![agent_id])?;
    }
    Ok(())
}

#[async_trait::async_trait]
impl CustomToolStorage for SqlStorage {
    async fn list_custom_tools(
        &self,
        agent_id: &str,
        include_disabled: bool,
    ) -> Result<Vec<CustomTool>> {
        let conn = self.conn.lock()?;
        let sql = format!(
            "SELECT {TOOL_COLUMNS} FROM custom_tool WHERE agent_id = ?1 {} ORDER BY name",
            if include_disabled { "" } else { "AND enabled = 1" }
        );
        let rows: Vec<ToolRow> = {
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_map(params![agent_id], read_tool_row)?
                .collect::<crate::storage::sql::Result<_>>()?
        };
        let mut tools = vec![];
        for row in rows {
            if let Some(tool) = tool_from_row(&conn, row)? {
                tools.push(tool);
            }
        }
        Ok(tools)
    }

    async fn get_custom_tool(&self, agent_id: &str, name: &str) -> Result<Option<CustomTool>> {
        let conn = self.conn.lock()?;
        let row = conn
            .query_row(
                &format!("SELECT {TOOL_COLUMNS} FROM custom_tool WHERE agent_id = ?1 AND name = ?2"),
                params![agent_id, name],
                read_tool_row,
            )
            .optional()?;
        match row {
            Some(row) => tool_from_row(&conn, row),
            None => Ok(None),
        }
    }

    async fn get_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: &str,
        seq: i64,
    ) -> Result<Option<CustomToolVersion>> {
        let conn = self.conn.lock()?;
        let row = conn
            .query_row(
                &format!(
                    "SELECT {VERSION_COLUMNS} FROM custom_tool_version
                     WHERE agent_id = ?1 AND tool_id = ?2 AND seq = ?3"
                ),
                params![agent_id, tool_id, seq],
                read_version_row,
            )
            .optional()?;
        row.map(version_from_row).transpose()
    }

    async fn list_custom_tool_versions(
        &self,
        agent_id: &str,
        tool_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolVersion>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {VERSION_COLUMNS} FROM custom_tool_version
             WHERE agent_id = ?1 AND tool_id = ?2 AND seq < ?3
             ORDER BY seq DESC LIMIT ?4"
        ))?;
        let rows: Vec<VersionRow> = stmt
            .query_map(
                params![agent_id, tool_id, before.unwrap_or(i64::MAX), limit as i64],
                read_version_row,
            )?
            .collect::<crate::storage::sql::Result<_>>()?;
        rows.into_iter().map(version_from_row).collect()
    }

    async fn list_custom_tool_categories(&self, agent_id: &str) -> Result<Vec<CustomToolCategory>> {
        let conn = self.conn.lock()?;
        let names: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT name FROM custom_tool_category WHERE agent_id = ?1 ORDER BY name",
            )?;
            stmt.query_map(params![agent_id], |row| row.get(0))?
                .collect::<crate::storage::sql::Result<_>>()?
        };
        let mut categories = vec![];
        for name in names {
            if let Some(category) = load_category(&conn, agent_id, &name)? {
                categories.push(category);
            }
        }
        Ok(categories)
    }

    async fn list_custom_tool_events(
        &self,
        agent_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolEvent>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_id, tool_name, kind, detail, actor_kind, actor_id, actor_name,
                    dream_cycle_id, at
             FROM custom_tool_event WHERE agent_id = ?1 AND at < ?2
             ORDER BY at DESC, id LIMIT ?3",
        )?;
        type EventRow = (
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
        );
        let rows: Vec<EventRow> = stmt
            .query_map(
                params![agent_id, before.unwrap_or(i64::MAX), limit as i64],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )?
            .collect::<crate::storage::sql::Result<_>>()?;
        rows.into_iter()
            .map(
                |(id, tool_id, tool_name, kind, detail, actor_kind, actor_id, actor_name, cycle, at)| {
                    Ok(CustomToolEvent {
                        id,
                        tool_id,
                        tool_name,
                        kind,
                        detail: detail
                            .map(|text| serde_json::from_str(&text))
                            .transpose()?
                            .unwrap_or_default(),
                        actor: CustomToolActor::from_columns(&actor_kind, actor_id, actor_name, cycle),
                        at,
                    })
                },
            )
            .collect()
    }

    async fn save_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: Option<&str>,
        expected_seq: Option<i64>,
        version: NewVersion,
        actor: &CustomToolActor,
        category_description: Option<&str>,
    ) -> Result<SaveOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();
        let definition = &version.definition;

        // The tool being updated, and the version a saver expects to replace.
        let existing = match tool_id {
            Some(id) => match load_tool_by_id(&tx, agent_id, id)? {
                Some(tool) => Some(tool),
                None => return Ok(SaveOutcome::NotFound),
            },
            None => None,
        };
        if let (Some(tool), Some(expected)) = (&existing, expected_seq) {
            if tool.current_seq != expected {
                return Ok(SaveOutcome::OutOfDate { current_seq: tool.current_seq });
            }
        }

        let other: Option<String> = tx
            .query_row(
                "SELECT id FROM custom_tool WHERE agent_id = ?1 AND name = ?2",
                params![agent_id, &definition.name],
                |row| row.get(0),
            )
            .optional()?;
        if other.is_some_and(|other| Some(other.as_str()) != tool_id) {
            return Ok(SaveOutcome::NameTaken);
        }

        let category_created = !category_exists(&tx, agent_id, &definition.category)?;
        if category_created {
            let Some(description) = category_description.map(str::trim).filter(|d| !d.is_empty())
            else {
                return Ok(SaveOutcome::CategoryDescriptionRequired);
            };
            tx.execute(
                "INSERT INTO custom_tool_category (agent_id, name, description, updated_by, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![agent_id, &definition.category, description, actor_json(actor)?, now],
            )?;
        }

        let (id, seq) = match &existing {
            Some(tool) => (tool.id.clone(), tool.current_seq + 1),
            None => (Uuid::new_v4().to_string(), 1),
        };
        let (actor_kind, actor_id, actor_name, cycle) = actor.to_columns();
        tx.execute(
            &format!(
                "INSERT INTO custom_tool_version (agent_id, {VERSION_COLUMNS})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"
            ),
            params![
                agent_id,
                &id,
                seq,
                &definition.name,
                &definition.category,
                &definition.description,
                definition.input_schema.to_string(),
                definition.output_description.as_deref(),
                &definition.script,
                serde_json::to_string(&definition.calls)?,
                definition.sample_input.as_ref().map(|value| value.to_string()),
                actor_kind,
                actor_id,
                actor_name,
                cycle,
                version.trigger.as_str(),
                version.restored_from,
                version.reason.as_deref(),
                now,
            ],
        )?;
        match &existing {
            Some(_) => {
                tx.execute(
                    "UPDATE custom_tool SET name = ?3, category = ?4, current_seq = ?5, updated_at = ?6
                     WHERE agent_id = ?1 AND id = ?2",
                    params![agent_id, &id, &definition.name, &definition.category, seq, now],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO custom_tool
                         (id, agent_id, name, category, current_seq, enabled, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
                    params![&id, agent_id, &definition.name, &definition.category, seq, now],
                )?;
            }
        }
        // A move may have emptied the old category.
        delete_empty_categories(&tx, agent_id)?;
        let tool = load_tool_by_id(&tx, agent_id, &id)?
            .ok_or_else(|| anyhow::anyhow!("custom tool {id} vanished inside its own save"))?;
        tx.commit()?;
        Ok(SaveOutcome::Saved { tool, category_created })
    }

    async fn set_custom_tool_enabled(
        &self,
        agent_id: &str,
        tool_id: &str,
        enabled: bool,
        actor: &CustomToolActor,
    ) -> Result<Option<CustomTool>> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let Some(tool) = load_tool_by_id(&tx, agent_id, tool_id)? else {
            return Ok(None);
        };
        let now = now_ms();
        let disabled_by = (!enabled).then(|| actor_json(actor)).transpose()?;
        tx.execute(
            "UPDATE custom_tool SET enabled = ?3, disabled_by = ?4, updated_at = ?5
             WHERE agent_id = ?1 AND id = ?2",
            params![agent_id, tool_id, enabled as i64, disabled_by, now],
        )?;
        event_row(
            &tx,
            agent_id,
            Some(tool_id),
            Some(&tool.name),
            if enabled { "enabled" } else { "disabled" },
            serde_json::Value::Null,
            actor,
            now,
        )?;
        let tool = load_tool_by_id(&tx, agent_id, tool_id)?;
        tx.commit()?;
        Ok(tool)
    }

    async fn delete_custom_tool(
        &self,
        agent_id: &str,
        tool_id: &str,
        actor: &CustomToolActor,
    ) -> Result<bool> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let Some(tool) = load_tool_by_id(&tx, agent_id, tool_id)? else {
            return Ok(false);
        };
        tx.execute(
            "DELETE FROM custom_tool_version WHERE agent_id = ?1 AND tool_id = ?2",
            params![agent_id, tool_id],
        )?;
        tx.execute(
            "DELETE FROM custom_tool WHERE agent_id = ?1 AND id = ?2",
            params![agent_id, tool_id],
        )?;
        event_row(
            &tx,
            agent_id,
            Some(tool_id),
            Some(&tool.name),
            "deleted",
            serde_json::json!({ "version": tool.current_seq, "category": tool.category }),
            actor,
            now_ms(),
        )?;
        delete_empty_categories(&tx, agent_id)?;
        tx.commit()?;
        Ok(true)
    }

    async fn update_custom_tool_category(
        &self,
        agent_id: &str,
        name: &str,
        description: Option<&str>,
        new_name: Option<&str>,
        actor: &CustomToolActor,
    ) -> Result<CategoryOutcome> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        if !category_exists(&tx, agent_id, name)? {
            return Ok(CategoryOutcome::NotFound);
        }
        let now = now_ms();
        let mut current = name.to_string();
        let mut moved = vec![];

        if let Some(new_name) = new_name.filter(|new_name| *new_name != name) {
            if category_exists(&tx, agent_id, new_name)? {
                return Ok(CategoryOutcome::NameTaken);
            }
            let tools: Vec<(String, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, name FROM custom_tool WHERE agent_id = ?1 AND category = ?2 ORDER BY name",
                )?;
                stmt.query_map(params![agent_id, name], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<crate::storage::sql::Result<_>>()?
            };
            tx.execute(
                "UPDATE custom_tool_category SET name = ?3, updated_by = ?4, updated_at = ?5
                 WHERE agent_id = ?1 AND name = ?2",
                params![agent_id, name, new_name, actor_json(actor)?, now],
            )?;
            tx.execute(
                "UPDATE custom_tool SET category = ?3, updated_at = ?4 WHERE agent_id = ?1 AND category = ?2",
                params![agent_id, name, new_name, now],
            )?;
            for (tool_id, tool_name) in &tools {
                event_row(
                    &tx,
                    agent_id,
                    Some(tool_id),
                    Some(tool_name),
                    "category_renamed",
                    serde_json::json!({ "from": name, "to": new_name }),
                    actor,
                    now,
                )?;
            }
            moved = tools.into_iter().map(|(_, tool_name)| tool_name).collect();
            current = new_name.to_string();
        }

        if let Some(description) = description.map(str::trim).filter(|d| !d.is_empty()) {
            tx.execute(
                "UPDATE custom_tool_category SET description = ?3, updated_by = ?4, updated_at = ?5
                 WHERE agent_id = ?1 AND name = ?2",
                params![agent_id, &current, description, actor_json(actor)?, now],
            )?;
            event_row(
                &tx,
                agent_id,
                None,
                None,
                "category_described",
                serde_json::json!({ "category": &current, "description": description }),
                actor,
                now,
            )?;
        }

        let category = load_category(&tx, agent_id, &current)?
            .ok_or_else(|| anyhow::anyhow!("category {current} vanished inside its own update"))?;
        tx.commit()?;
        Ok(CategoryOutcome::Updated { category, moved })
    }

    async fn touch_custom_tool_called(&self, tool_id: &str, at: i64) -> Result<()> {
        let conn = self.conn.lock()?;
        conn.execute(
            "UPDATE custom_tool SET last_called_at = ?2 WHERE id = ?1",
            params![tool_id, at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "custom_tool_tests.rs"]
mod tests;
