//! The 040 → 041 project channel migration (`specs/041-project-coding-workspace`
//! contracts/channel-and-migration.md §4).
//!
//! 040 stored a project chat as an ordinary HTTP session whose channel id was
//! `project__<id>`, so its slug was `http__<user>__project__<id>`. 041 gives it a variant of its
//! own and a slug `project__<user>__<id>`. Every column that matches on that slug — history,
//! session details, pending turns, scratch files, artifacts, usage, background jobs and the
//! dream journal — is rewritten once at startup under the marker
//! [`MARKER`]. The old-shape parser lives here and nowhere else: the variant is the only
//! representation code may use afterwards.

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::{Connection, params};
use serde_json::Value;

use crate::schema::VizierChannelId;

/// The state key that makes this migration run once.
pub const MARKER: &str = "migration.project_channel_v1";

/// The old 040 slug for one project chat: `http__<user>__project__<id>`.
fn parse_old_slug(slug: &str) -> Option<(String, String)> {
    let rest = slug.strip_prefix("http__")?;
    let (user, wire) = rest.split_once("__")?;
    if user.is_empty() {
        return None;
    }
    let id = crate::projects::channel::project_id(wire)?;
    Some((user.to_string(), id.to_string()))
}

fn new_slug(user: &str, id: &str) -> String {
    VizierChannelId::Project(user.to_string(), id.to_string()).to_slug()
}

fn migration_done(conn: &Connection) -> Result<bool> {
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            params![MARKER],
            |row| row.get(0),
        )
        .ok();
    Ok(done.as_deref() == Some("1"))
}

