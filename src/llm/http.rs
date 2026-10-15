//! The one HTTP path to model providers: a shared client, status → [`LlmError`] mapping and
//! key redaction for debug logs.

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use reqwest::header::{HeaderMap, RETRY_AFTER};

use super::error::{LlmError, parse_retry_after};

/// The shared client, built once and held on `VizierDependencies`.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("vizier/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("cannot build the configured http client ({e}); using the default");
            reqwest::Client::new()
        })
}

/// A successful response: status, headers and the raw body.
pub struct HttpResponse {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(&self) -> Result<serde_json::Value, LlmError> {
        serde_json::from_slice(&self.body).map_err(|e| {
            LlmError::Decode(format!(
                "{e}: {}",
                redact(&String::from_utf8_lossy(&self.body[..self.body.len().min(500)]))
            ))
        })
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Send a request and map the outcome: non-2xx → [`LlmError::Http`] (with `Retry-After`),
/// connect/reset/DNS/TLS → [`LlmError::Transport`], timeouts → [`LlmError::Timeout`].
pub async fn send(req: reqwest::RequestBuilder) -> Result<HttpResponse, LlmError> {
    if tracing::enabled!(tracing::Level::TRACE)
        && let Some(r) = req.try_clone().and_then(|r| r.build().ok())
    {
        let body = r
            .body()
            .and_then(|b| b.as_bytes())
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        tracing::trace!(
            method = %r.method(),
            url = %redact(r.url().as_str()),
            "llm request: {}",
            redact(&body)
        );
    }

    let response = req.send().await.map_err(map_reqwest_error)?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response.bytes().await.map_err(map_reqwest_error)?.to_vec();

    if !(200..300).contains(&status) {
        let retry_after = headers.get(RETRY_AFTER).and_then(parse_retry_after);
        let body = redact(&String::from_utf8_lossy(&body));
        tracing::debug!(status, "llm request failed: {body}");
        return Err(LlmError::Http {
            status,
            retry_after,
            body,
        });
    }
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

/// [`send`] for a JSON request: the body must parse as JSON.
pub async fn send_json(req: reqwest::RequestBuilder) -> Result<serde_json::Value, LlmError> {
    send(req).await?.json()
}

/// [`send`] for a multipart request (transcription uploads). Multipart bodies are streams, so
/// they are never logged.
pub async fn send_multipart(
    req: reqwest::RequestBuilder,
    form: reqwest::multipart::Form,
) -> Result<HttpResponse, LlmError> {
    send(req.multipart(form)).await
}

pub fn map_reqwest_error(e: reqwest::Error) -> LlmError {
    if e.is_timeout() {
        LlmError::Timeout
    } else if e.is_connect() || e.is_request() || e.is_body() {
        LlmError::Transport(redact(&error_chain(&e)))
    } else if e.is_decode() {
        LlmError::Decode(redact(&error_chain(&e)))
    } else {
        LlmError::Transport(redact(&error_chain(&e)))
    }
}

/// reqwest's own message hides the cause ("error sending request"); the chain names it
/// ("connection refused").
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        let next = s.to_string();
        if !text.contains(&next) {
            text.push_str(": ");
            text.push_str(&next);
        }
        source = s.source();
    }
    text
}

static SECRETS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]+", "${1}[redacted]"),
        (
            r#"(?i)("?(?:x-api-key|api-key|x-goog-api-key)"?\s*[:=]\s*"?)[^",\s}]+"#,
            "${1}[redacted]",
        ),
        (r"(?i)([?&]key=)[^&\s]+", "${1}[redacted]"),
        (r"\bsk-[A-Za-z0-9_-]{8,}", "sk-[redacted]"),
    ]
    .into_iter()
    .filter_map(|(p, r)| Regex::new(p).ok().map(|re| (re, r)))
    .collect()
});

/// Hide credentials (`Bearer …`, `x-api-key`, `api-key`, `x-goog-api-key`, `key=`) in text
/// bound for a log.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for (re, replacement) in SECRETS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys() {
        assert_eq!(
            redact("Authorization: Bearer abc.DEF-123"),
            "Authorization: Bearer [redacted]"
        );
        assert_eq!(redact("bearer abc123"), "bearer [redacted]");
        assert_eq!(
            redact(r#"{"x-api-key": "secret", "a": 1}"#),
            r#"{"x-api-key": "[redacted]", "a": 1}"#
        );
        assert_eq!(
            redact("https://x/v1beta/models?key=AIzaSECRET&alt=json"),
            "https://x/v1beta/models?key=[redacted]&alt=json"
        );
        assert_eq!(redact("api-key=topsecret"), "api-key=[redacted]");
        assert_eq!(redact("used sk-abcdefghijkl here"), "used sk-[redacted] here");
        assert_eq!(redact("nothing to hide"), "nothing to hide");
    }
}
