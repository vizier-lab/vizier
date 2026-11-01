//! Asking the person (`specs/042-agent-asking-question`): the two tools an agent uses when it
//! needs something from somebody rather than from a tool.
//!
//! - [`tool_question::AskQuestion`] asks one or more structured questions and **ends the turn**.
//!   The answer comes back as the person's next message, so it takes the ordinary history,
//!   queue and attribution path (research R7).
//! - [`tool_sensitive::AskSensitiveInput`] asks for one sensitive value and **does not** end the
//!   turn: it blocks the calling script, the WebUI shows a masked dialog, and the value is armed
//!   into the turn's [`crate::secrets::SecretRedactor`] on arrival so every copy that leaves the
//!   turn is scrubbed exactly as for `get_secret` (research R8–R10).
//!
//! One module for both tools because they share the conversation, the access rule, the limits and
//! the validation discipline; they differ only in where the answer goes.
//!
//! `model` and `limits` hold the shapes and the numbers, `validate` the rules both sides of a
//! round trip use, `surface` the per-channel presentation trait, `text_form` the fallback grammar,
//! `submit` the one path an answer takes, and `sensitive` the open-ask registry.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::schema::{ChannelPlatform, VizierSession};

pub mod limits;
pub mod model;
pub mod sensitive;
pub mod submit;
pub mod surface;
pub mod text_form;
pub mod tool_question;
pub mod tool_sensitive;
pub mod validate;

pub use model::{QuestionSet, QuestionSetRecord, QuestionState, SensitiveOutcome};
pub use surface::{PlatformSurfaces, QuestionSurface, Refusal};

/// What a person did with an open ask. A timeout is not here: it is what the waiter produces when
/// none of these arrives in time (FR-051).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskReply {
    /// The value, exactly as supplied (FR-037).
    Value(String),
    /// The person rejected or dismissed the dialog (FR-050).
    Declined,
}

/// Why an ask could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskError {
    /// Another ask is already open in this conversation (FR-053).
    Busy,
}

/// The process-wide half of the feature: where each platform's surfaces are, and which asks are
/// waiting for a person.
///
/// Built once with the dependencies and shared by every turn, because a surface outlives a turn
/// and an ask is answered by a socket that is not running one.
#[derive(Default, Clone)]
pub struct Questions {
    surfaces: PlatformSurfaces,
    waits: Arc<Mutex<HashMap<String, oneshot::Sender<AskReply>>>>,
    /// The one ask a conversation may have open (FR-053), by ask id.
    slots: Arc<Mutex<HashMap<VizierSession, String>>>,
}

impl std::fmt::Debug for Questions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Questions")
            .field("surfaces", &self.surfaces)
            .field("waits", &self.waits.lock().len())
            .field("slots", &self.slots.lock().len())
            .finish()
    }
}

impl Questions {
    pub fn new() -> Self {
        Self::default()
    }

    /// The surfaces registry, so a channel reader can announce itself while it runs.
    pub fn surfaces(&self) -> &PlatformSurfaces {
        &self.surfaces
    }

    /// The surface that posts for this conversation's platform, if its reader is running.
    pub fn surface_for(
        &self,
        session: &VizierSession,
    ) -> Option<(ChannelPlatform, Arc<dyn QuestionSurface>)> {
        let platform = session.1.platform()?;
        let surface = self.surfaces.surface(&session.0, platform)?;
        Some((platform, surface))
    }

    /// Open the wait for `ask_id` in `session`, refusing when the conversation already has one.
    ///
    /// The receiver carries what the person did; the caller turns silence into a timeout. The slot
    /// keeps a second dialog from appearing over the first.
    pub fn open_ask(
        &self,
        session: &VizierSession,
        ask_id: &str,
    ) -> Result<oneshot::Receiver<AskReply>, AskError> {
        let mut slots = self.slots.lock();
        if slots.contains_key(session) {
            return Err(AskError::Busy);
        }
        slots.insert(session.clone(), ask_id.to_string());

        let (tx, rx) = oneshot::channel();
        self.waits.lock().insert(ask_id.to_string(), tx);
        Ok(rx)
    }

    /// Deliver what the person did. `false` when no such ask is waiting — already answered,
    /// declined, timed out, or belonging to another conversation (FR-049).
    pub fn answer(&self, ask_id: &str, reply: AskReply) -> bool {
        let waiter = self.waits.lock().remove(ask_id);
        let Some(tx) = waiter else {
            return false;
        };
        // Free the conversation's slot in the same breath, so the agent's next ask can open.
        self.release_slot(ask_id);
        tx.send(reply).is_ok()
    }

    /// Stop waiting for `ask_id` — the wait timed out, the turn was aborted, or the process is
    /// shutting down — and free its slot.
    pub fn close_ask(&self, ask_id: &str) {
        self.waits.lock().remove(ask_id);
        self.release_slot(ask_id);
    }

    /// Every ask still open in this conversation, oldest-unordered. Used to replay a dialog to a
    /// socket that has just opened (FR-048).
    pub fn open_asks(&self, session: &VizierSession) -> Vec<String> {
        self.slots
            .lock()
            .get(session)
            .map(|ask_id| vec![ask_id.clone()])
            .unwrap_or_default()
    }

    /// Whether this ask is still waiting.
    pub fn is_open(&self, ask_id: &str) -> bool {
        self.waits.lock().contains_key(ask_id)
    }

    fn release_slot(&self, ask_id: &str) {
        let mut slots = self.slots.lock();
        if let Some(session) = slots
            .iter()
            .find(|(_, waiting)| waiting.as_str() == ask_id)
            .map(|(session, _)| session.clone())
        {
            slots.remove(&session);
        }
    }
}

