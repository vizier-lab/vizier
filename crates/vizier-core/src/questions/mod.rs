//! Asking the person (`specs/042-agent-asking-question`): the two tools an agent uses when it
//! needs something from somebody rather than from a tool.
//!
//! - [`tool_question::AskQuestion`] asks one or more structured questions and **ends the turn**.
//!   The answer comes back as the person's next message, so it takes the ordinary history,
//!   queue and attribution path (research R7).
//! - [`tool_sensitive::AskSensitiveInput`] asks for one sensitive value and **does not** end the
//!   turn: it blocks the calling script, the WebUI shows a masked dialog, and the value is armed
//!   into the turn's [`crate::secrets::SecretRedactor`] on arrival so every copy that leaves the
//!   turn is scrubbed exactly as for `get_secret` (research R8–R10).
//!
//! One module for both tools because they share the conversation, the access rule, the limits and
//! the validation discipline; they differ only in where the answer goes.
//!
//! `model` and `limits` hold the shapes and the numbers, `validate` the rules both sides of a
//! round trip use, `surface` the per-channel presentation trait, `text_form` the fallback grammar,
//! `submit` the one path an answer takes, and `sensitive` the open-ask registry.

pub mod limits;
pub mod model;
pub mod sensitive;
pub mod submit;
pub mod surface;
pub mod text_form;
pub mod tool_question;
pub mod tool_sensitive;
pub mod validate;
