//! The one error type of the service layer. An adapter maps it to its own wire format (the
//! HTTP envelope in `vizier-http`, an IPC error in the desktop app).

use crate::error::VizierError;

#[derive(Debug, Clone)]
pub enum ServiceError {
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
    Invalid { status: u16, message: String, code: Option<String> },
    Conflict { message: String, code: Option<String>, data: Option<serde_json::Value> },
    Gone { code: String, message: String },
    TooLarge(String),
    RateLimited(String),
    Unavailable(String),
    Internal(String),
}

impl ServiceError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid { status: 400, message: message.into(), code: None }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::Invalid { status, .. } => *status,
            Self::Conflict { .. } => 409,
            Self::Gone { .. } => 410,
            Self::TooLarge(_) => 413,
            Self::RateLimited(_) => 429,
            Self::Unavailable(_) => 503,
            Self::Internal(_) => 500,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::NotFound(m)
            | Self::Unauthorized(m)
            | Self::Forbidden(m)
            | Self::TooLarge(m)
            | Self::RateLimited(m)
            | Self::Unavailable(m)
            | Self::Internal(m) => m,
            Self::Invalid { message, .. } | Self::Conflict { message, .. } | Self::Gone { message, .. } => message,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Invalid { code, .. } | Self::Conflict { code, .. } => code.as_deref(),
            Self::Gone { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ServiceError {}

impl From<VizierError> for ServiceError {
    fn from(e: VizierError) -> Self {
        Self::Internal(e.0)
    }
}

impl From<anyhow::Error> for ServiceError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses() {
        assert_eq!(ServiceError::NotFound("x".into()).status(), 404);
        assert_eq!(ServiceError::invalid("x").status(), 400);
        assert_eq!(ServiceError::Gone { code: "c".into(), message: "m".into() }.status(), 410);
        assert_eq!(ServiceError::from(VizierError("boom".into())).status(), 500);
    }
}
