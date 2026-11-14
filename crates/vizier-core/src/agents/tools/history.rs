//! `recall_history` (`specs/060-prune-tool-outputs/contracts/recall-history.md`): read back a
//! tool output that was pruned from this conversation's view, by the id its placeholder names.
//! The output comes back exactly as stored and uncapped (FR-010); a miss is a `status` result,
//! never an exception (FR-011), so an unguarded script survives a wrong id.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::{SessionHistoryContent, VizierSession},
    storage::{VizierStorage, history::HistoryStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RecallHistoryArgs {
    /// The id from a pruned output's placeholder.
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecallResult {
    Found {
        id: String,
        /// The output as stored: parsed JSON when it is JSON (a script report is), the text
        /// otherwise.
        output: Value,
    },
    /// No tool output of this conversation has that id.
    NotFound { id: String },
}

pub struct RecallHistory {
    pub storage: Arc<VizierStorage>,
}

/// The tool output `id` of `session`, as [`RecallResult`]. The storage read is scoped to the
/// agent and channel; the topic is checked here, since one channel holds a person's many
/// conversations. Another conversation's id reads as not found.
async fn recall(
    storage: &VizierStorage,
    session: &VizierSession,
    id: String,
) -> Result<RecallResult, VizierError> {
    let entry = storage
        .get_session_history_entry(session, &id)
        .await
        .map_err(|e| VizierError(format!("history storage: {e}")))?;
    let entry = entry.filter(|entry| entry.vizier_session.2 == session.2);
    Ok(match entry.map(|entry| entry.content) {
        Some(SessionHistoryContent::ToolResult { content, .. }) => RecallResult::Found {
            output: serde_json::from_str(&content).unwrap_or(Value::String(content)),
            id,
        },
        _ => RecallResult::NotFound { id },
    })
}

#[async_trait::async_trait]
impl VizierTool for RecallHistory {
    type Input = RecallHistoryArgs;
    type Output = RecallResult;

    fn name() -> String {
        "recall_history".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::History)
    }

    fn description(&self) -> String {
        "Read back a tool output that was pruned from this conversation's view, by the id in its placeholder `[output pruned (<size>) - use recall_history(\"<id>\") to recall]`. Returns {status: \"found\", id, output} with the output exactly as stored (a script report comes back as a dict), or {status: \"not_found\", id}. Only this conversation's outputs can be recalled. Call it in a script, take the part you need, and return only that.".to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        recall(&self.storage, &ctx.session, args.id).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        agents::client::test_storage,
        schema::{VizierChannelId, VizierRequest, VizierRequestContent},
        storage::history::save_with_busy_retry,
    };

    fn session(agent: &str, user: &str) -> VizierSession {
        VizierSession(
            agent.into(),
            VizierChannelId::HTTP(user.into(), "chat".into()),
            Some("topic".into()),
        )
    }

    async fn save_result(storage: &VizierStorage, session: &VizierSession, text: String) -> String {
        save_with_busy_retry(
            storage,
            session,
            SessionHistoryContent::ToolResult {
                call_id: "execute_python".into(),
                content: text,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn recall_history_reads_back_its_own_conversations_outputs() {
        let storage = test_storage();
        let a = session("agent", "ana");
        let report = json!({"ok": true, "result": "x".repeat(1_100_000), "stdout": "",
                            "tool_calls": [], "duration_ms": 1});
        let report_id = save_result(&storage, &a, report.to_string()).await;
        let text_id = save_result(&storage, &a, "plain words".into()).await;

        assert_eq!(
            recall(&storage, &a, report_id.clone()).await.unwrap(),
            RecallResult::Found { id: report_id, output: report },
            "a report comes back whole and parsed, over 1 MB"
        );
        assert_eq!(
            recall(&storage, &a, text_id.clone()).await.unwrap(),
            RecallResult::Found { id: text_id, output: json!("plain words") }
        );
    }

    #[tokio::test]
    async fn recall_history_answers_not_found_for_anything_else() {
        let storage = test_storage();
        let a = session("agent", "ana");
        let theirs = save_result(&storage, &session("agent", "bo"), "bo's output".into()).await;
        let other_agent = save_result(&storage, &session("other", "ana"), "other's".into()).await;
        let mut other_topic = session("agent", "ana");
        other_topic.2 = Some("another topic".into());
        let other_topic = save_result(&storage, &other_topic, "same channel, other topic".into()).await;
        let request = save_with_busy_retry(
            &storage,
            &a,
            SessionHistoryContent::Request(VizierRequest {
                content: VizierRequestContent::Chat("hi".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap();

        for id in ["no-such-id".to_string(), theirs, other_agent, other_topic, request] {
            assert_eq!(
                recall(&storage, &a, id.clone()).await.unwrap(),
                RecallResult::NotFound { id }
            );
        }
    }
}
