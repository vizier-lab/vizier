//! An agent's custom tools for people (`specs/061-agent-custom-tools/contracts/http-api.md`): see
//! them grouped by category, read a version, edit, switch off, delete, roll back and rename a
//! category. Authorization lives here: view is `can_view_agent`, every change `can_edit_agent`,
//! and an agent the person cannot view is `404`. The work goes through the same
//! [`crate::custom_tools::CustomTools`] the agent's own tools use, with the person as the actor.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ServiceError, Services};
use crate::{
    access::{can_edit_agent, can_view_agent},
    auth::Principal,
    custom_tools::{Env, ToolNames, UpdateInput, naming, validate},
    schema::AgentConfig,
    storage::{
        agent::AgentStorage,
        custom_tool::{CustomTool, CustomToolActor, CustomToolStorage},
    },
};

/// A person's edit (`PUT /agents/{id}/custom-tools/{tool}`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EditCustomToolRequest {
    /// The version the person was editing; a newer one is `409 out_of_date`.
    pub expected_version: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    /// Required when `category` is new.
    #[serde(default)]
    pub category_description: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub input_schema: Option<Value>,
    #[serde(default)]
    pub output_description: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetCustomToolEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RollbackCustomToolRequest {
    pub to_version: i64,
    pub expected_version: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateCustomToolCategoryRequest {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub new_name: Option<String>,
}

/// The agent, when `who` may see it (and change it, with `edit`). An agent the person cannot
/// view does not exist for them.
async fn agent_for(s: &Services, who: &Principal, agent_id: &str, edit: bool) -> Result<AgentConfig, ServiceError> {
    let agent = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(agent)) => agent,
        Ok(None) => return Err(ServiceError::NotFound("agent not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !can_view_agent(who, &agent) {
        return Err(ServiceError::NotFound("agent not found".into()));
    }
    if edit && !can_edit_agent(who, &agent) {
        return Err(ServiceError::Forbidden("you may not change this agent's custom tools".into()));
    }
    Ok(agent)
}

fn internal(e: anyhow::Error) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

async fn tools(s: &Services, agent_id: &str) -> Result<Vec<CustomTool>, ServiceError> {
    s.deps().storage.list_custom_tools(agent_id, true).await.map_err(internal)
}

/// The tool a path segment names: its id or its name.
async fn find(s: &Services, agent_id: &str, key: &str) -> Result<CustomTool, ServiceError> {
    tools(s, agent_id)
        .await?
        .into_iter()
        .find(|tool| tool.id == key || tool.name == key)
        .ok_or_else(|| ServiceError::NotFound("custom tool not found".into()))
}

/// A person acting in their own name, checked against the agent's published tool names.
async fn person_env(s: &Services, who: &Principal, agent_id: &str) -> Env<'static> {
    Env {
        agent_id: agent_id.to_string(),
        actor: CustomToolActor::Person { user_id: who.user_id.clone(), name: who.username.clone() },
        names: s.deps().custom_tools.published_names(agent_id).await,
        runner: None,
        reserved: vec![],
    }
}

