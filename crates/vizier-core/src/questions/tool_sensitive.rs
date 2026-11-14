//! `ask_sensitive_input`: the non-turn-ending tool that hands a script a value (FR-032–FR-040).
//!
//! It does **not** end the turn, because the value has to reach the turn that is already running:
//! that is the only turn whose report, history entry, model copy, log line and telemetry the
//! secret redactor scrubs. Handing the value to the next turn would put it in exactly the copy
//! that must stay clean.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    questions::{limits, model::SensitiveOutcome, sensitive},
    secrets::{MAX_VALUE_BYTES, MIN_VALUE_BYTES},
    storage::VizierStorage,
    transport::VizierTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AskSensitiveInputInput {
    /// Why the value is wanted, in the agent's own words. Shown to the person and not secret.
    #[schemars(
        description = "What you need the value for and why, in one or two sentences, so the \
                       person can judge whether to supply it. The person reads this; it is not \
                       secret."
    )]
    pub purpose: String,

    /// The field's name, e.g. "GitHub token".
    #[schemars(description = "A short name for what is being asked, e.g. \"GitHub token\".")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

pub struct AskSensitiveInput {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
}

impl AskSensitiveInput {
    pub fn new(storage: Arc<VizierStorage>, transport: VizierTransport) -> Self {
        Self { storage, transport }
    }
}

/// The description the agent reads (FR-040). It says where the value goes, that the agent will not
/// see it, that the call does not end the turn, and — plainly — what the guarantee does not cover.
fn description() -> String {
    format!(
        "Ask the person for a sensitive value, such as a credential or a token, and get it back \
         in this script only. The value goes to your script and nowhere else: you will not see it, \
         it is not written to the conversation or the history, and it is not replayed to you in a \
         later turn. It is armed with the turn's redaction, so where your report, the log or the \
         telemetry would have shown it they show [redacted].\n\n\
         Use it when another tool needs a value the person holds, then pass the value straight to \
         that tool. Do not ask for ordinary text with it: that is `ask_question`, and a masked \
         dialog for a project name only confuses the person. This tool does not store anything; to \
         keep a credential use `set_secret`.\n\n\
         This call does NOT end your turn: the turn keeps running while the person decides. The \
         person may reject the dialog, dismiss it, or never answer, so branch on the result: \
         status `provided` with `value` and `protected`, status `declined`, or status `timeout`. \
         The value is {min} to {max} bytes; a value shorter than {min} comes back with \
         `protected: false`, because redaction cannot arm it.\n\n\
         Redaction is exact-string only. A form you transform (base64, a hash, a split or joined \
         copy) is outside the guarantee, and so is anything you write yourself: a file, memory, a \
         scratch file, an artifact, an outbound body. A person can also type the value into an \
         ordinary message; that is not protected, so always ask through this tool rather than in \
         prose.\n\n\
         Only available in the WebUI, where a masked input can be shown. It is absent on Discord \
         and Telegram, whose servers would store the value, and in any unattended turn. It is not \
         an approval or permission gate.",
        min = MIN_VALUE_BYTES,
        max = MAX_VALUE_BYTES,
    )
}

