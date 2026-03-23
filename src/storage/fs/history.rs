use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    schema::{
        SessionHistory, SessionHistoryContent, VizierRequest, VizierResponseStats, VizierSession,
    },
    storage::{
        fs::{FileSystemStorage, HISTORY_PATH},
        history::HistoryStorage,
    },
    utils,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_camel_case_types)]
enum ContentMetadata {
    request {
        user: String,
        is_silent_read: bool,
        is_task: bool,
        metadata: serde_json::Value,
    },
    response {
        stats: Option<VizierResponseStats>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionHistoryFrontMatter {
    pub uid: String,
    pub session: VizierSession,
    pub content_metadata: ContentMetadata,
    pub timestamp: chrono::DateTime<Utc>,
}

impl From<SessionHistory> for SessionHistoryFrontMatter {
    fn from(value: SessionHistory) -> Self {
        Self {
            uid: value.uid,
            timestamp: value.timestamp,
            session: value.session,
            content_metadata: match value.content {
                SessionHistoryContent::Request(req) => ContentMetadata::request {
                    user: req.user,
                    is_silent_read: req.is_silent_read,
                    is_task: req.is_task,
                    metadata: req.metadata,
                },
                SessionHistoryContent::Response(_, stats) => {
                    ContentMetadata::response { stats: stats }
                }
            },
        }
    }
}

#[async_trait::async_trait]
impl HistoryStorage for FileSystemStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let slug = format!("{}__{}", session.1.to_slug(), now);

        let history = SessionHistory {
            uid: slug.clone(),
            session: session.clone(),
            content: content.clone(),
            timestamp: Utc::now(),
        };

        let content = match content {
            SessionHistoryContent::Request(req) => req.content.clone(),
            SessionHistoryContent::Response(content, _) => content.clone(),
        };

        let frontmatter = SessionHistoryFrontMatter::from(history);
        let path = PathBuf::from(format!(
            "{}/agents/{}/{}/{}.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            slug
        ));

        utils::markdown::write_markdown(&frontmatter, content, path)?;
        Ok(())
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<chrono::DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let mut res = vec![];

        let path = format!(
            "{}/agents/{}/{}/{}__*.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            session.1.to_slug()
        );

        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            let (frontmatter, content) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry)?;

            res.push(SessionHistory {
                uid: frontmatter.uid,
                session: frontmatter.session,
                timestamp: frontmatter.timestamp,
                content: match frontmatter.content_metadata {
                    ContentMetadata::request {
                        user,
                        is_silent_read,
                        is_task,
                        metadata,
                    } => SessionHistoryContent::Request(VizierRequest {
                        user,
                        is_silent_read,
                        is_task,
                        metadata,
                        content,
                    }),
                    ContentMetadata::response { stats } => {
                        SessionHistoryContent::Response(content, stats)
                    }
                },
            });
        }

        // Sort by timestamp descending (most recent first) for proper cursor pagination
        res.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply cursor filter: get items before the given datetime
        if let Some(before_dt) = before {
            res.retain(|item| item.timestamp < before_dt);
        }

        // Apply limit
        if let Some(limit_val) = limit {
            res.truncate(limit_val);
        }

        // Sort back to ascending order (oldest first) for the final result
        res.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(res)
    }
}
