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
#[allow(clippy::too_many_arguments)] // one ask takes everything it touches, once, from its one caller
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

    if let Err(e) = storage.open_sensitive_ask(&record).await {
        // Nobody will ever answer this ask, so the conversation's one slot must not stay taken.
        questions.close_ask(&record.ask_id);
        return Err(e);
    }
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

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::{
        sandbox::{ExecutionError, ExecutionErrorKind, ExecutionReport, ToolInvocationRecord},
        schema::VizierChannelId,
        storage::{document::LocalDocumentStore, sqlite::SqliteStorage},
    };

    const TOKEN: &str = "ghp_super-secret-token-123";

    struct Fixture {
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
        questions: Arc<Questions>,
        redactor: Arc<SecretRedactor>,
        _dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_question_schema(&conn).unwrap();
        crate::storage::sqlite::init_sensitive_ask_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        Fixture {
            storage: Arc::new(VizierStorage::new(storage)),
            transport: VizierTransport::new(),
            questions: Arc::new(Questions::new()),
            redactor: Arc::new(SecretRedactor::default()),
            _dir: dir,
        }
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        )
    }

    /// Run one ask in the background and hand back its handle.
    fn spawn_ask(
        f: &Fixture,
        deadline: Option<tokio::time::Instant>,
    ) -> tokio::task::JoinHandle<anyhow::Result<SensitiveOutcome>> {
        let (storage, transport, questions, redactor) = (
            f.storage.clone(),
            f.transport.clone(),
            f.questions.clone(),
            f.redactor.clone(),
        );
        tokio::spawn(async move {
            ask(
                &storage,
                &transport,
                &questions,
                &redactor,
                &session(),
                "to push the branch",
                Some("GitHub token"),
                deadline,
            )
            .await
        })
    }

    /// The id of the dialog the conversation has open, once the ask has opened it.
    async fn open_ask_id(f: &Fixture) -> String {
        for _ in 0..200 {
            if let Some(id) = f.questions.open_asks(&session()).into_iter().next() {
                return id;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the ask never opened");
    }

    /// The script gets the value byte for byte; everything its report could show is scrubbed
    /// (SC-014, SC-015, FR-042).
    #[tokio::test]
    async fn the_script_gets_the_value_and_its_report_does_not() {
        let f = fixture();
        let handle = spawn_ask(&f, None);
        let id = open_ask_id(&f).await;
        assert!(f.questions.answer(&id, AskReply::Value(TOKEN.into())));

        let outcome = handle.await.unwrap().unwrap();
        // The script itself received it, exactly.
        assert_eq!(outcome.value(), Some(TOKEN));
        assert!(matches!(outcome, SensitiveOutcome::Provided { protected: true, .. }));

        // The script prints it, returns it, fails with it and passes it to a nested call.
        let mut report = ExecutionReport {
            ok: false,
            result: serde_json::json!({ "token": TOKEN, "nested": [TOKEN] }),
            stdout: format!("token={TOKEN}"),
            error: Some(ExecutionError {
                kind: ExecutionErrorKind::Script,
                message: format!("bad credentials {TOKEN}"),
                traceback: format!("line 3: {TOKEN}"),
                limit: None,
            }),
            tool_calls: vec![ToolInvocationRecord {
                seq: 1,
                name: "http_post".into(),
                arguments: serde_json::json!({ "headers": { "Authorization": format!("Bearer {TOKEN}") } }),
                ok: false,
                error: Some(format!("401 for {TOKEN}")),
                duration_ms: 1,
                ends_turn: false,
            }],
            duration_ms: 1,
        };
        f.redactor.report(&mut report);
        let shown = serde_json::to_string(&report).unwrap();
        assert!(!shown.contains(TOKEN), "the report still carries the value: {shown}");
        assert!(shown.contains("[redacted]"));
    }

    /// A value shorter than the redactor can arm is handed over, flagged as unprotected (FR-043).
    #[tokio::test]
    async fn a_short_value_is_flagged_unprotected() {
        let f = fixture();
        let handle = spawn_ask(&f, None);
        let id = open_ask_id(&f).await;
        f.questions.answer(&id, AskReply::Value("1234".into()));
        match handle.await.unwrap().unwrap() {
            SensitiveOutcome::Provided { value, protected } => {
                assert_eq!(value, "1234");
                assert!(!protected);
            }
            other => panic!("expected a value, got {other:?}"),
        }
        assert!(!f.redactor.is_armed(), "a too-short value cannot be armed");
    }

    /// Rejecting (and Escape, which sends the same reply) resolves to `declined` and frees the
    /// conversation; nothing is armed (FR-050).
    #[tokio::test]
    async fn a_rejection_resolves_to_declined_and_leaves_nothing_open() {
        let f = fixture();
        let handle = spawn_ask(&f, None);
        let id = open_ask_id(&f).await;
        assert!(f.questions.answer(&id, AskReply::Declined));
        assert!(matches!(handle.await.unwrap().unwrap(), SensitiveOutcome::Declined));
        assert!(f.questions.open_asks(&session()).is_empty());
        assert!(!f.redactor.is_armed());
    }

    /// Silence is a result the script can branch on, and an answer arriving after it is refused
    /// and delivers nothing (FR-049, FR-051).
    #[tokio::test]
    async fn the_wait_limit_times_out_and_a_late_answer_is_refused() {
        let f = fixture();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(150);
        let handle = spawn_ask(&f, Some(deadline));
        let id = open_ask_id(&f).await;

        assert!(matches!(handle.await.unwrap().unwrap(), SensitiveOutcome::Timeout));
        assert!(!f.questions.answer(&id, AskReply::Value(TOKEN.into())), "a late answer was accepted");
        assert!(!f.redactor.is_armed(), "a refused value must not be armed");
        assert!(f.questions.open_asks(&session()).is_empty());
    }

    /// An abort abandons the open ask and its script stops waiting; the late answer is refused
    /// (FR-049).
    #[tokio::test]
    async fn an_abort_closes_the_ask_and_frees_the_script() {
        let f = fixture();
        let handle = spawn_ask(&f, None);
        let id = open_ask_id(&f).await;

        abandon_session(&f.storage, &f.transport, &f.questions, &session()).await;
        assert!(matches!(handle.await.unwrap().unwrap(), SensitiveOutcome::Timeout));
        assert!(!f.questions.answer(&id, AskReply::Value(TOKEN.into())));
        assert!(f.questions.open_asks(&session()).is_empty());
    }

    /// After a decline the next ask opens a new dialog; the earlier one stays closed and cannot be
    /// answered (FR-053, US8 #4).
    #[tokio::test]
    async fn an_ask_after_a_decline_opens_a_new_dialog() {
        let f = fixture();
        let first = spawn_ask(&f, None);
        let first_id = open_ask_id(&f).await;
        f.questions.answer(&first_id, AskReply::Declined);
        assert!(matches!(first.await.unwrap().unwrap(), SensitiveOutcome::Declined));

        let second = spawn_ask(&f, None);
        let second_id = open_ask_id(&f).await;
        assert_ne!(first_id, second_id);
        assert!(!f.questions.answer(&first_id, AskReply::Value(TOKEN.into())), "the old dialog answered");
        assert!(f.questions.answer(&second_id, AskReply::Value(TOKEN.into())));
        assert_eq!(second.await.unwrap().unwrap().value(), Some(TOKEN));
    }

    /// A second ask in the same turn opens once the first has closed (FR-053, US7).
    #[tokio::test]
    async fn a_second_ask_waits_for_the_first_to_close() {
        let f = fixture();
        let first = spawn_ask(&f, None);
        let first_id = open_ask_id(&f).await;
        let second = spawn_ask(&f, None);
        // Only one dialog is ever open.
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(f.questions.open_asks(&session()), vec![first_id.clone()]);

        f.questions.answer(&first_id, AskReply::Declined);
        assert!(matches!(first.await.unwrap().unwrap(), SensitiveOutcome::Declined));

        // The second takes the freed slot (polled every 100 ms).
        let second_id = open_ask_id(&f).await;
        assert_ne!(first_id, second_id);
        f.questions.answer(&second_id, AskReply::Declined);
        assert!(matches!(second.await.unwrap().unwrap(), SensitiveOutcome::Declined));
    }

    /// The row records the outcome and never a value (FR-054).
    #[tokio::test]
    async fn the_stored_ask_holds_no_value() {
        let f = fixture();
        let handle = spawn_ask(&f, None);
        let id = open_ask_id(&f).await;
        f.questions.answer(&id, AskReply::Value(TOKEN.into()));
        handle.await.unwrap().unwrap();

        let stored = f.storage.get_sensitive_ask(&id).await.unwrap().unwrap();
        assert_eq!(stored.state, AskState::Provided);
        assert!(!format!("{stored:?}").contains(TOKEN));
    }

    /// A storage failure after the slot was taken must not leave the conversation unable to ask
    /// again: nobody can ever answer an ask that was never opened.
    #[tokio::test]
    async fn a_storage_failure_does_not_leak_the_conversations_slot() {
        let mut f = fixture();
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap(); // no sensitive_ask table
        f.storage = Arc::new(VizierStorage::new(SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        )));

        let result = spawn_ask(&f, None).await.unwrap();
        assert!(result.is_err());
        assert!(f.questions.open_asks(&session()).is_empty(), "the slot stayed taken");
    }
}
