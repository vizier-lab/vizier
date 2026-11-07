//! One language-server process: spawn, handshake, requests, progress, readiness and stop.
//!
//! Everything a server tells us arrives on one reader task. Requests are answered by id through a
//! pending map, so a stuck server delays only the calls waiting on it (FR-036).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::{Notify, mpsc, oneshot, watch};

use super::position::{Encoding, path_to_uri};
use super::rpc::{self, Incoming, RpcError};
use crate::agents::shell::process_tree;

/// A request with no answer by now is `lsp_timeout`.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a server may take to answer `initialize` (jdtls is slow).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(120);
/// Quiet time after the last progress before a server counts as ready (R8).
const QUIET: Duration = Duration::from_secs(1);
const STDERR_CAP: usize = 8 * 1024;

/// Everything needed to start one server.
#[derive(Debug, Clone)]
pub struct ServerSpec {
    pub root: PathBuf,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub initialization_options: Option<Value>,
}

/// Why a start did not produce a running server.
#[derive(Debug, Clone, PartialEq)]
pub enum StartError {
    /// The command is not installed (could not be spawned, or reported that it is missing).
    Missing(String),
    /// It started but never became usable.
    Failed(String),
}

/// Why a request had no answer.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestError {
    Timeout,
    Exited(String),
    Server(RpcError),
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestError::Timeout => write!(f, "no answer in {}s", REQUEST_TIMEOUT.as_secs()),
            RequestError::Exited(r) => write!(f, "server stopped: {r}"),
            RequestError::Server(e) => write!(f, "server error {}: {}", e.code, e.message),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub pull_diagnostics: bool,
    pub inter_file_dependencies: bool,
    pub raw: Value,
}

#[derive(Debug, Clone)]
pub struct ExitInfo {
    pub code: Option<i32>,
    pub stderr: String,
    /// We asked it to stop.
    pub requested: bool,
}

#[derive(Debug, Default)]
pub(super) struct Progress {
    /// Open work-done tokens, in the order they began.
    pub open: Vec<(String, String, Option<u32>)>,
    pub last_activity: Option<Instant>,
}

/// A published (pushed) diagnostics list.
#[derive(Debug, Clone)]
pub struct Pushed {
    pub version: Option<i64>,
    pub items: Vec<Value>,
    pub seq: u64,
}

enum Out {
    Frame(Value),
    Close,
}

pub struct Server {
    pub spec: ServerSpec,
    negotiated: std::sync::OnceLock<(Encoding, Capabilities)>,
    out: mpsc::UnboundedSender<Out>,
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, RpcError>>>>,
    next_id: AtomicI64,
    pub(super) progress: Mutex<Progress>,
    pub(super) pushed: Mutex<HashMap<String, Pushed>>,
    pub(super) push_seq: Mutex<u64>,
    /// Woken on every published diagnostics list and on progress changes.
    pub(super) activity: Notify,
    stderr: Mutex<VecDeque<u8>>,
    ready: watch::Sender<bool>,
    exited: watch::Sender<Option<ExitInfo>>,
    stop: Notify,
    stopping: Mutex<bool>,
    in_flight: AtomicUsize,
    last_used: Mutex<Instant>,
    on_change: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    /// Open documents and file snapshots (`sync.rs`).
    pub(super) docs: tokio::sync::Mutex<super::sync::Documents>,
}

struct InFlight<'a>(&'a AtomicUsize);
impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Does startup output say the program is not installed (R3)?
pub fn looks_not_installed(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "not installed",
        "not found",
        "unknown binary",
        "no such file",
        "is not available",
        "isn't installed",
        "unknown proxy",
    ]
    .iter()
    .any(|p| lower.contains(p))
}

