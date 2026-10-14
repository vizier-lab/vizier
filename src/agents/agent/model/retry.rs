//! Retries and a per-attempt deadline around one completion call (#41).
//!
//! rig hands back a non-success HTTP response as `ProviderError(body)` or
//! `ResponseError(body)`, without the status code or headers, so most failures are classified
//! from their text, and `Retry-After` can't be honoured. A typed status is used where rig keeps
//! one (`http_client::Error::InvalidStatusCode*`). Anything not recognised as transient is
//! treated as permanent and fails at once: retrying a bad key or a bad request only delays the
//! error.

use std::time::Duration;

use anyhow::Result;
use rig_core::{
    OneOrMany,
    completion::{CompletionError, ToolDefinition, Usage},
    http_client,
    message::{AssistantContent, Message},
};
use tokio::time::Instant;

use super::VizierModelTrait;
use crate::schema::agent::CompletionRetryConfig;

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

fn classify_status(status: reqwest::StatusCode) -> Failure {
    match status.as_u16() {
        429 => Failure::RateLimited,
        408 | 500 | 502 | 503 | 504 | 520..=529 => Failure::Unavailable,
        _ => Failure::Permanent,
    }
}

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

fn classify(err: &anyhow::Error) -> Failure {
    match err.downcast_ref::<CompletionError>() {
        Some(CompletionError::HttpError(http)) => match http {
            http_client::Error::InvalidStatusCode(status)
            | http_client::Error::InvalidStatusCodeWithMessage(status, _) => {
                classify_status(*status)
            }
            // The request never got an answer: refused, reset or timed out at the socket.
            http_client::Error::Instance(_) | http_client::Error::StreamEnded => {
                Failure::Unavailable
            }
            _ => Failure::Permanent,
        },
        Some(CompletionError::ProviderError(body) | CompletionError::ResponseError(body)) => {
            classify_text(body)
        }
        _ => Failure::Permanent,
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

/// Run `model.completion`, retrying transient failures under `config`. Every attempt is cut
/// off at `config.attempt_timeout`, and at `deadline` when one is given, so a stalled provider
/// can no longer hold a turn past its `prompt_timeout`.
pub async fn complete(
    model: &(dyn VizierModelTrait + Sync + Send),
    config: &CompletionRetryConfig,
    message: Message,
    history: Vec<Message>,
    tools: Vec<ToolDefinition>,
    deadline: Option<Instant>,
) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
    let max_attempts = config.max_attempts.max(1);
    let attempt_timeout = *config.attempt_timeout;

    let mut attempt = 0;
    loop {
        attempt += 1;

        let remaining = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        if remaining.is_some_and(|r| r.is_zero()) {
            anyhow::bail!("{PROMPT_TIMED_OUT} waiting for the model");
        }
        let limit = remaining.map_or(attempt_timeout, |r| r.min(attempt_timeout));

        let err = match tokio::time::timeout(
            limit,
            model.completion(message.clone(), history.clone(), tools.clone()),
        )
        .await
        {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(err)) => (classify(&err), err),
            // Cut off by the turn's deadline: the turn is over, retrying can't help.
            Err(_) if limit < attempt_timeout => {
                anyhow::bail!("{PROMPT_TIMED_OUT} waiting for the model")
            }
            Err(_) => (
                Failure::Unavailable,
                anyhow::anyhow!("no response within {attempt_timeout:?}"),
            ),
        };
        let (failure, err) = err;

        if failure == Failure::Permanent {
            return Err(err);
        }

        let exhausted = |err: anyhow::Error| {
            let prefix = match failure {
                Failure::RateLimited => RATE_LIMITED,
                _ => PROVIDER_UNAVAILABLE,
            };
            anyhow::anyhow!("{prefix} after {attempt} attempt(s): {err}")
        };

        if attempt >= max_attempts {
            return Err(exhausted(err));
        }

        let wait = backoff(config, attempt);
        if deadline.is_some_and(|d| Instant::now() + wait >= d) {
            return Err(exhausted(err));
        }

        tracing::warn!(
            attempt,
            max_attempts,
            wait_ms = wait.as_millis() as u64,
            "completion failed, retrying: {err}"
        );
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Answers each call with the next scripted outcome; `None` never answers.
    struct Scripted {
        outcomes: Mutex<Vec<Option<std::result::Result<(), CompletionError>>>>,
        calls: AtomicU32,
    }

    impl Scripted {
        fn new(outcomes: Vec<Option<std::result::Result<(), CompletionError>>>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                calls: AtomicU32::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl VizierModelTrait for Scripted {
        async fn completion(
            &self,
            _message: Message,
            _history: Vec<Message>,
            _tools: Vec<ToolDefinition>,
        ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let next = self.outcomes.lock().unwrap().remove(0);
            match next {
                Some(Ok(())) => Ok((
                    None,
                    OneOrMany::one(AssistantContent::text("ok")),
                    Usage::new(),
                )),
                Some(Err(e)) => Err(e.into()),
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

    fn provider(body: &str) -> CompletionError {
        CompletionError::ProviderError(body.into())
    }

    async fn run(
        model: &Scripted,
        config: &CompletionRetryConfig,
        deadline: Option<Instant>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        complete(model, config, Message::user("hi"), vec![], vec![], deadline).await
    }

    #[tokio::test]
    async fn retries_a_transient_failure_then_succeeds() {
        let model = Scripted::new(vec![
            Some(Err(provider("503 Service Unavailable"))),
            Some(Ok(())),
        ]);
        assert!(run(&model, &fast_config(4), None).await.is_ok());
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn never_retries_a_permanent_failure() {
        let model = Scripted::new(vec![Some(Err(provider(
            r#"{"error":{"message":"Incorrect API key provided","code":"invalid_api_key"}}"#,
        )))]);
        let err = run(&model, &fast_config(4), None).await.unwrap_err();
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
        assert!(err.to_string().contains("Incorrect API key"));
    }

    #[tokio::test]
    async fn gives_up_after_max_attempts_with_a_rate_limit_error() {
        let model = Scripted::new(vec![
            Some(Err(provider("Rate limit reached for requests"))),
            Some(Err(provider("Rate limit reached for requests"))),
            Some(Err(provider("Rate limit reached for requests"))),
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

    #[test]
    fn classifies_provider_bodies() {
        assert_eq!(
            classify_text("upstream: 502 Bad Gateway"),
            Failure::Unavailable
        );
        assert_eq!(
            classify_text(r#"{"type":"error","error":{"type":"overloaded_error"}}"#),
            Failure::Unavailable
        );
        assert_eq!(classify_text("429 Too Many Requests"), Failure::RateLimited);
        assert_eq!(
            classify_text("max_tokens: 500 is too large for this model"),
            Failure::Permanent
        );
        assert_eq!(
            classify_status(reqwest::StatusCode::UNAUTHORIZED),
            Failure::Permanent
        );
        assert_eq!(
            classify_status(reqwest::StatusCode::SERVICE_UNAVAILABLE),
            Failure::Unavailable
        );
    }
}
