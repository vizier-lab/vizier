use std::path::PathBuf;

use anyhow::Result;

use crate::{
    schema::{AgentId, Task, TaskFrontMatter},
    storage::{
        fs::{FileSystemStorage, TASK_PATH},
        task::TaskStorage,
    },
    utils,
};

#[async_trait::async_trait]
impl TaskStorage for FileSystemStorage {
    async fn save_task(&self, task: Task) -> Result<()> {
        let mut path = PathBuf::from(format!(
            "{}/agents/{}/{}",
            self.workspace,
            task.agent_id.clone(),
            TASK_PATH
        ));

        if !path.exists() {
            tokio::fs::create_dir_all(&path).await?;
        }

        path.push(format!("{}.md", task.slug.clone()));

        let frontmatter = TaskFrontMatter::from(task.clone());

        Ok(utils::markdown::write_markdown(
            &frontmatter,
            task.instruction.clone(),
            path,
        )?)
    }

    async fn delete_task(&self, agent_id: AgentId, slug: String) -> Result<()> {
        let mut path = PathBuf::from(format!(
            "{}/agents/{}/{}",
            self.workspace,
            agent_id.clone(),
            TASK_PATH
        ));

        path.push(format!("{}.md", slug));

        Ok(tokio::fs::remove_file(path).await?)
    }

    async fn get_task_list(
        &self,
        agent_id: Option<AgentId>,
        is_active: Option<bool>,
    ) -> Result<Vec<Task>> {
        let mut res = vec![];
        let path = format!(
            "{}/agents/{}/{}/*.md",
            self.workspace,
            agent_id.clone().unwrap_or("**".into()),
            TASK_PATH
        );

        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            let (frontmatter, content) = utils::markdown::read_markdown::<TaskFrontMatter>(entry)?;

            if let Some(is_active) = is_active {
                if frontmatter.is_active != is_active {
                    continue;
                }
            }

            res.push(Task {
                slug: frontmatter.slug,
                user: frontmatter.user,
                agent_id: frontmatter.agent_id,
                title: frontmatter.title,
                is_active: frontmatter.is_active,
                schedule: frontmatter.schedule,
                last_executed_at: frontmatter.last_executed_at,
                timestamp: frontmatter.timestamp,
                instruction: content,
            });
        }

        Ok(res)
    }

    async fn get_task(&self, agent_id: AgentId, slug: String) -> Result<Option<Task>> {
        let path = format!(
            "{}/agents/{}/{}/{}.md",
            self.workspace, agent_id, TASK_PATH, slug
        );

        let entry = match glob::glob(&path) {
            Ok(mut entries) => entries.next(),
            Err(_) => return Ok(None),
        };

        let entry = match entry {
            Some(e) => e?,
            None => return Ok(None),
        };

        if !entry.is_file() {
            return Ok(None);
        }

        let (frontmatter, content) = utils::markdown::read_markdown::<TaskFrontMatter>(entry)?;

        Ok(Some(Task {
            slug: frontmatter.slug,
            user: frontmatter.user,
            agent_id: frontmatter.agent_id,
            title: frontmatter.title,
            is_active: frontmatter.is_active,
            schedule: frontmatter.schedule,
            last_executed_at: frontmatter.last_executed_at,
            timestamp: frontmatter.timestamp,
            instruction: content,
        }))
    }
}
