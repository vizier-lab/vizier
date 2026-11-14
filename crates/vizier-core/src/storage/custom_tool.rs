//! An agent's custom tools (`specs/061-agent-custom-tools/data-model.md`): one row per tool,
//! immutable versions, agent-owned categories and a log of the lifecycle actions that are not
//! versions (enable, disable, delete, category edits).
//!
//! Every stored-state rule a save depends on (the name is free, the category exists or is
//! being created, the expected version is current) is re-checked inside the save's own
//! transaction and comes back as a [`SaveOutcome`], never as an error, so two concurrent saves
//! can never lose a version.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::storage::VizierStorage;

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CustomToolActor {
    /// The agent, from a conversation, task or worker.
    Agent,
    /// The agent's dream cycle.
    Dream { cycle_id: String },
    /// A person, from the WebUI or the API.
    Person { user_id: String, name: String },
}

impl CustomToolActor {
    pub fn is_person(&self) -> bool {
        matches!(self, Self::Person { .. })
    }

    /// Persisted form: `(actor_kind, actor_id, actor_name, dream_cycle_id)`.
    pub fn to_columns(&self) -> (&'static str, Option<String>, Option<String>, Option<String>) {
        match self {
            Self::Agent => ("agent", None, None, None),
            Self::Dream { cycle_id } => ("dream", None, None, Some(cycle_id.clone())),
            Self::Person { user_id, name } => {
                ("person", Some(user_id.clone()), Some(name.clone()), None)
            }
        }
    }

    pub fn from_columns(
        kind: &str,
        id: Option<String>,
        name: Option<String>,
        cycle: Option<String>,
    ) -> Self {
        match kind {
            "dream" => Self::Dream { cycle_id: cycle.unwrap_or_default() },
            "person" => Self::Person {
                user_id: id.unwrap_or_default(),
                name: name.unwrap_or_default(),
            },
            _ => Self::Agent,
        }
    }

    /// How the WebUI and the agent read the author.
    pub fn label(&self) -> String {
        match self {
            Self::Agent => "you".into(),
            Self::Dream { cycle_id } => format!("a dream ({cycle_id})"),
            Self::Person { name, .. } => name.clone(),
        }
    }
}

/// Why a version exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionTrigger {
    Create,
    Update,
    Rollback,
}

impl VersionTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Rollback => "rollback",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "update" => Self::Update,
            "rollback" => Self::Rollback,
            _ => Self::Create,
        }
    }
}

/// The definition fields of a version: what a save writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomToolDefinition {
    pub name: String,
    pub category: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_description: Option<String>,
    pub script: String,
    /// The functions the script calls, sorted, derived at save (research R4).
    pub calls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_input: Option<serde_json::Value>,
}

/// One immutable snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomToolVersion {
    pub tool_id: String,
    pub seq: i64,
    #[serde(flatten)]
    pub definition: CustomToolDefinition,
    pub actor: CustomToolActor,
    pub trigger: VersionTrigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_from: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Epoch milliseconds.
    pub created_at: i64,
}

/// A tool and its current version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomTool {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    /// The stored category; a category rename changes it without a version.
    pub category: String,
    pub current_seq: i64,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_by: Option<CustomToolActor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_called_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub current: CustomToolVersion,
}

impl CustomTool {
    /// The current definition, with the tool row's category (a rename moves it without a
    /// version).
    pub fn definition(&self) -> CustomToolDefinition {
        CustomToolDefinition {
            category: self.category.clone(),
            ..self.current.definition.clone()
        }
    }
}

/// One agent-owned category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomToolCategory {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<CustomToolActor>,
    pub updated_at: i64,
    pub tools: i64,
    pub enabled_tools: i64,
}

/// A lifecycle action that is not a version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomToolEvent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// `enabled`, `disabled`, `deleted`, `category_renamed`, `category_described`.
    pub kind: String,
    #[serde(default)]
    pub detail: serde_json::Value,
    pub actor: CustomToolActor,
    pub at: i64,
}

/// A save, as the storage transaction saw it.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveOutcome {
    Saved { tool: CustomTool, category_created: bool },
    /// The expected version is not the current one.
    OutOfDate { current_seq: i64 },
    NameTaken,
    /// A new category needs a description.
    CategoryDescriptionRequired,
    /// The tool to update no longer exists.
    NotFound,
}

/// A category edit.
#[derive(Debug, Clone, PartialEq)]
pub enum CategoryOutcome {
    Updated { category: CustomToolCategory, moved: Vec<String> },
    NotFound,
    NameTaken,
}

/// What a save writes, plus how it came about.
#[derive(Debug, Clone)]
pub struct NewVersion {
    pub definition: CustomToolDefinition,
    pub trigger: VersionTrigger,
    pub restored_from: Option<i64>,
    pub reason: Option<String>,
}

