//! SQLite implementation of the artifact metadata store (`specs/035-add-artifacts`).

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, Row, ToSql, Transaction, params, params_from_iter};

use crate::{
    schema::{
        Artifact, ArtifactCursor, ArtifactEvent, ArtifactEventAction, ArtifactFilters,
        ArtifactKind, ArtifactPage, ArtifactReference, ArtifactStats, Requester,
    },
    storage::{artifact::ArtifactStorage, sqlite::SqliteStorage},
};

fn to_ms(at: DateTime<Utc>) -> i64 {
    at.timestamp_millis()
}

fn from_ms(ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or_else(Utc::now)
}

fn actor_json(actor: &Requester) -> String {
    serde_json::to_string(actor).unwrap_or_else(|_| "\"unknown\"".to_string())
}

fn actor_from_json(raw: &str) -> Requester {
    serde_json::from_str(raw).unwrap_or_else(|_| Requester::User(raw.to_string()))
}

const ARTIFACT_COLUMNS: &str = "id, agent_id, name, kind, mime_type, size, version, content_file, \
     origin_channel, origin_topic, source_filename, created_by, created_at, updated_at";

fn map_artifact(row: &Row<'_>) -> rusqlite::Result<Artifact> {
    let kind: String = row.get(3)?;
    let created_by: String = row.get(11)?;
    Ok(Artifact {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        name: row.get(2)?,
        kind: ArtifactKind::parse(&kind).unwrap_or(ArtifactKind::Other),
        mime_type: row.get(4)?,
        size: row.get::<_, i64>(5)? as u64,
        version: row.get::<_, i64>(6)? as u32,
        content_file: row.get(7)?,
        origin_channel: row.get(8)?,
        origin_topic: row.get(9)?,
        source_filename: row.get(10)?,
        created_by: actor_from_json(&created_by),
        created_at: from_ms(row.get(12)?),
        updated_at: from_ms(row.get(13)?),
    })
}

fn insert_event(tx: &Transaction<'_>, event: &ArtifactEvent) -> Result<()> {
    tx.execute(
        "INSERT INTO artifact_event \
         (id, agent_id, artifact_id, artifact_name, action, actor, source_channel, source_topic, source_filename, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event.id,
            event.agent_id,
            event.artifact_id,
            event.artifact_name,
            event.action.as_str(),
            actor_json(&event.actor),
            event.source_channel,
            event.source_topic,
            event.source_filename,
            to_ms(event.created_at),
        ],
    )?;
    Ok(())
}

fn filter_conditions(filters: &ArtifactFilters, cursor: bool) -> (String, Vec<Box<dyn ToSql>>) {
    let mut sql = String::new();
    let mut p: Vec<Box<dyn ToSql>> = vec![Box::new(filters.agent_id.clone())];
    if let Some(q) = &filters.q {
        p.push(Box::new(q.clone()));
        sql.push_str(&format!(" AND name LIKE '%' || ?{} || '%'", p.len()));
    }
    if let Some(kind) = filters.kind {
        p.push(Box::new(kind.as_str().to_string()));
        sql.push_str(&format!(" AND kind = ?{}", p.len()));
    }
    if let Some(from) = filters.from {
        p.push(Box::new(to_ms(from)));
        sql.push_str(&format!(" AND updated_at >= ?{}", p.len()));
    }
    if let Some(to) = filters.to {
        p.push(Box::new(to_ms(to)));
        sql.push_str(&format!(" AND updated_at <= ?{}", p.len()));
    }
    if cursor {
        if let Some(c) = &filters.cursor {
            p.push(Box::new(c.at_ms));
            let at = p.len();
            p.push(Box::new(c.at_ms));
            let at2 = p.len();
            p.push(Box::new(c.id.clone()));
            let id = p.len();
            sql.push_str(&format!(
                " AND (updated_at < ?{at} OR (updated_at = ?{at2} AND id < ?{id}))"
            ));
        }
    }
    (sql, p)
}

