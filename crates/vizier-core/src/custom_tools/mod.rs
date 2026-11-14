//! Agent custom tools (`specs/061-agent-custom-tools`): an agent saves a Python script that
//! composes its tools as a named, versioned tool of its own, then calls it from any script as
//! `<name>(...)`, by the name it was saved under.
//!
//! [`CustomTools`] is the one write path for the agent, its dreams and people (research R11):
//! every save is validated here and written through [`CustomToolStorage`] in one transaction.
//! Agent-facing results are the `status` values of contracts/agent-tools.md §2; refusals are
//! data, never errors.

use std::{collections::HashSet, sync::Arc};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{
    agents::tools::{CategorySummary, ToolContext, ToolRouter},
    storage::{
        VizierStorage,
        custom_tool::{
            CategoryOutcome, CustomTool, CustomToolActor, CustomToolDefinition, CustomToolStorage,
            CustomToolVersion, NewVersion, SaveOutcome, VersionTrigger,
        },
    },
};

pub mod calls;
pub mod catalogue;
pub mod naming;
pub mod run;
pub mod schema;
pub mod validate;

/// One problem with a definition or a call, as the agent and the WebUI read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    pub code: String,
    pub message: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Problem {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), extra: Map::new() }
    }

    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

/// The per-turn custom tool state on [`ToolContext`] (research R3, R5, R9). Default: nothing
/// offered, which is what dreams, handovers and titling get.
#[derive(Clone, Default)]
pub struct CustomTurn {
    /// The service, while this turn offers the agent's custom tools.
    pub tools: Option<CustomTools>,
    /// The category lines, snapshotted when the turn started so `SANDBOX.md` stays stable.
    pub categories: Arc<Vec<CategorySummary>>,
    /// The custom tools already running in this call chain, outermost first.
    pub chain: Vec<String>,
    /// The router scripts in this context dispatch through: the authoring tools check names
    /// against it and sample-run with it. Set by `RouterBridge::new` and by `dream_call`.
    pub script_router: Option<ToolRouter>,
    /// Set by `dream_call` to the dream cycle id: saves validate against the full toolset, skip
    /// the sample run and are authored by the dream.
    pub dream_cycle: Option<String>,
}

/// What an agent can call besides its custom tools, and which of those are model-backed: read
/// from the router in a turn, or from the published view (`agent.{id}.tool_names`) elsewhere.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolNames {
    pub names: Vec<String>,
    pub model_backed: Vec<String>,
}

impl ToolNames {
    fn known(&self) -> HashSet<String> {
        self.names.iter().cloned().collect()
    }

    fn backed(&self) -> HashSet<String> {
        self.model_backed.iter().cloned().collect()
    }
}

/// Who is acting and what can be checked from where they act.
pub struct Env<'a> {
    pub agent_id: String,
    pub actor: CustomToolActor,
    /// `None`: the agent's tools cannot be read here, so names go unchecked.
    pub names: Option<ToolNames>,
    /// A router and context to run a sample input with: an agent's own turn only.
    pub runner: Option<(&'a ToolRouter, &'a ToolContext)>,
    /// Category names a custom category may not take (addon ids, MCP keys).
    pub reserved: Vec<String>,
}

