//! The shapes a question set and a sensitive ask take (`data-model.md` §1–§8).
//!
//! One module because the model is shared by everything that touches a set: the tool that asks,
//! the validator, both channel surfaces, the text form, the storage layer and the WebUI's
//! generated schema. The JSON names here are the wire contract
//! (`contracts/agent-tools.md`, `contracts/questions-wire.md`), so they are part of the public
//! surface of the feature and renaming one is a breaking change.

use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::schema::VizierSession;

/// What kind of answer a question wants (FR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    /// Free text, single- or multi-line.
    Text,
    /// Exactly one of the offered options.
    Choice,
    /// Any of the offered options, within the question's bounds.
    MultiChoice,
    /// A yes or a no.
    Confirm,
}

impl QuestionType {
    /// How the type is named in a refusal, a description and the text form.
    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Choice => "choice",
            Self::MultiChoice => "multi_choice",
            Self::Confirm => "confirm",
        }
    }
}

/// One value a person can give, or one a question offers as its default.
///
/// Untagged because the three shapes cannot be confused in JSON — a string, a list of strings and
/// a boolean — so the wire form is the natural one (`"sqlite"`, `["auth", "ci"]`, `true`) rather
/// than a tagged wrapper the agent would have to learn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum AnswerValue {
    Text(String),
    Choices(Vec<String>),
    Confirm(bool),
}

impl AnswerValue {
    /// The value as the text a person reads back, for a summary or a record.
    pub fn display(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Choices(values) => values.join(", "),
            Self::Confirm(true) => "yes".to_string(),
            Self::Confirm(false) => "no".to_string(),
        }
    }
}

/// One choice a `choice` or `multi_choice` question offers (FR-003).
///
/// Named `ChoiceOption` rather than `Option` so it does not shadow the prelude's `Option` in
/// every signature that mentions both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChoiceOption {
    /// What the agent receives when this option is chosen. Never travels inside a control id.
    pub value: String,
    /// What the person reads.
    pub label: String,
    /// A line of explanation under the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One question in a set (FR-002, FR-003).
///
/// The type-specific fields are flat, as the wire contract states them, and
/// [`crate::questions::validate`] is the one place that decides which apply to which type: a
/// `choice` with no `options` is refused, and `max_length` on a `choice` is ignored rather than
/// rejected, so an agent that sends a superset still gets a usable question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Question {
    /// Stable within the set; what the answer names.
    pub id: String,
    /// The question itself, shown as the field's label.
    pub prompt: String,
    /// The kind of answer wanted.
    #[serde(rename = "type")]
    pub kind: QuestionType,
    /// Shown under the prompt, for a question whose point is not obvious.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Required by default; an optional question may be left blank and arrives as `skipped`.
    #[serde(default)]
    pub optional: bool,
    /// Counts as an answer, so a required question with a default may be submitted untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<AnswerValue>,

    /// `choice`, `multi_choice`: the choices offered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ChoiceOption>,
    /// `text`: a hint shown in an empty field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// `text`: the longest answer accepted, in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    /// `text`: a roomier field, for an answer that is a sentence or more.
    #[serde(default)]
    pub multiline: bool,

    /// `multi_choice`: the fewest answers accepted. Defaults to none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_selections: Option<usize>,
    /// `multi_choice`: the most answers accepted. Defaults to every option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_selections: Option<usize>,

    /// `confirm`: the label of the affirmative button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_label: Option<String>,
    /// `confirm`: the label of the negative button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_label: Option<String>,
}

impl Question {
    /// The choices this question offers, whichever type it is.
    pub fn options(&self) -> &[ChoiceOption] {
        &self.options
    }

    /// The option whose value is `value`, if the question offers one.
    pub fn option(&self, value: &str) -> Option<&ChoiceOption> {
        self.options.iter().find(|option| option.value == value)
    }

    /// The longest text this question accepts.
    pub fn text_limit(&self) -> usize {
        self.max_length
            .unwrap_or(crate::questions::limits::MAX_TEXT_ANSWER_BYTES)
            .min(crate::questions::limits::MAX_TEXT_ANSWER_BYTES)
    }

    /// How many choices the person must pick, for a `multi_choice`.
    pub fn selection_bounds(&self) -> (usize, usize) {
        let min = self.min_selections.unwrap_or(0);
        let max = self
            .max_selections
            .unwrap_or(self.options.len())
            .min(self.options.len());
        (min, max)
    }
}


/// What the agent asks: the tool's whole input, and the set as it is stored (FR-002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct QuestionSet {
    /// A heading over the card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A sentence of context, under the heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// In the agent's order; every question is answered in one submission.
    pub questions: Vec<Question>,
    /// The submit action's label, where the surface lets it be chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_label: Option<String>,
    /// The dismiss action's label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismiss_label: Option<String>,
}

