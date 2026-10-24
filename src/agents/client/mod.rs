//! Client tools (`specs/019-client-tools`): tools a WebSocket client declares on a request,
//! offered to that one turn's scripts and answered by the client that declared them.
//!
//! The declaration rides on the request ([`ClientBinding`], research R1), is resolved against
//! the agent's catalogue at turn start ([`ClientTools::resolve`], R4) and reaches scripts
//! through `ToolContext.client` rather than the router (R5). Each call is saved to history
//! when it is made, its history uid being the call id, and goes straight to the declaring
//! connection (R2, R7).

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use serde::Serialize;
use tracing::Instrument;

use crate::{
    agents::tools::{
        CatalogueEntry, CategorySummary, ToolTarget, assign_names, client_category_name, with_ends_turn_notice,
        execute_tool_span, record_tool_arguments, record_tool_result,
    },
    llm::types::ToolDefinition,
    schema::{
        ClientCallOutcome, SessionHistoryContent, VizierResponse, VizierResponseContent,
        VizierSession,
    },
    storage::{VizierStorage, client_call::ClientCallStorage},
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

pub mod connection;
pub mod declare;

pub use connection::{ClientAnswer, ClientConnection};
pub use declare::{ClientCategoryDeclaration, ClientToolDeclaration};

/// The protocol version `client_tools_resolved` announces.
pub const PROTOCOL_VERSION: u32 = 1;

/// A request's checked declarations and the connection that answers them.
#[derive(Clone)]
pub struct ClientBinding {
    pub connection: Arc<ClientConnection>,
    pub tools: Vec<ClientToolDeclaration>,
    pub categories: Vec<ClientCategoryDeclaration>,
}

impl std::fmt::Debug for ClientBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBinding")
            .field("tools", &self.tools.len())
            .field("categories", &self.categories.len())
            .finish()
    }
}

/// One call as the protocol's `client_tool_call` frame carries it (§3).
#[derive(Debug, Clone, Serialize)]
pub struct ClientCallFrame {
    pub call_id: String,
    pub name: String,
    pub category: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

impl ClientCallFrame {
    /// The frame, with `answer` saying whether the receiving socket is the one to answer.
    pub fn wire(&self, answer: bool) -> serde_json::Value {
        let mut call = serde_json::to_value(self).unwrap_or_default();
        call["answer"] = serde_json::json!(answer);
        serde_json::json!({ "client_tool_call": call })
    }
}

struct ClientEntry {
    name: String,
    category_name: String,
    declaration: ClientToolDeclaration,
}

/// One turn's resolved client tools.
pub struct ClientTools {
    connection: Arc<ClientConnection>,
    entries: Vec<ClientEntry>,
    catalogue: Vec<CatalogueEntry>,
    categories: Vec<CategorySummary>,
    session: VizierSession,
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
}

const DESCRIPTION_PREFIX: &str = "(Provided by the person's client.) ";
const DEFERRABLE_PREFIX: &str = "(Provided by the person's client. If no client is connected, the call is kept and runs when one reconnects; you then get {\"status\": \"deferred\"} instead of a result.) ";
/// How long a reconnected client has to answer a deferred call.
const DEFERRED_ANSWER_LIMIT: Duration = Duration::from_secs(declare::DEFAULT_TIMEOUT_SECS);
const CATEGORY_LABEL: &str = "(provided by the person's client)";
const DEFAULT_CATEGORY_DESCRIPTION: &str = "Tools provided by the person's client";

impl ClientTools {
    /// Name the binding's tools against the agent's `catalogue`, the way MCP tools are named:
    /// a category clashing with a built-in one or with `reserved` gets `_client`, every name
    /// already in the catalogue is taken first, and collisions get `_2`, `_3`, ….
    pub fn resolve(
        binding: &ClientBinding,
        catalogue: &[CatalogueEntry],
        reserved: &[String],
        session: VizierSession,
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
    ) -> Arc<Self> {
        let category_names: Vec<String> = binding
            .tools
            .iter()
            .map(|tool| client_category_name(tool.category(), reserved))
            .collect();
        let taken: Vec<&str> = catalogue.iter().map(|entry| entry.name.as_str()).collect();
        let pairs: Vec<(&str, &str)> = binding
            .tools
            .iter()
            .zip(&category_names)
            .map(|(tool, category)| (category.as_str(), tool.name.as_str()))
            .collect();
        let names = assign_names(&taken, &pairs);

        let entries: Vec<ClientEntry> = binding
            .tools
            .iter()
            .zip(category_names)
            .zip(names)
            .map(|((declaration, category_name), name)| ClientEntry {
                name,
                category_name,
                declaration: declaration.clone(),
            })
            .collect();

        let catalogue = entries
            .iter()
            .map(|entry| CatalogueEntry {
                name: entry.name.clone(),
                category: entry.category_name.clone(),
                target: ToolTarget::Client {
                    category: entry.declaration.category().to_string(),
                    tool: entry.declaration.name.clone(),
                },
                definition: ToolDefinition {
                    name: entry.name.clone(),
                    description: with_ends_turn_notice(
                        format!(
                            "{}{}",
                            if entry.declaration.deferrable { DEFERRABLE_PREFIX } else { DESCRIPTION_PREFIX },
                            entry.declaration.description
                        ),
                        entry.declaration.ends_turn,
                    ),
                    parameters: entry.declaration.input_schema.clone(),
                },
                ends_turn: entry.declaration.ends_turn,
            })
            .collect();

        // One line per category, in the order its first tool was declared. Two declared
        // categories can land on one name (`memory` and `memory_client`); the first wins.
        let mut categories: Vec<CategorySummary> = vec![];
        for entry in &entries {
            if categories.iter().any(|c| c.name == entry.category_name) {
                continue;
            }
            let description = binding
                .categories
                .iter()
                .find(|c| c.name == entry.declaration.category())
                .and_then(|c| c.description.clone())
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_CATEGORY_DESCRIPTION.to_string());
            categories.push(CategorySummary {
                name: entry.category_name.clone(),
                description: format!("{CATEGORY_LABEL} {description}"),
            });
        }

