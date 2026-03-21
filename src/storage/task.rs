use anyhow::Result;

use crate::{
    schema::{AgentId, Task},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait TaskStorage {
    async fn save_task(&self, task: Task) -> Result<()>;

    async fn delete_task(&self, id: String) -> Result<()>;

    async fn get_task_list(
        &self,
        agent_id: Option<AgentId>,
        is_active: Option<bool>,
    ) -> Result<Vec<Task>>;
}

#[async_trait::async_trait]
impl TaskStorage for VizierStorage {
    async fn save_task(&self, task: Task) -> Result<()> {
        self.0.save_task(task).await
    }

    async fn delete_task(&self, id: String) -> Result<()> {
        self.0.delete_task(id).await
    }

    async fn get_task_list(
        &self,
        agent_id: Option<AgentId>,
        is_active: Option<bool>,
    ) -> Result<Vec<Task>> {
        self.0.get_task_list(agent_id, is_active).await
    }
}