impl<'a> Env<'a> {
    /// The environment of an authoring call made from `ctx`: a script in a turn, or a dream.
    pub async fn from_context(ctx: &'a ToolContext) -> Env<'a> {
        let agent_id = ctx.session.0.clone();
        let router = ctx.custom.script_router.as_ref();
        let names = match router {
            Some(router) => Some(router_names(router, ctx).await),
            None => None,
        };
        let actor = match &ctx.custom.dream_cycle {
            Some(cycle_id) => CustomToolActor::Dream { cycle_id: cycle_id.clone() },
            None => CustomToolActor::Agent,
        };
        let runner = match (&ctx.custom.dream_cycle, router) {
            (None, Some(router)) => Some((router, ctx)),
            _ => None,
        };
        Env {
            agent_id,
            actor,
            names,
            runner,
            reserved: router.map(ToolRouter::client_reserved_categories).unwrap_or_default(),
        }
    }
}

/// Every non-custom function the agent can call: the router's whole catalogue (availability
/// aside, so a dream-made tool may call a question tool) plus this turn's client and project
/// tools.
pub async fn router_names(router: &ToolRouter, ctx: &ToolContext) -> ToolNames {
    let mut names: Vec<String> = router.all_names().await;
    for entry in crate::agents::tools::turn_catalogue(router, ctx).await {
        if entry.target != (crate::agents::tools::ToolTarget::Turn { source: catalogue::CustomSource::ID }) {
            names.push(entry.name);
        }
    }
    names.sort();
    names.dedup();
    ToolNames { names, model_backed: router.model_backed_names() }
}

/// What `create_custom_tool` takes (contracts/agent-tools.md §2).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateInput {
    #[schemars(description = "The tool's name: lower-case letters, digits and _, and not the name of any tool you already have. You call it by this name.")]
    pub name: String,
    #[schemars(description = "What the tool does, in one or two sentences; shown when you list or describe it.")]
    pub description: String,
    #[schemars(description = "A JSON Schema for the arguments: {\"type\": \"object\", \"properties\": {…}, \"required\": […]}. Supported keywords: type, properties, required, additionalProperties, items, enum, const, description, default, title, minimum, maximum, minLength, maxLength, minItems, maxItems.")]
    pub input_schema: Value,
    #[schemars(description = "The Python script. The validated arguments are the dict `input` (defaults filled in); the value of the last expression is the result. Call your tools as in execute_python.")]
    pub script: String,
    #[serde(default)]
    #[schemars(description = "Its category (one of yours, or a new one). Leave it out to use `custom`.")]
    pub category: Option<String>,
    #[serde(default)]
    #[schemars(description = "Required when `category` is new: one line saying what the category is for.")]
    pub category_description: Option<String>,
    #[serde(default)]
    #[schemars(description = "What the result looks like.")]
    pub output_description: Option<String>,
    #[serde(default)]
    #[schemars(description = "Arguments to run the tool with once before saving. The run is real: its tool calls take effect.")]
    pub sample_input: Option<Value>,
    #[serde(default)]
    #[schemars(description = "Why you made it, kept with the version.")]
    pub reason: Option<String>,
}

/// What `update_custom_tool` takes: the name plus any field to change.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct UpdateInput {
    #[schemars(description = "The custom tool to change.")]
    pub name: String,
    #[serde(default)]
    #[schemars(description = "A new name; refused while other custom tools call this one.")]
    pub new_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<Value>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    #[schemars(description = "Move it to this category (a new one needs category_description).")]
    pub category: Option<String>,
    #[serde(default)]
    pub category_description: Option<String>,
    #[serde(default)]
    pub output_description: Option<String>,
    #[serde(default)]
    #[schemars(description = "Arguments to run the changed tool with once before saving; the run is real.")]
    pub sample_input: Option<Value>,
    #[serde(default)]
    #[schemars(description = "The version you read; the save is refused as out_of_date when it is not current.")]
    pub expected_version: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// The custom tool service, on `VizierDependencies.custom_tools`.
#[derive(Clone)]
pub struct CustomTools {
    storage: Arc<VizierStorage>,
    state: Option<Arc<dyn vizier_schema::SharedState>>,
}

fn status(status: &str) -> Value {
    json!({ "status": status })
}

fn invalid(problems: Vec<Problem>) -> Value {
    json!({ "status": "invalid", "problems": problems })
}

fn not_found(name: &str) -> Value {
    json!({ "status": "not_found", "message": format!("you have no custom tool named {name}") })
}

impl CustomTools {
    pub fn new(storage: Arc<VizierStorage>, state: Option<Arc<dyn vizier_schema::SharedState>>) -> Self {
        Self { storage, state }
    }

    pub fn storage(&self) -> &Arc<VizierStorage> {
        &self.storage
    }

    /// The agent's published tool names, if they are published and live.
    pub async fn published_names(&self, agent_id: &str) -> Option<ToolNames> {
        let state = self.state.as_ref()?;
        crate::agents::views::read_tool_names_from(state, agent_id).await
    }

