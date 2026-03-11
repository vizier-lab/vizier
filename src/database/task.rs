use std::collections::BTreeMap;

use anyhow::Result;
use futures::task;
use surrealdb_types::Value;

use crate::{
    database::{
        VizierDatabases,
        query::{AllAnd, Cond, Op},
    },
    schema::{AgentId, Task},
};

impl VizierDatabases {
    pub async fn save_task(&self, task: Task) -> Result<()> {
        let _: Option<Task> = self
            .conn
            .upsert(("task", task.slug.clone()))
            .content(task)
            .await?;
        Ok(())
    }

    pub async fn delete_task(&self, id: String) -> Result<()> {
        let _: Option<Task> = self.conn.delete(("task", id)).await?;
        Ok(())
    }

    pub async fn get_task_list(
        &self,
        agent_id: Option<AgentId>,
        is_active: Option<bool>,
    ) -> Result<Vec<Task>> {
        let mut conds = vec![];
        let mut params: BTreeMap<String, Value> = BTreeMap::new();

        if let Some(agent_id) = agent_id {
            params.insert(
                "agent_id".into(),
                surrealdb_types::Value::String(agent_id.into()),
            );
            conds.push(Cond("agent_id".into(), Op::Eq));
        }

        if let Some(is_active) = is_active {
            params.insert("is_active".into(), Value::Bool(is_active.into()));
            conds.push(Cond("is_active".into(), Op::Eq));
        }

        let mut query = "SELECT * FROM task".to_string();

        if conds.len() > 0 {
            query.push_str(" WHERE ");
            query.push_str(&format!("{}", AllAnd(conds)));
        }

        let mut response = self.conn.query(query).bind(params).await?;

        let tasks: Vec<Task> = response.take(0)?;

        Ok(tasks)
    }
}