        Arc::new(Self {
            connection: binding.connection.clone(),
            entries,
            catalogue,
            categories,
            session,
            storage,
            transport,
        })
    }

    pub fn catalogue(&self) -> &[CatalogueEntry] {
        &self.catalogue
    }

    pub fn categories(&self) -> &[CategorySummary] {
        &self.categories
    }

    /// The declaration behind a catalogue name, for its output schema.
    pub fn declaration(&self, name: &str) -> Option<&ClientToolDeclaration> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.declaration)
    }

    /// The `client_tools_resolved` frame (§2).
    pub fn resolved_frame(&self) -> serde_json::Value {
        let tools: Vec<serde_json::Value> = self
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "category": entry.declaration.category(),
                    "tool": entry.declaration.name,
                    "category_name": entry.category_name,
                    "name": entry.name,
                    "ends_turn": entry.declaration.ends_turn,
                })
            })
            .collect();
        serde_json::json!({
            "client_tools_resolved": { "protocol": PROTOCOL_VERSION, "tools": tools }
        })
    }

    /// Tell the declaring connection the final names, before the model runs.
    pub fn announce(&self) {
        self.connection.send(self.resolved_frame());
    }

    /// Call `name` on the client and wait for its answer (research R7). A disconnect, a
    /// timeout or the client's own error comes back as `Err`, which a script can catch; if
    /// the call is dropped first (the turn aborted, the script's time ran out), it is
    /// recorded as cancelled and the client told.
    ///
    /// The call id comes back with the result (`None` only for a name this turn doesn't
    /// have), so the caller can refer to the stored call instead of storing it again.
    pub async fn call(
        &self,
        name: &str,
        arguments: serde_json::Value,
        redactor: Option<&crate::secrets::SecretRedactor>,
    ) -> (Option<String>, anyhow::Result<VizierResponse>) {
        let span = execute_tool_span(name, None);
        if self.entries.iter().any(|entry| entry.name == name && entry.declaration.ends_turn) {
            span.record("vizier.tool.ends_turn", true);
        }
        let recorded = arguments.to_string();
        let (call_id, result) = self.round_trip(name, arguments).instrument(span.clone()).await;
        record_tool_arguments(&span, &recorded, redactor);
        record_tool_result(&span, &result, redactor);
        (call_id, result)
    }

    async fn round_trip(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> (Option<String>, anyhow::Result<VizierResponse>) {
        let Some(entry) = self.entries.iter().find(|entry| entry.name == name) else {
            return (None, Err(anyhow::anyhow!("{name} not found")));
        };

        let call = ClientCallFrame {
            call_id: String::new(),
            name: entry.name.clone(),
            category: entry.declaration.category().to_string(),
            tool: entry.declaration.name.clone(),
            arguments,
        };

        // Saved now, not with the turn's other entries, so the call survives a failed or
        // aborted turn and has a stable id for saved state and client events.
        let saved = crate::storage::history::save_with_busy_retry(
            &self.storage,
            &self.session,
            SessionHistoryContent::ClientToolCall {
                name: call.name.clone(),
                category: call.category.clone(),
                tool: call.tool.clone(),
                arguments: call.arguments.clone(),
                outcome: None,
                state: None,
            },
        )
        .await;
        let call_id = match saved {
            Ok(uid) => {
                if let Err(e) = self.storage.open_client_call(&uid, &self.session).await {
                    tracing::warn!(call_id = %uid, "failed to record client tool call: {e}");
                }
                uid
            }
            Err(e) => {
                let uid = uuid::Uuid::new_v4().to_string();
                tracing::warn!(call_id = %uid, "failed to save client tool call to history, calling anyway: {e}");
                uid
            }
        };
        let call = ClientCallFrame { call_id: call_id.clone(), ..call };

        let mut pending = PendingCall {
            connection: self.connection.clone(),
            storage: self.storage.clone(),
            transport: self.transport.clone(),
            session: self.session.clone(),
            call_id: call_id.clone(),
            done: false,
        };

        let outcome = match self.connection.register(&call_id) {
            None => ClientCallOutcome::Disconnected,
            Some(answer) => {
                self.connection.send(call.wire(true));
                self.transport.publish_session_event(SessionEvent {
                    session: self.session.clone(),
                    frame: SessionFrame::ClientToolCall {
                        connection_id: self.connection.id.clone(),
                        call,
                    },
                });

                let limit = entry.declaration.timeout();
                match tokio::time::timeout(limit, answer).await {
                    Ok(Ok(ClientAnswer::Ok(result))) => ClientCallOutcome::Ok { result },
                    Ok(Ok(ClientAnswer::Err(message))) => ClientCallOutcome::Error { message },
                    Ok(Ok(ClientAnswer::Disconnected)) | Ok(Err(_)) => ClientCallOutcome::Disconnected,
                    Err(_) => {
                        self.connection.cancel(&call_id);
                        ClientCallOutcome::Timeout
                    }
                }
            }
        };
        pending.done = true;

        // A deferrable tool's call outlives the connection that should have answered it: it
        // waits for a client to reconnect (research R18), and the script is told so.
        if outcome == ClientCallOutcome::Disconnected && entry.declaration.deferrable {
            self.defer(&call_id).await;
            let response = serde_json::json!({
                "status": "deferred",
                "call_id": call_id,
                "message": "The person's client is not connected. This call will run the next time a client opens this conversation; its result will not be reported back to you.",
            });
            return (
                Some(call_id),
                Ok(VizierResponse {
                    timestamp: Utc::now(),
                    content: VizierResponseContent::ToolResponse { response },
                    ..Default::default()
                }),
            );
        }

        finish(&self.storage, &self.transport, &self.session, &call_id, &outcome).await;

        let result = match outcome {
            ClientCallOutcome::Ok { result } => Ok(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::ToolResponse { response: result },
                ..Default::default()
            }),
            ClientCallOutcome::Error { message } => Err(anyhow::anyhow!(message)),
            ClientCallOutcome::Timeout => Err(anyhow::anyhow!(
                "timeout: the client did not answer within {}s",
                entry.declaration.timeout().as_secs()
            )),
            _ => Err(anyhow::anyhow!(
                "client_disconnected: the client that provides this tool is no longer connected"
            )),
        };
        (Some(call_id), result)
    }
}

