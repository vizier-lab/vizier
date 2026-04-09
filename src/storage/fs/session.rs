use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSession, VizierSessionDetail},
    storage::{
        fs::{FileSystemStorage, SESSION_PATH},
        session::SessionStorage,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionDetailFrontmatter {
    session: VizierSession,
}

#[async_trait::async_trait]
impl SessionStorage for FileSystemStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let path = PathBuf::from(format!(
            "{}/agents/{}/{}/{}/{}.md",
            self.workspace,
            session.agent_id.clone(),
            SESSION_PATH,
            session.channel.clone().to_slug(),
            session.topic.clone().unwrap_or("DEFAULT".into())
        ));

        crate::utils::markdown::write_markdown(
            &SessionDetailFrontmatter {
                session: VizierSession(
                    session.agent_id.clone(),
                    session.channel.clone(),
                    session.topic.clone(),
                ),
            },
            session.title.clone(),
            path,
        )?;

        Ok(())
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>> {
        let path = PathBuf::from(format!(
            "{}/agents/{}/{}/{}/{}.md",
            self.workspace,
            agent_id.clone(),
            SESSION_PATH,
            channel.clone().to_slug(),
            topic.clone().unwrap_or("DEFAULT".into())
        ));

        let (_, content) = crate::utils::markdown::read_markdown::<SessionDetailFrontmatter>(path)?;

        Ok(Some(VizierSessionDetail {
            agent_id,
            channel,
            topic,
            title: content,
        }))
    }

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
    ) -> Result<Vec<VizierSessionDetail>> {
        let path = format!(
            "{}/agents/{}/{}/{}/*.md",
            self.workspace,
            agent_id.clone(),
            SESSION_PATH,
            channel.clone().to_slug(),
        );

        let mut res = vec![];
        for entry in glob::glob(&path)? {
            let entry = entry?;

            let (frontmatter, title) =
                crate::utils::markdown::read_markdown::<SessionDetailFrontmatter>(entry)?;

            res.push(VizierSessionDetail {
                agent_id: frontmatter.session.0,
                channel: frontmatter.session.1,
                topic: frontmatter.session.2,
                title,
            });
        }

        Ok(res)
    }

    async fn delete_session(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()> {
        let path = PathBuf::from(format!(
            "{}/agents/{}/{}/{}/{}.md",
            self.workspace,
            agent_id.clone(),
            SESSION_PATH,
            channel.clone().to_slug(),
            topic.clone()
        ));

        std::fs::remove_file(path)?;

        Ok(())
    }
}
