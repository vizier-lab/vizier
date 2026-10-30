//! `set_secret` and `get_secret` (`specs/027-agent-secrets/contracts/agent-tools.md`): the
//! agent's own credential store. `get_secret` returns the value to the running script only —
//! the tool arms the turn redactor before it answers, so the report, logs and telemetry show
//! `[redacted]` — and setting is create-or-replace. Invalid input, a missing name and a refused
//! write are data results, so an unguarded script can inspect `status` and continue.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::{AgentId, Requester},
    secrets::service::{self, SecretError},
    storage::VizierStorage,
};

/// The rules both tool descriptions state.
const SECRET_RULES: &str = "Secrets belong to this agent only; another agent's name behaves exactly like a name that does not exist. Names are 1-128 characters of letters, digits, '-', '_' and '.', case-sensitive. Values are 8-65,536 bytes of text and are stored exactly, including spaces and line breaks. Setting an existing name replaces its value and the old one is gone. Names and values are never listed: use only names you were told.";

/// Only a real failure is raised. Validation and a missing name are results a script can read,
/// so an unguarded script keeps running on a typo.
fn storage_failure(err: SecretError) -> VizierError {
    VizierError(format!("secret storage: {}", err.message()))
}

/// What `set_secret` did.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SecretSetResult {
    Created { name: String },
    Replaced { name: String },
    Refused {
        /// `invalid_name` or `invalid_value`.
        reason: String,
        name: String,
        message: String,
    },
}

