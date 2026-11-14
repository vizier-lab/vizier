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
        Questions, limits,
        model::{QuestionAsked, QuestionSet, QuestionSetRecord, QuestionState},
        submit, validate,
    },
    schema::{VizierChannelId, VizierSession},
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
         within the set, 1–{max_id} bytes of lowercase letters, digits, `-` and `_`. \
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

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
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
            created_at: crate::time::now(),
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        agents::tools::{ENDS_TURN_NOTICE, with_ends_turn_notice},
        storage::{document::BlobDocumentStore, sqlite::SqlStorage},
    };

    fn tool() -> (AskQuestion, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_question_schema(&conn).unwrap();
        let storage = SqlStorage::new(
            crate::storage::sql::SqlStore::sqlite(conn),
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (
            AskQuestion::new(
                Arc::new(VizierStorage::new(storage)),
                VizierTransport::new(),
                Arc::new(Questions::new()),
            ),
            dir,
        )
    }

    fn ctx(interactive: bool) -> ToolContext {
        ToolContext {
            session: VizierSession(
                "agent-1".into(),
                VizierChannelId::HTTP("alice".into(), "webui".into()),
                Some("t".into()),
            ),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            subagent_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            questions: crate::questions::QuestionsHandle::new(Default::default()),
            interactive,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
            project: None,
            detached: false,
            parent_detached_id: None,
        }
    }

    fn one_question() -> AskQuestionInput {
        serde_json::from_value(serde_json::json!({
            "questions": [{"id": "name", "prompt": "Name?", "type": "text"}]
        }))
        .unwrap()
    }

    /// The agent reads this and nothing else, so it has to carry the whole contract (FR-009, FR-066).
    #[test]
    fn the_description_states_the_whole_contract() {
        let d = description();
        for needle in [
            "structured questions",
            "`text`",
            "`choice`",
            "`multi_choice`",
            "`confirm`",
            "up to 10 questions",
            "end your turn",
            "next turn",
            "`skipped`",
            "`default`",
            "unattended",
            "ask_sensitive_input",
        ] {
            assert!(d.contains(needle), "the description lacks {needle:?}:\n{d}");
        }
        // The limits are the constants, not copies of them.
        assert!(d.contains(&limits::MAX_TEXT_ANSWER_BYTES.to_string()));
        assert!(d.contains(&limits::MAX_OPTIONS.to_string()));
        // A sentence boundary is not run together.
        assert!(!d.contains("`_`.Limits"));
    }

    /// The notice is added where the definition is built, once, and is not part of the budget the
    /// author's own text is held to (FR-009, US6 #2).
    #[test]
    fn the_turn_ending_notice_is_appended_exactly_once() {
        let own = description();
        assert!(
            !own.contains(ENDS_TURN_NOTICE),
            "the author's text must not carry it"
        );
        let shown = with_ends_turn_notice(own.clone(), AskQuestion::ends_turn());
        assert_eq!(shown.matches(ENDS_TURN_NOTICE).count(), 1);
        // Applying it again changes nothing.
        assert_eq!(with_ends_turn_notice(shown.clone(), true), shown);
        assert!(AskQuestion::ends_turn());
    }

    /// Where nobody can answer the tool is not offered, and a direct call says so and fails — the
    /// turn continues (FR-007, FR-063).
    #[tokio::test]
    async fn an_unattended_turn_is_refused_without_ending_it() {
        let (tool, _dir) = tool();
        let unattended = ctx(false);
        assert!(!tool.available(&unattended));
        let err = tool.call(one_question(), &unattended).await.unwrap_err();
        assert!(err.0.contains("unattended"), "{}", err.0);
        // Nothing was recorded for the turn, so it may still ask in a later, attended turn.
        assert!(unattended.questions.may_ask());
        assert!(tool.available(&ctx(true)));
    }

    /// A refused set uses up nothing: the turn may correct the call (FR-005, FR-008).
    #[tokio::test]
    async fn an_invalid_set_is_refused_and_the_turns_one_set_is_kept() {
        let (tool, _dir) = tool();
        let attended = ctx(true);
        let empty: AskQuestionInput =
            serde_json::from_value(serde_json::json!({ "questions": [] })).unwrap();
        assert!(tool.call(empty, &attended).await.is_err());
        assert!(attended.questions.may_ask());
    }
}