impl ClientTools {
    async fn defer(&self, call_id: &str) {
        if let Err(e) = self.storage.defer_client_call(call_id).await {
            tracing::warn!(call_id, "failed to record client tool call as deferred: {e}");
        }
        for frame in [
            SessionFrame::ClientCallOutcome {
                call_id: call_id.to_string(),
                outcome: ClientCallOutcome::Deferred,
            },
            // Any socket already open on the session (another tab) picks it up at once.
            SessionFrame::ClientCallDeferred { call_id: call_id.to_string() },
        ] {
            self.transport.publish_session_event(SessionEvent {
                session: self.session.clone(),
                frame,
            });
        }
    }
}

/// The deferred calls some connection is delivering right now, process-wide, so two sockets
/// on one conversation never both run the same call.
#[derive(Default)]
pub struct DeferredClaims(std::sync::Mutex<std::collections::HashSet<String>>);

/// Held while a connection delivers a call; releases the claim when dropped.
pub struct DeferredClaim<'a> {
    claims: &'a DeferredClaims,
    call_id: String,
}

impl DeferredClaims {
    pub fn claim(&self, call_id: &str) -> Option<DeferredClaim<'_>> {
        let mut claimed = self.0.lock().ok()?;
        claimed.insert(call_id.to_string()).then(|| DeferredClaim {
            claims: self,
            call_id: call_id.to_string(),
        })
    }
}

