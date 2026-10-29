//! The completion seam (contract `provider-layer.md` § The completion seam): one trait, an
//! HTTP implementation shared by every dialect, and a retrying decorator.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;
use tracing::Instrument;

use super::auth::RequestInfo;
use super::dialect::{BodyKind, Dialect, PreparedRequest, join_url};
use super::error::LlmError;
use super::http;
use super::prepare::prepare;
use super::resolve::ResolvedModel;
use super::telemetry;
use super::types::{CompletionRequest, CompletionResponse, Message, Part};
use crate::VizierError;
use crate::schema::agent::CompletionRetryConfig;

/// One model listed by a provider.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedModel {
    pub id: String,
    pub context: Option<u64>,
}

#[async_trait::async_trait]
pub trait CompletionModel: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// [`CompletionModel::complete`], giving up at `deadline`. The error text carries the
    /// prefixes `ErrorKind::classify` reads. Only [`RetryingModel`] uses the deadline.
    async fn complete_until(
        &self,
        req: CompletionRequest,
        _deadline: Option<Instant>,
    ) -> crate::Result<CompletionResponse> {
        self.complete(req).await.map_err(Into::into)
    }

    async fn list_models(&self) -> Result<Vec<ListedModel>, LlmError> {
        Err(LlmError::Unsupported)
    }

    fn context_window(&self) -> Option<u64>;
}

/// The only completion model that touches the network: `prepare`, then the dialect's
/// `encode`, auth, one POST, status mapping, and the dialect's `decode`.
pub struct HttpModel<D: Dialect> {
    pub dialect: D,
    pub resolved: Arc<ResolvedModel>,
    pub http: reqwest::Client,
}

impl<D: Dialect> HttpModel<D> {
    pub fn new(dialect: D, resolved: Arc<ResolvedModel>, http: reqwest::Client) -> Self {
        Self {
            dialect,
            resolved,
            http,
        }
    }

    pub fn prepare(&self, req: CompletionRequest) -> PreparedRequest {
        prepare(req, &self.resolved, self.dialect.system_style())
    }

    async fn base_url(&self) -> Result<String, LlmError> {
        Ok(self
            .resolved
            .credential
            .base_url()
            .await?
            .unwrap_or_else(|| self.resolved.base_url.clone()))
    }

    fn with_fixed_headers(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if !self.resolved.query.is_empty() {
            req = req.query(&self.resolved.query);
        }
        for (name, value) in self.dialect.headers() {
            req = req.header(name, value);
        }
        for (name, value) in &self.resolved.headers {
            req = req.header(name.as_str(), value.as_str());
        }
        req
    }

    async fn send_once(
        &self,
        url: &str,
        body: &serde_json::Value,
        info: &RequestInfo,
    ) -> Result<http::HttpResponse, LlmError> {
        // Serialize once: the bytes signed by a body-aware credential (AWS SigV4) must be the
        // exact bytes sent. `.json(body)` would re-serialize after signing.
        let bytes = serde_json::to_vec(body)
            .map_err(|e| LlmError::Decode(format!("cannot serialize the request body: {e}")))?;
        let req = self
            .with_fixed_headers(self.http.post(url))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes.clone());
        let req = self
            .resolved
            .credential
            .authorize_with_body(req, info, "POST", url, &bytes)
            .await?;
        http::send(req).await
    }
}

fn request_info(req: &PreparedRequest) -> RequestInfo {
    RequestInfo {
        agent_initiated: req
            .messages
            .iter()
            .any(|m| matches!(m, Message::Assistant(_) | Message::ToolResult { .. })),
        has_images: req.messages.iter().any(|m| match m {
            Message::User(parts) | Message::ToolResult { content: parts, .. } => {
                parts.iter().any(|p| {
                    matches!(p, Part::Attachment { mime, .. } if mime.starts_with("image/"))
                })
            }
            _ => false,
        }),
    }
}