/// The one question set a turn may present (FR-008), and the handle a tool uses to reach the
/// service.
///
/// Per turn, because the limit is per turn: two turns in the same conversation each get their
/// own slot, which is why this is not part of [`Questions`].
#[derive(Clone)]
pub struct QuestionsHandle {
    service: Arc<Questions>,
    turn: Arc<TurnQuestions>,
}

/// The turn's own bookkeeping: whether it has already presented a set.
#[derive(Default)]
pub struct TurnQuestions {
    asked: Mutex<Option<String>>,
}

impl QuestionsHandle {
    pub fn new(service: Arc<Questions>) -> Self {
        Self {
            service,
            turn: Arc::new(TurnQuestions::default()),
        }
    }

    /// The shared service.
    pub fn service(&self) -> &Arc<Questions> {
        &self.service
    }

    /// The id of the set this turn presented, if it has.
    pub fn asked(&self) -> Option<String> {
        self.turn.asked.lock().clone()
    }

    /// Whether this turn may still present a set. A refused call never reaches
    /// [`Self::record_asked`], so a set that was rejected does not use up the turn's one.
    pub fn may_ask(&self) -> bool {
        self.turn.asked.lock().is_none()
    }

    /// Remember the set this turn presented.
    pub fn record_asked(&self, set_id: &str) {
        *self.turn.asked.lock() = Some(set_id.to_string());
    }
}

impl std::fmt::Debug for QuestionsHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuestionsHandle")
            .field("asked", &self.asked())
            .finish()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::VizierChannelId;

    fn session(topic: &str) -> VizierSession {
        VizierSession(
            "a".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some(topic.into()),
        )
    }

    /// The one open ask per conversation: a second one is refused while the first waits, and the
    /// slot frees the moment the first is resolved (FR-053, FR-052).
    #[tokio::test]
    async fn one_ask_at_a_time_per_conversation() {
        let questions = Questions::new();
        let first = questions.open_ask(&session("t"), "ask-1").unwrap();
        assert_eq!(
            questions.open_ask(&session("t"), "ask-2").unwrap_err(),
            AskError::Busy
        );
        // A different conversation is unaffected.
        assert!(questions.open_ask(&session("other"), "ask-3").is_ok());

        assert!(questions.answer("ask-1", AskReply::Value("secret-value".into())));
        assert_eq!(first.await.unwrap(), AskReply::Value("secret-value".into()));
        // Free again, so the agent may ask once more in the same turn.
        assert!(questions.open_ask(&session("t"), "ask-4").is_ok());
    }

    /// An answer for an ask nobody is waiting on is refused and changes nothing: a late
    /// submission after a timeout, or a submission from another conversation (FR-049).
    #[tokio::test]
    async fn a_late_or_unknown_answer_is_refused() {
        let questions = Questions::new();
        let _waiter = questions.open_ask(&session("t"), "ask-1").unwrap();
        questions.close_ask("ask-1");

        assert!(!questions.answer("ask-1", AskReply::Value("late".into())));
        assert!(!questions.answer("never-existed", AskReply::Declined));
        assert!(!questions.is_open("ask-1"));
        // Closing released the slot, so the conversation is usable again.
        assert!(questions.open_ask(&session("t"), "ask-2").is_ok());
    }

    /// A decline travels the same way as a value, so the script can branch on either (FR-050).
    #[tokio::test]
    async fn a_decline_resolves_the_waiter() {
        let questions = Questions::new();
        let waiter = questions.open_ask(&session("t"), "ask-1").unwrap();
        assert!(questions.answer("ask-1", AskReply::Declined));
        assert_eq!(waiter.await.unwrap(), AskReply::Declined);
    }

    /// The open asks of a conversation, for replaying a dialog to a socket that just opened
    /// (FR-048).
    #[test]
    fn open_asks_are_listed_per_conversation() {
        let questions = Questions::new();
        assert!(questions.open_asks(&session("t")).is_empty());

        let _waiter = questions.open_ask(&session("t"), "ask-1").unwrap();
        assert_eq!(questions.open_asks(&session("t")), vec!["ask-1".to_string()]);
        assert!(questions.open_asks(&session("other")).is_empty());
    }

    /// Dropping the waiter without closing leaves nothing behind: the slot is freed when the ask
    /// is closed, which the tool does on every outcome including an abort.
    #[test]
    fn closing_frees_the_conversation() {
        let questions = Questions::new();
        let _waiter = questions.open_ask(&session("t"), "ask-1").unwrap();
        questions.close_ask("ask-1");
        assert!(!questions.is_open("ask-1"));
        assert_eq!(questions.slots.lock().len(), 0);
    }

    /// One set per turn; a refused call does not use it up (FR-008).
    #[test]
    fn a_turn_may_present_one_set() {
        let handle = QuestionsHandle::new(Arc::new(Questions::new()));
        assert!(handle.may_ask());
        assert_eq!(handle.asked(), None);

        handle.record_asked("set-1");
        assert!(!handle.may_ask());
        assert_eq!(handle.asked().as_deref(), Some("set-1"));

        // Two turns in one conversation each get their own slot.
        let another = QuestionsHandle::new(Arc::new(Questions::new()));
        assert!(another.may_ask());
    }

    /// A conversation with no channel platform has no surface, which is what makes the call fail
    /// with a message rather than hang (FR-010, FR-028).
    #[test]
    fn a_conversation_without_a_platform_has_no_surface() {
        let questions = Questions::new();
        assert!(questions.surface_for(&session("webui")).is_none());
    }
}