impl QuestionSet {
    /// The question with this id, if the set has one.
    pub fn question(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|question| question.id == id)
    }

    /// A question's position in the set. Controls address a question by this, never by its id:
    /// an id may be long enough not to fit Telegram's 64-byte `callback_data`.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.questions.iter().position(|question| question.id == id)
    }
}

/// Whether a question was answered or deliberately left blank (FR-013, US2 #3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AnswerState {
    Answered,
    /// Optional and left blank: the agent is told so rather than being left to guess.
    Skipped,
}

/// One question's part of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Answer {
    pub question_id: String,
    pub state: AnswerState,
    /// Absent for a skipped question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<AnswerValue>,
    /// The chosen option's label, filled at submission so the agent and the record show what the
    /// person read rather than only the value it stands for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Answer {
    /// A question that was left blank.
    pub fn skipped(question_id: impl Into<String>) -> Self {
        Self {
            question_id: question_id.into(),
            state: AnswerState::Skipped,
            value: None,
            label: None,
        }
    }
}

/// Where a set stands (data-model.md §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum QuestionState {
    /// Shown and answerable.
    Pending,
    /// Answered; the answer is the person's next turn.
    Answered,
    /// Dismissed by the person, with no answers.
    Declined,
    /// The script that asked failed after asking, so the set was taken away (FR-010).
    Withdrawn,
}

impl QuestionState {
    /// Whether a submission or a dismissal may still act on the set.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Pending)
    }

    /// How the state is stored and shown.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Answered => "answered",
            Self::Declined => "declined",
            Self::Withdrawn => "withdrawn",
        }
    }

    /// Read a stored state back. An unknown value is treated as closed rather than pending, so a
    /// row written by a newer build cannot be answered by this one.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "answered" => Self::Answered,
            "declined" => Self::Declined,
            _ => Self::Withdrawn,
        }
    }
}

/// Where one platform message presents a set, so it can be edited or stripped later (FR-024).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ControlMessage {
    /// The platform's conversation id.
    pub chat_id: String,
    /// The platform's message id.
    pub message_id: String,
    /// Which question this message presents, when the platform needs one message per question
    /// (FR-029); `None` when one message holds the whole set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<usize>,
}

/// A set's platform side: which messages carry it, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Controls {
    /// The platform that posted them, e.g. `discord`.
    pub platform: String,
    pub messages: Vec<ControlMessage>,
}

/// A stored question set: the ask, plus everything that changes after it (`data-model.md` §1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionSetRecord {
    /// The set's id, which is also its history entry's uid.
    pub set_id: String,
    /// The short reference a platform control carries.
    pub short_id: String,
    /// The conversation the set belongs to.
    pub session: VizierSession,
    pub state: QuestionState,
    /// Immutable once written.
    pub set: QuestionSet,
    /// Filled once, when the set is answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<Vec<Answer>>,
    /// Who answered or dismissed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answerer: Option<String>,
    /// Where the platform messages are, once posted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<Controls>,
    pub created_at: DateTime<Local>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Local>>,
}


/// Where a sensitive ask stands (data-model.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AskState {
    /// The dialog is up and the script is waiting.
    Open,
    /// The person supplied a value. What it was is never recorded.
    Provided,
    /// The person rejected or dismissed the dialog.
    Declined,
    /// Nobody answered before the wait limit.
    TimedOut,
    /// The turn ended, was aborted, or the process restarted with the ask still open.
    Abandoned,
}

impl AskState {
    pub fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Provided => "provided",
            Self::Declined => "declined",
            Self::TimedOut => "timed_out",
            Self::Abandoned => "abandoned",
        }
    }

    /// Read a stored state back; an unknown value is reported as abandoned, i.e. closed.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "provided" => Self::Provided,
            "declined" => Self::Declined,
            "timed_out" => Self::TimedOut,
            "open" => Self::Open,
            _ => Self::Abandoned,
        }
    }
}

/// A stored sensitive ask. It holds **no value**: only that one was wanted, and what came of it
/// (FR-044, FR-054).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveAskRecord {
    pub ask_id: String,
    /// The agent's own reason, shown to the person. Not secret (FR-034).
    pub purpose: String,
    /// The field's name, e.g. "GitHub token".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub session: VizierSession,
    pub state: AskState,
    pub requested_at: DateTime<Local>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Local>>,
}

/// How a submission of two races ended (data-model.md §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// This submission took the set; the answer is recorded.
    Won,
    /// Somebody else closed it first, or the script withdrew it. Nothing was written.
    AlreadyClosed,
}

impl Claim {
    pub fn won(self) -> bool {
        matches!(self, Self::Won)
    }
}