impl Drop for DeferredClaim<'_> {
    fn drop(&mut self) {
        if let Ok(mut claimed) = self.claims.0.lock() {
            claimed.remove(&self.call_id);
        }
    }
}

/// Deliver one deferred call to `connection` and record its answer (research R18). Does
/// nothing if another connection is delivering it, or it is no longer a deferred call of
/// `session`. A connection that closes before answering leaves it deferred for the next one.
pub async fn deliver_deferred(
    storage: &VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    connection: &ClientConnection,
    claims: &DeferredClaims,
    call_id: &str,
) {
    use crate::storage::reaction::ReactionStorage;

    let Some(_claim) = claims.claim(call_id) else {
        return;
    };
    let entry = match storage.get_history_entry(call_id).await {
        Ok(Some(entry)) if &entry.vizier_session == session => entry,
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(call_id, "failed to read deferred client tool call: {e}");
            return;
        }
    };
    let SessionHistoryContent::ClientToolCall { name, category, tool, arguments, outcome: Some(ClientCallOutcome::Deferred), .. } =
        entry.content
    else {
        return;
    };
    let Some(answer) = connection.register(call_id) else {
        return;
    };

    let call = ClientCallFrame { call_id: call_id.to_string(), name, category, tool, arguments };
    let mut frame = call.wire(true);
    frame["client_tool_call"]["deferred"] = serde_json::json!(true);
    connection.send(frame);

    let outcome = match tokio::time::timeout(DEFERRED_ANSWER_LIMIT, answer).await {
        Ok(Ok(ClientAnswer::Ok(result))) => ClientCallOutcome::Ok { result },
        Ok(Ok(ClientAnswer::Err(message))) => ClientCallOutcome::Error { message },
        // Still deferred: the next connection gets it.
        Ok(Ok(ClientAnswer::Disconnected)) | Ok(Err(_)) => return,
        Err(_) => {
            connection.cancel(call_id);
            ClientCallOutcome::Timeout
        }
    };
    finish(storage, transport, session, call_id, &outcome).await;
}

/// Record a call's outcome and tell every socket on the session.
async fn finish(
    storage: &VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    call_id: &str,
    outcome: &ClientCallOutcome,
) {
    let publish = match storage.finish_client_call(call_id, outcome).await {
        Ok(applied) => applied,
        Err(e) => {
            tracing::warn!(call_id, "failed to record client tool call outcome: {e}");
            true
        }
    };
    if publish {
        transport.publish_session_event(SessionEvent {
            session: session.clone(),
            frame: SessionFrame::ClientCallOutcome {
                call_id: call_id.to_string(),
                outcome: outcome.clone(),
            },
        });
    }
}

/// Ends a call abandoned before its outcome: the turn was aborted, or the script running it
/// was cut off, dropping the future that awaited the answer.
struct PendingCall {
    connection: Arc<ClientConnection>,
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    session: VizierSession,
    call_id: String,
    done: bool,
}