/// An agent-facing `status` result as an HTTP outcome.
fn outcome(value: Value) -> Result<Value, ServiceError> {
    let status = value.get("status").and_then(Value::as_str).unwrap_or_default().to_string();
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value["problems"]
                .as_array()
                .and_then(|problems| problems.first())
                .and_then(|p| p["message"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| status.clone());
    match status.as_str() {
        "not_found" => Err(ServiceError::NotFound(message)),
        "invalid" | "sample_failed" => {
            Err(ServiceError::Unprocessable { code: status, message, data: value })
        }
        "out_of_date" | "has_callers" | "disabled_by_person" => {
            Err(ServiceError::Conflict { message, code: Some(status), data: Some(value) })
        }
        _ => Ok(value),
    }
}

fn author(actor: &CustomToolActor) -> Value {
    match actor {
        CustomToolActor::Agent => json!({ "kind": "agent" }),
        CustomToolActor::Dream { cycle_id } => json!({ "kind": "dream", "dream_cycle_id": cycle_id }),
        CustomToolActor::Person { name, .. } => json!({ "kind": "person", "name": name }),
    }
}

/// One row of the page (`ToolSummary`).
fn summary(tool: &CustomTool, all: &[CustomTool], names: Option<&ToolNames>) -> Value {
    let known: Option<HashSet<String>> = names.map(|names| names.names.iter().cloned().collect());
    let backed: HashSet<String> =
        names.map(|names| names.model_backed.iter().cloned().collect()).unwrap_or_default();
    let calls = &tool.current.definition.calls;
    let missing = validate::needs_attention(calls, known.as_ref(), all).filter(|missing| !missing.is_empty());
    json!({
        "id": tool.id,
        "name": tool.name,
        "call_as": naming::call_as(&tool.name),
        "category": tool.category,
        "description": tool.current.definition.description,
        "version": tool.current_seq,
        "author": author(&tool.current.actor),
        "updated_at": tool.updated_at,
        "last_called_at": tool.last_called_at,
        "enabled": tool.enabled,
        "disabled_by": tool.disabled_by.as_ref().map(author),
        "needs_attention": missing.map(|missing| json!({ "missing": missing })),
        "model_backed": validate::model_backed(calls, &backed, all),
    })
}

/// `GET /agents/{id}/custom-tools`: every tool grouped by category, by name with `custom` last.
pub async fn list(s: &Services, who: &Principal, agent_id: &str) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, false).await?;
    let all = tools(s, agent_id).await?;
    let names = s.deps().custom_tools.published_names(agent_id).await;
    let mut categories = s.deps().storage.list_custom_tool_categories(agent_id).await.map_err(internal)?;
    categories.sort_by_key(|c| (c.name == naming::DEFAULT_CATEGORY, c.name.clone()));
    let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for tool in &all {
        grouped.entry(tool.category.clone()).or_default().push(summary(tool, &all, names.as_ref()));
    }
    let groups: Vec<Value> = categories
        .into_iter()
        .map(|category| {
            json!({
                "name": category.name,
                "description": category.description,
                "updated_by": category.updated_by.as_ref().map(author),
                "tools": grouped.remove(&category.name).unwrap_or_default(),
            })
        })
        .collect();
    Ok(json!({ "categories": groups, "tool_names_known": names.is_some() }))
}

/// `GET /agents/{id}/custom-tools/{tool}`.
pub async fn detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    version: Option<i64>,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, false).await?;
    let tool = find(s, agent_id, key).await?;
    let names = s.deps().custom_tools.published_names(agent_id).await;
    let mut value = s
        .deps()
        .custom_tools
        .get(agent_id, names.as_ref(), &tool.name, version)
        .await
        .map_err(internal)
        .and_then(outcome)?;
    let shown = value["version"].as_i64().unwrap_or(tool.current_seq);
    if let Ok(Some(version)) = s.deps().storage.get_custom_tool_version(agent_id, &tool.id, shown).await {
        value["author"] = author(&version.actor);
        value["created_at"] = json!(version.created_at);
    }
    value["id"] = json!(tool.id);
    value["disabled_by"] = json!(tool.disabled_by.as_ref().map(author));
    Ok(value)
}

/// `PUT /agents/{id}/custom-tools/{tool}`: a person's edit, as a new version. No sample run;
/// calls are checked against the published view, or saved with a `tools_unchecked` warning.
pub async fn edit(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    body: EditCustomToolRequest,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, true).await?;
    let tool = find(s, agent_id, key).await?;
    let env = person_env(s, who, agent_id).await;
    let input = UpdateInput {
        name: tool.name.clone(),
        new_name: body.name.filter(|name| *name != tool.name),
        description: body.description,
        input_schema: body.input_schema,
        script: body.script,
        category: body.category,
        category_description: body.category_description,
        output_description: body.output_description,
        sample_input: None,
        expected_version: Some(body.expected_version),
        reason: body.reason,
    };
    s.deps().custom_tools.update(&env, input).await.map_err(internal).and_then(outcome)
}

