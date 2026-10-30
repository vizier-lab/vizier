use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

use crate::{
    schema::{VizierSession, session_file::SessionFileRecord},
    storage::{fs::FileSystemStorage, session_file::SessionFileStorage},
};

const SESSION_FILE_PATH: &str = "session_files";

impl FileSystemStorage {
    fn session_file_dir(&self, session: &VizierSession) -> PathBuf {
        PathBuf::from(format!(
            "{}/agents/{}/{}/{}/{}",
            self.workspace,
            session.0,
            SESSION_FILE_PATH,
            session.1.to_slug(),
            session.2.clone().unwrap_or("DEFAULT".to_string()),
        ))
    }
}

#[async_trait::async_trait]
impl SessionFileStorage for FileSystemStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord> {
        let dir = self.session_file_dir(session);
        tokio::fs::create_dir_all(&dir).await?;

        let record = SessionFileRecord {
            id: format!("{}/{}", session.to_slug(), filename),
            session_slug: session.to_slug(),
            agent_id: session.0.clone(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            file_id: file_id.to_string(),
            added_at: Utc::now(),
            sender: None,
            history_uid: None,
        };

        let json = serde_json::to_string_pretty(&record)?;
        let file_path = dir.join(format!("{}.json", filename.replace('/', "_")));
        tokio::fs::write(&file_path, json).await?;

        Ok(record)
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>> {
        let dir = self.session_file_dir(session);
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut records = vec![];
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let content = tokio::fs::read(entry.path()).await?;
                if let Ok(record) = serde_json::from_slice::<SessionFileRecord>(&content) {
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>> {
        let dir = self.session_file_dir(session);
        let file_path = dir.join(format!("{}.json", filename.replace('/', "_")));

        if !file_path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read(&file_path).await?;
        let record = serde_json::from_slice::<SessionFileRecord>(&content)?;
        Ok(Some(record))
    }

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()> {
        let dir = self.session_file_dir(session);
        let file_path = dir.join(format!("{}.json", filename.replace('/', "_")));

        if file_path.exists() {
            tokio::fs::remove_file(&file_path).await?;
        }

        Ok(())
    }

    async fn delete_session_files(&self, session: &VizierSession) -> Result<usize> {
        let dir = self.session_file_dir(session);
        if !dir.exists() {
            return Ok(0);
        }
        let mut removed = 0;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                tokio::fs::remove_file(entry.path()).await?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    async fn list_agent_files(&self, agent_id: &str) -> Result<Vec<SessionFileRecord>> {
        let root = PathBuf::from(&self.workspace)
            .join("agents")
            .join(agent_id)
            .join(SESSION_FILE_PATH);
        if !root.exists() {
            return Ok(vec![]);
        }
        let mut records = vec![];
        let mut channels = tokio::fs::read_dir(&root).await?;
        while let Some(channel) = channels.next_entry().await? {
            if !channel.path().is_dir() {
                continue;
            }
            let mut topics = tokio::fs::read_dir(channel.path()).await?;
            while let Some(topic) = topics.next_entry().await? {
                if !topic.path().is_dir() {
                    continue;
                }
                let mut files = tokio::fs::read_dir(topic.path()).await?;
                while let Some(file) = files.next_entry().await? {
                    if file.path().extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Ok(bytes) = tokio::fs::read(file.path()).await {
                            if let Ok(record) = serde_json::from_slice::<SessionFileRecord>(&bytes)
                            {
                                records.push(record);
                            }
                        }
                    }
                }
            }
        }
        Ok(records)
    }

    async fn session_file_reference_count(&self, file_id: &str) -> Result<usize> {
        fn walk(dir: &std::path::Path, file_id: &str, count: &mut usize) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, file_id, count);
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(bytes) = std::fs::read(&path) {
                        if let Ok(record) = serde_json::from_slice::<SessionFileRecord>(&bytes) {
                            if record.file_id == file_id {
                                *count += 1;
                            }
                        }
                    }
                }
            }
        }
        let agents = PathBuf::from(&self.workspace).join("agents");
        let mut count = 0;
        if agents.exists() {
            walk(&agents, file_id, &mut count);
        }
        Ok(count)
    }
}
