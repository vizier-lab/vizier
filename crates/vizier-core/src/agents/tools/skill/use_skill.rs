use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::skill::scopes::{ScopedSkill, SkillScope};

use super::scopes;

/// Load a skill's instructions, from whichever scope holds it: the project's own skills first,
/// then the agent's, then the global ones (`specs/041-project-coding-workspace` FR-036/FR-041).
pub struct UseSkill {
    workspace: String,
}

impl UseSkill {
    pub fn new(_agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self { workspace: deps.config.workspace.clone() }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UseSkillArgs {
    #[schemars(description = "slug of the skill to use")]
    pub slug: String,
}

#[async_trait::async_trait]
impl VizierTool for UseSkill {
    type Input = UseSkillArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Skills)
    }

    fn name() -> String {
        "use_skill".to_string()
    }

    fn description(&self) -> String {
        "load a skill's full instructions/content into context. Call list_skills or \
         get_skill_details first if you're not sure which skill to use."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let Some(scoped) = scopes(ctx, &self.workspace).find(&args.slug) else {
            return Err(VizierError(format!("Skill '{}' not found", args.slug)));
        };
        Ok(render(&scoped))
    }
}

/// A workspace skill is its content, as it always was. A project skill also says where it lives and
/// what it carries, because those bodies point at their own folder (`<skill-base-dir>/scripts/…`)
/// and at harness paths a person has to map by hand.
fn render(scoped: &ScopedSkill) -> String {
    if !matches!(scoped.scope, SkillScope::Project { .. }) {
        return scoped.skill.content.clone();
    }

    let folder = scoped.folder();
    let mut out = scoped.skill.content.trim().to_string();
    out.push_str(&format!(
        "\n\n## Skill folder\n{folder}\n\nRead a file of this skill with \
         `read_file(path=\"{folder}/<path>\")`, or `glob` to find one. Run a script of it with \
         `workspace_shell`, in the project folder; that is Build mode only.\n"
    ));

    let listing = scoped.resources();
    if !listing.paths.is_empty() {
        out.push_str("\nResources:\n");
        for path in &listing.paths {
            out.push_str(&format!("- {path}\n"));
        }
        if listing.left_out > 0 {
            out.push_str(&format!("- and {} more\n", listing.left_out));
        }
    }
    out
}
