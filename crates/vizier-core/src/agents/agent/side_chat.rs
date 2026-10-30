//! The agent side of `/btw` (`specs/033-btw-side-chat/contracts/agent-side-chat.md`):
//! one tool-less completion beside the host conversation. The prompt prefix is what the
//! next host turn would see — the same system prompts, the checkpoint handover, the host
//! history replayed through `history_entries_to_messages` — then the exchange's earlier
//! turns, then the question. Nothing here touches history, hooks, storage writes, session
//! events or the tool loop: no tools run, no trace is left.

use anyhow::Result;
use tokio::time::Instant;
use tracing::Instrument;

use super::system_prompt::context::{context_md, with_context};
use super::system_prompt::side_chat::{SIDE_CHAT_MD, SIDE_CHAT_NO_TOOLS};
use crate::agents::agent::model::usage::{TurnId, UsageScope};
use crate::agents::agent::{VizierAgent, checkpoint_context, completion_request, text_of};
use crate::llm::types::{AssistantPart, Message, Part};
use crate::schema::{
    SessionHistory, VizierRequest, VizierResponse, VizierResponseContent, VizierResponseStats,
    VizierSession, history_entries_to_messages,
};
use crate::storage::history::HistoryStorage;

impl VizierAgent {
    /// One answer for one `/btw` ask. The caller (`run_btw`) has already loaded the host
    /// history up to this request's timestamp; this only assembles the prompt and makes the
    /// single completion. The reply's `history_uid` is `None`: it is never saved.
    pub async fn side_chat(
        &self,
        req: VizierRequest,
        session: VizierSession,
        history: Vec<SessionHistory>,
        handover: Option<String>,
    ) -> Result<VizierResponse> {
        let span = self.invoke_agent_span(&req, &session, false);
        self.side_chat_inner(req, session, history, handover)
            .instrument(span)
            .await
    }

    async fn side_chat_inner(
        &self,
        req: VizierRequest,
        session: VizierSession,
        history: Vec<SessionHistory>,
        handover: Option<String>,
    ) -> Result<VizierResponse> {
        // The one snapshot this answer uses, system prompt and tool list alike (G2).
        let snapshot = self.tools.load_full();
        let mut messages = self
            .prepare_system_prompts(&snapshot.categories, &session)
            .await;
        if let Some(handover) = handover {
            messages.push(checkpoint_context(handover));
        }
        messages.extend(history_entries_to_messages(&history));
        // The side exchange's earlier turns, oldest first, exactly as they happened above.
        if let crate::schema::VizierRequestContent::Btw(exchange) = &req.content {
            for turn in &exchange.turns {
                messages.push(Message::User(vec![Part::Text(turn.question.clone())]));
                messages.push(Message::Assistant(vec![AssistantPart::Text(
                    turn.answer.clone(),
                )]));
            }
        }

        // The question, frontmattered like any turn's message, wrapped with the context
        // block (empty — no memory passages, no skills, no reactions — as the contract
        // pins) plus the side-chat framing.
        let message = with_context(
            Message::User(vec![Part::Text(req.to_prompt()?)]),
            format!("{}\n\n{SIDE_CHAT_MD}", context_md(&[], &[], None, None)),
        );

        // One round, bounded by the agent's `prompt_timeout`: the same deadline the host
        // conversation's completion races against. Tool definitions stay (providers reject
        // a history holding tool calls when no tools are), but `tool_choice: none` asks for
        // text (research R5).
        let now = Instant::now();
        let prompt_timeout = *self.config.prompt_timeout;
        let response = self
            .model
            .complete_until(
                completion_request(&messages, &message, &snapshot.tools().await?, true),
                Some(now + prompt_timeout),
                &UsageScope::turn(Some(session.clone()), &TurnId::new(), 1),
            )
            .await?;

        let usage = response.usage;
        let stats = VizierResponseStats {
            input_tokens: usage.input_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
            total_cached_input_tokens: usage.cached_input_tokens,
            total_input_tokens: usage.input_tokens,
            total_output_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            total_cache_creation_input_tokens: usage.cache_creation_input_tokens,
            duration: now.elapsed(),
            current_context_size: Some(usage.input_tokens),
            context_window: self.model.context_window(),
            depth_exhausted: false,
            ended_by_tool: None,
            provider: Some(self.model.info().provider.clone()),
            model: Some(self.model.info().model.clone()),
        };

        // A call the model still made (some providers ignore `tool_choice: none`) is
        // dropped; when nothing textual is left the answer is the fixed no-tools notice
        // (research R5).
        let text = text_of(&response.content);
        let content = if text.trim().is_empty() {
            SIDE_CHAT_NO_TOOLS.to_string()
        } else {
            text
        };

        Ok(VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content,
                stats: Some(stats),
            },
            attachments: vec![],
            ..Default::default()
        })
    }
}
