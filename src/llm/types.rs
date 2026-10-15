//! The provider-neutral request and response types every dialect translates to and from
//! (contract `provider-layer.md` § Types).

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{Result, VizierError};

/// One message of a conversation. `System` is allowed anywhere: each dialect's `prepare` step
/// normalises it for its wire format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Message {
    System(String),
    User(Vec<Part>),
    Assistant(Vec<AssistantPart>),
    /// The result of one tool call, as its own message: dialects group consecutive results
    /// as their wire format requires.
    ToolResult { call_id: String, content: Vec<Part> },
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Message::System(text.into())
    }

    pub fn user_text(text: impl Into<String>) -> Result<Self> {
        Self::user(vec![Part::Text(text.into())])
    }

    pub fn user(content: Vec<Part>) -> Result<Self> {
        if content.is_empty() {
            return Err(VizierError("a user message needs at least one part".into()));
        }
        Ok(Message::User(content))
    }

    pub fn assistant(content: Vec<AssistantPart>) -> Result<Self> {
        if content.is_empty() {
            return Err(VizierError(
                "an assistant message needs at least one part".into(),
            ));
        }
        Ok(Message::Assistant(content))
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Message::Assistant(vec![AssistantPart::Text(text.into())])
    }

    pub fn tool_result(call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Message::ToolResult {
            call_id: call_id.into(),
            content: vec![Part::Text(text.into())],
        }
    }
}

/// Content a person (or a tool) sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Text(String),
    Attachment {
        mime: String,
        #[serde(with = "base64_bytes")]
        data: Vec<u8>,
        name: Option<String>,
    },
}

impl Part {
    pub fn text(&self) -> Option<&str> {
        match self {
            Part::Text(t) => Some(t),
            Part::Attachment { .. } => None,
        }
    }
}

/// Content the model sends back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPart {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        /// Opaque per-dialect data a provider needs back with the call (e.g. Gemini's
        /// `thoughtSignature`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_data: Option<serde_json::Value>,
    },
    /// Reasoning kept in memory within a turn so dialects can send it back (thinking
    /// signatures, DeepSeek `reasoning_content`, encrypted Responses items). Never persisted.
    Reasoning {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_data: Option<serde_json::Value>,
    },
}

/// A tool the model may call. Serialises to the same `{name, description, parameters}` JSON
/// rig did: `sandbox/docs.rs` and the 014 fixtures depend on it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    /// Answer in text only. Tools stay defined, since providers reject a history holding tool
    /// calls when no tools are, but the model is told not to call them.
    None,
}

/// Token usage, with rig's field names so token accounting is unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

impl Usage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    /// `Some(ToolChoice::None)` asks for a text-only answer.
    pub tool_choice: Option<ToolChoice>,
    pub max_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: Option<String>,
    pub content: Vec<AssistantPart>,
    pub usage: Usage,
}

impl CompletionResponse {
    /// The text parts, joined.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                AssistantPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Attachment bytes as base64 in JSON (fixtures, debug dumps).
mod base64_bytes {
    use super::*;

    pub fn serialize<S: serde::Serializer>(data: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(data))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_definition_json_matches_rig() {
        let ours = ToolDefinition {
            name: "memory_search".into(),
            description: "Search memory".into(),
            parameters: json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        };
        let rig = rig_core::completion::ToolDefinition {
            name: "memory_search".into(),
            description: "Search memory".into(),
            parameters: json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        };
        assert_eq!(
            serde_json::to_value(&ours).unwrap(),
            serde_json::to_value(&rig).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&ours).unwrap(),
            serde_json::to_string(&rig).unwrap()
        );
    }

    #[test]
    fn constructors_reject_empty_content() {
        assert!(Message::user(vec![]).is_err());
        assert!(Message::assistant(vec![]).is_err());
        assert!(Message::user_text("hi").is_ok());
    }

    #[test]
    fn attachment_round_trips_as_base64() {
        let part = Part::Attachment {
            mime: "image/png".into(),
            data: vec![1, 2, 3],
            name: Some("a.png".into()),
        };
        let v = serde_json::to_value(&part).unwrap();
        assert_eq!(v["attachment"]["data"], "AQID");
        assert_eq!(serde_json::from_value::<Part>(v).unwrap(), part);
    }
}
