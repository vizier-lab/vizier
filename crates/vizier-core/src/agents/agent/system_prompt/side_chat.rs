//! The `## Side Chat` section and fixed no-tools reply (`specs/033-btw-side-chat/
//! contracts/agent-side-chat.md`).

/// Rendered as a section of the context block, never a system message, worded exactly as the
/// contract specifies.
pub const SIDE_CHAT_MD: &str = "## Side Chat\n\
This is an off-the-record side chat beside the conversation above. Nothing in it is saved: \
you won't remember it, and it won't appear in the conversation. Any messages after the \
conversation's last reply belong to this side chat. You can't use tools here — if asked to \
do something that needs one (searching memory, running code, reading files, contacting \
anything), say you can't in a side chat and suggest asking in the conversation instead.";

/// The reply when the model offered only a tool call (research R5).
pub const SIDE_CHAT_NO_TOOLS: &str = "I can't use tools in a side chat. Ask me in the conversation if you'd like me to do that.";
