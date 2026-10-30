//! The HTTP path end to end against an in-process axum server: status mapping, `Retry-After`,
//! retries, timeouts, SSE bodies and Copilot's token refresh.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::auth::{CopilotExchange, KeyStyle, StaticKey};
use super::dialect::{DialectKind, openai_chat::OpenAiChat, openai_responses::OpenAiResponses};
use super::error::LlmError;
use super::model::{CompletionModel, HttpModel, RetryingModel};
use super::resolve::ResolvedModel;
use super::types::{AssistantPart, CompletionRequest, Message};
use crate::schema::agent::CompletionRetryConfig;

#[derive(Clone)]
struct Scripted {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
    delay: Duration,
}

fn reply(status: u16, body: serde_json::Value) -> Scripted {
    Scripted {
        status,
        headers: vec![],
        body: body.to_string(),
        delay: Duration::ZERO,
    }
}

#[derive(Default)]
struct Server {
    script: VecDeque<Scripted>,
    seen: Vec<(String, HeaderMap)>,
}

type Shared = Arc<Mutex<Server>>;

async fn handle(State(s): State<Shared>, uri: Uri, headers: HeaderMap, _body: Bytes) -> Response {
    let next = {
        let mut s = s.lock().unwrap();
        s.seen.push((uri.path().to_string(), headers));
        s.script.pop_front().unwrap_or_else(|| reply(500, json!({"error": "script empty"})))
    };
    tokio::time::sleep(next.delay).await;
    let mut response = (StatusCode::from_u16(next.status).unwrap(), next.body).into_response();
    response
        .headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    for (k, v) in next.headers {
        response.headers_mut().insert(k, v.parse().unwrap());
    }
    response
}

async fn server(script: Vec<Scripted>) -> (String, Shared) {
    let state: Shared = Arc::new(Mutex::new(Server {
        script: script.into(),
        seen: vec![],
    }));
    let app = Router::new().fallback(handle).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, state)
}

fn ok_chat() -> Scripted {
    reply(
        200,
        json!({"choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", "content": "hi"}}],
               "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}),
    )
}

fn chat_model(url: &str, retry: CompletionRetryConfig) -> RetryingModel<HttpModel<OpenAiChat>> {
    let resolved = ResolvedModel {
        base_url: url.to_string(),
        credential: StaticKey::shared(KeyStyle::Bearer, Some("k".into())),
        ..ResolvedModel::test("m")
    };
    RetryingModel::new(
        HttpModel::new(OpenAiChat, Arc::new(resolved), reqwest::Client::new()),
        retry,
    )
}

fn retry(attempts: u32) -> CompletionRetryConfig {
    CompletionRetryConfig {
        max_attempts: attempts,
        base_delay: Duration::from_millis(1).into(),
        max_delay: Duration::from_millis(5).into(),
        attempt_timeout: Duration::from_secs(10).into(),
    }
}

fn hello() -> CompletionRequest {
    CompletionRequest {
        messages: vec![Message::user_text("hello").unwrap()],
        ..Default::default()
    }
}

#[tokio::test]
async fn a_429_with_retry_after_is_waited_out_then_succeeds() {
    let mut limited = reply(429, json!({"error": {"message": "slow down"}}));
    limited.headers.push(("retry-after", "2".into()));
    let (url, state) = server(vec![limited, ok_chat()]).await;
    let started = std::time::Instant::now();
    let r = chat_model(&url, retry(4)).complete_until(hello(), None).await.unwrap();
    assert!(started.elapsed() >= Duration::from_secs(2), "waited {:?}", started.elapsed());
    assert_eq!(r.content, vec![AssistantPart::Text("hi".into())]);
    assert_eq!(state.lock().unwrap().seen.len(), 2);
}

#[tokio::test]
async fn server_errors_are_retried_and_a_400_is_not() {
    let (url, state) = server(vec![
        reply(500, json!({})),
        reply(503, json!({})),
        reply(529, json!({})),
        ok_chat(),
    ])
    .await;
    assert!(chat_model(&url, retry(5)).complete_until(hello(), None).await.is_ok());
    assert_eq!(state.lock().unwrap().seen.len(), 4);

    let (url, state) = server(vec![reply(400, json!({"error": {"message": "bad request"}})), ok_chat()]).await;
    let err = chat_model(&url, retry(5)).complete_until(hello(), None).await.unwrap_err();
    assert!(err.0.contains("400"), "{err}");
    assert_eq!(state.lock().unwrap().seen.len(), 1);
}