fn capabilities_of(result: &Value) -> (Encoding, Capabilities) {
    let caps = result.get("capabilities").cloned().unwrap_or(Value::Null);
    let encoding = Encoding::from_capability(caps.get("positionEncoding").and_then(Value::as_str));
    let diag = caps.get("diagnosticProvider");
    (
        encoding,
        Capabilities {
            pull_diagnostics: diag.is_some_and(|d| !d.is_null() && d != &Value::Bool(false)),
            inter_file_dependencies: diag
                .and_then(|d| d.get("interFileDependencies"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            raw: caps,
        },
    )
}

fn initialize_params(spec: &ServerSpec) -> Value {
    let root_uri = path_to_uri(&spec.root);
    let name = spec
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    json!({
        "processId": std::process::id(),
        "clientInfo": { "name": "vizier" },
        "rootUri": root_uri,
        "workspaceFolders": [{ "uri": root_uri, "name": name }],
        "initializationOptions": spec.initialization_options,
        "capabilities": {
            "general": { "positionEncodings": ["utf-8", "utf-16"] },
            "window": { "workDoneProgress": true },
            "workspace": {
                "configuration": true,
                "workspaceFolders": true,
                "didChangeWatchedFiles": { "dynamicRegistration": false },
                "symbol": { "symbolKind": {} },
                "diagnostics": { "refreshSupport": true }
            },
            "textDocument": {
                "synchronization": { "didSave": true, "dynamicRegistration": false },
                "publishDiagnostics": { "versionSupport": true },
                "diagnostic": { "dynamicRegistration": false, "relatedDocumentSupport": true },
                "definition": { "linkSupport": false },
                "references": {},
                "hover": { "contentFormat": ["markdown", "plaintext"] },
                "documentSymbol": { "hierarchicalDocumentSymbolSupport": false }
            }
        }
    })
}

impl Server {
    /// Spawn the program and complete the handshake. The result is usable (`initialized` is
    /// sent) but may still be indexing: see [`Server::is_ready`].
    pub async fn start(
        spec: ServerSpec,
        on_change: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Arc<Server>, StartError> {
        let mut command = tokio::process::Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.root)
            .env_clear()
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        process_tree::isolate(&mut command);
        process_tree::bind_to_parent(&mut command);
        crate::process::no_window(&mut command);
        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StartError::Missing(format!("{} was not found", spec.program.display()))
            } else {
                StartError::Failed(format!("could not start {}: {e}", spec.program.display()))
            }
        })?;
        process_tree::bind_child_to_job(&child);

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            process_tree::kill(&mut child).await;
            return Err(StartError::Failed("could not open the server's pipes".into()));
        };

        let (out, mut out_rx) = mpsc::unbounded_channel::<Out>();
        let (ready, _) = watch::channel(false);
        let (exited, _) = watch::channel(None);
        let server = Arc::new(Server {
            spec: spec.clone(),
            negotiated: std::sync::OnceLock::new(),
            out,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
            progress: Mutex::new(Progress::default()),
            pushed: Mutex::new(HashMap::new()),
            push_seq: Mutex::new(0),
            activity: Notify::new(),
            stderr: Mutex::new(VecDeque::new()),
            ready,
            exited,
            stop: Notify::new(),
            stopping: Mutex::new(false),
            in_flight: AtomicUsize::new(0),
            last_used: Mutex::new(Instant::now()),
            on_change: Mutex::new(on_change),
            docs: tokio::sync::Mutex::new(Default::default()),
        });

        // Writer.
        tokio::spawn(async move {
            while let Some(message) = out_rx.recv().await {
                match message {
                    Out::Frame(v) => {
                        if rpc::write_frame(&mut stdin, &v).await.is_err() {
                            break;
                        }
                    }
                    Out::Close => break,
                }
            }
            drop(stdin);
        });

        // Stderr ring.
        {
            let s = server.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut buf = [0u8; 1024];
                while let Ok(n) = reader.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    let mut ring = s.stderr.lock();
                    ring.extend(&buf[..n]);
                    while ring.len() > STDERR_CAP {
                        ring.pop_front();
                    }
                }
            });
        }

        // Reader. It outlives the handshake, and its end means the server is gone.
        {
            let s = server.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                loop {
                    match rpc::read_frame(&mut reader).await {
                        Ok(Some(message)) => {
                            if let Some(incoming) = rpc::classify(message) {
                                s.handle(incoming);
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            tracing::debug!(error = %e.0, "language server stream ended");
                            break;
                        }
                    }
                }
            });
        }

        // Process owner: waits for exit, or performs the stop.
        {
            let s = server.clone();
            tokio::spawn(async move {
                let status = tokio::select! {
                    status = child.wait() => status.ok(),
                    _ = s.stop.notified() => {
                        let _ = s.out.send(Out::Frame(json!({"jsonrpc":"2.0","method":"exit"})));
                        let _ = s.out.send(Out::Close);
                        match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
                            Ok(status) => status.ok(),
                            Err(_) => {
                                process_tree::kill(&mut child).await;
                                None
                            }
                        }
                    }
                };
                // Reap a whole group, so a grandchild does not outlive its server.
                #[cfg(unix)]
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
                    }
                }
                let requested = *s.stopping.lock();
                let _ = s.exited.send(Some(ExitInfo {
                    code: status.and_then(|s| s.code()),
                    stderr: s.stderr_text(),
                    requested,
                }));
                s.fail_pending("the server exited");
                s.changed();
            });
        }

        // Handshake.
        let mut exited_rx = server.exited.subscribe();
        let init = server.send_request("initialize", initialize_params(&spec));
        let result = tokio::select! {
            r = tokio::time::timeout(HANDSHAKE_TIMEOUT, init) => match r {
                Ok(r) => r,
                Err(_) => Err(RequestError::Timeout),
            },
            _ = exited_rx.wait_for(|e| e.is_some()) => Err(RequestError::Exited("exited".into())),
        };
        let result = match result {
            Ok(v) => v,
            Err(e) => {
                let stderr = server.stderr_text();
                server.shutdown_quietly().await;
                let reason = if stderr.trim().is_empty() {
                    e.to_string()
                } else {
                    stderr.trim().to_owned()
                };
                return Err(if looks_not_installed(&stderr) {
                    StartError::Missing(reason)
                } else {
                    StartError::Failed(reason)
                });
            }
        };
        let _ = server.negotiated.set(capabilities_of(&result));
        server.notify("initialized", json!({}));
        server.progress.lock().last_activity = Some(Instant::now());
        server.spawn_readiness_monitor();
        Ok(server)
    }

    pub fn encoding(&self) -> Encoding {
        self.negotiated.get().map(|n| n.0).unwrap_or_default()
    }

    pub fn capabilities(&self) -> Capabilities {
        self.negotiated.get().map(|n| n.1.clone()).unwrap_or_default()
    }

    fn changed(&self) {
        let callback = self.on_change.lock().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn stderr_text(&self) -> String {
        let ring = self.stderr.lock();
        let bytes: Vec<u8> = ring.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn fail_pending(&self, _why: &str) {
        // Dropping the senders wakes every waiter with a closed channel.
        self.pending.lock().clear();
    }

    fn spawn_readiness_monitor(self: &Arc<Self>) {
        let s = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if s.exit_info().is_some() {
                    return;
                }
                let quiet = {
                    let p = s.progress.lock();
                    p.open.is_empty() && p.last_activity.is_none_or(|t| t.elapsed() >= QUIET)
                };
                if quiet {
                    // The state of the files when the server became ready, to tell later what a
                    // command changed (R7).
                    s.take_snapshot(&s.spec.root).await;
                    s.ready.send_replace(true);
                    s.changed();
                    return;
                }
            }
        });
    }

    /// Ready: initialised and quiet for a second (R8). One way: later indexing shows in
    /// [`Server::busy_detail`] but does not make a ready server starting again.
    pub fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.exited.borrow().clone()
    }

    pub fn subscribe_exit(&self) -> watch::Receiver<Option<ExitInfo>> {
        self.exited.subscribe()
    }

    /// Work in progress, for "indexing 41%". `None` when nothing is open.
    pub fn busy_detail(&self) -> Option<String> {
        let p = self.progress.lock();
        p.open.first().map(|(_, title, pct)| {
            let mut text = title.trim().to_owned();
            if let Some(first) = text.get(..1) {
                text = first.to_lowercase() + &text[1..];
            }
            match pct {
                Some(pct) => format!("{text} {pct}%"),
                None => text,
            }
        })
    }

    /// Work is running, or ended less than the grace period ago.
    pub fn is_busy(&self, grace: Duration) -> bool {
        let p = self.progress.lock();
        !p.open.is_empty() || p.last_activity.is_some_and(|t| t.elapsed() < grace)
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    pub fn touch(&self) {
        *self.last_used.lock() = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        self.last_used.lock().elapsed()
    }

    fn handle(self: &Arc<Self>, incoming: Incoming) {
        match incoming {
            Incoming::Response { id, result } => {
                if let Some(tx) = self.pending.lock().remove(&id) {
                    let _ = tx.send(result);
                }
            }
            Incoming::Request { id, method, params } => {
                let reply = match method.as_str() {
                    "workspace/applyEdit" => Ok(json!({ "applied": false })),
                    "workspace/configuration" => {
                        let n = params
                            .get("items")
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len);
                        Ok(Value::Array(vec![Value::Null; n]))
                    }
                    "window/showDocument" => Ok(json!({ "success": false })),
                    "client/registerCapability"
                    | "client/unregisterCapability"
                    | "window/workDoneProgress/create"
                    | "window/showMessageRequest"
                    | "workspace/workspaceFolders" => Ok(Value::Null),
                    m if m.starts_with("workspace/") && m.ends_with("/refresh") => Ok(Value::Null),
                    _ => Err(json!({ "code": -32601, "message": "method not found" })),
                };
                let message = match reply {
                    Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
                    Err(error) => json!({"jsonrpc":"2.0","id":id,"error":error}),
                };
                let _ = self.out.send(Out::Frame(message));
            }
            Incoming::Notification { method, params } => match method.as_str() {
                "$/progress" => self.on_progress(&params),
                "textDocument/publishDiagnostics" => self.on_publish(&params),
                _ => {}
            },
        }
    }

    fn on_progress(&self, params: &Value) {
        let token = match params.get("token") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => return,
        };
        let value = params.get("value").cloned().unwrap_or(Value::Null);
        {
            let mut p = self.progress.lock();
            p.last_activity = Some(Instant::now());
            match value.get("kind").and_then(Value::as_str) {
                Some("begin") => {
                    let title = value
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("working")
                        .to_owned();
                    let pct = value.get("percentage").and_then(Value::as_u64).map(|p| p as u32);
                    p.open.push((token, title, pct));
                }
                Some("report") => {
                    if let Some(entry) = p.open.iter_mut().find(|(t, _, _)| *t == token)
                        && let Some(pct) = value.get("percentage").and_then(Value::as_u64)
                    {
                        entry.2 = Some(pct as u32);
                    }
                }
                Some("end") => p.open.retain(|(t, _, _)| *t != token),
                _ => {}
            }
        }
        self.activity.notify_waiters();
        self.changed();
    }

    fn on_publish(&self, params: &Value) {
        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return;
        };
        let items = params
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let seq = {
            let mut s = self.push_seq.lock();
            *s += 1;
            *s
        };
        self.pushed.lock().insert(
            uri.to_owned(),
            Pushed {
                version: params.get("version").and_then(Value::as_i64),
                items,
                seq,
            },
        );
        self.activity.notify_waiters();
    }

    /// A notification to the server.
    pub fn notify(&self, method: &str, params: Value) {
        let _ = self.out.send(Out::Frame(
            json!({"jsonrpc":"2.0","method":method,"params":params}),
        ));
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value, RequestError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        if self
            .out
            .send(Out::Frame(
                json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            ))
            .is_err()
        {
            self.pending.lock().remove(&id);
            return Err(RequestError::Exited("the server is gone".into()));
        }
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(RequestError::Server(e)),
            Err(_) => Err(RequestError::Exited(
                self.exit_info()
                    .map(|e| e.stderr.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "the server exited".into()),
            )),
        }
    }

    /// A request with the 10 s limit. A "content modified" or "cancelled" answer is retried once.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, RequestError> {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let _guard = InFlight(&self.in_flight);
        self.touch();
        for attempt in 0..2 {
            let id_before = self.next_id.load(Ordering::SeqCst);
            let outcome =
                tokio::time::timeout(REQUEST_TIMEOUT, self.send_request(method, params.clone()))
                    .await;
            match outcome {
                Err(_) => {
                    // Forget the id we gave up on, so a late answer is dropped.
                    self.pending.lock().remove(&id_before);
                    return Err(RequestError::Timeout);
                }
                Ok(Err(RequestError::Server(e)))
                    if attempt == 0 && (e.code == -32801 || e.code == -32802) =>
                {
                    continue;
                }
                Ok(other) => return other,
            }
        }
        Err(RequestError::Timeout)
    }

    /// Stop: `shutdown`, `exit`, a 2 s wait, then kill. Returns once the process is gone.
    pub async fn stop(&self) {
        let already = {
            let mut stopping = self.stopping.lock();
            let was = *stopping;
            *stopping = true;
            was
        };
        if already {
            self.wait_exit(Duration::from_secs(6)).await;
            return;
        }
        if self.exit_info().is_none() {
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                self.send_request("shutdown", Value::Null),
            )
            .await;
            self.stop.notify_one();
        }
        self.wait_exit(Duration::from_secs(6)).await;
    }

    async fn wait_exit(&self, limit: Duration) {
        let mut rx = self.exited.subscribe();
        let _ = tokio::time::timeout(limit, rx.wait_for(|e| e.is_some())).await;
    }

    async fn shutdown_quietly(&self) {
        *self.stopping.lock() = true;
        self.stop.notify_one();
        self.wait_exit(Duration::from_secs(4)).await;
    }
}
