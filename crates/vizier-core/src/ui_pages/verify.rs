//! Checking a new version in an open tab (`specs/063-html-generative-ui` research R6,
//! contracts/verify-protocol.md). One bus request on `ui.{session}.verify`: every chat socket
//! watching the conversation answers it by asking its tab to mount the version off-screen, and the
//! first report wins. No responder means no tab, at once; a deadline means a tab that did not
//! answer. Either way the turn never waits for nobody.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::oneshot;
use vizier_schema::{BusError, Envelope, Subject};

use crate::{
    schema::{VizierChannelId, VizierSession},
    transport::VizierTransport,
};

use super::limits::{MAX_PROBLEM_CHARS, MAX_PROBLEMS, VERIFY_DEADLINE, VERIFY_OBSERVE_MS};

/// The envelope kind of a request and of its reply.
pub const VERIFY_KIND: &str = "ui.verify";

/// What a tab is asked to check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub request_id: String,
    pub page_id: String,
    pub version: u32,
    pub observe_ms: u64,
}

/// What a tab saw. `closed` is the socket's own answer when it went away first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifyReply {
    pub request_id: String,
    #[serde(default)]
    pub problems: Vec<Value>,
    #[serde(default)]
    pub observed_ms: u64,
    #[serde(default)]
    pub closed: bool,
}

/// The subject a conversation's tabs answer on.
pub fn subject(session: &VizierSession) -> Subject {
    Subject::ui_verify(&session.to_slug())
}

fn clip(value: &Value) -> String {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    text.chars().take(MAX_PROBLEM_CHARS).collect()
}

/// A reported problem, kept to the fields the contract names and cut to size.
fn tidy(problem: &Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("kind".into(), json!(clip(&problem["kind"])));
    out.insert("message".into(), json!(clip(&problem["message"])));
    for key in ["source", "detail"] {
        if !problem[key].is_null() {
            out.insert(key.into(), json!(clip(&problem[key])));
        }
    }
    for key in ["line", "column"] {
        if let Some(n) = problem[key].as_u64() {
            out.insert(key.into(), json!(n));
        }
    }
    Value::Object(out)
}

/// A reply as a verification outcome (data-model.md).
pub fn outcome(reply: &VerifyReply) -> Value {
    if reply.closed {
        return json!({ "state": "unverified", "reason": "closed" });
    }
    if reply.problems.is_empty() {
        return json!({ "state": "clean", "observed_ms": reply.observed_ms });
    }
    let kept: Vec<Value> = reply.problems.iter().take(MAX_PROBLEMS).map(tidy).collect();
    let mut value = json!({ "state": "problems", "observed_ms": reply.observed_ms, "problems": kept });
    if reply.problems.len() > MAX_PROBLEMS {
        value["more"] = json!(reply.problems.len() - MAX_PROBLEMS);
    }
    value
}

/// Ask the conversation's tabs to check `page_id` v`version`, and return the outcome.
pub async fn verify(transport: &VizierTransport, session: &VizierSession, page_id: &str, version: u32) -> Value {
    let request = VerifyRequest {
        request_id: uuid::Uuid::new_v4().simple().to_string(),
        page_id: page_id.to_string(),
        version,
        observe_ms: VERIFY_OBSERVE_MS,
    };
    let envelope = match Envelope::json(VERIFY_KIND, &request) {
        Ok(envelope) => envelope,
        Err(e) => {
            tracing::warn!(page_id, version, "could not build a verify request: {e}");
            return json!({ "state": "unverified", "reason": "timeout" });
        }
    };
    let answer = transport
        .bus()
        .messaging
        .request(&subject(session), envelope, VERIFY_DEADLINE)
        .await;
    match answer {
        Ok(reply) => match reply.decode::<VerifyReply>() {
            Ok(reply) => outcome(&reply),
            Err(e) => {
                tracing::warn!(page_id, version, "a tab sent an unreadable verify reply: {e}");
                json!({ "state": "unverified", "reason": "closed" })
            }
        },
        Err(BusError::NoResponders) => json!({ "state": "unverified", "reason": "no_tab" }),
        Err(BusError::Timeout) => json!({ "state": "unverified", "reason": "timeout" }),
        Err(e) => {
            tracing::warn!(page_id, version, "a verify request failed: {e}");
            json!({ "state": "unverified", "reason": "closed" })
        }
    }
}

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<VerifyReply>>>>;

