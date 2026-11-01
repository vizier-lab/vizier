//! The one path an answer takes, whatever surface it came from (research R7).
//!
//! A submission is validated against the stored set, claimed atomically, and turned into an
//! ordinary request. Every surface calls this: the WebUI through its socket, a Discord or
//! Telegram press through the reader. That is what keeps the rules in one place and gives the
//! answer the same history, queue and attribution as a typed message (FR-015) — the caller sends
//! the returned request exactly as it sends a chat one.

use chrono::Utc;

use crate::{
    questions::{
        Questions,
        model::{Claim, QuestionAnswer, QuestionSetRecord, QuestionState},
        validate::{self, Problem},
    },
    schema::{SessionHistoryContent, VizierRequest, VizierRequestContent, VizierSession},
    storage::{VizierStorage, question::QuestionStorage},
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

/// Why an answer was not accepted. Every variant is a refusal the person can be told about and
/// none of them starts a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitError {
    /// No such set in this conversation. A set from another conversation reads as missing rather
    /// than as forbidden, so a guessed id reveals nothing (FR-018).
    NotFound,
    /// The set was already answered, declined or withdrawn (FR-017).
    AlreadyClosed(QuestionState),
    /// The answer broke a rule; the surface names the question and the problem (FR-012).
    Invalid(Vec<Problem>),
    /// Saving the answer or queueing the turn failed.
    Failed(String),
}

impl SubmitError {
    /// A stable key for a `client_error` frame or an ephemeral notice.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "question_not_found",
            Self::AlreadyClosed(_) => "question_not_open",
            Self::Invalid(_) => "invalid_answer",
            Self::Failed(_) => "question_not_recorded",
        }
    }

    /// One line a person reads.
    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "This question is not part of this conversation.".to_string(),
            Self::AlreadyClosed(QuestionState::Answered) => {
                "This was already answered.".to_string()
            }
            Self::AlreadyClosed(QuestionState::Declined) => "This was dismissed.".to_string(),
            Self::AlreadyClosed(QuestionState::Withdrawn) => {
                "This question was withdrawn.".to_string()
            }
            Self::AlreadyClosed(QuestionState::Pending) => "This cannot be answered now.".to_string(),
            Self::Invalid(problems) => problems
                .iter()
                .map(Problem::to_string)
                .collect::<Vec<_>>()
                .join("; "),
            Self::Failed(reason) => reason.clone(),
        }
    }

    /// The problems, when the refusal was one.
    pub fn problems(&self) -> &[Problem] {
        match self {
            Self::Invalid(problems) => problems,
            _ => &[],
        }
    }
}

/// Record `answer` against its set and return the request that carries it.
///
/// The returned request is the caller's to send, so a WebUI socket queues it exactly as it queues
/// a typed message and a channel reader renders it the way it renders any turn.
pub async fn submit(
    storage: &VizierStorage,
    transport: &VizierTransport,
    questions: &Questions,
    session: &VizierSession,
    answer: QuestionAnswer,
    answerer: &str,
) -> Result<VizierRequest, SubmitError> {
    let record = storage
        .get_question_set(&answer.set_id)
        .await
        .map_err(|e| SubmitError::Failed(e.to_string()))?
        .filter(|record| record.session == *session)
        .ok_or(SubmitError::NotFound)?;

    if !record.state.is_open() {
        return Err(SubmitError::AlreadyClosed(record.state));
    }

    let claim = if answer.declined {
        // A dismissal is a legitimate answer to a question: the agent is told the set was
        // declined and may ask something else (FR-014).
        storage
            .decline_question_set(&answer.set_id, session, answerer)
            .await
            .map_err(|e| SubmitError::Failed(e.to_string()))?
    } else {
        let answers = validate::normalise(answer.answers.clone());
        validate::validate_answer(&record.set, &answers).map_err(SubmitError::Invalid)?;
        storage
            .claim_question_set(&answer.set_id, session, &answers, answerer)
            .await
            .map_err(|e| SubmitError::Failed(e.to_string()))?
    };

    if claim != Claim::Won {
        // Somebody else closed it between the read and the write: refuse, and say which way it
        // went rather than "already answered" for a set that was actually dismissed.
        let now = storage
            .get_question_set(&answer.set_id)
            .await
            .ok()
            .flatten()
            .map(|record| record.state)
            .unwrap_or(QuestionState::Withdrawn);
        return Err(SubmitError::AlreadyClosed(now));
    }

    // The card closes for every socket on the conversation, whether or not this one is watching.
    let closed = storage.get_question_set(&answer.set_id).await.ok().flatten();
    transport.publish_session_event(SessionEvent {
        session: session.clone(),
        frame: SessionFrame::QuestionSet {
            set_id: answer.set_id.clone(),
            state: closed.as_ref().map(|r| r.state).unwrap_or(QuestionState::Answered),
            answers: closed.as_ref().and_then(|r| r.answers.clone()),
            answerer: Some(answerer.to_string()),
        },
    });
    questions.clear_draft(&answer.set_id);
    close_controls(questions, closed.as_ref()).await;

    Ok(request_for(&record, answer, answerer))
}