/// What the running script receives from `ask_sensitive_input` (data-model.md §8).
///
/// The value lives only in this struct, only until the script has it. `Debug` is implemented by
/// hand to leave it out, so a stray `{:?}` in a log line or a panic message cannot print it
/// (FR-041).
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SensitiveOutcome {
    /// The person supplied a value.
    Provided {
        value: String,
        /// `false` when the value is too short for the redactor to arm, so it could not be
        /// scrubbed from the report (FR-043).
        protected: bool,
    },
    /// The person rejected the dialog or dismissed it (FR-050).
    Declined,
    /// Nobody answered before the wait limit (FR-051).
    Timeout,
}

impl SensitiveOutcome {
    /// The value, when there is one.
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Provided { value, .. } => Some(value),
            _ => None,
        }
    }
}

impl std::fmt::Debug for SensitiveOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provided { protected, .. } => f
                .debug_struct("Provided")
                .field("value", &"[redacted]")
                .field("protected", protected)
                .finish(),
            Self::Declined => f.write_str("Declined"),
            Self::Timeout => f.write_str("Timeout"),
        }
    }
}

/// What `ask_question` returns to the script (contracts/agent-tools.md §1).
///
/// It says what was asked and nothing else: the answers arrive as the person's next turn.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct QuestionAsked {
    /// Always `pending`: a set is either presented or refused, and a refusal is an error rather
    /// than an output. A `String` rather than `&'static str` so the type can round-trip through
    /// serde like every other tool output.
    pub status: String,
    pub question_set_id: String,
    /// The questions, in order, so a script can log what it asked without rebuilding it.
    pub questions: Vec<AskedQuestion>,
}

impl QuestionAsked {
    pub fn pending(set_id: impl Into<String>, set: &QuestionSet) -> Self {
        Self {
            status: "pending".to_string(),
            question_set_id: set_id.into(),
            questions: set
                .questions
                .iter()
                .map(|question| AskedQuestion {
                    id: question.id.clone(),
                    kind: question.kind,
                })
                .collect(),
        }
    }
}

/// One question in [`QuestionAsked`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AskedQuestion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: QuestionType,
}

/// The person's answer, carried as a turn of its own (`contracts/questions-wire.md` §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct QuestionAnswer {
    /// The set being answered.
    pub set_id: String,
    /// What the conversation shows as the person's message. Built at submission when the client
    /// does not supply one.
    #[serde(default)]
    pub text: String,
    /// One entry per question, in the set's order.
    #[serde(default)]
    pub answers: Vec<Answer>,
    /// Who submitted it, as the surface identifies them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answerer: Option<String>,
    /// The person dismissed the set instead of answering it (FR-014).
    #[serde(default)]
    pub declined: bool,
}

