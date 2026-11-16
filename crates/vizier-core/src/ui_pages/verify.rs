//! Checking a new version in an open tab (`specs/063-html-generative-ui` research R6,
//! contracts/verify-protocol.md). One bus request on `ui.{session}.verify`: every chat socket
//! watching the conversation answers it by asking its tab to mount the version off-screen, and the
//! first report wins. No responder means no tab, at once; a deadline means a tab that did not
//! answer. Either way the turn never waits for nobody.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vizier_schema::{BusError, Envelope, Subject};

use crate::{schema::VizierSession, transport::VizierTransport};

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
}
