//! An agent managing its own integrations (specs/017-agent-self-integrations, contracts/
//! agent-tools.md): five tools, present only while someone who manages the agent's
//! integrations has switched at least one family on. Every call re-reads the switches and goes
//! through `integrations::service`, so the agent is held to exactly the rules a person is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool, VizierToolSet, mcp_category_name},
    channels::connections::status_of,
    dependencies::VizierDependencies,
    error::VizierError,
    integrations::service::{self, Actor, IntegrationPatch, NewIntegration, ServiceError},
    schema::{AgentConfig, AgentId, ChannelPlatform, Integration, IntegrationFamily, IntegrationStatus},
    storage::{agent::AgentStorage, integration::IntegrationStorage},
};

const NEXT_TURN: &str = "available from your next turn";

const LOCKOUT: &str =
    "you will stop receiving messages on this connection; this reply may not be delivered";

/// The self-management tools for an agent granted `granted`, or none.
pub fn toolset(agent_id: &AgentId, deps: &VizierDependencies, granted: &[IntegrationFamily]) -> VizierToolSet {
    if granted.is_empty() {
        return VizierToolSet::new();
    }
    let base = Base { agent_id: agent_id.clone(), deps: deps.clone(), granted: granted.to_vec() };
    VizierToolSet::new()
        .tool(ListIntegrationKinds(base.clone()))
        .tool(ListIntegrations(base.clone()))
        .tool(CreateIntegration(base.clone()))
        .tool(UpdateIntegration(base.clone()))
        .tool(DeleteIntegration(base))
}

/// Whether disabling or deleting a connection on `connection` cuts off a conversation
/// happening on `session` (research R7).
fn lockout_warning(
    session: Option<ChannelPlatform>,
    connection: Option<ChannelPlatform>,
) -> Option<&'static str> {
    (session.is_some() && session == connection).then_some(LOCKOUT)
}

fn family_label(family: IntegrationFamily) -> &'static str {
    match family {
        IntegrationFamily::Mcp => "MCP servers",
        IntegrationFamily::Addon => "addons",
        IntegrationFamily::Channel => "channel connections",
    }
}

fn tool_error(e: ServiceError) -> VizierError {
    VizierError(format!("{e} (code: {})", e.code()))
}

#[derive(Clone)]
struct Base {
    agent_id: AgentId,
    deps: VizierDependencies,
    /// The families granted when the tools were composed; for descriptions only. Calls re-read.
    granted: Vec<IntegrationFamily>,
}

impl Base {
    /// What the description says the tool may touch.
    fn allowed(&self) -> String {
        let families: Vec<&str> = self.granted.iter().map(|f| family_label(*f)).collect();
        let mut out = format!("You may manage your {}.", families.join(", "));
        if self.granted.contains(&IntegrationFamily::Mcp) {
            out.push_str(" A local MCP server runs its command on the vizier host.");
        }
        out
    }

    /// The families granted right now (FR-008).
    async fn granted_now(&self) -> Result<Vec<IntegrationFamily>, VizierError> {
        self.deps
            .storage
            .list_grants(&self.agent_id)
            .await
            .map(|grants| grants.into_iter().map(|g| g.family).collect())
            .map_err(|e| VizierError(format!("cannot read what you may manage: {e}")))
    }

    fn parse_family(&self, family: Option<&str>) -> Result<Option<IntegrationFamily>, VizierError> {
        family
            .map(|f| {
                IntegrationFamily::parse(f)
                    .ok_or_else(|| VizierError(format!("unknown family '{f}': use mcp, addon or channel")))
            })
            .transpose()
    }

    fn refuse_family(family: IntegrationFamily) -> VizierError {
        VizierError(format!(
            "managing {} is not enabled for this agent; ask someone who manages its integrations to turn it on",
            family.as_str()
        ))
    }

    /// The families to list: `family` if granted, else every granted one.
    async fn families(&self, family: Option<&str>) -> Result<Vec<IntegrationFamily>, VizierError> {
        let granted = self.granted_now().await?;
        match self.parse_family(family)? {
            Some(f) if granted.contains(&f) => Ok(vec![f]),
            Some(f) => Err(Self::refuse_family(f)),
            None if granted.is_empty() => {
                Err(VizierError("managing integrations is not enabled for this agent".into()))
            }
            None => Ok(granted),
        }
    }