impl<D: Dialect> HttpModel<D> {
    /// One request to the provider, under the `chat` span [`CompletionModel::complete`] opens.
    async fn complete_once(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let prepared = self.prepare(req);
        let body = self.dialect.encode(&prepared, &self.resolved);
        let info = request_info(&prepared);
        let url = join_url(&self.base_url().await?, &self.dialect.path(&self.resolved));

        let response = match self.send_once(&url, &body, &info).await {
            // A token-exchange source may hold an expired token: drop it and try once more.
            Err(LlmError::Http { status: 401, .. })
                if self.resolved.credential.invalidate().await =>
            {
                self.send_once(&url, &body, &info).await?
            }
            other => other?,
        };

        let value = match self.dialect.read_body(&self.resolved) {
            BodyKind::Json => response.json()?,
            BodyKind::Sse => self.dialect.aggregate_sse(&response.text())?,
        };
        self.dialect.decode(value)
    }
}

#[async_trait::async_trait]
impl<D: Dialect> CompletionModel for HttpModel<D> {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let span = telemetry::chat_span(&self.resolved, req.max_tokens.or(self.resolved.max_tokens));
        if telemetry::capture_content() {
            telemetry::record_input(&span, &req);
        }
        let started = std::time::Instant::now();
        let result = self.complete_once(req).instrument(span.clone()).await;
        telemetry::record(&span, &self.resolved, started.elapsed(), &result);
        result
    }

    async fn list_models(&self) -> Result<Vec<ListedModel>, LlmError> {
        let path = self
            .dialect
            .list_models_path(&self.resolved)
            .ok_or(LlmError::Unsupported)?;
        let url = join_url(&self.base_url().await?, &path);
        let req = self.with_fixed_headers(self.http.get(url));
        let req = self
            .resolved
            .credential
            .authorize(req, &RequestInfo::default())
            .await?;
        let body = http::send_json(req.timeout(Duration::from_secs(15))).await?;
        Ok(self.dialect.decode_models(body))
    }

    fn context_window(&self) -> Option<u64> {
        self.resolved.context_window
    }
}

// Retries ------------------------------------------------------------------------------------

/// Starts the message of an error for a completion that ran out of the turn's time, so
/// `ErrorKind::classify` reads it as a prompt timeout.
pub const PROMPT_TIMED_OUT: &str = "prompt timed out";
/// Starts the message of an error for retries exhausted on rate limiting.
pub const RATE_LIMITED: &str = "rate limited by the provider";
/// Starts the message of an error for retries exhausted on any other transient failure.
pub const PROVIDER_UNAVAILABLE: &str = "provider unavailable";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Failure {
    RateLimited,
    Unavailable,
    Permanent,
}

/// Text that marks a provider's error body as rate limiting.
const RATE_LIMIT_MARKERS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "ratelimit",
    "too many requests",
    "resource_exhausted",
    "resource exhausted",
];

/// Text that marks a provider's error body as a transient server-side failure.
const UNAVAILABLE_MARKERS: &[&str] = &[
    "overloaded",
    "internal server error",
    "server_error",
    "server error",
    "bad gateway",
    "service unavailable",
    "service_unavailable",
    "temporarily unavailable",
    "gateway timeout",
    "gateway time-out",
    "upstream connect error",
    "try again later",
    "connection reset",
    "connection closed",
];

fn classify_status(status: u16) -> Failure {
    match status {
        429 => Failure::RateLimited,
        408 | 500 | 502 | 503 | 504 | 520..=529 => Failure::Unavailable,
        _ => Failure::Permanent,
    }
}

/// Only for errors reported inside a 2xx body, where there is no status to read.
fn classify_text(text: &str) -> Failure {
    let text = text.to_lowercase();
    if RATE_LIMIT_MARKERS.iter().any(|m| text.contains(m)) {
        Failure::RateLimited
    } else if UNAVAILABLE_MARKERS.iter().any(|m| text.contains(m)) {
        Failure::Unavailable
    } else {
        Failure::Permanent
    }
}

fn classify(err: &LlmError) -> Failure {
    match err {
        LlmError::Http { status, .. } => classify_status(*status),
        LlmError::Transport(_) | LlmError::Timeout => Failure::Unavailable,
        LlmError::Decode(body) => classify_text(body),
        LlmError::Config(_) | LlmError::Unsupported => Failure::Permanent,
    }
}