/// Strip or edit the platform messages of a closed set (FR-024). Best effort: the person has
/// already answered, so a failed edit is logged and must not undo the answer.
async fn close_controls(questions: &Questions, record: Option<&QuestionSetRecord>) {
    let Some(record) = record else { return };
    let Some(controls) = &record.controls else { return };
    // With the reader gone there is nothing to edit, and the set is still answered.
    let Some((_, surface)) = questions.surface_for(&record.session) else {
        return;
    };
    if let Err(e) = surface.close(record, controls).await {
        tracing::warn!(
            set_id = %record.set_id,
            "could not close the platform controls of a question set: {e}"
        );
    }
}

/// The turn an answer starts: the person's summary text, the structured answers behind it.
fn request_for(
    record: &QuestionSetRecord,
    answer: QuestionAnswer,
    answerer: &str,
) -> VizierRequest {
    let answers = validate::normalise(answer.answers);
    let text = if answer.text.trim().is_empty() {
        QuestionAnswer::summary(&record.set, &answers, answer.declined)
    } else {
        answer.text
    };
    VizierRequest {
        timestamp: Utc::now(),
        user: answerer.to_string(),
        content: VizierRequestContent::QuestionAnswer(QuestionAnswer {
            set_id: answer.set_id,
            text,
            answers,
            answerer: Some(answerer.to_string()),
            declined: answer.declined,
        }),
        metadata: serde_json::json!({ "via": "question_answer" }),
        ..Default::default()
    }
}

