use anyhow::Result;

use crate::{
    schema::AgentConfig,
    storage::{agent::AgentStorage, fs::FileSystemStorage},
    utils::build_path,
};

const AGENTS_PATH: &str = "agents";

#[async_trait::async_trait]
impl AgentStorage for FileSystemStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        let dir = build_path(&self.workspace, &[AGENTS_PATH]);

        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut agents = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let raw = std::fs::read_to_string(&path)?;
                if let Ok(config) = serde_json::from_str::<AgentConfig>(&raw) {
                    let agent_id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string();
                    agents.push((agent_id, config));
                }
            }
        }

        Ok(agents)
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        let path = build_path(&self.workspace, &[AGENTS_PATH, &format!("{}.json", agent_id)]);

        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(&path)?;
        let config = serde_json::from_str::<AgentConfig>(&raw)?;
        Ok(Some(config))
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let dir = build_path(&self.workspace, &[AGENTS_PATH]);
        let _ = std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.json", agent_id));

        if path.exists() {
            return Err(anyhow::anyhow!("Agent '{}' already exists", agent_id));
        }

        std::fs::write(path, serde_json::to_string_pretty(config)?)?;
        Ok(())
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let dir = build_path(&self.workspace, &[AGENTS_PATH]);
        let path = dir.join(format!("{}.json", agent_id));

        if !path.exists() {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }

        std::fs::write(path, serde_json::to_string_pretty(config)?)?;
        Ok(())
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let dir = build_path(&self.workspace, &[AGENTS_PATH]);
        let path = dir.join(format!("{}.json", agent_id));

        if !path.exists() {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }

        std::fs::remove_file(path)?;
        Ok(())
    }
}