#[tokio::test]
async fn a_refused_connection_is_a_transport_error() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let model = chat_model(&format!("http://127.0.0.1:{port}"), retry(1));
    let err = model.inner.complete(hello()).await.unwrap_err();
    assert!(matches!(err, LlmError::Transport(_)), "{err:?}");
    let err = model.complete_until(hello(), None).await.unwrap_err();
    assert!(err.0.starts_with(super::model::PROVIDER_UNAVAILABLE), "{err}");
}

#[tokio::test]
async fn a_slow_attempt_times_out_and_is_retried() {
    let mut slow = ok_chat();
    slow.delay = Duration::from_millis(500);
    let (url, state) = server(vec![slow, ok_chat()]).await;
    let config = CompletionRetryConfig {
        attempt_timeout: Duration::from_millis(100).into(),
        ..retry(3)
    };
    assert!(chat_model(&url, config).complete_until(hello(), None).await.is_ok());
    assert_eq!(state.lock().unwrap().seen.len(), 2);
}

#[tokio::test]
async fn an_sse_body_is_aggregated() {
    let completed = json!({"type": "response.completed", "response": {"status": "completed", "output": [
        {"type": "message", "id": "msg_1", "role": "assistant", "content": [{"type": "output_text", "text": "streamed"}]}
    ], "usage": {"input_tokens": 3, "output_tokens": 1, "total_tokens": 4}}});
    let sse = Scripted {
        status: 200,
        headers: vec![("content-type", "text/event-stream".into())],
        body: format!(
            "event: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {completed}\n\n",
            json!({"type": "response.output_text.delta", "delta": "streamed"})
        ),
        delay: Duration::ZERO,
    };
    let (url, _) = server(vec![sse]).await;
    let resolved = ResolvedModel {
        base_url: url,
        dialect: DialectKind::OpenAiResponses,
        stream_required: true,
        ..ResolvedModel::test("gpt-5.4")
    };
    let model = HttpModel::new(OpenAiResponses, Arc::new(resolved), reqwest::Client::new());
    let r = model.complete(hello()).await.unwrap();
    assert_eq!(r.content, vec![AssistantPart::Text("streamed".into())]);
    assert_eq!(r.usage.total_tokens, 4);
}

#[tokio::test]
async fn copilot_refreshes_its_token_after_a_401() {
    let token = |t: &str| {
        reply(200, json!({"token": t, "expires_at": 4102444800i64, "endpoints": {"api": null}}))
    };
    // Token exchange → chat 401 → token exchange again → chat ok.
    let (url, state) = server(vec![
        token("tok-1"),
        reply(401, json!({"error": "expired"})),
        token("tok-2"),
        ok_chat(),
    ])
    .await;
    let exchange = CopilotExchange::with_token_url(
        reqwest::Client::new(),
        "gho_github".into(),
        url.clone(),
        format!("{url}/copilot_internal/v2/token"),
    );
    let resolved = ResolvedModel {
        base_url: url,
        credential: exchange,
        ..ResolvedModel::test("gpt-4o")
    };
    let model = HttpModel::new(OpenAiChat, Arc::new(resolved), reqwest::Client::new());
    assert!(model.complete(hello()).await.is_ok());

    let seen = state.lock().unwrap();
    let paths: Vec<_> = seen.seen.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "/copilot_internal/v2/token",
            "/chat/completions",
            "/copilot_internal/v2/token",
            "/chat/completions"
        ]
    );
    assert_eq!(seen.seen[0].1["authorization"], "token gho_github");
    assert_eq!(seen.seen[3].1["authorization"], "Bearer tok-2");
    assert_eq!(seen.seen[3].1["x-initiator"], "user");
}

#[tokio::test]
async fn a_missing_context_window_is_read_from_the_model_listing() {
    let listing = reply(200, json!({"data": [
        {"id": "other", "context_length": 1},
        {"id": "m", "context_length": 131072}
    ]}));
    let (url, state) = server(vec![listing]).await;
    let mut resolved = ResolvedModel {
        base_url: url,
        ..ResolvedModel::test("m")
    };
    super::resolve::fill_context_from_provider(&mut resolved, &reqwest::Client::new()).await;
    assert_eq!(resolved.context_window, Some(131072));
    assert_eq!(resolved.context_source, super::resolve::ContextSource::Listing);
    assert_eq!(state.lock().unwrap().seen[0].0, "/models");

    // A known window is never looked up.
    let (url, state) = server(vec![]).await;
    let mut known = ResolvedModel {
        base_url: url,
        context_window: Some(5),
        ..ResolvedModel::test("m")
    };
    super::resolve::fill_context_from_provider(&mut known, &reqwest::Client::new()).await;
    assert!(state.lock().unwrap().seen.is_empty());
}
