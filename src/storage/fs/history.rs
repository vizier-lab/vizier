use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub uuid: uuid::Uuid,
    pub session: VizierSession,
    pub content_metadata: ContentMetadata,
    pub timestamp: chrono::DateTime<Utc>,
}

impl From<SessionHistory> for SessionHistoryFrontMatter {
    fn from(value: SessionHistory) -> Self {
        Self {
            uuid: value.uuid,
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
        let uuid = Uuid::new_v4();
        let history = SessionHistory {
            uuid: uuid.clone(),
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
            format!("{}__{}", session.1.to_slug(), uuid.clone())
        ));

        utils::markdown::write_markdown(&frontmatter, content, path)?;
        Ok(())
    }

    // TODO: cursor based pagination
    async fn list_session_history(&self, session: VizierSession) -> Result<Vec<SessionHistory>> {
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
                uuid: frontmatter.uuid,
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

        res.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(res)
    }
}
