//! The `## Woken turn` section (spec 067).
//!
//! A wake starts a turn in a conversation the agent already has. The turn must know three
//! things it cannot read off the request alone: that it started this turn itself, that the
//! request is not a person's message, and that nobody is waiting on the answer there.
//!
//! **In the context block, not in the message.** The block is a separate part of the user
//! message, so the request's own text stays exactly what the agent wrote — which is what a
//! script reads, and what the offline `dummyplug` harness parses. A framing inside the message
//! would make the request unreadable as a tool call.
//!
//! **Not a prompt section either.** The prompt sections are selected on the *session's channel*,
//! and a wake lands in an ordinary WebUI or Discord channel that also carries the person's own
//! messages — a channel-selected section could not tell a wake from a person's message. The
//! request's kind can, and only a wake carries it.

use crate::schema::VizierRequestContent;

/// The framing, or `None` for a turn that was not woken.
pub fn woken_note(content: &VizierRequestContent) -> Option<&'static str> {
    match content {
        VizierRequestContent::Woken(_) => Some(WOKEN_MD),
        _ => None,
    }
}

/// Rendered as a section of the context block, never a system message.
pub const WOKEN_MD: &str = "## Woken turn\n\
This turn is one you started yourself in this conversation, to do the work below. It is not a \
message from the person, and nobody is waiting on the answer here: your reply is saved into \
this conversation, for whoever reads it later. Do the work now. Do not ask the person anything \
— nothing here can answer a question — and do not offer to do it later.";
