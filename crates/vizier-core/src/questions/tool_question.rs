//! `ask_question`: the turn-ending structured question tool (FR-001–FR-010).
//!
//! The call ends the turn, so the questions are the agent's answer: any text it wrote in the same
//! step is kept and shown with them (`agents/agent/mod.rs`). The person's reply arrives as their
//! next message, which is why this tool stores a set and returns nothing but its id.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    questions::{
        Questions, submit,
        limits,
        model::{QuestionAsked, QuestionSet, QuestionSetRecord, QuestionState},
        validate,
    },
    schema::{VizierSession, VizierChannelId},
    storage::{VizierStorage, question::QuestionStorage},
    transport::VizierTransport,
};

/// What the agent asks. The shape is `contracts/agent-tools.md` §1: the set itself, because the
/// questions *are* the call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AskQuestionInput {
    /// A heading over the card.
    #[schemars(description = "A short heading over the questions, e.g. \"Project setup\".")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// A sentence of context.
    #[schemars(description = "One sentence of context, shown under the heading.")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// In the agent's order; all answered in one submission.
    #[schemars(
        description = "One to ten questions, answered together in one submission. Each needs a                        stable `id`, a `prompt`, and a `type`: `text`, `choice`, `multi_choice` or                        `confirm`. A `choice`/`multi_choice` needs `options` of \
                       {\"value\", \"label\"}; `text` accepts `max_length`, `multiline` and \
                       `placeholder`; `multi_choice` accepts `min_selections`/`max_selections`; \
                       `confirm` accepts `yes_label`/`no_label`. Every question is required unless                        `optional` is true, and `default` counts as an answer."
    )]
    pub questions: Vec<crate::questions::model::Question>,

    /// The submit action's label.
    #[schemars(description = "The label of the submit action, where the surface allows it.")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_label: Option<String>,

    /// The dismiss action's label.
    #[schemars(description = "The label of the dismiss action, which declines the whole set.")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismiss_label: Option<String>,
}

impl AskQuestionInput {
    fn into_set(self) -> QuestionSet {
        QuestionSet {
            title: self.title,
            description: self.description,
            questions: self.questions,
            submit_label: self.submit_label,
            dismiss_label: self.dismiss_label,
        }
    }
}

pub struct AskQuestion {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    questions: Arc<Questions>,
}

impl AskQuestion {
    pub fn new(
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
        questions: Arc<Questions>,
    ) -> Self {
        Self {
            storage,
            transport,
            questions,
        }
    }
}

/// The description the agent reads (FR-009). It states the purpose, the types, the limits, that a
/// successful call ends the turn, the shape of the answer, and where the tool is unavailable; the
/// turn-ending notice is appended once by `with_ends_turn_notice`, not here.
fn description() -> String {
    format!(
        "Ask the person one or more structured questions and end your turn. Use it when you need \
         a decision, a detail or a confirmation you cannot infer, rather than guessing or asking \
         in prose — the answer comes back as structured data you can act on.\n\n\
         Types: `text` (free text, single- or multi-line), `choice` (one of `options`), \
         `multi_choice` (any of `options`, within `min_selections`/`max_selections`), and \
         `confirm` (yes/no). Ask up to {max_questions} questions at once; they are presented \
         together and answered in one submission, in the order you give them.\n\n\
         Every question is required unless you set `optional: true`; an optional question left \
         blank comes back as `skipped`. A `default` counts as an answer. Question ids are unique \
         within the set, 1–{max_id} bytes of lowercase letters, digits, `-` and `_`.\
         Limits: prompts and help ≤ {max_prompt} bytes, option labels ≤ {max_label}, text answers \
         ≤ {max_text}, and at most {max_options} options per choice.\n\n\
         On success you get `{{\"status\": \"pending\", \"question_set_id\": \"…\", \
         \"questions\": […]}}` and the questions become your reply; the person's answers arrive \
         as your next turn, naming the set and each question id with its value. If the call is \
         refused you get every problem at once and the turn continues, so ask *last* in a script: \
         a script that fails after asking withdraws the questions.\n\n\
         Not for credentials or any other secret: use `ask_sensitive_input` instead, which keeps \
         the value out of the conversation. Not available in an unattended turn (a scheduled run, \
         a dream, a background piece, an inter-agent turn or a side chat) — answer in text there.",
        max_questions = limits::MAX_QUESTIONS,
        max_id = limits::MAX_QUESTION_ID_BYTES,
        max_prompt = limits::MAX_PROMPT_BYTES,
        max_label = limits::MAX_LABEL_BYTES,
        max_text = limits::MAX_TEXT_ANSWER_BYTES,
        max_options = limits::MAX_OPTIONS,
    )
}

/// 8 characters of a uuid's hex: long enough that two sets in one conversation cannot collide,
/// short enough that a control id fits Telegram's 64-byte `callback_data`.
fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..limits::SHORT_ID_BYTES].to_string()
}

#[async_trait::async_trait]
impl VizierTool for AskQuestion {
    type Input = AskQuestionInput;
    type Output = QuestionAsked;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Questions)
    }

    fn name() -> String {
        "ask_question".into()
    }

    fn ends_turn() -> bool {
        true
    }

    /// Offered only where somebody can answer: a chat, voice or client-event turn in a WebUI
    /// topic, a project chat, or a Discord/Telegram conversation (FR-007, FR-028).
    fn available(&self, ctx: &ToolContext) -> bool {
        ctx.interactive
    }

    fn description(&self) -> String {
        description()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !ctx.interactive {
            return Err(VizierError(
                "nobody can answer here: this turn is unattended, so ask in your reply instead"
                    .into(),
            ));
        }
        // One set per turn (FR-008). A refused call does not use the turn's one up.
        if !ctx.questions.may_ask() {
            return Err(VizierError(format!(
                "this turn already asked a question set ({}); a turn may present one, so use the \
                 answers you have or ask again in your next turn",
                ctx.questions.asked().unwrap_or_default()
            )));
        }

        let set = args.into_set();
        // Everything wrong, at once: the agent gets one round to correct the call (FR-005).
        if let Err(problems) = validate::validate_set(&set) {
            return Err(VizierError(
                problems
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }

        let record = QuestionSetRecord {
            set_id: uuid::Uuid::new_v4().to_string(),
            short_id: short_id(),
            session: ctx.session.clone(),
            state: QuestionState::Pending,
            set,
            answers: None,
            answerer: None,
            controls: None,
            created_at: chrono::Utc::now(),
            closed_at: None,
        };

        // A channel posts through its own reader, and a failed post fails the call so the turn
        // continues and the agent knows the questions were never shown (FR-010). The WebUI needs
        // no surface: its card is the history entry below.
        let mut record = record;
        if let Some((_, surface)) = self.questions.surface_for(&ctx.session) {
            match surface.present(&record).await {
                Ok(controls) => {
                    self.storage
                        .set_question_controls(&record.set_id, &controls)
                        .await
                        .map_err(|e| VizierError(e.to_string()))?;
                    record.controls = Some(controls);
                }
                Err(e) => {
                    return Err(VizierError(format!(
                        "the questions could not be delivered, so nothing was asked: {e}"
                    )));
                }
            }
        }

        self.storage
            .create_question_set(&record)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        let asked = QuestionAsked::pending(record.set_id.clone(), &record.set);
        submit::record_set(&self.storage, &self.transport, &ctx.session, &record).await;
        ctx.questions.record_asked(&record.set_id);

        Ok(asked)
    }
}