impl QuestionAnswer {
    /// The one line the conversation shows for this answer, when the surface did not supply one.
    ///
    /// Names the questions in order with what was given, so the person reading the transcript and
    /// the agent reading its history see the same thing.
    pub fn summary(set: &QuestionSet, answers: &[Answer], declined: bool) -> String {
        if declined {
            return "Dismissed the questions.".to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        for answer in answers {
            let Some(question) = set.question(&answer.question_id) else {
                continue;
            };
            let shown = match (&answer.state, &answer.value) {
                (AnswerState::Answered, Some(value)) => answer
                    .label
                    .clone()
                    .unwrap_or_else(|| value.display()),
                (AnswerState::Answered, None) => continue,
                (AnswerState::Skipped, _) => "skipped".to_string(),
            };
            parts.push(format!("{}: {shown}", question.prompt));
        }
        if parts.is_empty() {
            "Answered the questions.".to_string()
        } else {
            parts.join(" · ")
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set() -> QuestionSet {
        QuestionSet {
            title: Some("Project setup".into()),
            description: None,
            submit_label: None,
            dismiss_label: None,
            questions: vec![
                Question {
                    id: "name".into(),
                    prompt: "Project name".into(),
                    kind: QuestionType::Text,
                    help: None,
                    optional: false,
                    default: None,
                    options: vec![],
                    placeholder: Some("my-service".into()),
                    max_length: Some(60),
                    multiline: false,
                    min_selections: None,
                    max_selections: None,
                    yes_label: None,
                    no_label: None,
                },
                Question {
                    id: "db".into(),
                    prompt: "Database".into(),
                    kind: QuestionType::Choice,
                    help: None,
                    optional: false,
                    default: Some(AnswerValue::Text("sqlite".into())),
                    options: vec![
                        ChoiceOption {
                            value: "postgres".into(),
                            label: "PostgreSQL".into(),
                            description: None,
                        },
                        ChoiceOption {
                            value: "sqlite".into(),
                            label: "SQLite".into(),
                            description: Some("One file, no server".into()),
                        },
                    ],
                    placeholder: None,
                    max_length: None,
                    multiline: false,
                    min_selections: None,
                    max_selections: None,
                    yes_label: None,
                    no_label: None,
                },
            ],
        }
    }

    /// The wire form is the contract's: `type` on a question, `value`/`label` on an option, and
    /// no `question_type` anywhere. A provider-independent schema is generated from this struct,
    /// so the names here are what every model sees.
    #[test]
    fn a_set_serialises_the_way_the_contract_reads() {
        let value = serde_json::to_value(set()).unwrap();
        assert_eq!(value["title"], "Project setup");
        assert_eq!(value["questions"][0]["type"], "text");
        assert_eq!(value["questions"][0]["max_length"], 60);
        assert_eq!(value["questions"][1]["type"], "choice");
        assert_eq!(value["questions"][1]["default"], "sqlite");
        assert_eq!(value["questions"][1]["options"][1]["value"], "sqlite");
        // Absent rather than null, so a model writing the schema by hand does not have to guess.
        assert!(value["questions"][0].get("default").is_none());
        assert!(value.get("description").is_none());
    }

    /// The generated schema keeps `type` as a *property* of a question. If someone renames the
    /// field to `question_type`, this fails instead of the contract silently drifting.
    #[test]
    fn the_generated_schema_names_the_question_type_property_type() {
        let schema = serde_json::to_value(schemars::schema_for!(Question)).unwrap();
        let properties = &schema["properties"];
        assert!(
            properties.get("type").is_some(),
            "a question's type must stay a `type` property: {schema}"
        );
        assert!(properties.get("question_type").is_none());
    }

    /// A value is one of three JSON shapes and reads back as the same one.
    #[test]
    fn an_answer_value_is_untagged_but_unambiguous() {
        for (value, expected) in [
            (json!("sqlite"), AnswerValue::Text("sqlite".into())),
            (
                json!(["auth", "ci"]),
                AnswerValue::Choices(vec!["auth".into(), "ci".into()]),
            ),
            (json!(true), AnswerValue::Confirm(true)),
        ] {
            let parsed: AnswerValue = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(serde_json::to_value(&parsed).unwrap(), value);
        }
    }

    /// A question is addressed by its position, never by its id — a control cannot carry a
    /// 64-byte id.
    #[test]
    fn a_question_is_found_by_id_and_by_position() {
        let set = set();
        assert_eq!(set.position("db"), Some(1));
        assert_eq!(set.question("db").unwrap().prompt, "Database");
        assert_eq!(set.position("nope"), None);
    }

    /// An unknown stored state is closed, so a row written by a newer build cannot be answered.
    #[test]
    fn an_unknown_state_reads_back_as_closed() {
        assert_eq!(QuestionState::from_stored("pending"), QuestionState::Pending);
        assert!(QuestionState::from_stored("pending").is_open());
        for closed in ["answered", "declined", "withdrawn", "some-future-state"] {
            assert!(!QuestionState::from_stored(closed).is_open(), "{closed}");
        }
        assert_eq!(AskState::from_stored("timed_out"), AskState::TimedOut);
        for closed in ["provided", "declined", "abandoned", "some-future-state"] {
            assert!(!AskState::from_stored(closed).is_open(), "{closed}");
        }
    }

    /// The value must never be printable by accident: a log line or a panic message carrying
    /// `{:?}` of the outcome shows the marker instead (FR-041).
    #[test]
    fn the_outcome_never_prints_its_value() {
        let outcome = SensitiveOutcome::Provided {
            value: "ghp_verysecret123".into(),
            protected: true,
        };
        let printed = format!("{outcome:?}");
        assert!(!printed.contains("ghp_verysecret123"), "{printed}");
        assert!(printed.contains("[redacted]"), "{printed}");
        assert_eq!(outcome.value(), Some("ghp_verysecret123"));
    }

    /// The summary is what a person sees in the transcript and the agent reads in its history.
    #[test]
    fn a_summary_names_each_question_with_what_was_given() {
        let set = set();
        let answers = vec![
            Answer {
                question_id: "name".into(),
                state: AnswerState::Answered,
                value: Some(AnswerValue::Text("my-service".into())),
                label: None,
            },
            Answer {
                question_id: "db".into(),
                state: AnswerState::Answered,
                value: Some(AnswerValue::Text("sqlite".into())),
                label: Some("SQLite".into()),
            },
        ];
        assert_eq!(
            QuestionAnswer::summary(&set, &answers, false),
            "Project name: my-service · Database: SQLite"
        );
        // A skipped optional question says so rather than disappearing.
        let skipped = vec![Answer::skipped("name")];
        assert_eq!(
            QuestionAnswer::summary(&set, &skipped, false),
            "Project name: skipped"
        );
        assert_eq!(
            QuestionAnswer::summary(&set, &[], true),
            "Dismissed the questions."
        );
    }
}