/// What `get_secret` found. `Found` carries the real value to the script; the report copy is
/// redacted before it leaves the sandbox.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SecretGetResult {
    Found { name: String, value: String },
    NotFound { name: String },
    Refused {
        reason: String,
        name: String,
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetSecretArgs {
    /// The secret's name, e.g. `LINEAR_TOKEN`.
    pub name: String,
    /// The value to store. It is never shown back.
    pub value: String,
}

pub struct SetSecret {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[async_trait::async_trait]
impl VizierTool for SetSecret {
    type Input = SetSecretArgs;
    type Output = SecretSetResult;

    fn name() -> String {
        "set_secret".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Secrets)
    }

    fn description(&self) -> String {
        format!(
            "Store a credential for this agent, or replace the value of an existing name. Returns {{status: \"created\"|\"replaced\", name}} or {{status: \"refused\", reason, message}}; a storage failure is an exception. The value you pass is the input that sets it, so avoid printing it afterwards; every later retrieval is redacted automatically. {SECRET_RULES}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let write = match service::set(
            &self.storage,
            &self.agent_id,
            &args.name,
            &args.value,
            &Requester::Agent(self.agent_id.clone()),
        )
        .await
        {
            Ok(write) => write,
            Err(SecretError::InvalidName(message)) => {
                return Ok(SecretSetResult::Refused {
                    reason: "invalid_name".into(),
                    name: args.name,
                    message,
                });
            }
            Err(SecretError::InvalidValue(message)) => {
                return Ok(SecretSetResult::Refused {
                    reason: "invalid_value".into(),
                    name: args.name,
                    message,
                });
            }
            Err(other) => return Err(storage_failure(other)),
        };

        // Armed only after the value is really stored, so a refusal teaches the turn nothing
        // about the value it was handed.
        ctx.secret_redactor.arm(&args.value);
        Ok(match write.outcome {
            crate::storage::secret::SecretOutcome::Created => {
                SecretSetResult::Created { name: args.name }
            }
            crate::storage::secret::SecretOutcome::Replaced => {
                SecretSetResult::Replaced { name: args.name }
            }
        })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetSecretArgs {
    /// The secret's name, exactly as it was set.
    pub name: String,
}

pub struct GetSecret {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[async_trait::async_trait]
impl VizierTool for GetSecret {
    type Input = GetSecretArgs;
    type Output = SecretGetResult;

    fn name() -> String {
        "get_secret".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Secrets)
    }

    fn description(&self) -> String {
        format!(
            "Read one of this agent's secrets by name. The value goes to the running script only: it is NOT shown to you or in the report, which shows [redacted]. Use the returned variable by passing it to another tool (e.g. as an http_client header) rather than printing it. Returns {{status: \"found\", name, value}}, {{status: \"not_found\", name}} or {{status: \"refused\", reason, message}}; a storage failure is an exception. {SECRET_RULES}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let value = match service::get(&self.storage, &self.agent_id, &args.name).await {
            Ok(value) => value,
            Err(SecretError::NotFound) => return Ok(SecretGetResult::NotFound { name: args.name }),
            Err(SecretError::InvalidName(message)) => {
                return Ok(SecretGetResult::Refused {
                    reason: "invalid_name".into(),
                    name: args.name,
                    message,
                });
            }
            Err(other) => return Err(storage_failure(other)),
        };

        // Armed before the value is returned, so anything later in this script that echoes it
        // is scrubbed.
        ctx.secret_redactor.arm(&value);
        Ok(SecretGetResult::Found { name: args.name, value })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{
        schema::{VizierChannelId, VizierSession},
        storage::{
            document::LocalDocumentStore,
            secret::SecretStorage,
            sqlite::{SqliteStorage, init_secret_schema},
        },
    };

    const TOKEN: &str = "agent-token-abc123";

    fn storage() -> (Arc<VizierStorage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_secret_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (Arc::new(VizierStorage::new(storage)), dir)
    }

    fn ctx() -> ToolContext {
        ToolContext {
            session: VizierSession("dp-a".into(), VizierChannelId::System, None),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
        }
    }

    async fn set(
        storage: &Arc<VizierStorage>,
        agent: &str,
        name: &str,
        value: &str,
        ctx: &ToolContext,
    ) -> SecretSetResult {
        SetSecret { storage: storage.clone(), agent_id: agent.into() }
            .call(SetSecretArgs { name: name.into(), value: value.into() }, ctx)
            .await
            .unwrap()
    }

    async fn get(
        storage: &Arc<VizierStorage>,
        agent: &str,
        name: &str,
        ctx: &ToolContext,
    ) -> SecretGetResult {
        GetSecret { storage: storage.clone(), agent_id: agent.into() }
            .call(GetSecretArgs { name: name.into() }, ctx)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_set_then_a_get_returns_the_value() {
        let (storage, _dir) = storage();
        let ctx = ctx();
        match set(&storage, "dp-a", "TOKEN", TOKEN, &ctx).await {
            SecretSetResult::Created { name } => assert_eq!(name, "TOKEN"),
            other => panic!("expected created, got {other:?}"),
        }
        assert!(ctx.secret_redactor.is_armed());

        match get(&storage, "dp-a", "TOKEN", &ctx).await {
            SecretGetResult::Found { name, value } => {
                assert_eq!(name, "TOKEN");
                assert_eq!(value, TOKEN);
            }
            other => panic!("expected found, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_second_set_replaces() {
        let (storage, _dir) = storage();
        let ctx = ctx();
        set(&storage, "dp-a", "TOKEN", "first-value-1", &ctx).await;
        match set(&storage, "dp-a", "TOKEN", "second-value-2", &ctx).await {
            SecretSetResult::Replaced { name } => assert_eq!(name, "TOKEN"),
            other => panic!("expected replaced, got {other:?}"),
        }
        match get(&storage, "dp-a", "TOKEN", &ctx).await {
            SecretGetResult::Found { value, .. } => assert_eq!(value, "second-value-2"),
            other => panic!("expected found, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invalid_input_is_refused_and_writes_nothing() {
        let (storage, _dir) = storage();
        let ctx = ctx();
        match set(&storage, "dp-a", "a/b", TOKEN, &ctx).await {
            SecretSetResult::Refused { reason, name, message } => {
                assert_eq!(reason, "invalid_name");
                assert_eq!(name, "a/b");
                assert_eq!(message, crate::secrets::NAME_RULE);
            }
            other => panic!("expected refused, got {other:?}"),
        }
        // A refused set teaches the redactor nothing: it never held the value.
        assert!(!ctx.secret_redactor.is_armed());

        match set(&storage, "dp-a", "TOKEN", "short", &ctx).await {
            SecretSetResult::Refused { reason, message, .. } => {
                assert_eq!(reason, "invalid_value");
                assert_eq!(message, crate::secrets::VALUE_RULE);
            }
            other => panic!("expected refused, got {other:?}"),
        }
        assert!(storage.list_agent_secrets("dp-a").await.unwrap().is_empty());

        match get(&storage, "dp-a", "a/b", &ctx).await {
            SecretGetResult::Refused { reason, .. } => assert_eq!(reason, "invalid_name"),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_missing_name_is_not_found_and_creates_nothing() {
        let (storage, _dir) = storage();
        let ctx = ctx();
        match get(&storage, "dp-a", "NO_SUCH_TOKEN", &ctx).await {
            SecretGetResult::NotFound { name } => assert_eq!(name, "NO_SUCH_TOKEN"),
            other => panic!("expected not_found, got {other:?}"),
        }
        assert!(storage.list_agent_secrets("dp-a").await.unwrap().is_empty());
        assert!(storage.list_agent_secret_events("dp-a", 20).await.unwrap().is_empty());
        // Nothing was returned, so nothing is armed.
        assert!(!ctx.secret_redactor.is_armed());
    }

    #[tokio::test]
    async fn whitespace_and_line_breaks_round_trip_exactly() {
        let (storage, _dir) = storage();
        let ctx = ctx();
        let value = "  line one\nline two\r\n  trailing  ";
        set(&storage, "dp-a", "MULTI", value, &ctx).await;
        match get(&storage, "dp-a", "MULTI", &ctx).await {
            SecretGetResult::Found { value: found, .. } => assert_eq!(found, value),
            other => panic!("expected found, got {other:?}"),
        }
    }

    /// Two agents, one name: each reads its own, and the other name does not exist.
    #[tokio::test]
    async fn agents_cannot_reach_each_others_secrets() {
        let (storage, _dir) = storage();
        let ctx_a = ctx();
        let ctx_b = ctx();
        set(&storage, "dp-a", "TOKEN", "agent-a-token-1", &ctx_a).await;
        set(&storage, "dp-b", "TOKEN", "agent-b-token-2", &ctx_b).await;
        set(&storage, "dp-b", "B_ONLY", "agent-b-secret-3", &ctx_b).await;

        match get(&storage, "dp-a", "TOKEN", &ctx_a).await {
            SecretGetResult::Found { value, .. } => assert_eq!(value, "agent-a-token-1"),
            other => panic!("expected A's value, got {other:?}"),
        }
        match get(&storage, "dp-a", "B_ONLY", &ctx_a).await {
            SecretGetResult::NotFound { name } => assert_eq!(name, "B_ONLY"),
            other => panic!("expected not_found with no hint, got {other:?}"),
        }
    }

    /// The value the tool returns is real, but the turn's report is scrubbed.
    #[tokio::test]
    async fn a_get_arms_the_report_redactor() {
        let (storage, _dir) = storage();
        let setup = ctx();
        set(&storage, "dp-a", "TOKEN", TOKEN, &setup).await;
        let mut report = crate::sandbox::ExecutionReport {
            ok: true,
            result: serde_json::json!({ "value": TOKEN }),
            stdout: TOKEN.into(),
            error: None,
            tool_calls: vec![],
            duration_ms: 1,
        };
        // A fresh context, like a separate turn, is unarmed until it retrieves.
        let later = ctx();
        assert!(!later.secret_redactor.is_armed());
        get(&storage, "dp-a", "TOKEN", &later).await;
        later.secret_redactor.report(&mut report);
        let text = serde_json::to_string(&report).unwrap();
        assert!(!text.contains(TOKEN), "{text}");
        assert!(text.contains("[redacted]"));
    }
}