fn retry_after(err: &LlmError) -> Option<Duration> {
    match err {
        LlmError::Http { retry_after, .. } => *retry_after,
        _ => None,
    }
}

/// The wait before retry number `retry` (1-based): doubling from `base`, capped at `max`, then
/// scaled into its upper half at random so agents that failed together don't retry together.
fn backoff(config: &CompletionRetryConfig, retry: u32) -> Duration {
    let base = *config.base_delay;
    let max = *config.max_delay;
    let doubled = base.saturating_mul(1u32 << retry.saturating_sub(1).min(16));
    doubled.min(max).mul_f64(0.5 + rand::random::<f64>() * 0.5)
}

/// Retries transient failures under a [`CompletionRetryConfig`] (#41). Every attempt is cut
/// off at `attempt_timeout`, and at the turn's deadline when one is given, so a stalled
/// provider can't hold a turn past its `prompt_timeout`. A `Retry-After` is honoured: the wait
/// is `max(backoff, retry_after)`, capped by the deadline (FR-019).
pub struct RetryingModel<M> {
    pub inner: M,
    pub config: CompletionRetryConfig,
}

impl<M: CompletionModel> RetryingModel<M> {
    pub fn new(inner: M, config: CompletionRetryConfig) -> Self {
        Self { inner, config }
    }
}

#[async_trait::async_trait]
impl<M: CompletionModel> CompletionModel for RetryingModel<M> {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.complete_until(req, None)
            .await
            .map_err(|e| LlmError::Config(e.0))
    }

    async fn complete_until(
        &self,
        req: CompletionRequest,
        deadline: Option<Instant>,
    ) -> crate::Result<CompletionResponse> {
        complete_with_retries(&self.inner, &self.config, req, deadline).await
    }

    async fn list_models(&self) -> Result<Vec<ListedModel>, LlmError> {
        self.inner.list_models().await
    }

    fn context_window(&self) -> Option<u64> {
        self.inner.context_window()
    }
}