/// Rewrite every string in the value: exact old slugs, and old slugs embedded in a longer slug
/// such as `DREAM__…`.
fn rewrite_strings(value: &mut Value, slugs: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            for (old, new) in slugs {
                if text.contains(old.as_str()) {
                    *text = text.replace(old.as_str(), new);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_strings(item, slugs);
            }
        }
        Value::Object(map) => {
            // A serialised channel: `{"HTTP":[user,"project__<id>"]}` → `{"Project":[user,"<id>"]}`.
            let project = match map.get("HTTP") {
                Some(Value::Array(channel)) => match (channel.first(), channel.get(1)) {
                    (Some(Value::String(user)), Some(Value::String(wire))) => {
                        crate::projects::channel::project_id(wire)
                            .map(|id| (user.clone(), id.to_string()))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some((user, id)) = project {
                map.remove("HTTP");
                map.insert(
                    "Project".to_string(),
                    Value::Array(vec![Value::String(user), Value::String(id)]),
                );
            }
            for item in map.values_mut() {
                rewrite_strings(item, slugs);
            }
        }
        _ => {}
    }
}

fn rewrite_json_column(
    conn: &Connection,
    table: &str,
    key: &str,
    column: &str,
    slugs: &BTreeMap<String, String>,
) -> Result<usize> {
    let sql = format!(
        "SELECT CAST({key} AS TEXT), {column} FROM {table} WHERE {column} IS NOT NULL"
    );
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        mapped.collect::<rusqlite::Result<_>>()?
    };
    let mut changed = 0;
    for (id, text) in rows {
        let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let before = value.clone();
        rewrite_strings(&mut value, slugs);
        if value == before {
            continue;
        }
        conn.execute(
            &format!("UPDATE {table} SET {column} = ?1 WHERE CAST({key} AS TEXT) = ?2"),
            params![serde_json::to_string(&value)?, id],
        )?;
        changed += 1;
    }
    Ok(changed)
}

/// One exact-match column rewrite, with the number of rows it changed.
fn rewrite_exact(
    conn: &Connection,
    table: &str,
    column: &str,
    old: &str,
    new: &str,
) -> Result<usize> {
    let sql = format!("UPDATE {table} SET {column} = ?1 WHERE {column} = ?2");
    Ok(conn.execute(&sql, params![new, old])?)
}

/// One `REPLACE` rewrite for a composite column (a slug embedded in a longer key).
fn rewrite_embedded(
    conn: &Connection,
    table: &str,
    column: &str,
    old: &str,
    new: &str,
) -> Result<usize> {
    let sql = format!(
        "UPDATE {table} SET {column} = REPLACE({column}, ?1, ?2) WHERE {column} LIKE ?3 ESCAPE '\\'"
    );
    let like = format!("%{}%", old.replace('\\', "\\\\").replace('_', "\\_"));
    Ok(conn.execute(&sql, params![old, new, like])?)
}

/// Rewrite every stored channel shape. A second run is a no-op, both because the marker is set
/// and because no old slug remains to match.
pub fn migrate_project_channels(conn: &Connection) -> Result<()> {
    if migration_done(conn)? {
        return Ok(());
    }

    // Every old slug, discovered where a chat can begin.
    let mut old_slugs: Vec<String> = Vec::new();
    for table in ["session_history", "session_detail", "session_turn"] {
        let sql = format!(
            "SELECT DISTINCT channel FROM {table} WHERE channel LIKE 'http\\_\\_%\\_\\_project\\_\\_%' ESCAPE '\\'"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for slug in rows.flatten() {
            if !old_slugs.contains(&slug) {
                old_slugs.push(slug);
            }
        }
    }

    let mut slugs: BTreeMap<String, String> = BTreeMap::new();
    for old in &old_slugs {
        if let Some((user, id)) = parse_old_slug(old) {
            slugs.insert(old.clone(), new_slug(&user, &id));
        }
    }

    let tx = conn.unchecked_transaction()?;
    let mut rewritten = 0usize;

    for (old, new) in &slugs {
        // Exact channel columns.
        for (table, column) in [
            ("session_history", "channel"),
            ("session_detail", "channel"),
            ("session_turn", "channel"),
            ("scratch_file", "channel"),
            ("client_tool_call", "channel"),
            ("artifact", "origin_channel"),
            ("artifact_event", "source_channel"),
            ("artifact_reference", "channel"),
            ("usage_event", "channel"),
            ("background_job", "origin_channel"),
            ("background_piece", "session_channel"),
        ] {
            rewritten += rewrite_exact(&tx, table, column, old, new)?;
        }

        // Composite keys with the slug embedded.
        rewritten += rewrite_embedded(&tx, "session_detail", "id", old, new)?;
        rewritten += rewrite_embedded(&tx, "session_file", "session_slug", old, new)?;
        rewritten += rewrite_embedded(&tx, "artifact_reference", "session_slug", old, new)?;

        // Usage's place and activity, which changed shape with the variant.
        let old_place_prefix = format!("http:{old}:");
        let new_place_prefix = format!("project:{new}:");
        rewritten += tx.execute(
            "UPDATE usage_event SET place = ?1 || substr(place, ?2) \
             WHERE place LIKE ?3 ESCAPE '\\'",
            params![
                new_place_prefix,
                old_place_prefix.len() as i64 + 1,
                format!("{}%", old_place_prefix.replace('\\', "\\\\").replace('_', "\\_"))
            ],
        )?;
        rewritten += tx.execute(
            "UPDATE usage_event SET activity = 'project' WHERE channel = ?1",
            params![new],
        )?;
    }

    // Serialised channels in JSON columns. `session_history.data` and `session_turn.session`
    // hold the whole `VizierSession`, `session_detail.data` the detail's channel,
    // `session_turn.request` the request's session, and the dream journal its source sessions
    // (slugs and serialised sessions).
    if !slugs.is_empty() {
        for (table, key, column) in [
            ("session_history", "uid", "data"),
            ("session_detail", "id", "data"),
            ("session_turn", "id", "session"),
            ("session_turn", "id", "request"),
            ("dream_journal", "id", "data"),
        ] {
            rewritten += rewrite_json_column(&tx, table, key, column, &slugs)?;
        }
    }

    tx.execute(
        "INSERT OR REPLACE INTO state (key, value) VALUES (?1, '1')",
        params![MARKER],
    )?;
    tx.commit()?;

    if rewritten > 0 || !slugs.is_empty() {
        tracing::info!(
            channels = slugs.len(),
            rows = rewritten,
            "migrated project channels to the Project variant"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    /// A 040-shaped database with one project row in every table the migration touches, plus
    /// an ordinary WebUI row that must be left alone.
    fn legacy_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE session_history (uid TEXT PRIMARY KEY, agent_id TEXT, channel TEXT, topic TEXT, timestamp INTEGER, content_type TEXT, data TEXT, seq INTEGER);
            CREATE TABLE session_detail (id TEXT PRIMARY KEY, agent_id TEXT, channel TEXT, topic TEXT, data TEXT);
            CREATE TABLE session_turn (id INTEGER PRIMARY KEY, agent_id TEXT, channel TEXT, topic TEXT, session TEXT, request TEXT, state TEXT, accepted_at TEXT);
            CREATE TABLE session_file (id TEXT PRIMARY KEY, agent_id TEXT, session_slug TEXT);
            CREATE TABLE scratch_file (agent_id TEXT, channel TEXT, topic TEXT, name TEXT, content TEXT, size INTEGER, updated_at TEXT);
            CREATE TABLE client_tool_call (history_uid TEXT PRIMARY KEY, agent_id TEXT, channel TEXT, topic TEXT, status TEXT, created_at TEXT);
            CREATE TABLE artifact (id TEXT PRIMARY KEY, agent_id TEXT, origin_channel TEXT, origin_topic TEXT);
            CREATE TABLE artifact_event (id TEXT PRIMARY KEY, agent_id TEXT, source_channel TEXT);
            CREATE TABLE artifact_reference (artifact_id TEXT, agent_id TEXT, kind TEXT, target TEXT, session_slug TEXT, channel TEXT, PRIMARY KEY (artifact_id, kind, target));
            CREATE TABLE usage_event (id INTEGER PRIMARY KEY, agent_id TEXT, activity TEXT, channel TEXT, topic TEXT, place TEXT);
            CREATE TABLE background_job (id TEXT PRIMARY KEY, origin_agent TEXT, origin_channel TEXT, origin_topic TEXT);
            CREATE TABLE background_piece (job_id TEXT, session_channel TEXT, session_topic TEXT);
            CREATE TABLE dream_journal (id TEXT PRIMARY KEY, agent_id TEXT, data TEXT);
            ",
        )
        .unwrap();

        let old = "http__alice__project__p3x";
        let detail = format!("dp-a#{old}#t1");
        let session = serde_json::json!({
            "0": "dp-a", "1": { "HTTP": ["alice", "project__p3x"] }, "2": "t1"
        });
        let request = serde_json::json!({
            "timestamp": "2026-01-01T00:00:00Z",
            "user": "alice",
            "content": { "chat": "hi" },
            "session": session,
        });
        conn.execute(
            "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) \
             VALUES ('h1', 'dp-a', ?1, 't1', 1, 'request', ?2, 1)",
            params![old, serde_json::json!({ "vizier_session": ["dp-a", { "HTTP": ["alice", "project__p3x"] }, "t1"] }).to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_detail (id, agent_id, channel, topic, data) VALUES (?1, 'dp-a', ?2, 't1', ?3)",
            params![detail, old, serde_json::json!({ "agent_id": "dp-a", "channel": { "HTTP": ["alice", "project__p3x"] }, "topic": "t1" }).to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_turn (agent_id, channel, topic, session, request, state, accepted_at) \
             VALUES ('dp-a', ?1, 't1', ?2, ?3, 'queued', '2026-01-01')",
            params![old, session.to_string(), request.to_string()],
        )
        .unwrap();
        conn.execute("INSERT INTO session_file (id, agent_id, session_slug) VALUES ('f1','dp-a',?1)", params![format!("dp-a__{old}__t1")]).unwrap();
        conn.execute("INSERT INTO scratch_file VALUES ('dp-a', ?1, 't1', 'n', 'x', 1, '2026')", params![old]).unwrap();
        conn.execute("INSERT INTO client_tool_call (history_uid, agent_id, channel, topic, status, created_at) VALUES ('c1','dp-a',?1,'t1','done','2026')", params![old]).unwrap();
        conn.execute("INSERT INTO artifact (id, agent_id, origin_channel, origin_topic) VALUES ('a1','dp-a',?1,'t1')", params![old]).unwrap();
        conn.execute("INSERT INTO artifact_event (id, agent_id, source_channel) VALUES ('e1','dp-a',?1)", params![old]).unwrap();
        conn.execute("INSERT INTO artifact_reference (artifact_id, agent_id, kind, target, session_slug, channel) VALUES ('a1','dp-a','session','x',?1,?2)", params![format!("dp-a__{old}__t1"), old]).unwrap();
        conn.execute(
            "INSERT INTO usage_event (agent_id, activity, channel, topic, place) VALUES ('dp-a','chat',?1,'t1',?2)",
            params![old, format!("http:{old}:t1")],
        )
        .unwrap();
        conn.execute("INSERT INTO background_job (id, origin_agent, origin_channel, origin_topic) VALUES ('j1','dp-a',?1,'t1')", params![old]).unwrap();
        conn.execute("INSERT INTO background_piece (job_id, session_channel, session_topic) VALUES ('j1',?1,'t1')", params![old]).unwrap();
        conn.execute(
            "INSERT INTO dream_journal (id, agent_id, data) VALUES ('d1','dp-a',?1)",
            params![serde_json::json!({
                "source_sessions": [format!("dp-a__{old}__t1"), "dp-a__http__alice__vizier-webui__t0"],
            }).to_string()],
        )
        .unwrap();

        // An ordinary WebUI row in each channel column: untouched.
        conn.execute("INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) VALUES ('h2','dp-a','http__alice__vizier-webui','t0',1,'request','{}',2)", []).unwrap();
        conn.execute("INSERT INTO session_detail (id, agent_id, channel, topic, data) VALUES ('dp-a#http__alice__vizier-webui#t0','dp-a','http__alice__vizier-webui','t0','{}')", []).unwrap();
        conn
    }

    #[test]
    fn every_040_row_is_rewritten_and_ordinary_rows_are_untouched() {
        let conn = legacy_db();
        migrate_project_channels(&conn).unwrap();

        let old = "http__alice__project__p3x";
        let new = "project__alice__p3x";
        let count = |table: &str, column: &str| -> i64 {
            conn.query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
                params![old],
                |row| row.get(0),
            )
            .unwrap()
        };
        for (table, column) in [
            ("session_history", "channel"),
            ("session_detail", "channel"),
            ("session_turn", "channel"),
            ("scratch_file", "channel"),
            ("client_tool_call", "channel"),
            ("artifact", "origin_channel"),
            ("artifact_event", "source_channel"),
            ("artifact_reference", "channel"),
            ("usage_event", "channel"),
            ("background_job", "origin_channel"),
            ("background_piece", "session_channel"),
        ] {
            assert_eq!(count(table, column), 0, "{table}.{column} still has the old slug");
        }
        let one: String = conn
            .query_row("SELECT channel FROM session_history WHERE uid = 'h1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(one, new);
        let id: String = conn
            .query_row(
                "SELECT id FROM session_detail WHERE channel = ?1",
                params![new],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(id, format!("dp-a#{new}#t1"));
        let (activity, place): (String, String) = conn
            .query_row("SELECT activity, place FROM usage_event WHERE agent_id = 'dp-a'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(activity, "project");
        assert_eq!(place, format!("project:{new}:t1"));

        // Serialised channels move to the variant, in every JSON column.
        let history: String = conn
            .query_row("SELECT data FROM session_history WHERE uid = 'h1'", [], |r| r.get(0))
            .unwrap();
        assert!(history.contains("\"Project\":[\"alice\",\"p3x\"]"), "{history}");
        assert!(!history.contains("\"HTTP\""), "{history}");
        let session: String = conn
            .query_row("SELECT session FROM session_turn", [], |r| r.get(0))
            .unwrap();
        assert!(session.contains("\"Project\""), "{session}");
        let request: String = conn
            .query_row("SELECT request FROM session_turn", [], |r| r.get(0))
            .unwrap();
        assert!(request.contains("\"Project\""), "{request}");
        let data: String = conn
            .query_row("SELECT data FROM session_detail", [], |r| r.get(0))
            .unwrap();
        assert!(data.contains("\"Project\""), "{data}");
        let dream: String = conn
            .query_row("SELECT data FROM dream_journal", [], |r| r.get(0))
            .unwrap();
        assert!(dream.contains(new), "{dream}");
        assert!(!dream.contains(old), "{dream}");
        assert!(dream.contains("http__alice__vizier-webui"), "{dream}");

        // The ordinary WebUI row is untouched.
        let ordinary: String = conn
            .query_row("SELECT channel FROM session_history WHERE uid = 'h2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ordinary, "http__alice__vizier-webui");

        // A second run is a no-op.
        migrate_project_channels(&conn).unwrap();
    }
}
