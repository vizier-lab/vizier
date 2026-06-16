use anyhow::Result;

use crate::{
    schema::{AgentId, Task},
    storage::{sqlite::SqliteStorage, task::TaskStorage},
};

#[async_trait::async_trait]
impl TaskStorage for SqliteStorage {
    async fn save_task(&self, task: Task) -> Result<()> {
        let id = format!("{}/{}", task.agent_id, task.slug);
        let data = serde_json::to_string(&task)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO task (id, agent_id, slug, is_active, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, task.agent_id, task.slug, task.is_active as i32, data],
        )?;
        Ok(())
    }

    async fn delete_task(&self, agent_id: AgentId, slug: String) -> Result<()> {
        let id = format!("{}/{}", agent_id, slug);
        let conn = self.conn.lock();
        conn.execute("DELETE FROM task WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    async fn get_task_list(
        &self,
        agent_id: Option<AgentId>,
        is_active: Option<bool>,
    ) -> Result<Vec<Task>> {
        let conn = self.conn.lock();

        let mut sql = String::from("SELECT data FROM task");
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref aid) = agent_id {
            conditions.push(format!("agent_id = ?{}", idx));
            params.push(Box::new(aid.clone()));
            idx += 1;
        }
        if let Some(active) = is_active {
            conditions.push(format!("is_active = ?{}", idx));
            params.push(Box::new(active as i32));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        let mut stmt = conn.prepare(&sql)?;
        let tasks: Vec<Task> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data: String| serde_json::from_str::<Task>(&data).ok())
            .collect();

        Ok(tasks)
    }

    async fn get_task(&self, agent_id: AgentId, slug: String) -> Result<Option<Task>> {
        let id = format!("{}/{}", agent_id, slug);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM task WHERE id = ?1")?;
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
}