impl Drop for PendingCall {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        self.connection.cancel(&self.call_id);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (storage, transport, session, call_id) = (
            self.storage.clone(),
            self.transport.clone(),
            self.session.clone(),
            std::mem::take(&mut self.call_id),
        );
        runtime.spawn(async move {
            finish(&storage, &transport, &session, &call_id, &ClientCallOutcome::Cancelled).await;
        });
    }
}

/// How long a dropped call's cancellation write may take to land, for tests.
#[cfg(test)]
const SETTLE: Duration = Duration::from_millis(200);

#[cfg(test)]
pub(crate) fn test_storage() -> Arc<VizierStorage> {
    use parking_lot::Mutex;
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::storage::sqlite::init_history_schema(&conn).unwrap();
    crate::storage::sqlite::init_reaction_schema(&conn).unwrap();
    crate::storage::sqlite::init_client_call_schema(&conn).unwrap();
    Arc::new(VizierStorage::new(crate::storage::sqlite::SqliteStorage::new(
        Arc::new(Mutex::new(conn)),
        Arc::new(crate::storage::document::LocalDocumentStore::new(std::env::temp_dir())),
    )))
}

#[cfg(test)]
impl ClientTools {
    /// One declared tool, resolved against `catalogue`, on a test connection.
    pub fn for_tests(
        catalogue: &[CatalogueEntry],
        reserved: &[String],
        category: &str,
        tool: &str,
    ) -> Arc<Self> {
        let (connection, _rx) = ClientConnection::for_tests();
        let binding = ClientBinding {
            connection,
            tools: vec![ClientToolDeclaration {
                name: tool.into(),
                category: Some(category.into()),
                description: "test tool".into(),
                input_schema: serde_json::json!({ "type": "object" }),
                output_schema: None,
                timeout_secs: None,
                deferrable: false,
            ends_turn: false,
            }],
            categories: vec![],
        };
        Self::resolve(
            &binding,
            catalogue,
            reserved,
            VizierSession("a".into(), crate::schema::VizierChannelId::System, None),
            test_storage(),
            VizierTransport::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::ws::Message;
    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        agents::client::connection::frame_json,
        schema::VizierChannelId,
        storage::history::HistoryStorage,
    };

    fn declaration(name: &str, category: Option<&str>) -> ClientToolDeclaration {
        ClientToolDeclaration {
            name: name.into(),
            category: category.map(str::to_string),
            description: format!("{name} does a thing"),
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            timeout_secs: None,
            deferrable: false,
            ends_turn: false,
        }
    }

    fn native(name: &str, category: &str) -> CatalogueEntry {
        CatalogueEntry {
            name: name.into(),
            category: category.into(),
            target: ToolTarget::Native(name.into()),
            definition: ToolDefinition {
                name: name.into(),
                description: String::new(),
                parameters: json!({}),
            },
            ends_turn: false,
        }
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        )
    }

    struct Setup {
        tools: Arc<ClientTools>,
        connection: Arc<ClientConnection>,
        rx: mpsc::Receiver<Message>,
        storage: Arc<VizierStorage>,
        transport: VizierTransport,
    }

    fn setup(declarations: Vec<ClientToolDeclaration>, categories: Vec<ClientCategoryDeclaration>) -> Setup {
        let (connection, rx) = ClientConnection::for_tests();
        let storage = test_storage();
        let transport = VizierTransport::new();
        let binding = ClientBinding {
            connection: connection.clone(),
            tools: declarations,
            categories,
        };
        let tools = ClientTools::resolve(
            &binding,
            &[native("memory_search", "memory"), native("github_create_issue", "github")],
            &["github".to_string()],
            session(),
            storage.clone(),
            transport.clone(),
        );
        Setup { tools, connection, rx, storage, transport }
    }

