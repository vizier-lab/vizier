//! The window's way into the engine, independent of Tauri: one API request, one asset GET and
//! the open topic streams (contracts/ipc-protocol.md §1–§3). The `shell` module wraps these in
//! commands and the `vizier-asset` scheme.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{body::Body, http::Request};
use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinHandle;
use vizier_core::service::ServiceError;
use vizier_http::topic::TopicConnection;

use crate::engine::Engine;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl IpcError {
    pub fn not_ready() -> Self {
        Self { code: "engine_not_ready".into(), message: "the engine is not ready".into() }
    }
    fn other(message: impl Into<String>) -> Self {
        Self { code: "ipc".into(), message: message.into() }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiReply {
    pub status: u16,
    pub body: Value,
}

/// One request through the route table, as the desktop principal. `path` is relative to
/// `/api/v1` and may carry a query. A non-JSON response is `406` with a null body.
pub async fn api(
    engine: &Engine,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> Result<ApiReply, IpcError> {
    let dispatcher = engine.dispatcher().await.ok_or_else(IpcError::not_ready)?;
    if !path.starts_with('/') {
        return Err(IpcError::other("x-path must start with '/'"));
    }
    let mut request = Request::builder()
        .method(method.to_uppercase().as_str())
        .uri(format!("/api/v1{path}"));
    if let Some(ct) = content_type {
        request = request.header("content-type", ct);
    }
    let request = request.body(Body::from(body)).map_err(|e| IpcError::other(e.to_string()))?;
    let response = dispatcher.dispatch(request).await;
    let status = response.status().as_u16();
    let is_json = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.contains("json"));
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|e| IpcError::other(e.to_string()))?;
    if bytes.is_empty() {
        return Ok(ApiReply { status, body: Value::Null });
    }
    if !is_json {
        return Ok(ApiReply { status: 406, body: Value::Null });
    }
    let body = serde_json::from_slice(&bytes).map_err(|e| IpcError::other(e.to_string()))?;
    Ok(ApiReply { status, body })
}

/// The response headers an asset reply forwards (§2).
pub const ASSET_HEADERS: [&str; 5] =
    ["content-type", "content-length", "content-range", "accept-ranges", "content-disposition"];

#[derive(Debug, Clone)]
pub struct AssetReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A binary GET. `path_and_query` is the full `/api/v1/...` path of the scheme URL.
pub async fn asset(
    engine: &Engine,
    method: &str,
    path_and_query: &str,
    range: Option<&str>,
) -> Result<AssetReply, IpcError> {
    if !method.eq_ignore_ascii_case("GET") {
        return Ok(AssetReply { status: 405, headers: vec![("allow".into(), "GET".into())], body: vec![] });
    }
    let dispatcher = engine.dispatcher().await.ok_or_else(IpcError::not_ready)?;
    if !path_and_query.starts_with("/api/v1/") {
        return Ok(AssetReply { status: 404, headers: vec![], body: vec![] });
    }
    let mut request = Request::builder().method("GET").uri(path_and_query);
    if let Some(range) = range {
        request = request.header("range", range);
    }
    let response = dispatcher
        .dispatch(request.body(Body::empty()).map_err(|e| IpcError::other(e.to_string()))?)
        .await;
    let status = response.status().as_u16();
    let headers = ASSET_HEADERS
        .iter()
        .filter_map(|name| {
            response
                .headers()
                .get(*name)
                .and_then(|v| v.to_str().ok())
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|e| IpcError::other(e.to_string()))?
        .to_vec();
    Ok(AssetReply { status, headers, body })
}

struct OpenTopic {
    connection: Arc<TopicConnection>,
    tasks: Vec<JoinHandle<()>>,
}

/// The conversation streams a window has open, by id.
#[derive(Default)]
pub struct Topics {
    next: AtomicU64,
    open: Mutex<HashMap<u64, OpenTopic>>,
}

/// Why a topic did not open: the status the WebSocket upgrade would have answered.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TopicRefused {
    pub status: u16,
}

impl Topics {
    /// Opens a stream; every frame the engine pushes goes to `sink`, in order.
    pub async fn open(
        &self,
        engine: &Engine,
        agent_id: &str,
        channel_id: &str,
        topic_id: &str,
        sink: impl Fn(Value) + Send + 'static,
    ) -> Result<u64, TopicRefused> {
        let dispatcher = engine.dispatcher().await.ok_or(TopicRefused { status: 503 })?;
        let (out_tx, out_rx) = flume::unbounded::<Value>();
        let mut connection = dispatcher
            .open_topic(agent_id, channel_id, topic_id, out_tx)
            .await
            .map_err(|e: ServiceError| TopicRefused { status: e.status() })?;
        let mut tasks = vec![];
        if let Some(outbound) = connection.outbound() {
            tasks.push(tokio::spawn(outbound.run()));
        }
        tasks.push(tokio::spawn(async move {
            while let Ok(frame) = out_rx.recv_async().await {
                sink(frame);
            }
        }));
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.open
            .lock()
            .unwrap()
            .insert(id, OpenTopic { connection: Arc::new(connection), tasks });
        Ok(id)
    }

    /// Hands a client frame to the stream; an unknown id is ignored.
    pub async fn send(&self, id: u64, frame: Value) {
        let connection = self.open.lock().unwrap().get(&id).map(|t| t.connection.clone());
        if let Some(connection) = connection {
            connection.handle_inbound(frame).await;
        }
    }

    pub fn close(&self, id: u64) {
        if let Some(topic) = self.open.lock().unwrap().remove(&id) {
            topic.connection.close();
            topic.tasks.iter().for_each(|t| t.abort());
        }
    }

    /// Every stream, e.g. when the window is destroyed or the engine restarts.
    pub fn close_all(&self) {
        let ids: Vec<u64> = self.open.lock().unwrap().keys().copied().collect();
        ids.into_iter().for_each(|id| self.close(id));
    }

    pub fn len(&self) -> usize {
        self.open.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
