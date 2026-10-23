use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    schema::{
        SessionHistory, SessionHistoryContent, VizierResponseContent, VizierSession,
    },
    storage::{
        fs::{FileSystemStorage, HISTORY_PATH},
        history::{HistoryStorage, is_non_user_channel},
    },
};

fn topic_dir(workspace: &str, session: &VizierSession) -> PathBuf {
    PathBuf::from(format!(
        "{}/agents/{}/{}/{}/{}",
        workspace,
        session.0,
        HISTORY_PATH,
        session.1.to_slug(),
        session.2.clone().unwrap_or("DEFAULT".to_string()),
    ))
}

fn entry_path(workspace: &str, session: &VizierSession, uid: &str) -> PathBuf {
    topic_dir(workspace, session).join(format!("{}.json", uid))
}

fn read_entry(path: &PathBuf) -> Option<SessionHistory> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[async_trait::async_trait]
impl HistoryStorage for FileSystemStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<String> {
        let uid = Uuid::new_v4().to_string();
        let entry = SessionHistory {
            uid: uid.clone(),
            vizier_session: session.clone(),
            content,
            timestamp: Utc::now(),
            reactions: vec![],
            seq: None,
        };

        let path = entry_path(&self.workspace, &session, &uid);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(&entry)?;
        tokio::fs::write(&path, json).await?;

        Ok(uid)
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        // This backend survives only as a read source for the one-time migration into
        // sqlite, and it never assigned a `seq`, so there is no cursor position to honour.
        _before_seq: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut res: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .collect();

        res.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        if let Some(before_dt) = before {
            res.retain(|item| item.timestamp < before_dt);
        }

        if let Some(limit_val) = limit {
            res.truncate(limit_val);
        }

        res.sort_by_key(|a| a.timestamp);

        Ok(res)
    }

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<SessionHistory>> {
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut res: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .filter(|entry| {
                if let Some(start) = start_datetime
                    && entry.timestamp < start
                {
                    return false;
                }
                if let Some(end) = end_datetime
                    && entry.timestamp > end
                {
                    return false;
                }
                true
            })
            .collect();

        res.sort_by_key(|a| a.timestamp);

        Ok(res)
    }

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>> {
        let path_pattern = format!(
            "{}/agents/{}/{}/*/*/*.json",
            self.workspace, agent_id, HISTORY_PATH
        );

        let mut seen = HashSet::new();
        let mut sessions = vec![];

        for entry in glob::glob(&path_pattern)?
            .filter_map(|e| e.ok())
            .filter(|e| e.is_file())
        {
            let Some(entry) = read_entry(&entry) else {
                continue;
            };

            if entry.timestamp < start || entry.timestamp > end {
                continue;
            }

            let channel_slug = entry.vizier_session.1.to_slug();

            if is_non_user_channel(&channel_slug) {
                continue;
            }

            let slug = entry.vizier_session.to_slug();
            if seen.insert(slug) {
                sessions.push(entry.vizier_session);
            }
        }

        Ok(sessions)
    }

    async fn list_session_history_until_checkpoint(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<SessionHistory>, Option<String>)> {
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut all_entries: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .collect();

        all_entries.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        // Filter by timestamp if before is specified
        if let Some(before_dt) = before {
            all_entries.retain(|item| item.timestamp < before_dt);
        }

        // Find the latest checkpoint
        let checkpoint = all_entries
            .iter()
            .find(|e| matches!(e.content, SessionHistoryContent::Checkpoint(_)));

        let (checkpoint_timestamp, handover) = if let Some(cp) = checkpoint {
            let handover = match &cp.content {
                SessionHistoryContent::Checkpoint(h) => h.clone(),
                _ => None,
            };
            (Some(cp.timestamp), handover)
        } else {
            (None, None)
        };

        // Get all history after the checkpoint
        let mut history: Vec<SessionHistory> = all_entries
            .into_iter()
            .filter(|e| {
                if let Some(cp_ts) = checkpoint_timestamp {
                    e.timestamp > cp_ts
                } else {
                    true
                }
            })
            .collect();

        history.sort_by_key(|a| a.timestamp);

        Ok((history, handover))
    }

    async fn save_checkpoint(
        &self,
        session: VizierSession,
        handover: Option<String>,
    ) -> Result<SessionHistory> {
        let uid = Uuid::new_v4().to_string();
        let entry = SessionHistory {
            uid: uid.clone(),
            vizier_session: session.clone(),
            content: SessionHistoryContent::Checkpoint(handover),
            timestamp: Utc::now(),
            reactions: vec![],
            seq: None,
        };

        let path = entry_path(&self.workspace, &session, &uid);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(&entry)?;
        tokio::fs::write(&path, json).await?;

        Ok(entry)
    }
}


