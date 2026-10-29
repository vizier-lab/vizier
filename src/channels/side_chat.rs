//! The `/btw` ephemeral side chat (`specs/033-btw-side-chat/`).
//!
//! One shared path for every channel: `ask` validates a question, rate-limits the person,
//! sends a transient `VizierRequestContent::Btw` request on the host conversation's session,
//! and awaits the one answer. Nothing about an exchange is stored here; the store at the
//! bottom of this file (`SideChats`) keeps a Discord or Telegram reader's live exchanges in
//! process memory only, so a restart ends them, as the privacy model requires.

/// Limits for the side chat (research R8), as constants rather than config
/// (constitution Principle I).
pub const MAX_SIDE_CHAT_CHARS: usize = 32_000;
pub const MAX_QUESTION_CHARS: usize = 4_000;
/// A person may start/continue at most this many side chats in a window.
pub const ASKS_PER_WINDOW: usize = 5;
pub const ASK_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::dependencies::VizierDependencies;
use crate::schema::{
    ErrorKind,
    SideChat,
    VizierAttachment,
    VizierRequest,
    VizierRequestContent,
    VizierResponse,
    VizierResponseContent,
    VizierResponseStats,
    VizierSession,
};

/// Who is asking, as the rate-limit key (`research R9`): a string derived from the platform
/// identity, never stored. It is also what the in-memory exchange store uses to decide
/// whether a reply may continue a side chat.
///
/// The username/ids are passed in by each channel; they never carry question or answer text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PersonKey {
    Http(String),
    Discord(u64),
    Telegram(u64),
}

impl PersonKey {
    pub fn http(username: &str) -> Self {
        Self::Http(username.to_string())
    }

    pub fn discord(id: u64) -> Self {
        Self::Discord(id)
    }

    pub fn telegram(id: u64) -> Self {
        Self::Telegram(id)
    }
}

impl std::fmt::Display for PersonKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(username) => write!(f, "http:{username}"),
            Self::Discord(id) => write!(f, "discord:{id}"),
            Self::Telegram(id) => write!(f, "telegram:{id}"),
        }
    }
}

/// What `ask` returns when there is no answer; each channel maps it to its own surface
/// (`specs/033-btw-side-chat/contracts/channel-commands.md`).
#[derive(Debug, Clone)]
pub enum SideChatError {
    RateLimited { retry_after_secs: u64 },
    Invalid(String),
    Timeout,
    Agent { kind: &'static str, message: String },
}

impl SideChatError {
    /// The one message every channel shows the person (per its own contract rendering).
    pub fn user_message(&self) -> String {
        match self {
            Self::RateLimited { retry_after_secs } => {
                format!("Slow down — try again in {retry_after_secs}s.")
            }
            Self::Invalid(reason) => reason.to_string(),
            Self::Timeout => "That took too long to answer — try again.".to_string(),
            Self::Agent { kind, message } => format!("**{kind}**: {message}"),
        }
    }
}

impl std::fmt::Display for SideChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited { retry_after_secs } => {
                write!(f, "rate limited, retry after {retry_after_secs}s")
            }
            Self::Invalid(reason) => write!(f, "invalid side chat: {reason}"),
            Self::Timeout => write!(f, "the side chat timed out"),
            Self::Agent { kind, message } => write!(f, "agent error ({kind}): {message}"),
        }
    }
}

impl std::error::Error for SideChatError {}

/// An answer to share, as one channel reads the response channel: the first frame that
/// carries an answer, with everything a WebUI or chat client renders (`research R10`).
/// Frames that carry no answer are skipped.
#[derive(Debug, Clone)]
pub struct MessageReply {
    pub content: String,
    pub stats: Option<VizierResponseStats>,
    pub attachments: Vec<VizierAttachment>,
}

/// What the response channel said, once. `NoAgent` is a channel that closed before any
/// answer — a restart or a stopped agent.
#[derive(Debug)]
pub enum SideChatOutcome {
    Answered(Box<MessageReply>),
    Aborted,
    AgentError { kind: ErrorKind, message: String },
    NoAgent,
}