    async fn actor(&self, ctx: &ToolContext) -> Result<Actor<'_>, VizierError> {
        Ok(Actor::Agent {
            agent_id: &self.agent_id,
            session: Some(ctx.session.to_slug()),
            granted: self.granted_now().await?,
        })
    }

    async fn agent(&self) -> Result<AgentConfig, VizierError> {
        self.deps
            .storage
            .get_agent(&self.agent_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError("this agent no longer exists".into()))
    }

    /// The agent's own integrations (legacy multi-linked rows excluded, FR-023) in `families`.
    async fn own(&self, families: &[IntegrationFamily]) -> Result<Vec<Integration>, VizierError> {
        Ok(self
            .deps
            .storage
            .list_agent_integrations(&self.agent_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .into_iter()
            .filter(|i| !i.is_legacy() && families.contains(&i.family))
            .collect())
    }

    /// One integration as the tools show it: settings redacted, status, its tool category.
    fn view(&self, integration: &Integration, statuses: &std::collections::HashMap<String, IntegrationStatus>) -> Value {
        let kind = self.deps.integration_kinds.get(&integration.kind);
        let status = if !integration.enabled {
            IntegrationStatus::Disabled
        } else {
            status_of(&self.deps.channel_board, &integration.id)
                .or_else(|| statuses.get(&integration.id).cloned())
                .unwrap_or(IntegrationStatus::Unknown)
        };
        let tool_category = match integration.family {
            IntegrationFamily::Mcp => {
                Some(mcp_category_name(&integration.name, &self.deps.integration_kinds.addon_ids()))
            }
            IntegrationFamily::Addon => Some(integration.kind.clone()),
            IntegrationFamily::Channel => None,
        };
        json!({
            "id": integration.id,
            "kind": integration.kind,
            "family": integration.family,
            "name": integration.name,
            "enabled": integration.enabled,
            "status": status,
            "missing": kind.as_ref().map(|k| k.missing(&integration.settings)).unwrap_or_default(),
            "settings": kind.as_ref().map(|k| k.redact(&integration.settings)).unwrap_or(json!({})),
            "tool_category": tool_category,
            "creator": integration.creator,
        })
    }

    async fn view_one(&self, integration: &Integration) -> Value {
        let statuses = service::agent_statuses(&self.deps, &self.agent_id).await;
        self.view(integration, &statuses)
    }

    /// The warning for disabling or deleting `integration` from this conversation, if any.
    fn lockout(&self, integration: &Integration, ctx: &ToolContext) -> Option<&'static str> {
        let connection = self
            .deps
            .integration_kinds
            .get(&integration.kind)
            .and_then(|k| k.platform());
        lockout_warning(ctx.session.1.platform(), connection)
    }

    /// The agent's own integration `id`; another agent's or a legacy row reads as missing.
    async fn find(&self, id: &str) -> Result<Integration, VizierError> {
        self.deps
            .storage
            .get_integration(id)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .filter(|i| i.owner_agent().is_some_and(|owner| owner == &self.agent_id))
            .ok_or_else(|| VizierError(format!("no integration '{id}'")))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct FamilyArgs {
    /// `mcp`, `addon` or `channel`; leave out for every family you may manage.
    pub family: Option<String>,
}

pub struct ListIntegrationKinds(Base);

#[async_trait::async_trait]
impl VizierTool for ListIntegrationKinds {
    type Input = FamilyArgs;
    type Output = Value;

    fn name() -> String {
        "list_integration_kinds".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Integrations)
    }

    fn description(&self) -> String {
        format!(
            "List the kinds of integration you can create on yourself and the settings each takes. {} For an addon, `installed` says whether you already have it.",
            self.0.allowed()
        )
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let families = self.0.families(args.family.as_deref()).await?;
        let own = self.0.own(&families).await?;
        let kinds: Vec<Value> = families
            .iter()
            .flat_map(|family| self.0.deps.integration_kinds.by_family(Some(*family)))
            .map(|kind| {
                let listing = kind.listing();
                let installed = (kind.family() == IntegrationFamily::Addon)
                    .then(|| own.iter().any(|i| i.kind == kind.id()));
                json!({
                    "kind": kind.id(),
                    "family": kind.family(),
                    "name": listing.name,
                    "description": listing.description,
                    "installed": installed,
                    "fields": listing.fields,
                })
            })
            .collect();
        Ok(json!({ "kinds": kinds }))
    }
}

pub struct ListIntegrations(Base);

#[async_trait::async_trait]
impl VizierTool for ListIntegrations {
    type Input = FamilyArgs;
    type Output = Value;