#[async_trait::async_trait]
impl VizierTool for AskSensitiveInput {
    type Input = AskSensitiveInputInput;
    type Output = SensitiveOutcome;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Secrets)
    }

    fn name() -> String {
        "ask_sensitive_input".into()
    }

    /// Never: the script waits for the value inside the same turn (FR-033).
    fn ends_turn() -> bool {
        false
    }

    /// Only where a masked input can be shown to the person: a WebUI topic or a project chat, in
    /// a turn somebody is waiting on (FR-046, FR-055).
    fn available(&self, ctx: &ToolContext) -> bool {
        ctx.interactive && ctx.session.1.web_user().is_some()
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
                "nobody can answer here: this turn is unattended, so explain what you need in \
                 your reply instead"
                    .into(),
            ));
        }
        if ctx.session.1.web_user().is_none() {
            return Err(VizierError(
                "this conversation is on a channel whose servers would store the value; a secure \
                 input is only available in the WebUI, so ask the person to supply it another way"
                    .into(),
            ));
        }
        let purpose = args.purpose.trim();
        if purpose.is_empty() {
            return Err(VizierError(
                "`purpose` is required: say what you need the value for, so the person can judge \
                 whether to supply it"
                    .into(),
            ));
        }
        if purpose.len() > limits::MAX_ASK_PURPOSE_BYTES {
            return Err(VizierError(format!(
                "`purpose` is {} bytes; the limit is {}",
                purpose.len(),
                limits::MAX_ASK_PURPOSE_BYTES
            )));
        }
        if let Some(label) = &args.label
            && label.len() > limits::MAX_LABEL_BYTES
        {
            return Err(VizierError(format!(
                "`label` is {} bytes; the limit is {}",
                label.len(),
                limits::MAX_LABEL_BYTES
            )));
        }

        sensitive::ask(
            &self.storage,
            &self.transport,
            ctx.questions.service(),
            &ctx.secret_redactor,
            &ctx.session,
            purpose,
            args.label.as_deref(),
            ctx.deadline,
        )
        .await
        .map_err(|e| VizierError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        schema::{VizierChannelId, VizierSession},
        storage::{document::BlobDocumentStore, sqlite::SqlStorage},
    };

    fn tool() -> (AskSensitiveInput, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_question_schema(&conn).unwrap();
        crate::storage::sqlite::init_sensitive_ask_schema(&conn).unwrap();
        let storage = SqlStorage::new(
            crate::storage::sql::SqlStore::sqlite(conn),
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (
            AskSensitiveInput::new(
                Arc::new(VizierStorage::new(storage)),
                VizierTransport::new(),
            ),
            dir,
        )
    }

    fn ctx(channel: VizierChannelId, interactive: bool) -> ToolContext {
        ToolContext {
            session: VizierSession("agent-1".into(), channel, Some("t".into())),
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

    fn web() -> VizierChannelId {
        VizierChannelId::HTTP("alice".into(), "webui".into())
    }

    fn input() -> AskSensitiveInputInput {
        AskSensitiveInputInput {
            purpose: "to push the branch".into(),
            label: None,
        }
    }

    /// What the agent reads has to say where the value goes and what the guarantee does not cover
    /// (FR-040, FR-057, FR-058).
    #[test]
    fn the_description_states_the_guarantee_and_its_limits() {
        let d = description();
        for needle in [
            // where the value goes
            "this script only",
            "you will not see it",
            "[redacted]",
            // it keeps the turn running, and the result shapes
            "does NOT end your turn",
            "`provided`",
            "`declined`",
            "`timeout`",
            "reject the dialog",
            // the length limits
            &format!("{} to {} bytes", MIN_VALUE_BYTES, MAX_VALUE_BYTES),
            "`protected: false`",
            // exact-string only, and what is outside it (FR-057)
            "exact-string only",
            "base64",
            "a hash",
            "a file, memory, a scratch file, an artifact, an outbound body",
            // a person can still type it into a message (FR-058)
            "ordinary message",
            "rather than in prose",
            // availability
            "Only available in the WebUI",
            "Discord",
            "Telegram",
            "unattended",
            // pointers to the neighbouring tools
            "ask_question",
            "set_secret",
        ] {
            // The sentence may wrap in the source, so compare on collapsed whitespace.
            let flat = d.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                flat.contains(needle),
                "the description lacks {needle:?}:\n{d}"
            );
        }
    }

    /// It is a plain tool: the turn goes on while the person decides (FR-033).
    #[test]
    fn the_call_does_not_end_the_turn_and_sits_beside_the_secret_tools() {
        assert!(!AskSensitiveInput::ends_turn());
        assert_eq!(AskSensitiveInput::category(), Some(ToolCategory::Secrets));
        assert_eq!(AskSensitiveInput::name(), "ask_sensitive_input");
    }

    /// Offered in the WebUI and in a project chat while somebody is waiting, and nowhere else
    /// (FR-046, SC-017).
    #[test]
    fn it_is_offered_only_where_a_masked_input_can_be_shown() {
        let (tool, _dir) = tool();
        assert!(tool.available(&ctx(web(), true)));
        assert!(tool.available(&ctx(
            VizierChannelId::Project("alice".into(), "p1".into()),
            true
        )));

        // A channel whose servers would keep the value.
        assert!(!tool.available(&ctx(VizierChannelId::DiscordChanel(1), true)));
        assert!(!tool.available(&ctx(VizierChannelId::TelegramChannel(1), true)));
        // An unattended turn, even in a WebUI conversation.
        assert!(!tool.available(&ctx(web(), false)));
        assert!(!tool.available(&ctx(VizierChannelId::Subagent, false)));
    }

    /// A direct call where the tool is not offered says why and fails the call, not the turn
    /// (FR-047).
    #[tokio::test]
    async fn a_call_where_it_is_not_offered_names_the_reason() {
        let (tool, _dir) = tool();

        let err = tool
            .call(input(), &ctx(VizierChannelId::DiscordChanel(1), true))
            .await
            .unwrap_err();
        assert!(err.0.contains("WebUI"), "{}", err.0);

        let err = tool.call(input(), &ctx(web(), false)).await.unwrap_err();
        assert!(err.0.contains("unattended"), "{}", err.0);
    }

    /// A call that is malformed is refused before any dialog opens.
    #[tokio::test]
    async fn a_missing_purpose_is_refused_before_any_dialog_opens() {
        let (tool, _dir) = tool();
        let ctx = ctx(web(), true);
        let err = tool
            .call(
                AskSensitiveInputInput {
                    purpose: "  ".into(),
                    label: None,
                },
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(err.0.contains("purpose"), "{}", err.0);
        assert!(ctx.questions.service().open_asks(&ctx.session).is_empty());
    }
}
