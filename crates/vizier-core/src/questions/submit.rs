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