    fn name() -> String {
        "list_integrations".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Integrations)
    }

    fn description(&self) -> String {
        format!(
            "List your own integrations with their status and settings, secrets redacted. {} `tool_category` is where an MCP server's or addon's tools are listed: pass it to list_tool_functions to see them.",
            self.0.allowed()
        )
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let families = self.0.families(args.family.as_deref()).await?;
        let own = self.0.own(&families).await?;
        let statuses = service::agent_statuses(&self.0.deps, &self.0.agent_id).await;
        let integrations: Vec<Value> = own.iter().map(|i| self.0.view(i, &statuses)).collect();
        Ok(json!({ "integrations": integrations }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateArgs {
    /// The kind, from list_integration_kinds: `mcp`, `brave_search`, `discord`, `telegram`, …
    pub kind: String,
    /// Its name. For an MCP server this names its tools (`<name>_<tool>`): lowercase letters,
    /// digits, `-` and `_`.
    pub name: String,
    /// The kind's settings, keyed as list_integration_kinds shows them.
    #[serde(default = "empty_object")]
    pub settings: Value,
    /// Start it enabled (default true).
    pub enabled: Option<bool>,
}

fn empty_object() -> Value {
    json!({})
}

pub struct CreateIntegration(Base);

#[async_trait::async_trait]
impl VizierTool for CreateIntegration {
    type Input = CreateArgs;
    type Output = Value;

    fn name() -> String {
        "create_integration".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Integrations)
    }

    fn description(&self) -> String {
        format!(
            "Create one of your own integrations, available from your next turn. {}",
            self.0.allowed()
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let actor = self.0.actor(ctx).await?;
        let agent = self.0.agent().await?;
        let new = NewIntegration {
            kind: args.kind,
            name: args.name,
            settings: args.settings,
            enabled: args.enabled.unwrap_or(true),
        };
        let integration = service::create(&self.0.deps, &actor, &self.0.agent_id, &agent, new)
            .await
            .map_err(tool_error)?;
        let mut out = self.0.view_one(&integration).await;
        out["note"] = json!(NEXT_TURN);
        Ok(out)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateArgs {
    /// The integration's id, from list_integrations.
    pub id: String,
    pub name: Option<String>,
    /// Settings to change. A redacted secret sent back as it was shown is kept.
    pub settings: Option<Value>,
    pub enabled: Option<bool>,
}

pub struct UpdateIntegration(Base);

#[async_trait::async_trait]
impl VizierTool for UpdateIntegration {
    type Input = UpdateArgs;
    type Output = Value;

    fn name() -> String {
        "update_integration".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Integrations)
    }

    fn description(&self) -> String {
        let mut out = format!(
            "Edit, enable or disable one of your own integrations; changes apply from your next turn. {}",
            self.0.allowed()
        );
        if self.0.granted.contains(&IntegrationFamily::Channel) {
            out.push_str(" Disabling the connection you are talking on stops your messages there.");
        }
        out
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let current = self.0.find(&args.id).await?;
        let actor = self.0.actor(ctx).await?;
        let warning = (args.enabled == Some(false) && current.enabled)
            .then(|| self.0.lockout(&current, ctx))
            .flatten();
        // The agent sends only what it changes; the service takes whole settings.
        let settings = args.settings.map(|changes| {
            let mut merged = current.settings.clone();
            if let (Some(merged), Some(changes)) = (merged.as_object_mut(), changes.as_object()) {
                merged.extend(changes.clone());
            }
            merged
        });
        let patch = IntegrationPatch { name: args.name, settings, enabled: args.enabled };
        let (integration, _) = service::update(&self.0.deps, &actor, &args.id, patch, &|_| false)
            .await
            .map_err(tool_error)?;
        let mut out = self.0.view_one(&integration).await;
        out["note"] = json!(NEXT_TURN);
        if let Some(warning) = warning {
            out["warning"] = json!(warning);
        }
        Ok(out)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteArgs {
    /// The integration's id, from list_integrations.
    pub id: String,
}

pub struct DeleteIntegration(Base);

#[async_trait::async_trait]
impl VizierTool for DeleteIntegration {
    type Input = DeleteArgs;
    type Output = Value;

    fn name() -> String {
        "delete_integration".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Integrations)
    }

    fn description(&self) -> String {
        let mut out = format!("Delete one of your own integrations. {}", self.0.allowed());
        if self.0.granted.contains(&IntegrationFamily::Channel) {
            out.push_str(" Deleting the connection you are talking on stops your messages there.");
        }
        out
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let current = self.0.find(&args.id).await?;
        let actor = self.0.actor(ctx).await?;
        let warning = self.0.lockout(&current, ctx);
        service::delete(&self.0.deps, &actor, &args.id, None, &|_| false)
            .await
            .map_err(tool_error)?;
        let mut out = json!({ "deleted": args.id });
        if let Some(warning) = warning {
            out["warning"] = json!(warning);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockout_only_on_the_platform_being_talked_on() {
        use ChannelPlatform::{Discord, Telegram};
        assert_eq!(lockout_warning(Some(Telegram), Some(Telegram)), Some(LOCKOUT));
        assert_eq!(lockout_warning(Some(Discord), Some(Discord)), Some(LOCKOUT));
        // A WebUI (or task, dream…) conversation isn't on any platform.
        assert_eq!(lockout_warning(None, Some(Telegram)), None);
        assert_eq!(lockout_warning(Some(Discord), Some(Telegram)), None);
        // An MCP server or addon has no platform.
        assert_eq!(lockout_warning(Some(Telegram), None), None);
        assert_eq!(lockout_warning(None, None), None);
    }
}
