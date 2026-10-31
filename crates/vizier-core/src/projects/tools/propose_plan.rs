//! `propose_plan`: a Plan-mode turn's final answer, drawn by the WebUI as a plan card.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ProposePlanInput {
    /// The plan's title, at most 120 characters.
    pub title: String,
    /// An optional short summary, at most 2,000 characters.
    #[serde(default)]
    pub summary: Option<String>,
    /// One to thirty ordered steps, each at most 500 characters.
    pub steps: Vec<String>,
}

pub struct ProposePlan;

#[async_trait::async_trait]
impl VizierTool for ProposePlan {
    type Input = ProposePlanInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "propose_plan".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn ends_turn() -> bool {
        true
    }

    fn description(&self) -> String {
        "Present a plan for the person to approve. Finish a Plan-mode turn with this: the plan is \
         shown with a **Build this plan** action, and the call ends the turn."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let title = args.title.trim();
        if title.is_empty() || title.chars().count() > 120 {
            return Err(VizierError(
                "invalid_plan: the title must be 1–120 characters".into(),
            ));
        }
        if args.summary.as_deref().is_some_and(|s| s.chars().count() > 2_000) {
            return Err(VizierError(
                "invalid_plan: the summary must be at most 2,000 characters".into(),
            ));
        }
        if args.steps.is_empty() || args.steps.len() > 30 {
            return Err(VizierError(
                "invalid_plan: a plan needs 1–30 steps".into(),
            ));
        }
        if args.steps.iter().any(|step| step.chars().count() > 500) {
            return Err(VizierError(
                "invalid_plan: each step must be at most 500 characters".into(),
            ));
        }
        Ok(serde_json::json!({ "status": "proposed" }))
    }
}