/// `DELETE /agents/{id}/custom-tools/{tool}`: the tool and its versions go; tools that called it
/// show *needs attention*; the deletion stays in the events.
pub async fn delete(s: &Services, who: &Principal, agent_id: &str, key: &str) -> Result<(), ServiceError> {
    agent_for(s, who, agent_id, true).await?;
    let tool = find(s, agent_id, key).await?;
    let env = person_env(s, who, agent_id).await;
    s.deps().custom_tools.delete(&env, &tool.name).await.map_err(internal).and_then(outcome)?;
    Ok(())
}

/// `PUT /agents/{id}/custom-tools/{tool}/enabled`.
pub async fn set_enabled(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    enabled: bool,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, true).await?;
    let tool = find(s, agent_id, key).await?;
    let env = person_env(s, who, agent_id).await;
    s.deps().custom_tools.set_enabled(&env, &tool.name, enabled).await.map_err(internal).and_then(outcome)?;
    Ok(json!({ "enabled": enabled }))
}

/// `GET /agents/{id}/custom-tools/{tool}/versions`: newest first.
pub async fn versions(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    before: Option<i64>,
    limit: Option<usize>,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, false).await?;
    let tool = find(s, agent_id, key).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let versions = s
        .deps()
        .storage
        .list_custom_tool_versions(agent_id, &tool.id, before, limit)
        .await
        .map_err(internal)?;
    let next = (versions.len() == limit).then(|| versions.last().map(|v| v.seq)).flatten();
    let rows: Vec<Value> = versions
        .iter()
        .map(|version| {
            let mut row = crate::custom_tools::version_summary(version);
            row["author"] = author(&version.actor);
            row["name"] = json!(version.definition.name);
            row["category"] = json!(version.definition.category);
            row
        })
        .collect();
    Ok(json!({ "versions": rows, "next": next }))
}

/// `GET /agents/{id}/custom-tools/{tool}/diff?from=&to=`.
pub async fn diff(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    from: i64,
    to: Option<i64>,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, false).await?;
    let tool = find(s, agent_id, key).await?;
    s.deps().custom_tools.diff(agent_id, &tool.name, from, to).await.map_err(internal).and_then(outcome)
}

/// `POST /agents/{id}/custom-tools/{tool}/rollback`: validated like an edit.
pub async fn rollback(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    key: &str,
    body: RollbackCustomToolRequest,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, true).await?;
    let tool = find(s, agent_id, key).await?;
    let env = person_env(s, who, agent_id).await;
    s.deps()
        .custom_tools
        .rollback(&env, &tool.name, body.to_version, Some(body.expected_version), body.reason)
        .await
        .map_err(internal)
        .and_then(outcome)
}

/// `GET /agents/{id}/custom-tools/events`: newest first, deletions included.
pub async fn events(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    before: Option<i64>,
    limit: Option<usize>,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, false).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let events = s
        .deps()
        .storage
        .list_custom_tool_events(agent_id, before, limit)
        .await
        .map_err(internal)?;
    let next = (events.len() == limit).then(|| events.last().map(|e| e.at)).flatten();
    let rows: Vec<Value> = events
        .into_iter()
        .map(|event| {
            json!({
                "id": event.id,
                "tool_id": event.tool_id,
                "tool_name": event.tool_name,
                "kind": event.kind,
                "detail": event.detail,
                "author": author(&event.actor),
                "at": event.at,
            })
        })
        .collect();
    Ok(json!({ "events": rows, "next": next }))
}

/// `PUT /agents/{id}/custom-tool-categories/{name}`.
pub async fn update_category(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    name: &str,
    body: UpdateCustomToolCategoryRequest,
) -> Result<Value, ServiceError> {
    agent_for(s, who, agent_id, true).await?;
    let env = person_env(s, who, agent_id).await;
    let result = s
        .deps()
        .custom_tools
        .update_category(&env, name, body.description.as_deref(), body.new_name.as_deref())
        .await
        .map_err(internal)?;
    // A taken name is a conflict for people (contracts/http-api.md).
    if result["problems"][0]["code"] == "category_name_taken" {
        return Err(ServiceError::Conflict {
            message: result["problems"][0]["message"].as_str().unwrap_or("name taken").to_string(),
            code: Some("name_taken".into()),
            data: Some(result),
        });
    }
    outcome(result)
}