    /// The category lines a turn starting now shows: categories with at least one enabled
    /// tool, by name with `custom` last, each shown `<name>_custom` while it clashes with an
    /// addon or MCP server of `router` (research R12).
    pub async fn turn_categories(&self, agent_id: &str, router: &ToolRouter) -> Vec<CategorySummary> {
        let categories = match self.storage.list_custom_tool_categories(agent_id).await {
            Ok(categories) => categories,
            Err(err) => {
                tracing::warn!(agent_id, "could not read custom tool categories: {err}");
                return vec![];
            }
        };
        let reserved = router.client_reserved_categories();
        let mut lines: Vec<CategorySummary> = categories
            .into_iter()
            .filter(|category| category.enabled_tools > 0)
            .map(|category| CategorySummary {
                name: catalogue::shown_category(&category.name, &reserved),
                description: category.description,
            })
            .collect();
        lines.sort_by_key(|line| (line.name == naming::DEFAULT_CATEGORY, line.name.clone()));
        lines
    }

    async fn tools(&self, agent_id: &str) -> anyhow::Result<Vec<CustomTool>> {
        self.storage.list_custom_tools(agent_id, true).await
    }

    /// The stored category for a requested one, and whether the clash rule renamed it.
    fn category_for(requested: Option<&str>, reserved: &[String]) -> (String, Option<String>) {
        let requested = requested.map(str::trim).filter(|c| !c.is_empty());
        match requested {
            None => (naming::DEFAULT_CATEGORY.to_string(), None),
            Some(requested) if requested == naming::DEFAULT_CATEGORY => (requested.to_string(), None),
            Some(requested) => {
                let stored = naming::category_name(requested, reserved);
                let renamed = (stored != requested).then(|| stored.clone());
                (stored, renamed)
            }
        }
    }

    /// The description a new category gets: the given one, or the fixed one for `custom`.
    fn category_description(category: &str, given: Option<&str>) -> Option<String> {
        given
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .or_else(|| (category == naming::DEFAULT_CATEGORY).then(|| naming::DEFAULT_CATEGORY_DESCRIPTION.to_string()))
    }

