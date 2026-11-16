//! Steering a running turn (`specs/062-steer-busy-turn`, contracts/steering.md).
//!
//! A person's message sent while their conversation's agent is running a turn waits in the
//! session's queue. The queue is shared between the agent's loop (`agents/process.rs`), which
//! pushes to the back and pops from the front as it always has, and the running chat turn, which
//! takes every `Steer` item at a round boundary (`prompt_with`). Taking and popping both hold the
//! queue's lock, which is the in-memory half of "exactly once"; `TurnStorage::steer_turns` is the
//! durable half.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::agents::steps::TurnRef;
use crate::schema::{VizierRequest, VizierRequestContent, VizierSession};
use crate::skill::invocation::{self, Parsed};
use crate::transport::ResponseSender;

/// What a waiting message may do (research R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Enter the running turn at its next round boundary; run as its own turn if there is none.
    Steer,
    /// Run as its own turn after the running one, as before spec 062.
    Queue,
}

/// One request waiting for its session's running job.
pub struct Queued {
    pub request: VizierRequest,
    pub response_tx: Option<ResponseSender>,
    pub turn: TurnRef,
    pub mode: Mode,
}

impl Queued {
    /// A waiting request with its mode decided now, once (FR-011b). `mid_turn` is the engine's to
    /// set, never a sender's.
    pub fn new(
        mut request: VizierRequest,
        response_tx: Option<ResponseSender>,
        turn: TurnRef,
        session: &VizierSession,
    ) -> Self {
        request.mid_turn = false;
        let mode = mode(&request, session);
        Self {
            request,
            response_tx,
            turn,
            mode,
        }
    }
}

/// A session's waiting requests, shared by the agent's loop and the session's running turn.
pub type SessionQueue = Arc<Mutex<VecDeque<Queued>>>;

/// The queue's contents, even after a panic elsewhere poisoned the lock: the deque itself is
/// never left half-changed by the few operations done under it.
pub fn lock(queue: &SessionQueue) -> MutexGuard<'_, VecDeque<Queued>> {
    match queue.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The running turn's handle on its session's queue.
#[derive(Clone)]
pub struct SteerInbox(pub SessionQueue);

impl SteerInbox {
    /// Every `Steer` item, in arrival order; the others stay where they are.
    pub fn take(&self) -> Vec<Queued> {
        let mut queue = lock(&self.0);
        if !queue.iter().any(|item| item.mode == Mode::Steer) {
            return vec![];
        }
        let (steer, rest): (VecDeque<Queued>, VecDeque<Queued>) =
            queue.drain(..).partition(|item| item.mode == Mode::Steer);
        *queue = rest;
        steer.into_iter().collect()
    }
}

impl std::fmt::Debug for SteerInbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SteerInbox")
    }
}

/// Whether a request sent while its session is busy steers the running turn (research R3).
/// Only a person's typed message in a conversation with people on it steers; the person can opt
/// out with `queue`, and commands, skill invocations, voice messages, client events, answers,
/// workers' messages and transient requests keep their place in the queue (FR-007, FR-008).
pub fn mode(request: &VizierRequest, session: &VizierSession) -> Mode {
    let VizierRequestContent::Chat(text) = &request.content else {
        return Mode::Queue;
    };
    if request.queue
        || request.transient
        || invocation::parse(text) != Parsed::Plain
        || !crate::agents::tools::session_has_person(session)
    {
        return Mode::Queue;
    }
    Mode::Steer
}

/// What `strip_queue_command` found (contracts/queue-command.md Q2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueCommand {
    /// Not a `/queue` command.
    Plain,
    /// `/queue <message>`: the prefix is gone and `queue` is set.
    Queued,
    /// `/queue` with nothing after it: answered with [`QUEUE_USAGE`], nothing else happens.
    UsageOnly,
}

/// The direct answer to a bare `/queue`.
pub const QUEUE_USAGE: &str = "Usage: `/queue <message>` runs the message as a new turn after the current one, instead of adding it to the turn in progress.";

/// Recognises `/queue` at the start of a chat message (any channel, any case, Telegram's
/// `@bot` suffix allowed) and turns it into the `queue` flag, so the history, the frontmatter
/// and every view show the message without the command.
pub fn strip_queue_command(request: &mut VizierRequest) -> QueueCommand {
    let VizierRequestContent::Chat(text) = &request.content else {
        return QueueCommand::Plain;
    };
    let Some(message) = queue_message(text) else {
        return QueueCommand::Plain;
    };
    if message.is_empty() {
        return QueueCommand::UsageOnly;
    }
    let message = message.to_string();
    request.content = VizierRequestContent::Chat(message);
    request.queue = true;
    QueueCommand::Queued
}

