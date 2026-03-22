use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    schema::{AgentId, Task, TaskSchedule},
    storage::{
        fs::{FileSystemStorage, TASK_PATH},
        task::TaskStorage,
    },
    utils,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TaskFrontMatter {
    pub slug: String,
    pub user: String,
    pub agent_id: String,
    pub title: String,
    pub is_active: bool,
    pub schedule: TaskSchedule,
    pub last_executed_at: Option<chrono::DateTime<Utc>>,
    pub timestamp: chrono::DateTime<Utc>,
}

impl From<Task> for TaskFrontMatter {
    fn from(value: Task) -> Self {
        Self {
            slug: value.slug,
            user: value.user,
            agent_id: value.agent_id,
            title: value.title,
            is_active: value.is_active,
            schedule: value.schedule,
            last_executed_at: value.last_executed_at,
            timestamp: value.timestamp,
        }
    }
}

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
            std::fs::create_dir_all(&path)?;
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

        Ok(std::fs::remove_file(path)?)
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
}