#[async_trait::async_trait]
pub trait CustomToolStorage {
    /// Every tool of the agent with its current version, by name.
    async fn list_custom_tools(&self, agent_id: &str, include_disabled: bool)
    -> Result<Vec<CustomTool>>;

    async fn get_custom_tool(&self, agent_id: &str, name: &str) -> Result<Option<CustomTool>>;

    async fn get_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: &str,
        seq: i64,
    ) -> Result<Option<CustomToolVersion>>;

    /// Newest first; `before` is an exclusive `seq` bound.
    async fn list_custom_tool_versions(
        &self,
        agent_id: &str,
        tool_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolVersion>>;

    /// The agent's categories with their tool counts, by name.
    async fn list_custom_tool_categories(&self, agent_id: &str) -> Result<Vec<CustomToolCategory>>;

    /// Newest first; `before` is an exclusive `at` bound.
    async fn list_custom_tool_events(
        &self,
        agent_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolEvent>>;

    /// Write one version, creating the tool when `tool_id` is `None`, in one transaction.
    async fn save_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: Option<&str>,
        expected_seq: Option<i64>,
        version: NewVersion,
        actor: &CustomToolActor,
        category_description: Option<&str>,
    ) -> Result<SaveOutcome>;

    /// Switch a tool on or off with its event. `None` when the tool does not exist.
    async fn set_custom_tool_enabled(
        &self,
        agent_id: &str,
        tool_id: &str,
        enabled: bool,
        actor: &CustomToolActor,
    ) -> Result<Option<CustomTool>>;

    /// Remove a tool and its versions, keeping a `deleted` event. False when it did not exist.
    async fn delete_custom_tool(
        &self,
        agent_id: &str,
        tool_id: &str,
        actor: &CustomToolActor,
    ) -> Result<bool>;

    /// Rename and/or redescribe a category.
    async fn update_custom_tool_category(
        &self,
        agent_id: &str,
        name: &str,
        description: Option<&str>,
        new_name: Option<&str>,
        actor: &CustomToolActor,
    ) -> Result<CategoryOutcome>;

    /// Best-effort: when a tool was last called.
    async fn touch_custom_tool_called(&self, tool_id: &str, at: i64) -> Result<()>;
}

#[async_trait::async_trait]
impl CustomToolStorage for VizierStorage {
    async fn list_custom_tools(
        &self,
        agent_id: &str,
        include_disabled: bool,
    ) -> Result<Vec<CustomTool>> {
        self.0.list_custom_tools(agent_id, include_disabled).await
    }

    async fn get_custom_tool(&self, agent_id: &str, name: &str) -> Result<Option<CustomTool>> {
        self.0.get_custom_tool(agent_id, name).await
    }

    async fn get_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: &str,
        seq: i64,
    ) -> Result<Option<CustomToolVersion>> {
        self.0.get_custom_tool_version(agent_id, tool_id, seq).await
    }

    async fn list_custom_tool_versions(
        &self,
        agent_id: &str,
        tool_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolVersion>> {
        self.0.list_custom_tool_versions(agent_id, tool_id, before, limit).await
    }

    async fn list_custom_tool_categories(&self, agent_id: &str) -> Result<Vec<CustomToolCategory>> {
        self.0.list_custom_tool_categories(agent_id).await
    }

    async fn list_custom_tool_events(
        &self,
        agent_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CustomToolEvent>> {
        self.0.list_custom_tool_events(agent_id, before, limit).await
    }

    async fn save_custom_tool_version(
        &self,
        agent_id: &str,
        tool_id: Option<&str>,
        expected_seq: Option<i64>,
        version: NewVersion,
        actor: &CustomToolActor,
        category_description: Option<&str>,
    ) -> Result<SaveOutcome> {
        self.0
            .save_custom_tool_version(agent_id, tool_id, expected_seq, version, actor, category_description)
            .await
    }

    async fn set_custom_tool_enabled(
        &self,
        agent_id: &str,
        tool_id: &str,
        enabled: bool,
        actor: &CustomToolActor,
    ) -> Result<Option<CustomTool>> {
        self.0.set_custom_tool_enabled(agent_id, tool_id, enabled, actor).await
    }

    async fn delete_custom_tool(
        &self,
        agent_id: &str,
        tool_id: &str,
        actor: &CustomToolActor,
    ) -> Result<bool> {
        self.0.delete_custom_tool(agent_id, tool_id, actor).await
    }

    async fn update_custom_tool_category(
        &self,
        agent_id: &str,
        name: &str,
        description: Option<&str>,
        new_name: Option<&str>,
        actor: &CustomToolActor,
    ) -> Result<CategoryOutcome> {
        self.0
            .update_custom_tool_category(agent_id, name, description, new_name, actor)
            .await
    }

    async fn touch_custom_tool_called(&self, tool_id: &str, at: i64) -> Result<()> {
        self.0.touch_custom_tool_called(tool_id, at).await
    }
}
