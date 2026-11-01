//! The sensitive-input ask: the wait a script blocks on, and the one ask a conversation may have
//! open (research R8–R9).
//!
//! The value never touches storage, the history, the log or the model: it goes from the socket
//! straight into the running script ([`crate::questions::model::SensitiveOutcome`]), and the
//! turn's [`crate::secrets::SecretRedactor`] is armed with it the moment it arrives, so every
//! copy that leaves the turn carries `[redacted]` instead (FR-041, FR-042, FR-044).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::{
    questions::{
        AskReply, Questions,
        limits,
        model::{AskState, SensitiveAskRecord, SensitiveOutcome},
    },
    schema::{SessionHistoryContent, VizierSession},
    secrets::{MIN_VALUE_BYTES, SecretRedactor},
    storage::{VizierStorage, question::SensitiveAskStorage},
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

/// Wait for one answer, with the deadline the script itself has.
///
/// `deadline` is the agent loop's tool deadline: the sandbox wraps a script's nested call in its
/// own timeout, so an ask that waited past it would hand the script `timed out after …` instead
/// of a result it can branch on (FR-051). The wait is therefore always strictly shorter.
pub async fn ask(
    storage: &Arc<VizierStorage>,
    transport: &VizierTransport,
    questions: &Questions,
    redactor: &SecretRedactor,
    session: &VizierSession,
    purpose: &str,
    label: Option<&str>,
    deadline: Option<tokio::time::Instant>,
) -> anyhow::Result<SensitiveOutcome> {
    let wait = match deadline {
        Some(at) => {
            let left = at.saturating_duration_since(tokio::time::Instant::now());
            left.min(limits::ASK_WAIT)
        }
        None => limits::ASK_WAIT,
    };

    let record = SensitiveAskRecord {
        ask_id: uuid::Uuid::new_v4().to_string(),
        purpose: purpose.to_string(),
        label: label.map(str::to_string),
        session: session.clone(),
        state: AskState::Open,
        requested_at: Utc::now(),
        closed_at: None,
    };

    // One dialog at a time per conversation (FR-053): a second ask waits for the first to close
    // rather than stacking another modal over it.
    let waiter = wait_for_slot(questions, session, &record.ask_id, wait).await?;
    let Some(waiter) = waiter else {
        // The slot never freed inside this ask's own budget: the script gets a timeout it can
        // branch on, not a hang.
        return Ok(SensitiveOutcome::Timeout);
    };

    storage.open_sensitive_ask(&record).await?;
    // The transcript says a value was requested, and later what came of it (FR-054).
    crate::agents::steps::record_step(
        storage,
        transport,
        session,
        SessionHistoryContent::SensitiveAsk {
            ask_id: record.ask_id.clone(),
            purpose: record.purpose.clone(),
            label: record.label.clone(),
            state: AskState::Open,
        },
    )
    .await;
    transport.publish_session_event(SessionEvent {
        session: session.clone(),
        frame: SessionFrame::SensitiveAsk(record.clone()),
    });

    let outcome = tokio::time::timeout(wait, waiter).await;
    let (state, result) = match outcome {
        Ok(Ok(AskReply::Value(value))) => {
            // Armed the moment it arrives, before anything can carry it: the report the script
            // returns, the history entry, the model's copy, the log line and the telemetry are
            // all scrubbed from here on (`specs/027-agent-secrets`).
            redactor.arm(&value);
            let protected = value.len() >= MIN_VALUE_BYTES;
            (AskState::Provided, SensitiveOutcome::Provided { value, protected })
        }
        Ok(Ok(AskReply::Declined)) => (AskState::Declined, SensitiveOutcome::Declined),
        // Silence is a timeout the script can branch on (FR-051).
        _ => (AskState::TimedOut, SensitiveOutcome::Timeout),
    };

    // A late answer after this point finds nothing waiting (FR-049).
    questions.close_ask(&record.ask_id);
    if let Err(e) = storage.close_sensitive_ask(&record.ask_id, state).await {
        tracing::warn!(ask_id = %record.ask_id, "could not close a sensitive ask: {e}");
    }
    crate::agents::steps::record_step(
        storage,
        transport,
        session,
        SessionHistoryContent::SensitiveAsk {
            ask_id: record.ask_id.clone(),
            purpose: record.purpose.clone(),
            label: record.label.clone(),
            state,
        },
    )
    .await;
    transport.publish_session_event(SessionEvent {
        session: session.clone(),
        frame: SessionFrame::SensitiveAskClosed {
            ask_id: record.ask_id.clone(),
            outcome: state,
        },
    });

    Ok(result)
}

/// Take the conversation's one ask slot, waiting a bounded time for a previous ask to close.
async fn wait_for_slot(
    questions: &Questions,
    session: &VizierSession,
    ask_id: &str,
    wait: Duration,
) -> anyhow::Result<Option<tokio::sync::oneshot::Receiver<AskReply>>> {
    match questions.open_ask(session, ask_id) {
        Ok(waiter) => Ok(Some(waiter)),
        Err(crate::questions::AskError::Busy) => {
            // Poll for the slot: the previous ask closes when its person answers, declines, or
            // its own wait expires, and none of those can notify this call without a second
            // channel between two tools.
            let steps = (wait.as_millis() / 100).max(1);
            for _ in 0..steps {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Ok(waiter) = questions.open_ask(session, ask_id) {
                    return Ok(Some(waiter));
                }
            }
            Ok(None)
        }
    }
}

/// Close every ask of one conversation as abandoned. Called when a turn ends abnormally, so an
/// ask is never answerable against a dead turn (FR-049).
pub async fn abandon_session(
    storage: &VizierStorage,
    transport: &VizierTransport,
    questions: &Questions,
    session: &VizierSession,
) {
    for ask_id in questions.open_asks(session) {
        questions.close_ask(&ask_id);
        let _ = storage.close_sensitive_ask(&ask_id, AskState::Abandoned).await;
        transport.publish_session_event(SessionEvent {
            session: session.clone(),
            frame: SessionFrame::SensitiveAskClosed {
                ask_id,
                outcome: AskState::Abandoned,
            },
        });
    }
}

/// Startup sweep: a dialog cannot outlive the process awaiting it, so every ask still `open` is
/// abandoned before any turn runs (research R9).
pub async fn abandon_open_asks(storage: &VizierStorage) {
    match storage.open_sensitive_asks_all().await {
        Ok(asks) => {
            let count = asks.len();
            for ask in asks {
                let _ = storage
                    .close_sensitive_ask(&ask.ask_id, AskState::Abandoned)
                    .await;
            }
            if count > 0 {
                tracing::info!("abandoned {count} sensitive ask(s) left open by a restart");
            }
        }
        Err(e) => tracing::warn!("could not sweep open sensitive asks: {e}"),
    }
}