/// The message after a leading `/queue` (or `/queue@bot`), trimmed, or `None` for any other text.
fn queue_message(text: &str) -> Option<&str> {
    let body = text.trim_start().strip_prefix('/')?;
    let end = body.find(char::is_whitespace).unwrap_or(body.len());
    let (command, rest) = body.split_at(end);
    let name = command.split('@').next().unwrap_or(command);
    name.eq_ignore_ascii_case("queue").then(|| rest.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::VizierChannelId;

    fn web() -> VizierSession {
        VizierSession(
            "a".into(),
            VizierChannelId::HTTP("dana".into(), "chat".into()),
            Some("t".into()),
        )
    }

    fn chat(text: &str) -> VizierRequest {
        VizierRequest {
            content: VizierRequestContent::Chat(text.into()),
            ..Default::default()
        }
    }

    fn queued(text: &str, mode: Mode) -> Queued {
        Queued {
            request: chat(text),
            response_tx: None,
            turn: TurnRef::default(),
            mode,
        }
    }

    fn texts(items: &[Queued]) -> Vec<String> {
        items
            .iter()
            .map(|item| item.request.content.to_string())
            .collect()
    }

    #[test]
    fn a_plain_message_from_a_person_steers() {
        assert_eq!(mode(&chat("keep the old format"), &web()), Mode::Steer);
    }

    #[test]
    fn the_person_can_choose_to_queue() {
        let mut request = chat("later");
        request.queue = true;
        assert_eq!(mode(&request, &web()), Mode::Queue);
    }

    #[test]
    fn commands_skills_and_other_kinds_queue() {
        assert_eq!(mode(&chat("/skills"), &web()), Mode::Queue);
        assert_eq!(mode(&chat("/skill release-notes v2"), &web()), Mode::Queue);
        for content in [
            VizierRequestContent::Command("checkpoint".into()),
            VizierRequestContent::SilentRead("hi".into()),
            VizierRequestContent::DetachedDelivery(String::new()),
            VizierRequestContent::Unattended("run".into()),
        ] {
            let request = VizierRequest {
                content,
                ..Default::default()
            };
            assert_eq!(mode(&request, &web()), Mode::Queue);
        }
        let mut transient = chat("x");
        transient.transient = true;
        assert_eq!(mode(&transient, &web()), Mode::Queue);
    }

    #[test]
    fn a_conversation_without_people_never_steers() {
        let worker = VizierSession("a".into(), VizierChannelId::Subagent, Some("w".into()));
        let system = VizierSession("a".into(), VizierChannelId::System, None);
        assert_eq!(mode(&chat("x"), &worker), Mode::Queue);
        assert_eq!(mode(&chat("x"), &system), Mode::Queue);
    }

    #[test]
    fn a_new_item_never_carries_mid_turn_from_its_sender() {
        let mut request = chat("x");
        request.mid_turn = true;
        let item = Queued::new(request, None, TurnRef::default(), &web());
        assert!(!item.request.mid_turn);
        assert_eq!(item.mode, Mode::Steer);
    }

    #[test]
    fn take_removes_only_steer_items_and_keeps_order() {
        let inbox = SteerInbox(Arc::new(Mutex::new(VecDeque::from([
            queued("q1", Mode::Queue),
            queued("s1", Mode::Steer),
            queued("q2", Mode::Queue),
            queued("s2", Mode::Steer),
        ]))));
        assert_eq!(texts(&inbox.take()), ["s1", "s2"]);
        let left: Vec<_> = lock(&inbox.0).drain(..).collect();
        assert_eq!(texts(&left), ["q1", "q2"]);
        assert!(inbox.take().is_empty());
    }

    #[test]
    fn queue_command_table() {
        for (input, outcome, text, queue) in [
            (
                "/queue check the logs too",
                QueueCommand::Queued,
                "check the logs too",
                true,
            ),
            (
                "/QUEUE@vizier_bot check",
                QueueCommand::Queued,
                "check",
                true,
            ),
            (
                "  /queue\n line one\nline two",
                QueueCommand::Queued,
                "line one\nline two",
                true,
            ),
            ("/queue", QueueCommand::UsageOnly, "/queue", false),
            ("/queue   ", QueueCommand::UsageOnly, "/queue   ", false),
            (
                "/queueing is fun",
                QueueCommand::Plain,
                "/queueing is fun",
                false,
            ),
            (
                "/skill queue now",
                QueueCommand::Plain,
                "/skill queue now",
                false,
            ),
            ("queue this", QueueCommand::Plain, "queue this", false),
        ] {
            let mut request = chat(input);
            assert_eq!(strip_queue_command(&mut request), outcome, "{input:?}");
            assert_eq!(request.content.to_string(), text, "{input:?}");
            assert_eq!(request.queue, queue, "{input:?}");
        }
    }
}