/// The shared send-and-await loop (`research R10`): every reply-reader — `side_chat::ask`
/// and the HTTP `agent_chat` — waits for the same first answer. `Err` is only `Timeout`
/// (`SideChatError` has only that variant as an error).
pub async fn await_reply(
    rx: flume::Receiver<VizierResponse>,
    timeout: Duration,
) -> Result<SideChatOutcome, SideChatError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let reply = match tokio::time::timeout_at(deadline, rx.recv_async()).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(_)) => return Ok(SideChatOutcome::NoAgent),
            Err(_) => return Err(SideChatError::Timeout),
        };
        match reply.content {
            VizierResponseContent::Message { content, stats } => {
                return Ok(SideChatOutcome::Answered(Box::new(MessageReply {
                    content,
                    stats,
                    attachments: reply.attachments,
                })));
            }
            VizierResponseContent::Error { kind, message } => {
                return Ok(SideChatOutcome::AgentError { kind, message });
            }
            VizierResponseContent::Abort => return Ok(SideChatOutcome::Aborted),
            VizierResponseContent::ThinkingStart
            | VizierResponseContent::Thinking(_)
            | VizierResponseContent::ToolChoice { .. }
            | VizierResponseContent::ToolResponse { .. }
            | VizierResponseContent::AudioReply(..)
            | VizierResponseContent::Empty
            | VizierResponseContent::Checkpoint { .. } => continue,
        }
    }
}

fn kind_label(kind: ErrorKind) -> &'static str {
    kind.label()
}

/// The one constructor of a `Btw` request, on every channel
/// (`specs/033-btw-side-chat/contracts/agent-side-chat.md`).
///
/// Validates the exchange, takes a rate-limit hit for `person`, then sends one transient
/// request on the host conversation's session and awaits the first answer, error, abort or
/// a closed channel under `timeout`. Nothing about the question or the answer is stored
/// here: the exchange's turns live with whoever called this.
#[allow(clippy::too_many_arguments)]
pub async fn ask(
    deps: &VizierDependencies,
    session: VizierSession,
    person: &PersonKey,
    user: String,
    side_chat: SideChat,
    timeout: Duration,
) -> Result<SideChatAnswer, SideChatError> {
    validate(&side_chat)?;
    let key = person.to_string();
    if let Some(wait) = deps.side_chat_limits.retry_after(&key) {
        return Err(SideChatError::RateLimited {
            retry_after_secs: wait.max(Duration::from_secs(1)).as_secs(),
        });
    }
    // Counts the ask itself; the limit is checked before anything is sent.
    let _ = deps.side_chat_limits.check(&key);

    let request = VizierRequest {
        timestamp: chrono::Utc::now(),
        user,
        content: VizierRequestContent::Btw(side_chat),
        attachments: vec![],
        transient: true,
        ..Default::default()
    };

    let (tx, rx) = flume::bounded(1);
    deps.transport
        .send_request(session, request, Some(tx))
        .await
        .map_err(|_| no_agent())?;

    match await_reply(rx, timeout).await? {
        SideChatOutcome::Answered(reply) => Ok(SideChatAnswer {
            answer: reply.content,
            stats: reply.stats,
        }),
        SideChatOutcome::Aborted => Err(SideChatError::Agent {
            kind: "Aborted",
            message: "The side chat was stopped.".to_string(),
        }),
        SideChatOutcome::AgentError { kind, message } => {
            Err(SideChatError::Agent { kind: kind_label(kind), message })
        }
        SideChatOutcome::NoAgent => Err(no_agent()),
    }
}

fn no_agent() -> SideChatError {
    SideChatError::Agent {
        kind: "Agent unavailable",
        message: "The agent is not available right now.".to_string(),
    }
}

/// A successful `ask`: the answer text with the reply's stats, so the WebUI can show the
/// model (`specs/033-btw-side-chat/data-model.md`).
#[derive(Debug, Clone)]
pub struct SideChatAnswer {
    pub answer: String,
    pub stats: Option<VizierResponseStats>,
}