    /// Validate, sample-run when asked and possible, and save. Shared by create, update and
    /// rollback; `editing` is the tool being changed.
    async fn save(
        &self,
        env: &Env<'_>,
        mut definition: CustomToolDefinition,
        editing: Option<&CustomTool>,
        expected_version: Option<i64>,
        category_description: Option<&str>,
        trigger: VersionTrigger,
        restored_from: Option<i64>,
        reason: Option<String>,
    ) -> anyhow::Result<(Value, Option<CustomTool>)> {
        let tools = self.tools(&env.agent_id).await?;
        let known = env.names.as_ref().map(ToolNames::known);
        let checked = validate::validate_definition(
            &mut definition,
            known.as_ref(),
            &tools,
            editing.map(|tool| tool.id.as_str()),
        );
        let mut problems = checked.problems;

        // Stored-state rules too, so every problem comes back at once; the save re-checks them.
        let categories = self.storage.list_custom_tool_categories(&env.agent_id).await?;
        let new_category = !categories.iter().any(|c| c.name == definition.category);
        let category_description = Self::category_description(&definition.category, category_description);
        if new_category && category_description.is_none() {
            problems.push(
                Problem::new(
                    "category_description_required",
                    format!("{} is a new category: give category_description, one line saying what it is for", definition.category),
                )
                .with("category", definition.category.as_str()),
            );
        }
        if let Some(tool) = editing {
            if !env.actor.is_person() {
                if tool.name != definition.name {
                    let callers = validate::callers(&tool.name, &tools);
                    if !callers.is_empty() {
                        return Ok((
                            json!({
                                "status": "has_callers",
                                "message": format!("other custom tools call {}; change them first", tool.name),
                                "callers": callers,
                            }),
                            None,
                        ));
                    }
                }
            }
        }
        if !problems.is_empty() {
            return Ok((invalid(problems), None));
        }

        let sample_run = match (&definition.sample_input, env.runner) {
            (Some(sample), Some((router, ctx))) => {
                let input = schema::validate(&definition.input_schema, sample).unwrap_or_else(|_| sample.clone());
                let (report, _attachments) = run::run_definition(router, ctx, &definition, input).await;
                if !report.ok {
                    return Ok((
                        json!({
                            "status": "sample_failed",
                            "message": "the sample run failed, so nothing was saved; its tool calls did take effect",
                            "report": report,
                        }),
                        None,
                    ));
                }
                Some(json!({
                    "ok": true,
                    "result": report.result,
                    "tool_calls": report.tool_calls,
                    "took_effect": true,
                }))
            }
            _ => None,
        };

        let outcome = self
            .storage
            .save_custom_tool_version(
                &env.agent_id,
                editing.map(|tool| tool.id.as_str()),
                expected_version,
                NewVersion { definition, trigger, restored_from, reason },
                &env.actor,
                category_description.as_deref(),
            )
            .await?;
        let (tool, category_created) = match outcome {
            SaveOutcome::Saved { tool, category_created } => (tool, category_created),
            SaveOutcome::OutOfDate { current_seq } => {
                return Ok((
                    json!({
                        "status": "out_of_date",
                        "message": "the tool changed since you read it; read it again and reapply your change",
                        "current_version": current_seq,
                    }),
                    None,
                ));
            }
            SaveOutcome::NameTaken => {
                return Ok((invalid(vec![Problem::new("name_taken", "that name was just taken")]), None));
            }
            SaveOutcome::CategoryDescriptionRequired => {
                return Ok((
                    invalid(vec![Problem::new("category_description_required", "that category is new: give category_description")]),
                    None,
                ));
            }
            SaveOutcome::NotFound => return Ok((not_found(editing.map(|t| t.name.as_str()).unwrap_or("")), None)),
        };

        let mut result = json!({
            "status": match trigger {
                VersionTrigger::Create => "created",
                VersionTrigger::Update => "updated",
                VersionTrigger::Rollback => "rolled_back",
            },
            "tool": tool_summary(&tool),
            "version": tool.current_seq,
        });
        if category_created {
            result["category_created"] = json!(true);
        }
        if let Some(sample_run) = sample_run {
            result["sample_run"] = sample_run;
        }
        if let Some(restored_from) = restored_from {
            result["restored_from"] = json!(restored_from);
        }
        if !checked.warnings.is_empty() {
            result["warnings"] = json!(checked.warnings);
        }
        Ok((result, Some(tool)))
    }

    /// `create_custom_tool`.
    pub async fn create(&self, env: &Env<'_>, input: CreateInput) -> anyhow::Result<Value> {
        let (category, renamed_to) = Self::category_for(input.category.as_deref(), &env.reserved);
        let definition = CustomToolDefinition {
            name: input.name.trim().to_string(),
            category,
            description: input.description.trim().to_string(),
            input_schema: input.input_schema,
            output_description: input.output_description.filter(|d| !d.trim().is_empty()),
            script: input.script,
            calls: vec![],
            sample_input: input.sample_input,
        };
        let (mut result, _) = self
            .save(
                env,
                definition,
                None,
                None,
                input.category_description.as_deref(),
                VersionTrigger::Create,
                None,
                input.reason,
            )
            .await?;
        if let Some(renamed_to) = renamed_to {
            if result["status"] == "created" {
                result["category_renamed_to"] = json!(renamed_to);
            }
        }
        Ok(result)
    }