/// One chat socket's answer to its conversation's verify requests (contracts/verify-protocol.md):
/// while it lives, it is subscribed on [`subject`], hands each request to its tab through `ask`
/// and replies with the tab's report. Dropping or closing it unsubscribes and answers every
/// request still waiting on it with `closed`, so a request never hangs on a socket that left.
pub struct VerifyResponder {
    pending: Pending,
    task: tokio::task::AbortHandle,
}

impl VerifyResponder {
    /// Start answering for `session`, or `None` for a conversation that has no pages (only WebUI
    /// and project chats do, FR-002) or when the bus refuses the subscription. `ask` sends the
    /// tab its `ui_verify` frame and returns `false` once the tab is gone.
    pub async fn start<F>(transport: &VizierTransport, session: &VizierSession, ask: F) -> Option<Self>
    where
        F: Fn(Value) -> bool + Send + Sync + 'static,
    {
        if !matches!(session.1, VizierChannelId::HTTP(..) | VizierChannelId::Project(..)) {
            return None;
        }
        let messaging = transport.bus().messaging.clone();
        let mut requests = match messaging.subscribe((&subject(session)).into()).await {
            Ok(subscription) => subscription,
            Err(e) => {
                tracing::warn!(?session, "a chat socket could not answer page checks: {e}");
                return None;
            }
        };
        let pending: Pending = Arc::default();
        let waiting = pending.clone();
        let task = tokio::spawn(async move {
            while let Some((_, envelope)) = requests.recv().await {
                let Some(reply_to) = envelope.reply_to.clone() else { continue };
                let Ok(request) = envelope.decode::<VerifyRequest>() else { continue };
                let (tx, rx) = oneshot::channel();
                lock(&waiting).insert(request.request_id.clone(), tx);
                if !ask(json!({ "ui_verify": request })) {
                    lock(&waiting).remove(&request.request_id);
                    continue;
                }
                let messaging = messaging.clone();
                let waiting = waiting.clone();
                tokio::spawn(async move {
                    let reply = match tokio::time::timeout(VERIFY_DEADLINE, rx).await {
                        Ok(Ok(reply)) => reply,
                        // The socket closed first.
                        Ok(Err(_)) => VerifyReply { request_id: request.request_id, closed: true, ..Default::default() },
                        // The requester has given up by now: nobody to answer.
                        Err(_) => {
                            lock(&waiting).remove(&request.request_id);
                            return;
                        }
                    };
                    if let Ok(envelope) = Envelope::json(VERIFY_KIND, &reply) {
                        let _ = messaging.publish(&reply_to, envelope).await;
                    }
                });
            }
        });
        Some(Self { pending, task: task.abort_handle() })
    }

    /// The tab's `ui_verify_report`. `false` when no request with that id waits on this socket
    /// (a late or foreign report, which is ignored). A tab never reports `closed` for itself.
    pub fn report(&self, payload: &Value) -> bool {
        let Ok(reply) = serde_json::from_value::<VerifyReply>(payload.clone()) else { return false };
        let Some(waiter) = lock(&self.pending).remove(&reply.request_id) else { return false };
        waiter.send(VerifyReply { closed: false, ..reply }).is_ok()
    }

    /// Stop answering; every waiting request is answered `closed`.
    pub fn close(&self) {
        self.task.abort();
        lock(&self.pending).clear();
    }
}

impl Drop for VerifyResponder {
    fn drop(&mut self) {
        self.close();
    }
}