    fn resolved(setup: &Setup) -> Vec<(String, String)> {
        setup.tools.resolved_frame()["client_tools_resolved"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t["category_name"].as_str().unwrap().into(), t["name"].as_str().unwrap().into()))
            .collect()
    }

    #[test]
    fn names_follow_the_category_rules() {
        let s = setup(
            vec![
                declaration("echo", None),
                declaration("search", Some("memory")),
                declaration("create_issue", Some("github")),
                declaration("pick_color", Some("palette")),
            ],
            vec![ClientCategoryDeclaration { name: "palette".into(), description: Some("Colour pickers".into()) }],
        );
        assert_eq!(
            resolved(&s),
            [
                ("client".to_string(), "client_echo".to_string()),
                ("memory_client".into(), "memory_client_search".into()),
                ("github_client".into(), "github_client_create_issue".into()),
                ("palette".into(), "palette_pick_color".into()),
            ]
        );
        assert_eq!(s.tools.resolved_frame()["client_tools_resolved"]["protocol"], 1);

        let lines: Vec<(String, String)> =
            s.tools.categories().iter().map(|c| (c.name.clone(), c.description.clone())).collect();
        assert_eq!(lines[0], ("client".into(), "(provided by the person's client) Tools provided by the person's client".into()));
        assert_eq!(lines[3], ("palette".into(), "(provided by the person's client) Colour pickers".into()));

        let described = &s.tools.catalogue()[3].definition.description;
        assert_eq!(described, "(Provided by the person's client.) pick_color does a thing");
    }

    #[test]
    fn a_name_already_taken_gets_a_number() {
        // `memory_client_search` would be free, so take it in the catalogue too.
        let (connection, _rx) = ClientConnection::for_tests();
        let binding = ClientBinding {
            connection,
            tools: vec![declaration("b_c", Some("a")), declaration("c", Some("a_b"))],
            categories: vec![],
        };
        let tools = ClientTools::resolve(
            &binding,
            &[native("a_b_c", "x")],
            &[],
            session(),
            test_storage(),
            VizierTransport::new(),
        );
        let names: Vec<&str> = tools.catalogue().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a_b_c_2", "a_b_c_3"]);
    }

    #[test]
    fn the_same_declaration_resolves_the_same_way() {
        let make = || setup(vec![declaration("echo", None), declaration("pick", Some("palette"))], vec![]);
        let (a, b) = (make(), make());
        assert_eq!(a.tools.resolved_frame(), b.tools.resolved_frame());
        assert_eq!(a.tools.categories(), b.tools.categories());
    }

    async fn outcome_of(storage: &VizierStorage, uid: &str) -> Option<ClientCallOutcome> {
        let history = storage.list_session_history(session(), None, None, None).await.unwrap();
        history.into_iter().find(|e| e.uid == uid).and_then(|e| match e.content {
            SessionHistoryContent::ClientToolCall { outcome, .. } => outcome,
            _ => None,
        })
    }

    async fn next_call(rx: &mut mpsc::Receiver<Message>) -> serde_json::Value {
        loop {
            let frame = frame_json(rx.recv().await.unwrap());
            if frame.get("client_tool_call").is_some() {
                return frame["client_tool_call"].clone();
            }
        }
    }

    #[tokio::test]
    async fn an_answered_call_returns_the_result() {
        let mut s = setup(vec![declaration("pick", Some("palette"))], vec![]);
        let mut events = s.transport.subscribe_session_events();
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("palette_pick", json!({ "prompt": "x" }), None).await.1 });

        let frame = next_call(&mut s.rx).await;
        assert_eq!(frame["answer"], true);
        assert_eq!(frame["tool"], "pick");
        assert_eq!(frame["arguments"]["prompt"], "x");
        let id = frame["call_id"].as_str().unwrap().to_string();
        assert!(s.connection.answer(&id, ClientAnswer::Ok(json!({ "hex": "#3366ff" }))));

        let response = call.await.unwrap().unwrap();
        assert!(matches!(response.content, VizierResponseContent::ToolResponse { ref response } if response["hex"] == "#3366ff"));
        assert_eq!(
            outcome_of(&s.storage, &id).await,
            Some(ClientCallOutcome::Ok { result: json!({ "hex": "#3366ff" }) })
        );
        // The display copy went out, then the outcome.
        assert!(matches!(events.recv().await.unwrap().frame, SessionFrame::ClientToolCall { .. }));
        assert!(matches!(events.recv().await.unwrap().frame, SessionFrame::ClientCallOutcome { .. }));
    }

    #[tokio::test]
    async fn a_client_error_is_an_error() {
        let mut s = setup(vec![declaration("pick", None)], vec![]);
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("client_pick", json!({}), None).await.1 });
        let id = next_call(&mut s.rx).await["call_id"].as_str().unwrap().to_string();
        s.connection.answer(&id, ClientAnswer::Err("closed".into()));
        assert_eq!(call.await.unwrap().unwrap_err().to_string(), "closed");
        assert_eq!(outcome_of(&s.storage, &id).await, Some(ClientCallOutcome::Error { message: "closed".into() }));
    }

    #[tokio::test]
    async fn closing_the_connection_mid_call_disconnects_it() {
        let mut s = setup(vec![declaration("pick", None)], vec![]);
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("client_pick", json!({}), None).await.1 });
        let id = next_call(&mut s.rx).await["call_id"].as_str().unwrap().to_string();
        s.connection.close();
        let err = call.await.unwrap().unwrap_err().to_string();
        assert!(err.starts_with("client_disconnected"), "{err}");
        assert_eq!(outcome_of(&s.storage, &id).await, Some(ClientCallOutcome::Disconnected));
    }

    #[tokio::test]
    async fn an_unanswered_call_times_out_and_is_cancelled() {
        let mut timed = declaration("pick", None);
        timed.timeout_secs = Some(1);
        let mut s = setup(vec![timed], vec![]);
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("client_pick", json!({}), None).await.1 });
        let id = next_call(&mut s.rx).await["call_id"].as_str().unwrap().to_string();

        let err = call.await.unwrap().unwrap_err().to_string();
        assert!(err.starts_with("timeout"), "{err}");
        let cancelled = frame_json(s.rx.recv().await.unwrap());
        assert_eq!(cancelled["client_tool_cancelled"]["call_id"], id.as_str());
        assert_eq!(outcome_of(&s.storage, &id).await, Some(ClientCallOutcome::Timeout));
        // A late answer is not pending.
        assert!(!s.connection.answer(&id, ClientAnswer::Ok(json!(1))));
    }

    #[tokio::test]
    async fn a_dropped_call_is_cancelled() {
        let mut s = setup(vec![declaration("pick", None)], vec![]);
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("client_pick", json!({}), None).await.1 });
        let id = next_call(&mut s.rx).await["call_id"].as_str().unwrap().to_string();

        call.abort();
        let _ = call.await;
        let cancelled = frame_json(s.rx.recv().await.unwrap());
        assert_eq!(cancelled["client_tool_cancelled"]["call_id"], id.as_str());
        tokio::time::sleep(SETTLE).await;
        assert_eq!(outcome_of(&s.storage, &id).await, Some(ClientCallOutcome::Cancelled));
        assert!(!s.connection.answer(&id, ClientAnswer::Ok(json!(1))));
    }

    fn deferrable(name: &str) -> ClientToolDeclaration {
        ClientToolDeclaration { deferrable: true, ..declaration(name, None) }
    }

    #[tokio::test]
    async fn a_deferrable_call_without_a_client_is_deferred_then_delivered() {
        let s = setup(vec![deferrable("pick")], vec![]);
        assert!(s.tools.catalogue()[0].definition.description.contains("deferred"));
        let mut events = s.transport.subscribe_session_events();
        s.connection.close();

        let (call_id, result) = s.tools.call("client_pick", json!({ "prompt": "x" }), None).await;
        let call_id = call_id.unwrap();
        let response = result.unwrap();
        assert!(matches!(response.content, VizierResponseContent::ToolResponse { ref response } if response["status"] == "deferred"));
        assert_eq!(outcome_of(&s.storage, &call_id).await, Some(ClientCallOutcome::Deferred));
        assert!(matches!(events.recv().await.unwrap().frame, SessionFrame::ClientCallOutcome { outcome: ClientCallOutcome::Deferred, .. }));
        assert!(matches!(events.recv().await.unwrap().frame, SessionFrame::ClientCallDeferred { .. }));
        assert_eq!(s.storage.deferred_client_calls(&session()).await.unwrap(), [call_id.clone()]);

        // A connection that drops before answering leaves it deferred.
        let claims = DeferredClaims::default();
        let (dropped, mut dropped_rx) = ClientConnection::for_tests();
        let delivery = {
            let (storage, transport, dropped, call_id) = (s.storage.clone(), s.transport.clone(), dropped.clone(), call_id.clone());
            let claims = Arc::new(claims);
            let c = claims.clone();
            (tokio::spawn(async move { deliver_deferred(&storage, &transport, &session(), &dropped, &c, &call_id).await }), claims)
        };
        let frame = next_call(&mut dropped_rx).await;
        assert_eq!(frame["deferred"], true);
        assert_eq!(frame["arguments"]["prompt"], "x");
        // Claimed: a second connection does not get it meanwhile.
        assert!(delivery.1.claim(&call_id).is_none());
        dropped.close();
        delivery.0.await.unwrap();
        assert_eq!(outcome_of(&s.storage, &call_id).await, Some(ClientCallOutcome::Deferred));

        // The next connection answers it.
        let (next, mut next_rx) = ClientConnection::for_tests();
        let task = {
            let (storage, transport, next, call_id, claims) = (s.storage.clone(), s.transport.clone(), next.clone(), call_id.clone(), delivery.1.clone());
            tokio::spawn(async move { deliver_deferred(&storage, &transport, &session(), &next, &claims, &call_id).await })
        };
        next_call(&mut next_rx).await;
        assert!(next.answer(&call_id, ClientAnswer::Ok(json!({ "hex": "#fff" }))));
        task.await.unwrap();
        assert_eq!(outcome_of(&s.storage, &call_id).await, Some(ClientCallOutcome::Ok { result: json!({ "hex": "#fff" }) }));
        assert!(s.storage.deferred_client_calls(&session()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_deferrable_call_whose_client_drops_mid_call_is_deferred() {
        let mut s = setup(vec![deferrable("pick")], vec![]);
        let tools = s.tools.clone();
        let call = tokio::spawn(async move { tools.call("client_pick", json!({}), None).await });
        let id = next_call(&mut s.rx).await["call_id"].as_str().unwrap().to_string();
        s.connection.close();
        let (_, result) = call.await.unwrap();
        assert!(matches!(result.unwrap().content, VizierResponseContent::ToolResponse { ref response } if response["status"] == "deferred"));
        assert_eq!(outcome_of(&s.storage, &id).await, Some(ClientCallOutcome::Deferred));
    }

    #[tokio::test]
    async fn a_call_on_a_closed_connection_fails_at_once() {
        let s = setup(vec![declaration("pick", None)], vec![]);
        s.connection.close();
        let err = s.tools.call("client_pick", json!({}), None).await.1.unwrap_err().to_string();
        assert!(err.starts_with("client_disconnected"), "{err}");
    }

    #[test]
    fn a_flagged_client_tool_carries_the_notice_once_after_the_prefix_and_echoes_the_flag() {
        let s = setup(
            vec![
                ClientToolDeclaration { ends_turn: true, ..declaration("render", Some("ui")) },
                declaration("plain", Some("ui")),
            ],
            vec![],
        );
        let catalogue = s.tools.catalogue();
        assert_eq!(
            catalogue[0].definition.description,
            format!(
                "{DESCRIPTION_PREFIX}render does a thing {}",
                crate::agents::tools::ENDS_TURN_NOTICE
            )
        );
        assert!(catalogue[0].ends_turn);
        assert!(!catalogue[1].definition.description.contains(crate::agents::tools::ENDS_TURN_NOTICE));
        assert!(!catalogue[1].ends_turn);

        let frame = s.tools.resolved_frame();
        let tools = frame["client_tools_resolved"]["tools"].as_array().unwrap();
        assert_eq!(tools[0]["ends_turn"], true);
        assert_eq!(tools[1]["ends_turn"], false);
    }

    /// The 2 KB limit is on the author's own text: the notice vizier adds afterwards does not
    /// make a description that just fit a refusal (spec 026 FR-009).
    #[test]
    fn a_description_at_the_limit_is_accepted_and_still_gets_the_notice() {
        let tool = ClientToolDeclaration {
            description: "x".repeat(2048),
            ends_turn: true,
            ..declaration("render", Some("ui"))
        };
        assert_eq!(declare::validate(&[tool.clone()], &[]), Ok(()));
        let s = setup(vec![tool], vec![]);
        let described = &s.tools.catalogue()[0].definition.description;
        assert!(described.len() > 2048);
        assert!(described.ends_with(crate::agents::tools::ENDS_TURN_NOTICE));
    }
}
