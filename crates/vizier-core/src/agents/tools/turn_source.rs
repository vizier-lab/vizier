//! Per-turn tool sources (`specs/061-agent-custom-tools` research R1,
//! contracts/turn-tool-source.md): the tools a script can call in one turn beyond the
//! router's own — the person's client tools, the project coding tools and the agent's custom
//! tools. Each source reads what it needs from the [`ToolContext`] and offers nothing when the
//! turn has none, so the catalogue, the category lines, the docs and dispatch iterate one
//! list instead of special-casing each kind.

use std::{sync::Mutex, time::Duration};

use anyhow::Result;

use crate::{
    agents::tools::{CatalogueEntry, CategorySummary, ToolContext, ToolRouter},
    sandbox::ToolInvocationRecord,
    schema::VizierResponse,
};

/// What the bridge hands a source for one call: the router (so a source may run a nested
/// script), the per-call timeout, and the call's output slots.
pub struct BridgeEnv<'a> {
    pub router: &'a ToolRouter,
    pub tool_timeout: Duration,
    /// A client call's history uid (the report keeps a reference instead of the arguments).
    pub call_id: &'a Mutex<Option<String>>,
    /// The calls a custom tool's script made, kept on the custom tool's own record.
    pub nested: &'a Mutex<Vec<ToolInvocationRecord>>,
}

#[async_trait::async_trait]
pub trait TurnToolSource: Send + Sync {
    /// Stable id, also the `ToolTarget::Turn { source }` value.
    fn id(&self) -> &'static str;

    /// Script-callable entries for this turn, names final.
    async fn entries(&self, router: &ToolRouter, ctx: &ToolContext) -> Vec<CatalogueEntry>;

    /// Category lines this source adds to `SANDBOX.md` for this turn.
    fn categories(&self, router: &ToolRouter, ctx: &ToolContext) -> Vec<CategorySummary>;

    /// The output schema `describe_tool_function` shows, when the source knows one.
    fn output_schema(&self, _ctx: &ToolContext, _name: &str) -> Option<serde_json::Value> {
        None
    }

    /// Extra fields merged into `describe_tool_function`'s answer.
    async fn describe_extra(&self, _ctx: &ToolContext, _name: &str) -> Option<serde_json::Value> {
        None
    }

    /// Run one call.
    async fn call(
        &self,
        name: &str,
        arguments: String,
        ctx: &ToolContext,
        env: &BridgeEnv<'_>,
    ) -> Result<VizierResponse>;
}

/// Every source, in catalogue and category-line order: client, project, custom.
pub fn sources() -> [&'static dyn TurnToolSource; 3] {
    [
        &crate::agents::client::ClientSource,
        &crate::projects::tools::ProjectSource,
        &crate::custom_tools::catalogue::CustomSource,
    ]
}

/// The source behind a `ToolTarget::Turn { source }` id.
pub fn source(id: &str) -> Option<&'static dyn TurnToolSource> {
    sources().into_iter().find(|source| source.id() == id)
}