/// The data-model limits: each question 1–4,000 characters and every turn so far plus the
/// new question at most 32,000 (`chars().count()`). Over the budget is the same
/// fixed message on every channel.
fn validate(side_chat: &SideChat) -> Result<(), SideChatError> {
    let question = side_chat.question.chars().count();
    if question == 0 {
        return Err(SideChatError::Invalid("Ask a question — the side chat needs one.".to_string()));
    }
    if question > MAX_QUESTION_CHARS {
        return Err(SideChatError::Invalid(format!(
            "That question is too long — a side-chat question can be at most {MAX_QUESTION_CHARS} characters."
        )));
    }
    let turns: usize = side_chat
        .turns
        .iter()
        .map(|t| t.question.chars().count() + t.answer.chars().count())
        .sum();
    if turns + question > MAX_SIDE_CHAT_CHARS {
        return Err(SideChatError::Invalid(
            "This side chat is full — start a new one.".to_string(),
        ));
    }
    Ok(())
}

/// How long a live exchange may sit untouched before the sweeper ends it.
#[derive(Debug, Clone, Copy)]
pub struct Lifetime {
    pub idle: Duration,
}

impl Default for Lifetime {
    fn default() -> Self {
        Self { idle: Duration::from_secs(300) }
    }
}

/// Aborts a sweeper (`SideChats::spawn_sweeper`) when the reader that started it stops, so a
/// restarted reader never leaves one running beside its replacement.
pub struct AbortOnDrop(pub tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// An exchange id, generated per start, used only to look exchanges up and address replies.
pub type ExchangeId = String;

/// One live side chat, as a Discord or Telegram reader holds it (research R11): the person
/// who started it, the host conversation it reads, the turns so far, and every message the
/// bot (and, on Telegram, the person) posted in it. Generic only over the platform's
/// posted-message handle `P` (`{chat id, message id}` for Telegram, a message id for
/// Discord). Never written to storage: a restart ends every exchange.
#[derive(Debug, Clone)]
pub struct LiveExchange<P> {
    pub id: ExchangeId,
    pub person: PersonKey,
    pub session: VizierSession,
    pub turns: Vec<crate::schema::SideChatTurn>,
    pub last_active: tokio::time::Instant,
    pub posted: Vec<P>,
}

/// The in-memory exchange store: `SideChats` (`specs/033-btw-side-chat/data-model.md`).
/// One instance per Discord or Telegram reader.
pub struct SideChats<P> {
    lifetime: Lifetime,
    exchanges: std::sync::Mutex<HashMap<ExchangeId, LiveExchange<P>>>,
}

/// How a reply should be treated, else both readers decide on their own
/// (research R11–R13). Pure: no state is touched.
#[derive(Debug)]
pub enum ReplyKind {
    /// A follow-up to a live exchange this person owns.
    FollowUp(ExchangeId),
    /// Someone else replying to a live exchange, or the starter's unrelated message.
    Ignore,
    /// A reply to a side-chat message whose exchange has ended.
    Ended,
    /// Not a side-chat message by any check available.
    NotSideChat,
}

impl<P> Default for SideChats<P> {
    fn default() -> Self {
        Self {
            lifetime: Lifetime { idle: Duration::from_secs(300) },
            exchanges: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl<P: Clone + PartialEq> SideChats<P> {
    pub fn new(lifetime: Lifetime) -> Self {
        Self {
            lifetime,
            exchanges: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// A new exchange for `person`, reading `session`.
    pub fn start(&self, person: PersonKey, session: VizierSession) -> ExchangeId {
        let id = nanoid::nanoid!(10);
        let exchange = LiveExchange {
            id: id.clone(),
            person,
            session,
            turns: vec![],
            last_active: tokio::time::Instant::now(),
            posted: vec![],
        };
        self.exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id.clone(), exchange);
        id
    }

    /// The exchange `id`, when it lives and belongs to `person`.
    pub fn get_for(
        &self,
        id: &ExchangeId,
        person: &PersonKey,
    ) -> Option<(VizierSession, Vec<crate::schema::SideChatTurn>)> {
        let exchanges = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let exchange = exchanges.get(id)?;
        if &exchange.person != person {
            return None;
        }
        Some((exchange.session.clone(), exchange.turns.clone()))
    }

    /// Appends one turn to a live exchange.
    pub fn record(&self, id: &ExchangeId, turn: crate::schema::SideChatTurn) {
        if let Some(exchange) = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(id)
        {
            exchange.turns.push(turn);
            exchange.last_active = tokio::time::Instant::now();
        }
    }

    /// Moves `last_active` — a failed ask still counts as activity.
    pub fn touch(&self, id: &ExchangeId) {
        if let Some(exchange) = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(id)
        {
            exchange.last_active = tokio::time::Instant::now();
        }
    }

    /// Remembers one posted message as belonging to the exchange.
    pub fn add_posted(&self, id: &ExchangeId, posted: P) {
        if let Some(exchange) = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(id)
        {
            exchange.posted.push(posted);
        }
    }

    /// Removes every ended exchange and returns it, so the reader can present "ended"
    /// and, on Telegram, delete the messages before forgetting them. A failure of the
    /// lock is not worth the readers' messages: the sides are swept next round either way.
    pub fn sweep(&self, now: tokio::time::Instant) -> Vec<LiveExchange<P>> {
        let mut exchanges = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ended: Vec<ExchangeId> = exchanges
            .iter()
            .filter(|(_, exchange)| now.duration_since(exchange.last_active) >= self.lifetime.idle)
            .map(|(id, _)| id.clone())
            .collect();
        ended
            .into_iter()
            .filter_map(|id| exchanges.remove(&id))
            .collect()
    }

    /// Which live exchange, if any, a reply is replying to — by a message id the exchange
    /// posted. No live exchange owns it: the reader's own side-chat check decides.
    pub fn owner_of(&self, posted: &P) -> Option<(ExchangeId, PersonKey)> {
        let exchanges = self
            .exchanges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        exchanges
            .iter()
            .find(|(_, exchange)| exchange.posted.iter().any(|p| p == posted))
            .map(|(id, exchange)| (id.clone(), exchange.person.clone()))
    }

    /// The full decision for one reply, from what the reader knows
    /// (`specs/033-btw-side-chat/contracts/channel-commands.md`).
    pub fn classify_reply(
        &self,
        owner: Option<(ExchangeId, PersonKey)>,
        from: &PersonKey,
        is_side_chat_message: bool,
    ) -> ReplyKind {
        match owner {
            Some((id, person)) if &person == from => ReplyKind::FollowUp(id),
            Some((_, _)) => ReplyKind::Ignore,
            None if is_side_chat_message => ReplyKind::Ended,
            None => ReplyKind::NotSideChat,
        }
    }

    /// Runs `on_end` for every ended exchange, every `every`, forever. The sweeper owns
    /// the interval and the loop; the reader supplies only what ending means on its
    /// platform (Telegram deletes, Discord only forgets) — and holds no per-exchange state.
    pub fn spawn_sweeper<F, Fut>(
        self: &Arc<Self>,
        every: Duration,
        mut on_end: F,
    ) -> tokio::task::JoinHandle<()>
    where
        P: Send + 'static,
        F: FnMut(LiveExchange<P>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                for exchange in store.sweep(tokio::time::Instant::now()) {
                    on_end(exchange).await;
                }
            }
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VizierConfig;
    use crate::dependencies::VizierDependencies;
    use crate::schema::{SideChatTurn, VizierChannelId};

    fn short_side_chat(question: &str) -> SideChat {
        SideChat {
            question: question.to_string(),
            turns: vec![],
        }
    }

    fn session(agent: &str) -> VizierSession {
        VizierSession(
            agent.to_string(),
            VizierChannelId::HTTP("user".into(), "topic".into()),
            None,
        )
    }

    // Data-model limits (validate).

    #[test]
    fn an_empty_question_is_invalid() {
        assert!(matches!(
            validate(&short_side_chat("")),
            Err(SideChatError::Invalid(_))
        ));
    }

    #[test]
    fn an_over_long_question_is_invalid() {
        assert!(matches!(
            validate(&short_side_chat(&"x".repeat(MAX_QUESTION_CHARS + 1))),
            Err(SideChatError::Invalid(_))
        ));
        assert!(validate(&short_side_chat(&"x".repeat(MAX_QUESTION_CHARS))).is_ok());
    }

    #[test]
    fn exactly_the_budget_is_valid_and_one_more_is_not() {
        let turn = SideChatTurn {
            question: "q".repeat(15_000),
            answer: "a".repeat(15_000),
        };
        let full = SideChat {
            question: "q".repeat(2_000),
            turns: vec![turn],
        };
        assert!(validate(&full).is_ok());

        let one_over = SideChat {
            question: "q".repeat(2_001),
            ..full
        };
        assert!(matches!(
            validate(&one_over),
            Err(SideChatError::Invalid(reason)) if reason.contains("full"),
        ));
    }

    #[test]
    fn twenty_short_turns_are_valid() {
        let turn = SideChatTurn {
            question: "q".to_string(),
            answer: "a".to_string(),
        };
        let side_chat = SideChat {
            question: "next".to_string(),
            turns: vec![turn; 20],
        };
        assert!(validate(&side_chat).is_ok());
    }

    // `ask`: the rate limit is checked before anything is sent; the transport with no
    // registered agent would fail any send with a different error.

    async fn no_agent_deps() -> (VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let deps = VizierDependencies::new(config).await.unwrap();
        (deps, dir)
    }

    #[tokio::test]
    async fn the_sixth_ask_in_a_window_is_rate_limited_and_sends_nothing() {
        let (deps, _dir) = no_agent_deps().await;

        // Five asks pass the limiter and fail at the transport (no agent registered): the
        // agent error is the outcome, not a rate limit.
        for _ in 0..ASKS_PER_WINDOW {
            let result = ask(
                &deps,
                session("no-agent"),
                &PersonKey::http("user"),
                "user".into(),
                short_side_chat("what?"),
                Duration::from_secs(5),
            )
            .await;
            assert!(
                matches!(result, Err(SideChatError::Agent { .. })),
                "expected the unregistered agent's error, got {result:?}"
            );
        }

        // The sixth is refused before anything is sent.
        let result = ask(
            &deps,
            session("no-agent"),
            &PersonKey::http("user"),
            "user".into(),
            short_side_chat("what?"),
            Duration::from_secs(5),
        )
        .await;
        let Err(SideChatError::RateLimited { retry_after_secs }) = result else {
            panic!("expected a rate limit, got {result:?}");
        };
        assert!(retry_after_secs > 0 && retry_after_secs <= ASK_WINDOW.as_secs());
    }

    #[tokio::test]
    async fn another_person_has_its_own_window() {
        let (deps, _dir) = no_agent_deps().await;
        let first = PersonKey::http("p-one");
        for _ in 0..ASKS_PER_WINDOW {
            let _ = ask(
                &deps,
                session("no-agent"),
                &first,
                "p-one".into(),
                short_side_chat("what?"),
                Duration::from_secs(5),
            )
            .await;
        }
        // The first person is over their own budget now.
        let result = ask(
            &deps,
            session("no-agent"),
            &first,
            "p-one".into(),
            short_side_chat("what?"),
            Duration::from_secs(5),
        )
        .await;
        assert!(matches!(result, Err(SideChatError::RateLimited { .. })), "got {result:?}");

        // A second person has its own window entirely.
        let second = PersonKey::http("p-two");
        let result = ask(
            &deps,
            session("no-agent"),
            &second,
            "p-two".into(),
            short_side_chat("what?"),
            Duration::from_secs(5),
        )
        .await;
        assert!(matches!(result, Err(SideChatError::Agent { .. })), "got {result:?}");
    }

    // SideChats (in-memory exchange store).

    #[test]
    fn an_exchange_belongs_to_its_person_only() {
        let store: SideChats<String> = SideChats::new(Lifetime { idle: Duration::from_secs(300) });
        let alice = PersonKey::discord(1);
        let bob = PersonKey::discord(2);
        let id = store.start(alice.clone(), session("agent"));
        let (got, turns) = store.get_for(&id, &alice).unwrap();
        assert_eq!(got.0, "agent");
        assert!(turns.is_empty());
        assert!(store.get_for(&id, &bob).is_none(), "another person's id is refused");
    }

    #[test]
    fn an_unknown_or_ended_id_gives_none() {
        let store: SideChats<String> = SideChats::new(Lifetime { idle: Duration::from_secs(10) });
        assert!(
            store.get_for(&"nope".to_string(), &PersonKey::discord(1)).is_none(),
            "an unknown id gives none"
        );

        let alice = PersonKey::discord(1);
        let id = store.start(alice.clone(), session("agent"));
        // A synthetic "now" past the lifetime ends the exchange without waiting for one.
        let ended = store.sweep(tokio::time::Instant::now() + Duration::from_secs(11));
        assert_eq!(ended.len(), 1);
        assert!(
            store.get_for(&id, &alice).is_none(),
            "an ended id gives none"
        );
    }

    #[test]
    fn sweeps_only_the_idle_exchanges() {
        let store: SideChats<String> =
            SideChats::new(Lifetime { idle: Duration::from_millis(10) });
        let stale = store.start(PersonKey::discord(2), session("agent"));
        std::thread::sleep(Duration::from_millis(250));
        let live = store.start(PersonKey::discord(1), session("agent"));

        let ended = store.sweep(tokio::time::Instant::now());
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, stale);
        assert_eq!(ended[0].person, PersonKey::discord(2));
        assert!(store.get_for(&stale, &PersonKey::discord(2)).is_none());
        assert!(store.get_for(&live, &PersonKey::discord(1)).is_some());

        // A nothing-to-sweep sweep is empty, and the live exchange stays live.
        assert!(store.sweep(tokio::time::Instant::now()).is_empty());
    }

    #[test]
    fn recording_and_touching_keep_an_exchange_alive() {
        let store: SideChats<String> =
            SideChats::new(Lifetime { idle: Duration::from_millis(10) });
        let alice = PersonKey::discord(3);
        let id = store.start(alice.clone(), session("agent"));
        store.record(&id, SideChatTurn {
            question: "q".into(),
            answer: "a".into(),
        });
        assert!(store.sweep(tokio::time::Instant::now()).is_empty());
        assert_eq!(store.get_for(&id, &alice).unwrap().1.len(), 1);
    }

    #[test]
    fn owner_of_finds_a_posted_message() {
        let store: SideChats<u64> = SideChats::new(Lifetime { idle: Duration::from_secs(30) });
        let alice = PersonKey::telegram(42);
        let id = store.start(alice.clone(), session("agent"));
        let _ = store.get_for(&id, &alice);
        store.add_posted(&id, 7777);
        assert_eq!(store.owner_of(&7777), Some((id, alice)));
        assert_eq!(store.owner_of(&1), None);
    }

    #[test]
    fn classify_reply_covers_every_branch() {
        let store: SideChats<String> = SideChats::new(Lifetime::default());
        let alice = PersonKey::discord(7);
        let bob = PersonKey::discord(8);
        let id = store.start(alice.clone(), session("agent"));

        match store.classify_reply(Some((id.clone(), alice.clone())), &alice, false) {
            ReplyKind::FollowUp(found) => assert_eq!(found, id),
            other => panic!("expected FollowUp, got {other:?}"),
        }
        match store.classify_reply(Some((id.clone(), alice.clone())), &bob, false) {
            ReplyKind::Ignore => {}
            other => panic!("expected Ignore for another person, got {other:?}"),
        }
        match store.classify_reply(None, &alice, true) {
            ReplyKind::Ended => {}
            other => panic!("expected Ended, got {other:?}"),
        }
        match store.classify_reply(None, &alice, false) {
            ReplyKind::NotSideChat => {}
            other => panic!("expected NotSideChat, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_sweeper_calls_on_end_once_per_ended_exchange() {
        let store: Arc<SideChats<String>> =
            Arc::new(SideChats::new(Lifetime { idle: Duration::from_secs(2) }));
        let started = store.start(PersonKey::discord(11), session("agent"));

        let ended: Arc<std::sync::Mutex<Vec<ExchangeId>>> = Arc::new(std::sync::Mutex::new(vec![]));
        let sink = ended.clone();
        let sweeper = store.spawn_sweeper(Duration::from_millis(500), move |exchange| {
            let sink = sink.clone();
            async move {
                sink.lock().unwrap().push(exchange.id);
            }
        });

        // Within the lifetime: still live.
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(ended.lock().unwrap().is_empty());

        // Past the lifetime, over ticks at 500 ms: ended exactly once.
        tokio::time::advance(Duration::from_secs(3)).await;
        let seen = ended.lock().unwrap().clone();
        assert_eq!(seen, vec![started.clone()]);
        assert!(store.get_for(&started, &PersonKey::discord(11)).is_none());

        sweeper.abort();
    }
}
