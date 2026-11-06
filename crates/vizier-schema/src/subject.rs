//! Subjects and subject patterns (contracts/subjects.md).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::BusError;

/// Longest subject, in bytes.
pub const MAX_SUBJECT_LEN: usize = 255;

/// Subject for agent lifecycle commands.
pub const AGENTS_CMD: &str = "agents.cmd";
/// Subject for file upload/resolve requests.
pub const FILES_CMD: &str = "files.cmd";
/// Subject for dream triggers.
pub const DREAM_CMD: &str = "dream.cmd";
/// Subject for the process command socket.
pub const PROCESS_CMD: &str = "process.cmd";
/// Subject for integration change fan-out.
pub const INTEGRATION_CHANGED: &str = "integration.changed";
/// Subject for inbox fan-out: notifications created or read, and live setup status.
pub const INBOX_EVENTS: &str = "inbox.events";

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '%' | '~')
}

fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(is_token_char)
}

/// A validated, concrete subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Subject(String);

impl Subject {
    /// Parse a literal subject.
    pub fn parse(s: impl Into<String>) -> Result<Self, BusError> {
        let s = s.into();
        if s.is_empty() || s.len() > MAX_SUBJECT_LEN {
            return Err(BusError::InvalidSubject(format!("length {} out of range", s.len())));
        }
        if let Some(bad) = s.split('.').find(|t| !valid_token(t)) {
            return Err(BusError::InvalidSubject(format!("bad token {bad:?} in {s:?}")));
        }
        Ok(Self(s))
    }