    /// `update_custom_tool` (and a person's edit, with `expected_version`).
    pub async fn update(&self, env: &Env<'_>, input: UpdateInput) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(&env.agent_id, &input.name).await? else {
            return Ok(not_found(&input.name));
        };
        if let Some(refusal) = Self::person_disabled(env, &tool) {
            return Ok(refusal);
        }
        let current = tool.definition();
        let (category, renamed_to) = match input.category.as_deref() {
            Some(requested) if requested != current.category => Self::category_for(Some(requested), &env.reserved),
            _ => (current.category.clone(), None),
        };
        let definition = CustomToolDefinition {
            name: input.new_name.map(|n| n.trim().to_string()).unwrap_or(current.name),
            category,
            description: input.description.map(|d| d.trim().to_string()).unwrap_or(current.description),
            input_schema: input.input_schema.unwrap_or(current.input_schema),
            output_description: match input.output_description {
                Some(text) if text.trim().is_empty() => None,
                Some(text) => Some(text),
                None => current.output_description,
            },
            script: input.script.unwrap_or(current.script),
            calls: vec![],
            sample_input: input.sample_input,
        };
        let (mut result, _) = self
            .save(
                env,
                definition,
                Some(&tool),
                input.expected_version,
                input.category_description.as_deref(),
                VersionTrigger::Update,
                None,
                input.reason,
            )
            .await?;
        if let Some(renamed_to) = renamed_to {
            if result["status"] == "updated" {
                result["category_renamed_to"] = json!(renamed_to);
            }
        }
        Ok(result)
    }

    /// `rollback_custom_tool`: the old version's definition, validated and saved as a new one.
    pub async fn rollback(
        &self,
        env: &Env<'_>,
        name: &str,
        to_version: i64,
        expected_version: Option<i64>,
        reason: Option<String>,
    ) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(&env.agent_id, name).await? else {
            return Ok(not_found(name));
        };
        if let Some(refusal) = Self::person_disabled(env, &tool) {
            return Ok(refusal);
        }
        let Some(old) = self.storage.get_custom_tool_version(&env.agent_id, &tool.id, to_version).await? else {
            return Ok(json!({ "status": "not_found", "message": format!("{name} has no version {to_version}") }));
        };
        let mut definition = old.definition;
        definition.sample_input = None;
        // The category it lives in now stays, unless that version moved it; a removed category
        // would need a description, so the current one is used then.
        let categories = self.storage.list_custom_tool_categories(&env.agent_id).await?;
        if !categories.iter().any(|c| c.name == definition.category) {
            definition.category = tool.category.clone();
        }
        let (result, _) = self
            .save(
                env,
                definition,
                Some(&tool),
                expected_version,
                None,
                VersionTrigger::Rollback,
                Some(to_version),
                reason,
            )
            .await?;
        Ok(result)
    }

    /// An agent (or its dream) may not change a tool a person switched off.
    fn person_disabled(env: &Env<'_>, tool: &CustomTool) -> Option<Value> {
        let by_person = tool.disabled_by.as_ref().is_some_and(CustomToolActor::is_person);
        (!tool.enabled && by_person && !env.actor.is_person()).then(|| {
            json!({
                "status": "disabled_by_person",
                "message": format!(
                    "{} was switched off by {}; only a person can change or switch it back on",
                    tool.name,
                    tool.disabled_by.as_ref().map(CustomToolActor::label).unwrap_or_default()
                ),
            })
        })
    }

    /// `delete_custom_tool` (agent: refused while other tools call it).
    pub async fn delete(&self, env: &Env<'_>, name: &str) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(&env.agent_id, name).await? else {
            return Ok(not_found(name));
        };
        if !env.actor.is_person() {
            let callers = validate::callers(name, &self.tools(&env.agent_id).await?);
            if !callers.is_empty() {
                return Ok(json!({
                    "status": "has_callers",
                    "message": format!("other custom tools call {}; change them first", name),
                    "callers": callers,
                }));
            }
        }
        self.storage.delete_custom_tool(&env.agent_id, &tool.id, &env.actor).await?;
        Ok(status("deleted"))
    }

    /// `set_custom_tool_enabled`.
    pub async fn set_enabled(&self, env: &Env<'_>, name: &str, enabled: bool) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(&env.agent_id, name).await? else {
            return Ok(not_found(name));
        };
        if enabled {
            if let Some(refusal) = Self::person_disabled(env, &tool) {
                return Ok(refusal);
            }
        }
        if tool.enabled == enabled {
            return Ok(status(if enabled { "enabled" } else { "disabled" }));
        }
        self.storage.set_custom_tool_enabled(&env.agent_id, &tool.id, enabled, &env.actor).await?;
        Ok(status(if enabled { "enabled" } else { "disabled" }))
    }

    /// `update_custom_tool_category`: rename and/or redescribe; applies from the next turn.
    pub async fn update_category(
        &self,
        env: &Env<'_>,
        name: &str,
        description: Option<&str>,
        new_name: Option<&str>,
    ) -> anyhow::Result<Value> {
        let new_name = new_name.map(str::trim).filter(|n| !n.is_empty() && *n != name);
        let mut renamed_to = None;
        if let Some(requested) = new_name {
            if !naming::valid_name(requested) {
                return Ok(invalid(vec![Problem::new("category_invalid", format!("`{requested}` is not a valid category name"))]));
            }
            let stored = naming::category_name(requested, &env.reserved);
            renamed_to = Some(stored);
        }
        if renamed_to.is_none() && description.map(str::trim).is_none_or(str::is_empty) {
            return Ok(invalid(vec![Problem::new("nothing_to_change", "give a description, a new_name or both")]));
        }
        match self
            .storage
            .update_custom_tool_category(&env.agent_id, name, description, renamed_to.as_deref(), &env.actor)
            .await?
        {
            CategoryOutcome::NotFound => Ok(json!({ "status": "not_found", "message": format!("you have no category named {name}") })),
            CategoryOutcome::NameTaken => Ok(invalid(vec![Problem::new(
                "category_name_taken",
                format!("you already have a category named {}", renamed_to.unwrap_or_default()),
            )])),
            CategoryOutcome::Updated { category, moved } => {
                let mut result = json!({
                    "status": "updated",
                    "category": { "name": category.name, "description": category.description, "tools": category.tools },
                    "applies": "from your next turn",
                });
                if let Some(renamed_to) = renamed_to {
                    result["renamed_to"] = json!(renamed_to);
                    result["moved"] = json!(moved);
                }
                Ok(result)
            }
        }
    }

    /// `list_custom_tools`.
    pub async fn list(
        &self,
        agent_id: &str,
        names: Option<&ToolNames>,
        category: Option<&str>,
        include_disabled: bool,
    ) -> anyhow::Result<Value> {
        let tools = self.tools(agent_id).await?;
        let known = names.map(ToolNames::known);
        let backed = names.map(ToolNames::backed).unwrap_or_default();
        let listed: Vec<Value> = tools
            .iter()
            .filter(|tool| include_disabled || tool.enabled)
            .filter(|tool| category.is_none_or(|category| tool.category == category))
            .map(|tool| {
                let mut row = tool_summary(tool);
                row["enabled"] = json!(tool.enabled);
                if let Some(by) = &tool.disabled_by {
                    row["disabled_by"] = json!(by.label());
                }
                row["model_backed"] = json!(validate::model_backed(&tool.current.definition.calls, &backed, &tools));
                if let Some(attention) = validate::attention(tool, known.as_ref(), &tools) {
                    row["needs_attention"] = attention;
                }
                row["author"] = json!(tool.current.actor.label());
                row["description"] = json!(tool.current.definition.description);
                row["updated_at"] = json!(tool.updated_at);
                row["last_called_at"] = json!(tool.last_called_at);
                row
            })
            .collect();
        let categories: Vec<Value> = self
            .storage
            .list_custom_tool_categories(agent_id)
            .await?
            .into_iter()
            .map(|c| json!({ "name": c.name, "description": c.description, "tools": c.tools }))
            .collect();
        Ok(json!({ "tools": listed, "categories": categories }))
    }

    /// `get_custom_tool`: one version's full definition plus what it calls and who calls it.
    pub async fn get(
        &self,
        agent_id: &str,
        names: Option<&ToolNames>,
        name: &str,
        version: Option<i64>,
    ) -> anyhow::Result<Value> {
        let tools = self.tools(agent_id).await?;
        let Some(tool) = tools.iter().find(|tool| tool.name == name) else {
            return Ok(not_found(name));
        };
        let shown = match version {
            Some(seq) if seq != tool.current_seq => {
                match self.storage.get_custom_tool_version(agent_id, &tool.id, seq).await? {
                    Some(version) => version,
                    None => return Ok(json!({ "status": "not_found", "message": format!("{name} has no version {seq}") })),
                }
            }
            _ => tool.current.clone(),
        };
        let known = names.map(ToolNames::known);
        let backed = names.map(ToolNames::backed).unwrap_or_default();
        let attention = validate::attention(tool, known.as_ref(), &tools);
        Ok(json!({
            "name": tool.name,
            "call_as": tool.name,
            "category": tool.category,
            "enabled": tool.enabled,
            "disabled_by": tool.disabled_by.as_ref().map(CustomToolActor::label),
            "current_version": tool.current_seq,
            "version": shown.seq,
            "author": shown.actor.label(),
            "description": shown.definition.description,
            "input_schema": shown.definition.input_schema,
            "output_description": shown.definition.output_description,
            "script": shown.definition.script,
            "calls": shown.definition.calls,
            "callers": validate::callers(&tool.name, &tools),
            "model_backed": validate::model_backed(&tool.current.definition.calls, &backed, &tools),
            "needs_attention": attention,
            "updated_at": tool.updated_at,
            "last_called_at": tool.last_called_at,
        }))
    }

    /// `custom_tool_history`: versions newest first, and this tool's events.
    pub async fn history(&self, agent_id: &str, name: &str) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(agent_id, name).await? else {
            return Ok(not_found(name));
        };
        let versions: Vec<Value> = self
            .storage
            .list_custom_tool_versions(agent_id, &tool.id, None, 200)
            .await?
            .iter()
            .map(version_summary)
            .collect();
        let events: Vec<Value> = self
            .storage
            .list_custom_tool_events(agent_id, None, 200)
            .await?
            .into_iter()
            .filter(|event| event.tool_id.as_deref() == Some(tool.id.as_str()))
            .map(|event| json!({ "kind": event.kind, "detail": event.detail, "author": event.actor.label(), "at": event.at }))
            .collect();
        Ok(json!({ "name": tool.name, "versions": versions, "events": events }))
    }

    /// `diff_custom_tool`: line diffs of description, schema and script between two versions.
    pub async fn diff(&self, agent_id: &str, name: &str, from: i64, to: Option<i64>) -> anyhow::Result<Value> {
        let Some(tool) = self.storage.get_custom_tool(agent_id, name).await? else {
            return Ok(not_found(name));
        };
        let to = to.unwrap_or(tool.current_seq);
        let load = |seq: i64| {
            let storage = self.storage.clone();
            let agent_id = agent_id.to_string();
            let tool_id = tool.id.clone();
            async move { storage.get_custom_tool_version(&agent_id, &tool_id, seq).await }
        };
        let (Some(old), Some(new)) = (load(from).await?, load(to).await?) else {
            return Ok(json!({ "status": "not_found", "message": format!("{name} has no version {from} or {to}") }));
        };
        let diff = |a: &str, b: &str| {
            let (hunks, additions, deletions) = crate::storage::diff::diff_lines(a, b);
            crate::schema::RevisionDiff { from_seq: from, to_seq: to, additions, deletions, hunks }
        };
        let mut result = json!({
            "name": tool.name,
            "from": from,
            "to": to,
            "description": diff(&old.definition.description, &new.definition.description),
            "input_schema": diff(&validate::pretty(&old.definition.input_schema), &validate::pretty(&new.definition.input_schema)),
            "script": diff(&old.definition.script, &new.definition.script),
        });
        if old.definition.category != new.definition.category {
            result["category"] = json!({ "from": old.definition.category, "to": new.definition.category });
        }
        if old.definition.name != new.definition.name {
            result["renamed"] = json!({ "from": old.definition.name, "to": new.definition.name });
        }
        Ok(result)
    }
}

/// The short form every result carries.
pub fn tool_summary(tool: &CustomTool) -> Value {
    json!({
        "id": tool.id,
        "name": tool.name,
        "call_as": tool.name,
        "category": tool.category,
        "version": tool.current_seq,
    })
}

pub fn version_summary(version: &CustomToolVersion) -> Value {
    let mut row = json!({
        "version": version.seq,
        "author": version.actor.label(),
        "trigger": version.trigger,
        "at": version.created_at,
    });
    if let CustomToolActor::Dream { cycle_id } = &version.actor {
        row["dream_cycle_id"] = json!(cycle_id);
    }
    if let Some(reason) = &version.reason {
        row["reason"] = json!(reason);
    }
    if let Some(restored_from) = version.restored_from {
        row["restored_from"] = json!(restored_from);
    }
    row
}

#[cfg(test)]
mod tests;