#[async_trait::async_trait]
impl ArtifactStorage for SqliteStorage {
    async fn create_artifact(&self, artifact: &Artifact, event: &ArtifactEvent) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let inserted = tx.execute(
            &format!(
                "INSERT INTO artifact ({ARTIFACT_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                artifact.id,
                artifact.agent_id,
                artifact.name,
                artifact.kind.as_str(),
                artifact.mime_type,
                artifact.size as i64,
                artifact.version as i64,
                artifact.content_file,
                artifact.origin_channel,
                artifact.origin_topic,
                artifact.source_filename,
                actor_json(&artifact.created_by),
                to_ms(artifact.created_at),
                to_ms(artifact.updated_at),
            ],
        );
        match inserted {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                return Err(anyhow!(
                    "an artifact named '{}' already exists; replace it instead",
                    artifact.name
                ));
            }
            Err(e) => return Err(e.into()),
        }
        insert_event(&tx, event)?;
        tx.commit()?;
        Ok(())
    }

    async fn get_artifact(&self, agent_id: &str, id: &str) -> Result<Option<Artifact>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                &format!("SELECT {ARTIFACT_COLUMNS} FROM artifact WHERE agent_id = ?1 AND id = ?2"),
                params![agent_id, id],
                map_artifact,
            )
            .optional()?)
    }

    async fn get_artifact_by_name(&self, agent_id: &str, name: &str) -> Result<Option<Artifact>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                &format!(
                    "SELECT {ARTIFACT_COLUMNS} FROM artifact WHERE agent_id = ?1 AND name = ?2"
                ),
                params![agent_id, name],
                map_artifact,
            )
            .optional()?)
    }

    async fn list_artifacts(&self, filters: &ArtifactFilters) -> Result<ArtifactPage> {
        let (conditions, params) = filter_conditions(filters, false);
        let conn = self.conn.lock();

        let (total, total_bytes): (i64, i64) = conn.query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM artifact WHERE agent_id = ?1{conditions}"
            ),
            params_from_iter(params.iter()),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (agent_total, agent_total_bytes): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM artifact WHERE agent_id = ?1",
            params![filters.agent_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let limit = filters.limit.clamp(1, 100);
        let (cursor_sql, mut page_params) = filter_conditions(filters, true);
        page_params.push(Box::new((limit + 1) as i64));
        let limit_placeholder = page_params.len();
        let sql = format!(
            "SELECT {ARTIFACT_COLUMNS} FROM artifact WHERE agent_id = ?1{cursor_sql} \
             ORDER BY updated_at DESC, id DESC LIMIT ?{limit_placeholder}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut items: Vec<Artifact> = stmt
            .query_map(params_from_iter(page_params.iter()), map_artifact)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let next = if items.len() > limit {
            items.truncate(limit);
            items
                .last()
                .map(|a| ArtifactCursor::new(a.updated_at, &a.id).encode())
        } else {
            None
        };

        Ok(ArtifactPage {
            items,
            next,
            total: total as usize,
            total_bytes: total_bytes as u64,
            agent_total: agent_total as usize,
            agent_total_bytes: agent_total_bytes as u64,
        })
    }

    async fn artifact_stats(&self, agent_id: &str) -> Result<ArtifactStats> {
        let conn = self.conn.lock();
        let (total, total_bytes): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM artifact WHERE agent_id = ?1",
            params![agent_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(ArtifactStats {
            total: total as usize,
            total_bytes: total_bytes as u64,
        })
    }

    async fn replace_artifact(
        &self,
        agent_id: &str,
        id: &str,
        expected_version: u32,
        content_file: &str,
        size: u64,
        mime_type: &str,
        kind: ArtifactKind,
        source_filename: &str,
        updated_at: DateTime<Utc>,
        event: &ArtifactEvent,
    ) -> Result<Artifact> {
        {
            let conn = self.conn.lock();
            let tx = conn.unchecked_transaction()?;
            let updated = tx.execute(
                "UPDATE artifact SET content_file = ?3, size = ?4, mime_type = ?5, kind = ?6, \
                 source_filename = ?7, updated_at = ?8, version = version + 1 \
                 WHERE agent_id = ?1 AND id = ?2 AND version = ?9",
                params![
                    agent_id,
                    id,
                    content_file,
                    size as i64,
                    mime_type,
                    kind.as_str(),
                    source_filename,
                    to_ms(updated_at),
                    expected_version as i64,
                ],
            )?;
            if updated == 0 {
                return Err(anyhow!(
                    "artifact '{id}' was changed by another operation; reload and try again"
                ));
            }
            insert_event(&tx, event)?;
            tx.commit()?;
        }
        self.get_artifact(agent_id, id)
            .await?
            .ok_or_else(|| anyhow!("artifact '{id}' disappeared during replacement"))
    }

    async fn delete_artifact(&self, agent_id: &str, id: &str, event: &ArtifactEvent) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM artifact_reference WHERE agent_id = ?1 AND artifact_id = ?2",
            params![agent_id, id],
        )?;
        let deleted = tx.execute(
            "DELETE FROM artifact WHERE agent_id = ?1 AND id = ?2",
            params![agent_id, id],
        )?;
        if deleted == 0 {
            return Err(anyhow!("artifact '{id}' not found"));
        }
        insert_event(&tx, event)?;
        tx.commit()?;
        Ok(())
    }

    async fn list_artifact_events(
        &self,
        agent_id: &str,
        before: Option<ArtifactCursor>,
        limit: usize,
    ) -> Result<Vec<ArtifactEvent>> {
        let limit = limit.clamp(1, 100) as i64;
        let conn = self.conn.lock();
        let mut p: Vec<Box<dyn ToSql>> = vec![Box::new(agent_id.to_string())];
        let mut sql = String::from(
            "SELECT id, agent_id, artifact_id, artifact_name, action, actor, source_channel, source_topic, source_filename, created_at \
             FROM artifact_event WHERE agent_id = ?1",
        );
        if let Some(c) = before {
            p.push(Box::new(c.at_ms));
            let a = p.len();
            p.push(Box::new(c.at_ms));
            let b = p.len();
            p.push(Box::new(c.id));
            let d = p.len();
            sql.push_str(&format!(
                " AND (created_at < ?{a} OR (created_at = ?{b} AND id < ?{d}))"
            ));
        }
        p.push(Box::new(limit));
        let l = p.len();
        sql.push_str(&format!(" ORDER BY created_at DESC, id DESC LIMIT ?{l}"));

        let mut stmt = conn.prepare(&sql)?;
        let events = stmt
            .query_map(params_from_iter(p.iter()), |row| {
                let action: String = row.get(4)?;
                let actor: String = row.get(5)?;
                Ok(ArtifactEvent {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    artifact_id: row.get(2)?,
                    artifact_name: row.get(3)?,
                    action: ArtifactEventAction::parse(&action)
                        .unwrap_or(ArtifactEventAction::Promoted),
                    actor: actor_from_json(&actor),
                    source_channel: row.get(6)?,
                    source_topic: row.get(7)?,
                    source_filename: row.get(8)?,
                    created_at: from_ms(row.get(9)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(events)
    }

    async fn list_artifact_references(
        &self,
        agent_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactReference>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT artifact_id, agent_id, kind, target, session_slug, history_uid, channel, topic, bundle, path, label, created_at \
             FROM artifact_reference WHERE agent_id = ?1 AND artifact_id = ?2 ORDER BY created_at DESC, kind, target",
        )?;
        let refs = stmt
            .query_map(params![agent_id, artifact_id], map_reference)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(refs)
    }

    async fn count_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artifact_reference WHERE agent_id = ?1 AND artifact_id = ?2",
            params![agent_id, artifact_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    async fn upsert_artifact_reference(&self, reference: &ArtifactReference) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO artifact_reference \
             (artifact_id, agent_id, kind, target, session_slug, history_uid, channel, topic, bundle, path, label, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                reference.artifact_id,
                reference.agent_id,
                reference.kind,
                reference.target,
                reference.session_slug,
                reference.history_uid,
                reference.channel,
                reference.topic,
                reference.bundle,
                reference.path,
                reference.label,
                to_ms(reference.created_at),
            ],
        )?;
        Ok(())
    }

    async fn delete_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM artifact_reference WHERE agent_id = ?1 AND artifact_id = ?2",
            params![agent_id, artifact_id],
        )?;
        Ok(())
    }

    async fn delete_message_references_for_session(
        &self,
        agent_id: &str,
        session_slug: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM artifact_reference WHERE kind = 'message' AND agent_id = ?1 AND session_slug = ?2",
            params![agent_id, session_slug],
        )?;
        Ok(())
    }

    async fn delete_memory_references(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM artifact_reference WHERE kind = 'memory' AND agent_id = ?1 AND bundle = ?2 AND path = ?3",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    async fn delete_agent_artifact_data(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM artifact_reference WHERE agent_id = ?1",
            params![agent_id],
        )?;
        tx.execute(
            "DELETE FROM artifact_event WHERE agent_id = ?1",
            params![agent_id],
        )?;
        tx.execute(
            "DELETE FROM artifact WHERE agent_id = ?1",
            params![agent_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn artifact_content_file(&self, agent_id: &str, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT content_file FROM artifact WHERE agent_id = ?1 AND id = ?2",
                params![agent_id, id],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }
}

fn map_reference(row: &Row<'_>) -> rusqlite::Result<ArtifactReference> {
    Ok(ArtifactReference {
        artifact_id: row.get(0)?,
        agent_id: row.get(1)?,
        kind: row.get(2)?,
        target: row.get(3)?,
        session_slug: row.get(4)?,
        history_uid: row.get(5)?,
        channel: row.get(6)?,
        topic: row.get(7)?,
        bundle: row.get(8)?,
        path: row.get(9)?,
        label: row.get(10)?,
        created_at: from_ms(row.get(11)?),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::storage::document::LocalDocumentStore;

    fn storage() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_artifact_schema(&conn).unwrap();
        (
            SqliteStorage::new(
                Arc::new(Mutex::new(conn)),
                Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
            ),
            dir,
        )
    }

    fn artifact(name: &str, at: i64) -> Artifact {
        Artifact {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: "a".into(),
            name: name.into(),
            kind: ArtifactKind::Text,
            mime_type: "text/plain".into(),
            size: 10,
            version: 1,
            content_file: "content.v1".into(),
            origin_channel: None,
            origin_topic: None,
            source_filename: Some("src.txt".into()),
            created_by: Requester::Agent("a".into()),
            created_at: from_ms(at),
            updated_at: from_ms(at),
        }
    }

    fn event(a: &Artifact, action: ArtifactEventAction) -> ArtifactEvent {
        ArtifactEvent {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: a.agent_id.clone(),
            artifact_id: a.id.clone(),
            artifact_name: a.name.clone(),
            action,
            actor: Requester::Agent("a".into()),
            source_channel: None,
            source_topic: None,
            source_filename: None,
            created_at: a.updated_at,
        }
    }

    async fn create(s: &SqliteStorage, a: &Artifact) {
        s.create_artifact(a, &event(a, ArtifactEventAction::Promoted))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_get_duplicate_and_case_sensitive_names() {
        let (s, _d) = storage();
        let a = artifact("Report.pdf", 100);
        create(&s, &a).await;
        assert_eq!(
            s.get_artifact("a", &a.id).await.unwrap().unwrap().name,
            "Report.pdf"
        );
        assert_eq!(
            s.get_artifact_by_name("a", "Report.pdf")
                .await
                .unwrap()
                .unwrap()
                .id,
            a.id
        );
        assert!(
            s.get_artifact_by_name("a", "report.pdf")
                .await
                .unwrap()
                .is_none()
        );

        let dup = artifact("Report.pdf", 200);
        assert!(
            s.create_artifact(&dup, &event(&dup, ArtifactEventAction::Promoted))
                .await
                .is_err()
        );

        let other = artifact("report.pdf", 300);
        s.create_artifact(&other, &event(&other, ArtifactEventAction::Promoted))
            .await
            .unwrap();
        assert_eq!(s.artifact_stats("a").await.unwrap().total, 2);
    }

    #[tokio::test]
    async fn replacement_switches_version_and_records_an_event() {
        let (s, _d) = storage();
        let a = artifact("x.txt", 100);
        create(&s, &a).await;
        let ev = ArtifactEvent {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: "a".into(),
            artifact_id: a.id.clone(),
            artifact_name: a.name.clone(),
            action: ArtifactEventAction::Replaced,
            actor: Requester::User("ana".into()),
            source_channel: Some("http__ana__t".into()),
            source_topic: Some("t".into()),
            source_filename: Some("v2.txt".into()),
            created_at: from_ms(500),
        };
        let updated = s
            .replace_artifact(
                "a",
                &a.id,
                1,
                "content.v2",
                22,
                "text/plain",
                ArtifactKind::Text,
                "v2.txt",
                from_ms(500),
                &ev,
            )
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.size, 22);
        assert_eq!(updated.content_file, "content.v2");
        assert_eq!(
            s.artifact_content_file("a", &a.id).unwrap().unwrap(),
            "content.v2"
        );

        // A stale expected version is refused and changes nothing.
        let stale = ArtifactEvent {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: "a".into(),
            artifact_id: a.id.clone(),
            artifact_name: a.name.clone(),
            action: ArtifactEventAction::Replaced,
            actor: Requester::Agent("a".into()),
            source_channel: None,
            source_topic: None,
            source_filename: None,
            created_at: from_ms(600),
        };
        assert!(
            s.replace_artifact(
                "a",
                &a.id,
                1,
                "content.v3",
                1,
                "text/plain",
                ArtifactKind::Text,
                "s",
                from_ms(600),
                &stale
            )
            .await
            .is_err()
        );
        assert_eq!(
            s.get_artifact("a", &a.id).await.unwrap().unwrap().version,
            2
        );

        let events = s.list_artifact_events("a", None, 10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, ArtifactEventAction::Replaced);
    }

    #[tokio::test]
    async fn delete_removes_refs_but_keeps_the_event() {
        let (s, _d) = storage();
        let a = artifact("gone.txt", 100);
        create(&s, &a).await;
        let reference = ArtifactReference {
            artifact_id: a.id.clone(),
            agent_id: "a".into(),
            kind: "message".into(),
            target: ArtifactReference::message_target("s", "u1"),
            session_slug: Some("s".into()),
            history_uid: Some("u1".into()),
            channel: Some("http__ana__t".into()),
            topic: Some("t".into()),
            bundle: None,
            path: None,
            label: "WebUI conversation".into(),
            created_at: from_ms(120),
        };
        s.upsert_artifact_reference(&reference).await.unwrap();
        assert_eq!(s.count_artifact_references("a", &a.id).await.unwrap(), 1);

        let ev = ArtifactEvent {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: "a".into(),
            artifact_id: a.id.clone(),
            artifact_name: a.name.clone(),
            action: ArtifactEventAction::Deleted,
            actor: Requester::User("ana".into()),
            source_channel: None,
            source_topic: None,
            source_filename: None,
            created_at: from_ms(400),
        };
        s.delete_artifact("a", &a.id, &ev).await.unwrap();
        assert!(s.get_artifact("a", &a.id).await.unwrap().is_none());
        assert_eq!(s.count_artifact_references("a", &a.id).await.unwrap(), 0);
        let events = s.list_artifact_events("a", None, 10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, ArtifactEventAction::Deleted);
    }

    #[tokio::test]
    async fn paging_is_stable_with_tied_timestamps() {
        let (s, _d) = storage();
        let mut ids = vec![];
        for i in 0..5 {
            // Same timestamp on purpose: the id breaks the tie.
            let a = artifact(&format!("f{i}.txt"), 1000);
            ids.push(a.id.clone());
            create(&s, &a).await;
        }
        let first = s
            .list_artifacts(&ArtifactFilters {
                agent_id: "a".into(),
                limit: 2,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.total, 5);
        assert_eq!(first.agent_total, 5);
        let cursor = ArtifactCursor::decode(first.next.as_ref().unwrap()).unwrap();
        let second = s
            .list_artifacts(&ArtifactFilters {
                agent_id: "a".into(),
                limit: 2,
                cursor: Some(cursor),
                ..Default::default()
            })
            .await
            .unwrap();
        let seen: Vec<String> = first
            .items
            .iter()
            .chain(second.items.iter())
            .map(|a| a.id.clone())
            .collect();
        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(seen.len(), unique.len(), "no duplicate across pages");
    }

    #[tokio::test]
    async fn filters_and_stats_are_independent_of_the_cursor() {
        let (s, _d) = storage();
        let mut image = artifact("photo.png", 200);
        image.kind = ArtifactKind::Image;
        image.size = 5;
        create(&s, &image).await;
        create(&s, &artifact("report.pdf", 300)).await;
        create(&s, &artifact("notes.txt", 400)).await;

        let page = s
            .list_artifacts(&ArtifactFilters {
                agent_id: "a".into(),
                q: Some("report".into()),
                limit: 50,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.agent_total, 3);
        assert_eq!(page.agent_total_bytes, 25);

        let page = s
            .list_artifacts(&ArtifactFilters {
                agent_id: "a".into(),
                kind: Some(ArtifactKind::Image),
                limit: 50,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "photo.png");
    }

    #[tokio::test]
    async fn session_and_memory_reference_cleanup_are_scoped() {
        let (s, _d) = storage();
        let a = artifact("x", 100);
        create(&s, &a).await;
        for (kind, target, slug, bundle, path) in [
            ("message", "m1", Some("s1"), None, None),
            ("message", "m2", Some("s2"), None, None),
            ("memory", "mem1", None, Some("default"), Some("p")),
            ("memory", "mem2", None, Some("default"), Some("q")),
        ] {
            s.upsert_artifact_reference(&ArtifactReference {
                artifact_id: a.id.clone(),
                agent_id: "a".into(),
                kind: kind.into(),
                target: target.into(),
                session_slug: slug.map(str::to_string),
                history_uid: None,
                channel: None,
                topic: None,
                bundle: bundle.map(str::to_string),
                path: path.map(str::to_string),
                label: target.into(),
                created_at: from_ms(100),
            })
            .await
            .unwrap();
        }
        s.delete_message_references_for_session("a", "s1")
            .await
            .unwrap();
        s.delete_memory_references("a", "default", "p")
            .await
            .unwrap();
        assert_eq!(s.count_artifact_references("a", &a.id).await.unwrap(), 2);
        s.delete_agent_artifact_data("a").await.unwrap();
        assert_eq!(s.count_artifact_references("a", &a.id).await.unwrap(), 0);
        assert!(s.get_artifact("a", &a.id).await.unwrap().is_none());
    }
}
