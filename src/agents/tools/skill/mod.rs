use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{AgentId, Skill};
use crate::storage::VizierStorage;
use crate::storage::skill::SkillStorage;

pub struct CreateSkill(AgentId, Arc<VizierStorage>);

impl CreateSkill {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self(agent_id, deps.storage.clone())
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateSkillArgs {
    #[schemars(description = "name of the skill, in snake_case format")]
    pub name: String,

    #[schemars(description = "short description of the skill")]
    pub description: String,

    #[schemars(description = "content/instruction of the skill")]
    pub instruction: String,
}

#[async_trait::async_trait]
impl VizierTool for CreateSkill {
    type Input = CreateSkillArgs;
    type Output = ();

    fn name() -> String {
        "create_skill".to_string()
    }

    fn description(&self) -> String {
        "create a new skill you have learn, to be reusable".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let slug = slugify::slugify!(&args.name);

        self.1
            .save_skill(
                Some(self.0.clone()),
                Skill {
                    author: self.0.clone(),
                    agent_id: Some(self.0.clone()),
                    description: args.description,
                    name: slug,
                    content: args.instruction,
                },
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;
        Ok(())
    }
}