    /// Percent-encode an identifier into exactly one token.
    pub fn encode_token(id: &str) -> String {
        let mut out = String::with_capacity(id.len());
        for b in id.bytes() {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        if out.is_empty() {
            // An empty id still has to be one token.
            out.push_str("%00");
        }
        out
    }

    /// The inverse of [`Subject::encode_token`].
    pub fn decode_token(token: &str) -> Result<String, BusError> {
        let bytes = token.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let hex = token
                    .get(i + 1..i + 3)
                    .ok_or_else(|| BusError::Decode(format!("truncated escape in {token:?}")))?;
                let v = u8::from_str_radix(hex, 16)
                    .map_err(|_| BusError::Decode(format!("bad escape in {token:?}")))?;
                out.push(v);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        if out == [0] {
            return Ok(String::new());
        }
        String::from_utf8(out).map_err(|_| BusError::Decode(format!("not utf-8: {token:?}")))
    }

    /// `prefix.{id}.suffix` with the id encoded. A subject made of an over-long identifier
    /// is still returned (the caller's id is bounded elsewhere); [`Subject::parse`] is the
    /// validating entry point for literals.
    fn with_id(prefix: &str, id: &str, suffix: Option<&str>) -> Self {
        let mut s = format!("{prefix}.{}", Self::encode_token(id));
        if let Some(suffix) = suffix {
            s.push('.');
            s.push_str(suffix);
        }
        Self(s)
    }

    /// `agent.{id}.req`
    pub fn agent_req(id: &str) -> Self {
        Self::with_id("agent", id, Some("req"))
    }

    /// `agent.{id}.memop`
    pub fn agent_memop(id: &str) -> Self {
        Self::with_id("agent", id, Some("memop"))
    }

    /// `agent.{id}.cmd` - one agent's lifecycle commands. The id is one encoded token, so a
    /// command reaches exactly the agent it names and nothing else (`specs/048-agent-service-seam`
    /// contracts/agents-client.md, rule 3).
    pub fn agent_cmd(id: &str) -> Self {
        Self::with_id("agent", id, Some("cmd"))
    }

    /// `session.{id}.events`
    pub fn session_events(id: &str) -> Self {
        Self::with_id("session", id, Some("events"))
    }

    /// `reply.{uuid}` — the uuid in simple (no-dash) form.
    pub fn reply(id: uuid::Uuid) -> Self {
        Self(format!("reply.{}", id.simple()))
    }

    /// `lease.{name}`; the name is encoded whole into one token.
    pub fn lease(name: &str) -> Self {
        Self::with_id("lease", name, None)
    }

    pub fn agents_cmd() -> Self {
        Self(AGENTS_CMD.into())
    }

    pub fn files_cmd() -> Self {
        Self(FILES_CMD.into())
    }

    pub fn dream_cmd() -> Self {
        Self(DREAM_CMD.into())
    }

    pub fn process_cmd() -> Self {
        Self(PROCESS_CMD.into())
    }

    pub fn integration_changed() -> Self {
        Self(INTEGRATION_CHANGED.into())
    }

    /// `inbox.events`
    pub fn inbox_events() -> Self {
        Self(INBOX_EVENTS.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn tokens(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Subject {
    type Error = BusError;
    fn try_from(s: String) -> Result<Self, BusError> {
        Self::parse(s)
    }
}

impl From<Subject> for String {
    fn from(s: Subject) -> String {
        s.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PatternToken {
    Literal(String),
    One,
    Rest,
}

/// A subscription pattern: exact tokens, `*` for one token, `>` for one or more trailing tokens.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectPattern {
    raw: String,
    tokens: Vec<PatternToken>,
}

impl SubjectPattern {
    pub fn parse(s: impl Into<String>) -> Result<Self, BusError> {
        let raw = s.into();
        let bad = |why: &str| BusError::InvalidPattern(format!("{why}: {raw:?}"));
        if raw.is_empty() || raw.len() > MAX_SUBJECT_LEN {
            return Err(bad("length out of range"));
        }
        let parts: Vec<&str> = raw.split('.').collect();
        let mut tokens = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            tokens.push(match *part {
                "*" => PatternToken::One,
                ">" if i + 1 == parts.len() => PatternToken::Rest,
                ">" => return Err(bad("`>` is only valid as the last token")),
                t if valid_token(t) => PatternToken::Literal(t.to_string()),
                _ => return Err(bad("bad token")),
            });
        }
        Ok(Self { raw, tokens })
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn matches(&self, subject: &Subject) -> bool {
        let mut subject_tokens = subject.tokens();
        for token in &self.tokens {
            match token {
                PatternToken::Rest => return subject_tokens.next().is_some(),
                PatternToken::One => {
                    if subject_tokens.next().is_none() {
                        return false;
                    }
                }
                PatternToken::Literal(l) => {
                    if subject_tokens.next() != Some(l.as_str()) {
                        return false;
                    }
                }
            }
        }
        subject_tokens.next().is_none()
    }
}

impl fmt::Display for SubjectPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl From<&Subject> for SubjectPattern {
    fn from(s: &Subject) -> Self {
        // A concrete subject is always a valid pattern.
        Self { raw: s.0.clone(), tokens: s.tokens().map(|t| PatternToken::Literal(t.to_string())).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Subject {
        Subject::parse(x).unwrap()
    }
    fn p(x: &str) -> SubjectPattern {
        SubjectPattern::parse(x).unwrap()
    }

    #[test]
    fn an_id_with_dots_is_one_token() {
        let subject = Subject::agent_req("my.agent");
        assert_eq!(subject.as_str(), "agent.my%2Eagent.req");
        assert_eq!(subject.tokens().count(), 3);
        assert_eq!(Subject::decode_token("my%2Eagent").unwrap(), "my.agent");
    }

    #[test]
    fn an_id_round_trips() {
        for id in ["", "a b", "\u{fc}n\u{ef}", "x*y", ">", "a.b.c", "100%"] {
            let t = Subject::encode_token(id);
            assert!(valid_token(&t), "{t}");
            assert_eq!(Subject::decode_token(&t).unwrap(), id);
        }
    }

    #[test]
    fn the_inbox_subject_is_fixed_and_apart_from_sessions() {
        let subject = Subject::inbox_events();
        assert_eq!(subject.as_str(), "inbox.events");
        assert_eq!(Subject::parse("inbox.events").unwrap(), subject);
        assert!(p("inbox.events").matches(&subject));
        assert!(!p("session.>").matches(&subject));
    }

    #[test]
    fn an_agent_command_is_one_token_too() {
        let subject = Subject::agent_cmd("my.agent");
        assert_eq!(subject.as_str(), "agent.my%2Eagent.cmd");
        assert_eq!(subject.tokens().count(), 3);
        assert_eq!(Subject::decode_token("my%2Eagent").unwrap(), "my.agent");
        // It matches its own pattern and not the request subject's.
        assert!(p("agent.*.cmd").matches(&subject));
        assert!(!p("agent.*.req").matches(&subject));
        assert!(!p("agent.my.agent.cmd").matches(&subject));
    }

    #[test]
    fn a_sibling_pattern_cannot_match_an_encoded_id() {
        assert!(!p("agent.my.req").matches(&Subject::agent_req("my.agent")));
        assert!(!p("agent.my.agent.req").matches(&Subject::agent_req("my.agent")));
        assert!(p("agent.*.req").matches(&Subject::agent_req("my.agent")));
    }

    #[test]
    fn wildcards() {
        assert!(p("a.*.c").matches(&s("a.b.c")));
        assert!(!p("a.*.c").matches(&s("a.b.b.c")));
        assert!(!p("a.*").matches(&s("a")));
        assert!(p("a.>").matches(&s("a.b")));
        assert!(p("a.>").matches(&s("a.b.c")));
        assert!(!p("a.>").matches(&s("a")));
        assert!(p(">").matches(&s("a")));
        assert!(p("a.b").matches(&s("a.b")));
        assert!(!p("a.b").matches(&s("a.b.c")));
    }

    #[test]
    fn invalid_patterns_are_refused() {
        for bad in ["", "a..b", "a.>.b", "ag*nt", "a.b>", "a b", ".a", "a."] {
            assert!(SubjectPattern::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn invalid_subjects_are_refused() {
        for bad in ["", "a..b", "a.*", "a.>", "a b", "a.\u{e9}"] {
            assert!(Subject::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Subject::parse("a".repeat(256)).is_err());
        assert!(Subject::parse("a".repeat(255)).is_ok());
    }

    #[test]
    fn a_subject_serialises_as_a_string() {
        let json = serde_json::to_string(&Subject::agents_cmd()).unwrap();
        assert_eq!(json, "\"agents.cmd\"");
        assert!(serde_json::from_str::<Subject>("\"a b\"").is_err());
    }
}
