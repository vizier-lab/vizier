//! The agent's custom tools as a per-turn source (`specs/061-agent-custom-tools` R1, R3): one
//! storage read per script run for the entries, so a tool saved earlier in the turn is callable
//! from the next script; category lines from the snapshot taken when the turn started.

use std::collections::HashSet;

use super::{naming, validate};
use crate::{
    agents::tools::{
        CatalogueEntry, CategorySummary, ToolContext, ToolRouter, ToolTarget,
        turn_source::{BridgeEnv, TurnToolSource},
    },
    llm::types::ToolDefinition,
    schema::VizierResponse,
    storage::custom_tool::{CustomTool, CustomToolStorage},
};

/// A category's name as a turn shows it: `<name>_custom` while an addon or MCP server of the
/// agent has the stored name (research R12), the stored name otherwise.
pub fn shown_category(stored: &str, reserved: &[String]) -> String {
    if reserved.iter().any(|taken| taken == stored) {
        format!("{stored}_{}", naming::CLASH_SUFFIX)
    } else {
        stored.to_string()
    }
}

/// The description a catalogue entry carries: the tool's own, plus one line of provenance and
/// warnings.
fn entry_description(tool: &CustomTool, model_backed: bool, missing: &[String]) -> String {
    let mut suffix = format!("Custom tool v{} by {}.", tool.current_seq, tool.current.actor.label());
    if model_backed {
        suffix.push_str(" Calls a model-backed tool.");
    }
    if !missing.is_empty() {
        suffix.push_str(&format!(" Needs attention: missing {}.", missing.join(", ")));
    }
    let description = tool.current.definition.description.trim();
    match &tool.current.definition.output_description {
        Some(output) if !output.trim().is_empty() => {
            format!("{description} Returns: {}. {suffix}", output.trim())
        }
        _ => format!("{description} {suffix}"),
    }
}

pub struct CustomSource;

impl CustomSource {
    pub const ID: &'static str = "custom";
}

#[async_trait::async_trait]
impl TurnToolSource for CustomSource {
    fn id(&self) -> &'static str {
        Self::ID
    }

    async fn entries(&self, router: &ToolRouter, ctx: &ToolContext) -> Vec<CatalogueEntry> {
        let Some(service) = &ctx.custom.tools else {
            return vec![];
        };
        let tools = match service.storage().list_custom_tools(&ctx.session.0, true).await {
            Ok(tools) => tools,
            Err(err) => {
                tracing::warn!(agent_id = %ctx.session.0, "could not list custom tools: {err}");
                return vec![];
            }
        };
        let reserved = router.client_reserved_categories();
        let backed: HashSet<String> = router.model_backed_names().into_iter().collect();
        tools
            .iter()
            .filter(|tool| tool.enabled)
            .map(|tool| {
                let calls = &tool.current.definition.calls;
                let model_backed = validate::model_backed(calls, &backed, &tools);
                let name = tool.name.clone();
                CatalogueEntry {
                    name: name.clone(),
                    category: shown_category(&tool.category, &reserved),
                    target: ToolTarget::Turn { source: Self::ID },
                    definition: ToolDefinition {
                        name,
                        description: entry_description(tool, model_backed, &[]),
                        parameters: tool.current.definition.input_schema.clone(),
                    },
                    ends_turn: false,
                }
            })
            .collect()
    }

    fn categories(&self, _router: &ToolRouter, ctx: &ToolContext) -> Vec<CategorySummary> {
        ctx.custom.categories.as_ref().clone()
    }

    async fn describe_extra(&self, ctx: &ToolContext, name: &str) -> Option<serde_json::Value> {
        let service = ctx.custom.tools.as_ref()?;
        let tool_name = name;
        let tools = service.storage().list_custom_tools(&ctx.session.0, true).await.ok()?;
        let tool = tools.iter().find(|tool| tool.name == tool_name)?;
        let router = ctx.custom.script_router.as_ref();
        let names = match router {
            Some(router) => Some(super::router_names(router, ctx).await),
            None => service.published_names(&ctx.session.0).await,
        };
        let known: Option<HashSet<String>> = names.as_ref().map(|names| names.names.iter().cloned().collect());
        let backed: HashSet<String> =
            names.as_ref().map(|names| names.model_backed.iter().cloned().collect()).unwrap_or_default();
        let calls = &tool.current.definition.calls;
        let attention = validate::attention(tool, known.as_ref(), &tools);
        Some(serde_json::json!({
            "custom": {
                "version": tool.current_seq,
                "author": tool.current.actor.label(),
                "model_backed": validate::model_backed(calls, &backed, &tools),
                "needs_attention": attention,
            }
        }))
    }

    async fn call(
        &self,
        name: &str,
        arguments: String,
        ctx: &ToolContext,
        env: &BridgeEnv<'_>,
    ) -> anyhow::Result<VizierResponse> {
        let Some(service) = &ctx.custom.tools else {
            anyhow::bail!("{name}: custom tools are not offered here");
        };
        super::run::call(service, name, arguments, ctx, env).await
    }
}
