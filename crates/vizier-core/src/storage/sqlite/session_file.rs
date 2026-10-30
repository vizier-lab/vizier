use anyhow::Result;
use chrono::Utc;

use crate::{
    schema::{VizierSession, session_file::SessionFileRecord},
    storage::{session_file::SessionFileStorage, sqlite::SqliteStorage},
};

#[async_trait::async_trait]
impl SessionFileStorage for SqliteStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord> {
        let session_slug = session.to_slug();
        let id = format!("{}/{}", session_slug, filename);

        let record = SessionFileRecord {
            id: id.clone(),
            session_slug: session_slug.clone(),
            agent_id: session.0.clone(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            file_id: file_id.to_string(),
            added_at: Utc::now(),
            sender: Some(session.0.clone()),
            history_uid: None,
        };

        let data = serde_json::to_string(&record)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO session_file (id, session_slug, agent_id, data) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, session_slug, session.0, data],
        )?;

        drop(conn);
        let _ = self.session_files.send((session.clone(), record.clone()));
        Ok(record)
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>> {
        let session_slug = session.to_slug();
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT data FROM session_file WHERE session_slug = ?1 AND agent_id = ?2")?;
        let records = stmt
            .query_map(rusqlite::params![session_slug, session.0], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionFileRecord>(&data).ok())
            .collect();
        Ok(records)
    }

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>> {
        let id = format!("{}/{}", session.to_slug(), filename);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM session_file WHERE id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()> {
        let id = format!("{}/{}", session.to_slug(), filename);
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM session_file WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    async fn delete_session_files(&self, session: &VizierSession) -> Result<usize> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM session_file WHERE session_slug = ?1 AND agent_id = ?2",
            rusqlite::params![session.to_slug(), session.0],
        )?;
        Ok(deleted)
    }

    async fn list_agent_files(&self, agent_id: &str) -> Result<Vec<SessionFileRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM session_file WHERE agent_id = ?1")?;
        let records = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionFileRecord>(&data).ok())
            .collect();
        Ok(records)
    }

    async fn session_file_reference_count(&self, file_id: &str) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM session_file WHERE json_extract(data, '$.file_id') = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    async fn label_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        sender: Option<&str>,
        history_uid: Option<&str>,
    ) -> Result<()> {
        let Some(mut record) = self.get_session_file(session, filename).await? else {
            return Ok(());
        };
        if let Some(sender) = sender {
            record.sender = Some(sender.to_string());
        }
        if let Some(uid) = history_uid {
            record.history_uid = Some(uid.to_string());
        }
        let data = serde_json::to_string(&record)?;
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE session_file SET data = ?2 WHERE id = ?1",
                rusqlite::params![record.id, data],
            )?;
        }
        let _ = self.session_files.send((session.clone(), record));
        Ok(())
    }

    fn subscribe_session_files(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<(VizierSession, SessionFileRecord)>> {
        Some(self.session_files.subscribe())
    }
}