/// Save the set's history entry, which is what the WebUI draws the card from.
pub async fn record_set(
    storage: &VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    record: &QuestionSetRecord,
) -> Option<String> {
    crate::agents::steps::record_step(
        storage,
        transport,
        session,
        SessionHistoryContent::QuestionSet {
            set_id: record.set_id.clone(),
            title: record.set.title.clone(),
            description: record.set.description.clone(),
            questions: record.set.questions.clone(),
            submit_label: record.set.submit_label.clone(),
            dismiss_label: record.set.dismiss_label.clone(),
            state: record.state,
            answers: record.answers.clone(),
            answerer: record.answerer.clone(),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{
        questions::model::{
            Answer, AnswerState, AnswerValue, ChoiceOption, Question, QuestionSet, QuestionType,
        },
        schema::VizierChannelId,
        storage::{document::LocalDocumentStore, sqlite::SqliteStorage},
    };

    fn session(topic: &str) -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some(topic.into()),
        )
    }

    fn question(id: &str, kind: QuestionType) -> Question {
        Question {
            id: id.into(),
            prompt: format!("Prompt {id}"),
            kind,
            help: None,
            optional: false,
            default: None,
            options: if kind == QuestionType::Choice {
                vec![ChoiceOption { value: "sqlite".into(), label: "SQLite".into(), description: None }]
            } else {
                vec![]
            },
            placeholder: None,
            max_length: None,
            multiline: false,
            min_selections: None,
            max_selections: None,
            yes_label: None,
            no_label: None,
        }
    }

    struct Fixture {
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
        questions: Questions,
        _dir: tempfile::TempDir,
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_question_schema(&conn).unwrap();
        let storage = Arc::new(VizierStorage::new(SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        )));
        let f = Fixture {
            storage,
            transport: VizierTransport::new(),
            questions: Questions::new(),
            _dir: dir,
        };
        let record = QuestionSetRecord {
            set_id: "set-1".into(),
            short_id: "7f3a91c2".into(),
            session: session("t"),
            state: QuestionState::Pending,
            set: QuestionSet {
                title: None,
                description: None,
                questions: vec![question("name", QuestionType::Text), question("db", QuestionType::Choice)],
                submit_label: None,
                dismiss_label: None,
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: Utc::now(),
            closed_at: None,
        };
        f.storage.create_question_set(&record).await.unwrap();
        f
    }

    fn answer(name: &str) -> QuestionAnswer {
        QuestionAnswer {
            set_id: "set-1".into(),
            text: String::new(),
            answers: vec![
                Answer {
                    question_id: "name".into(),
                    state: AnswerState::Answered,
                    value: Some(AnswerValue::Text(name.into())),
                    label: None,
                },
                Answer {
                    question_id: "db".into(),
                    state: AnswerState::Answered,
                    value: Some(AnswerValue::Text("sqlite".into())),
                    label: Some("SQLite".into()),
                },
            ],
            answerer: None,
            declined: false,
        }
    }

    async fn submit_as(f: &Fixture, session: &VizierSession, answer: QuestionAnswer) -> Result<VizierRequest, SubmitError> {
        submit(&f.storage, &f.transport, &f.questions, session, answer, "alice").await
    }

    /// A person who is not in the conversation reads the set as missing, answers nothing, and the
    /// set stays answerable by those who are (FR-018).
    #[tokio::test]
    async fn another_conversation_cannot_answer_a_set() {
        let f = fixture().await;
        let refused = submit_as(&f, &session("somewhere-else"), answer("demo")).await.unwrap_err();
        assert_eq!(refused, SubmitError::NotFound);
        let still = f.storage.get_question_set("set-1").await.unwrap().unwrap();
        assert_eq!(still.state, QuestionState::Pending);
        assert!(still.answers.is_none());

        assert!(submit_as(&f, &session("t"), answer("demo")).await.is_ok());
    }

    /// The second submission on a closed card is refused and returns no request, so no turn can
    /// start from it — a second tab, a double click and a Discord press all land here (FR-017).
    #[tokio::test]
    async fn a_second_submission_is_refused_and_starts_no_turn() {
        let f = fixture().await;
        let first = submit_as(&f, &session("t"), answer("first")).await.expect("the first wins");
        assert!(matches!(first.content, VizierRequestContent::QuestionAnswer(_)));

        let second = submit_as(&f, &session("t"), answer("second")).await.unwrap_err();
        assert_eq!(second, SubmitError::AlreadyClosed(QuestionState::Answered));
        let dismissed = submit_as(
            &f,
            &session("t"),
            QuestionAnswer { declined: true, answers: vec![], ..answer("x") },
        )
        .await
        .unwrap_err();
        assert_eq!(dismissed, SubmitError::AlreadyClosed(QuestionState::Answered));

        // What is stored is the first answer, untouched.
        let stored = f.storage.get_question_set("set-1").await.unwrap().unwrap();
        let name = stored.answers.unwrap().into_iter().find(|a| a.question_id == "name").unwrap();
        assert_eq!(name.value, Some(AnswerValue::Text("first".into())));
    }

    /// A refused answer keeps the set open and every other answer, so the person fixes one field
    /// (FR-012).
    #[tokio::test]
    async fn an_invalid_answer_is_refused_and_the_set_stays_open() {
        let f = fixture().await;
        let mut bad = answer("demo");
        bad.answers[1].value = Some(AnswerValue::Text("oracle".into()));
        let refused = submit_as(&f, &session("t"), bad).await.unwrap_err();
        assert_eq!(refused.problems().len(), 1);
        assert_eq!(refused.problems()[0].rule, "value_not_offered");
        assert_eq!(
            f.storage.get_question_set("set-1").await.unwrap().unwrap().state,
            QuestionState::Pending
        );
    }

    /// Markup, script and command-looking answers are data: stored, summarised and delivered to
    /// the agent byte for byte, and nothing interprets them (FR-059).
    #[tokio::test]
    async fn markup_in_an_answer_is_stored_and_delivered_as_text() {
        let f = fixture().await;
        let hostile = "<script>alert(1)</script> $(rm -rf /) `id` {{7*7}} <!-- --> \u{202e}";
        let request = submit_as(&f, &session("t"), answer(hostile)).await.unwrap();

        let VizierRequestContent::QuestionAnswer(delivered) = &request.content else { panic!() };
        let name = delivered.answers.iter().find(|a| a.question_id == "name").unwrap();
        assert_eq!(name.value, Some(AnswerValue::Text(hostile.into())), "delivered verbatim");
        // The agent reads it inside a JSON string, never as markup of its own.
        let prompt = request.to_prompt().unwrap();
        assert!(prompt.contains("```json"), "{prompt}");
        let data = prompt.split("```json\n").nth(1).unwrap().split("\n```").next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(data).expect("the answers are valid JSON");
        assert_eq!(parsed["answers"][0]["value"], hostile);

        let stored = f.storage.get_question_set("set-1").await.unwrap().unwrap();
        let stored_name = stored.answers.unwrap().into_iter().find(|a| a.question_id == "name").unwrap();
        assert_eq!(stored_name.value, Some(AnswerValue::Text(hostile.into())), "stored verbatim");
    }
}
