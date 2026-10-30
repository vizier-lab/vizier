//! A typed provider error, so retry classification reads the HTTP status (and `Retry-After`)
//! instead of guessing from error text (research R6).

use std::time::Duration;

use reqwest::header::HeaderValue;

use crate::VizierError;

#[derive(Debug, Clone, PartialEq)]
pub enum LlmError {
    /// A non-2xx response.
    Http {
        status: u16,
        retry_after: Option<Duration>,
        body: String,
    },
    /// Refused, reset, DNS or TLS failure: the request never got an answer.
    Transport(String),
    Timeout,
    /// A 2xx body that could not be parsed, or that carried an error object.
    Decode(String),
    /// A missing credential, an unsupported dialect, a bad URL.
    Config(String),
    /// The operation is not offered by this provider (e.g. a model listing).
    Unsupported,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::Http { status, body, .. } => {
                let reason = reqwest::StatusCode::from_u16(*status)
                    .ok()
                    .and_then(|s| s.canonical_reason())
                    .unwrap_or("");
                write!(f, "HTTP {status} {reason}: {}", truncate(body, 2000))
            }
            LlmError::Transport(e) => write!(f, "connection failed: {e}"),
            LlmError::Timeout => write!(f, "request timed out"),
            LlmError::Decode(e) => write!(f, "unexpected provider response: {e}"),
            LlmError::Config(e) => write!(f, "{e}"),
            LlmError::Unsupported => write!(f, "not supported by this provider"),
        }
    }
}

impl std::error::Error for LlmError {}

impl From<LlmError> for VizierError {
    fn from(value: LlmError) -> Self {
        VizierError(value.to_string())
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `Retry-After` as delta-seconds or an HTTP date. A date in the past is a zero wait.
pub fn parse_retry_after(value: &HeaderValue) -> Option<Duration> {
    let text = value.to_str().ok()?.trim();
    if let Ok(secs) = text.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    if let Ok(secs) = text.parse::<f64>()
        && secs.is_finite()
        && secs >= 0.0
    {
        return Some(Duration::from_secs_f64(secs));
    }
    let when = chrono::DateTime::parse_from_rfc2822(text).ok()?;
    let wait = when.with_timezone(&chrono::Utc) - chrono::Utc::now();
    Some(wait.to_std().unwrap_or(Duration::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_seconds() {
        assert_eq!(
            parse_retry_after(&HeaderValue::from_static("2")),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            parse_retry_after(&HeaderValue::from_static(" 120 ")),
            Some(Duration::from_secs(120))
        );
    }

    #[test]
    fn retry_after_http_date() {
        let future = chrono::Utc::now() + chrono::Duration::seconds(30);
        let header = future.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let wait = parse_retry_after(&HeaderValue::from_str(&header).unwrap()).unwrap();
        assert!(wait <= Duration::from_secs(30) && wait >= Duration::from_secs(28));

        let past = HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT");
        assert_eq!(parse_retry_after(&past), Some(Duration::ZERO));
    }

    #[test]
    fn retry_after_garbage() {
        assert_eq!(parse_retry_after(&HeaderValue::from_static("soon")), None);
        assert_eq!(parse_retry_after(&HeaderValue::from_static("-3")), None);
    }

    #[test]
    fn http_error_display_names_the_status() {
        let e = LlmError::Http {
            status: 429,
            retry_after: None,
            body: "slow down".into(),
        };
        assert_eq!(e.to_string(), "HTTP 429 Too Many Requests: slow down");
    }
}