pub async fn complete_with_retries(
    model: &(impl CompletionModel + ?Sized),
    config: &CompletionRetryConfig,
    req: CompletionRequest,
    deadline: Option<Instant>,
) -> crate::Result<CompletionResponse> {
    let max_attempts = config.max_attempts.max(1);
    let attempt_timeout = *config.attempt_timeout;

    let mut attempt = 0;
    loop {
        attempt += 1;

        let remaining = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        if remaining.is_some_and(|r| r.is_zero()) {
            return Err(VizierError(format!(
                "{PROMPT_TIMED_OUT} waiting for the model"
            )));
        }
        let limit = remaining.map_or(attempt_timeout, |r| r.min(attempt_timeout));

        let err = match tokio::time::timeout(limit, model.complete(req.clone())).await {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(err)) => err,
            // Cut off by the turn's deadline: the turn is over, retrying can't help.
            Err(_) if limit < attempt_timeout => {
                return Err(VizierError(format!(
                    "{PROMPT_TIMED_OUT} waiting for the model"
                )));
            }
            Err(_) => LlmError::Timeout,
        };
        let failure = classify(&err);
        let message = match &err {
            LlmError::Timeout => format!("no response within {attempt_timeout:?}"),
            other => other.to_string(),
        };

        if failure == Failure::Permanent {
            return Err(VizierError(message));
        }

        let exhausted = |message: String| {
            let prefix = match failure {
                Failure::RateLimited => RATE_LIMITED,
                _ => PROVIDER_UNAVAILABLE,
            };
            VizierError(format!("{prefix} after {attempt} attempt(s): {message}"))
        };

        if attempt >= max_attempts {
            return Err(exhausted(message));
        }

        let mut wait = backoff(config, attempt);
        if let Some(after) = retry_after(&err) {
            wait = wait.max(after);
        }
        if deadline.is_some_and(|d| Instant::now() + wait >= d) {
            return Err(exhausted(message));
        }

        tracing::warn!(
            attempt,
            max_attempts,
            wait_ms = wait.as_millis() as u64,
            "completion failed, retrying: {message}"
        );
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{AssistantPart, Usage};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Answers each call with the next scripted outcome; `None` never answers.
    struct Scripted {
        outcomes: Mutex<Vec<Option<Result<(), LlmError>>>>,
        calls: AtomicU32,
    }

    impl Scripted {
        fn new(outcomes: Vec<Option<Result<(), LlmError>>>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                calls: AtomicU32::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl CompletionModel for Scripted {
        async fn complete(&self, _req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let next = self.outcomes.lock().unwrap().remove(0);
            match next {
                Some(Ok(())) => Ok(CompletionResponse {
                    id: None,
                    content: vec![AssistantPart::Text("ok".into())],
                    usage: Usage::new(),
                    model: None,
                    finish_reason: None,
                }),
                Some(Err(e)) => Err(e),
                None => std::future::pending().await,
            }
        }

        fn context_window(&self) -> Option<u64> {
            None
        }
    }

    fn fast_config(max_attempts: u32) -> CompletionRetryConfig {
        CompletionRetryConfig {
            max_attempts,
            base_delay: Duration::from_millis(1).into(),
            max_delay: Duration::from_millis(5).into(),
            attempt_timeout: Duration::from_secs(5).into(),
        }
    }

    fn http(status: u16, body: &str) -> LlmError {
        LlmError::Http {
            status,
            retry_after: None,
            body: body.into(),
        }
    }

    async fn run(
        model: &Scripted,
        config: &CompletionRetryConfig,
        deadline: Option<Instant>,
    ) -> crate::Result<CompletionResponse> {
        complete_with_retries(model, config, CompletionRequest::default(), deadline).await
    }

    #[tokio::test]
    async fn retries_a_transient_failure_then_succeeds() {
        let model = Scripted::new(vec![Some(Err(http(503, "Service Unavailable"))), Some(Ok(()))]);
        assert!(run(&model, &fast_config(4), None).await.is_ok());
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn never_retries_a_permanent_failure() {
        let model = Scripted::new(vec![Some(Err(http(
            401,
            r#"{"error":{"message":"Incorrect API key provided","code":"invalid_api_key"}}"#,
        )))]);
        let err = run(&model, &fast_config(4), None).await.unwrap_err();
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
        assert!(err.to_string().contains("Incorrect API key"));
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts_with_a_rate_limit_error() {
        let model = Scripted::new(vec![
            Some(Err(http(429, "slow down"))),
            Some(Err(http(429, "slow down"))),
            Some(Err(http(429, "slow down"))),
        ]);
        let err = run(&model, &fast_config(3), None).await.unwrap_err();
        assert_eq!(model.calls.load(Ordering::SeqCst), 3);
        assert!(err.to_string().starts_with(RATE_LIMITED));
    }

    #[tokio::test]
    async fn a_stalled_provider_ends_at_the_deadline() {
        let model = Scripted::new(vec![None]);
        let deadline = Instant::now() + Duration::from_millis(50);
        let err = run(&model, &fast_config(4), Some(deadline))
            .await
            .unwrap_err();
        assert!(err.to_string().starts_with(PROMPT_TIMED_OUT));
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_stalled_attempt_is_abandoned_and_retried() {
        let model = Scripted::new(vec![None, Some(Ok(()))]);
        let config = CompletionRetryConfig {
            attempt_timeout: Duration::from_millis(20).into(),
            ..fast_config(4)
        };
        assert!(run(&model, &config, None).await.is_ok());
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_error_body_inside_a_2xx_is_classified_by_its_text() {
        let model = Scripted::new(vec![
            Some(Err(LlmError::Decode(
                r#"{"type":"error","error":{"type":"overloaded_error"}}"#.into(),
            ))),
            Some(Ok(())),
        ]);
        assert!(run(&model, &fast_config(4), None).await.is_ok());
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_retry_after_past_the_deadline_gives_up() {
        let model = Scripted::new(vec![Some(Err(LlmError::Http {
            status: 429,
            retry_after: Some(Duration::from_secs(60)),
            body: String::new(),
        }))]);
        let deadline = Instant::now() + Duration::from_secs(1);
        let err = run(&model, &fast_config(4), Some(deadline))
            .await
            .unwrap_err();
        assert!(err.to_string().starts_with(RATE_LIMITED));
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn classifies() {
        assert_eq!(classify_text("upstream: 502 Bad Gateway"), Failure::Unavailable);
        assert_eq!(
            classify_text(r#"{"type":"error","error":{"type":"overloaded_error"}}"#),
            Failure::Unavailable
        );
        assert_eq!(classify_text("429 Too Many Requests"), Failure::RateLimited);
        assert_eq!(
            classify_text("max_tokens: 500 is too large for this model"),
            Failure::Permanent
        );
        assert_eq!(classify_status(401), Failure::Permanent);
        assert_eq!(classify_status(503), Failure::Unavailable);
        assert_eq!(classify_status(529), Failure::Unavailable);
        assert_eq!(classify_status(400), Failure::Permanent);
        assert_eq!(classify(&LlmError::Transport("refused".into())), Failure::Unavailable);
        assert_eq!(classify(&LlmError::Config("no key".into())), Failure::Permanent);
    }

    /// Bedrock's error names ride on the HTTP status: 400/403/404 are permanent, 408/429/5xx are
    /// retried, and an exhausted budget keeps AWS's message (contract `bedrock-wire.md` §7).
    #[tokio::test]
    async fn bedrock_errors_follow_the_provider_policy() {
        for (status, name) in [
            (403, "AccessDeniedException"),
            (404, "ResourceNotFoundException"),
            (400, "ValidationException"),
        ] {
            let body = format!(r#"{{"message":"{name}: the request was refused"}}"#);
            let model = Scripted::new(vec![Some(Err(http(status, &body)))]);
            let err = run(&model, &fast_config(4), None).await.unwrap_err();
            assert_eq!(model.calls.load(Ordering::SeqCst), 1, "{name} must not retry");
            assert!(err.to_string().contains(name), "{name}: {err}");
        }

        for (status, name) in [
            (429, "ThrottlingException"),
            (429, "ModelNotReadyException"),
            (503, "ServiceUnavailableException"),
            (500, "InternalServerException"),
            (408, "ModelTimeoutException"),
        ] {
            let body = format!(r#"{{"message":"{name}: try again"}}"#);
            let model = Scripted::new(vec![Some(Err(http(status, &body))), Some(Ok(()))]);
            run(&model, &fast_config(4), None).await.unwrap();
            assert_eq!(model.calls.load(Ordering::SeqCst), 2, "{name} is retried");
        }

        // Exhausted throttling keeps AWS's message and reads as rate limiting.
        let body = r#"{"message":"ThrottlingException: too many requests"}"#;
        let model = Scripted::new(vec![
            Some(Err(http(429, body))),
            Some(Err(http(429, body))),
        ]);
        let err = run(&model, &fast_config(2), None).await.unwrap_err();
        assert!(err.to_string().starts_with(RATE_LIMITED), "{err}");
        assert!(err.to_string().contains("ThrottlingException"), "{err}");
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);

        // An exhausted 503 reads as provider unavailable.
        let model = Scripted::new(vec![
            Some(Err(http(503, "ServiceUnavailableException"))),
            Some(Err(http(503, "ServiceUnavailableException"))),
        ]);
        let err = run(&model, &fast_config(2), None).await.unwrap_err();
        assert!(err.to_string().starts_with(PROVIDER_UNAVAILABLE), "{err}");

        // A Retry-After is honoured even when it is longer than the backoff.
        let model = Scripted::new(vec![
            Some(Err(LlmError::Http {
                status: 429,
                retry_after: Some(Duration::from_millis(40)),
                body: "ThrottlingException".into(),
            })),
            Some(Ok(())),
        ]);
        let started = Instant::now();
        run(&model, &fast_config(4), None).await.unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(40),
            "the provider's retry hint is honoured: {:?}",
            started.elapsed()
        );
    }
}