fn lock(pending: &Pending) -> std::sync::MutexGuard<'_, HashMap<String, oneshot::Sender<VerifyReply>>> {
    pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_map_to_outcomes() {
        let clean = VerifyReply { request_id: "r".into(), observed_ms: 2000, ..Default::default() };
        assert_eq!(outcome(&clean), json!({ "state": "clean", "observed_ms": 2000 }));
        let closed = VerifyReply { closed: true, ..Default::default() };
        assert_eq!(outcome(&closed)["reason"], "closed");

        let many: Vec<Value> = (0..25).map(|i| json!({ "kind": "error", "message": format!("e{i}"), "line": i, "secret": "x" })).collect();
        let bad = VerifyReply { problems: many, observed_ms: 2000, ..Default::default() };
        let value = outcome(&bad);
        assert_eq!(value["state"], "problems");
        assert_eq!(value["problems"].as_array().unwrap().len(), MAX_PROBLEMS);
        assert_eq!(value["more"], 5);
        assert!(value["problems"][0].get("secret").is_none(), "only the contract's fields are kept");
    }

    #[tokio::test]
    async fn with_no_tab_the_answer_is_immediate() {
        let transport = VizierTransport::new();
        let session = VizierSession("a".into(), crate::schema::VizierChannelId::HTTP("u".into(), "webui".into()), Some("t".into()));
        let started = std::time::Instant::now();
        let value = verify(&transport, &session, "pg_1", 1).await;
        assert_eq!(value, json!({ "state": "unverified", "reason": "no_tab" }));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    async fn the_first_tab_report_is_the_outcome() {
        let transport = VizierTransport::new();
        let session = VizierSession("a".into(), crate::schema::VizierChannelId::HTTP("u".into(), "webui".into()), Some("t".into()));
        let mut tab = transport.bus().messaging.subscribe((&subject(&session)).into()).await.unwrap();
        let bus = transport.bus().messaging.clone();
        tokio::spawn(async move {
            let (_, envelope) = tab.recv().await.unwrap();
            let request: VerifyRequest = envelope.decode().unwrap();
            let reply = VerifyReply {
                request_id: request.request_id,
                problems: vec![json!({ "kind": "error", "message": "boom", "line": 3 })],
                observed_ms: 2000,
                closed: false,
            };
            bus.publish(&envelope.reply_to.unwrap(), Envelope::json(VERIFY_KIND, &reply).unwrap()).await.unwrap();
        });
        let value = verify(&transport, &session, "pg_1", 1).await;
        assert_eq!(value["state"], "problems");
        assert_eq!(value["problems"][0], json!({ "kind": "error", "message": "boom", "line": 3 }));
    }

    fn webui() -> VizierSession {
        VizierSession("a".into(), VizierChannelId::HTTP("u".into(), "webui".into()), Some("t".into()))
    }

    /// A responder whose tab is a channel the test reads.
    async fn tab(transport: &VizierTransport) -> (VerifyResponder, tokio::sync::mpsc::UnboundedReceiver<Value>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let responder = VerifyResponder::start(transport, &webui(), move |frame| tx.send(frame).is_ok()).await.unwrap();
        (responder, rx)
    }

    #[tokio::test]
    async fn a_socket_relays_the_request_to_its_tab_and_its_report_back() {
        let transport = VizierTransport::new();
        let (responder, mut frames) = tab(&transport).await;
        let asked = tokio::spawn({
            let transport = transport.clone();
            async move { verify(&transport, &webui(), "pg_1", 2).await }
        });
        let frame = frames.recv().await.unwrap();
        assert_eq!(frame["ui_verify"]["page_id"], "pg_1");
        assert_eq!(frame["ui_verify"]["version"], 2);
        assert_eq!(frame["ui_verify"]["observe_ms"], VERIFY_OBSERVE_MS);
        let request_id = frame["ui_verify"]["request_id"].as_str().unwrap().to_string();
        assert!(!responder.report(&json!({ "request_id": "someone-else", "problems": [] })));
        assert!(responder.report(&json!({ "request_id": request_id, "problems": [], "observed_ms": 2000, "closed": true })));
        assert_eq!(asked.await.unwrap(), json!({ "state": "clean", "observed_ms": 2000 }));
        assert!(!responder.report(&json!({ "request_id": request_id })), "a report counts once");
    }

    #[tokio::test]
    async fn a_socket_that_closes_answers_closed_and_stops_answering() {
        let transport = VizierTransport::new();
        let (responder, mut frames) = tab(&transport).await;
        let asked = tokio::spawn({
            let transport = transport.clone();
            async move { verify(&transport, &webui(), "pg_1", 1).await }
        });
        frames.recv().await.unwrap();
        drop(responder);
        assert_eq!(asked.await.unwrap(), json!({ "state": "unverified", "reason": "closed" }));
        tokio::task::yield_now().await;
        assert_eq!(verify(&transport, &webui(), "pg_1", 1).await["reason"], "no_tab", "unsubscribed");
    }

    #[tokio::test]
    async fn only_webui_and_project_chats_answer() {
        let transport = VizierTransport::new();
        let task = VizierSession("a".into(), VizierChannelId::Task("t".into(), crate::time::now()), None);
        assert!(VerifyResponder::start(&transport, &task, |_| true).await.is_none());
        let project = VizierSession("a".into(), VizierChannelId::Project("u".into(), "p".into()), Some("t".into()));
        assert!(VerifyResponder::start(&transport, &project, |_| true).await.is_some());
    }
}
