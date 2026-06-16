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
        };

        let data = serde_json::to_string(&record)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO session_file (id, session_slug, agent_id, data) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, session_slug, session.0, data],
        )?;

        Ok(record)
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>> {
        let session_slug = session.to_slug();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM session_file WHERE session_slug = ?1 AND agent_id = ?2",
        )?;
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
        conn.execute("DELETE FROM session_file WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }
}
