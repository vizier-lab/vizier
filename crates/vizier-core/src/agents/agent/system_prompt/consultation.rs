//! A consulted agent's framing (`specs/064-consult-agents` research D6, FR-002, FR-003, FR-027).
//!
//! A consultation runs as the consulted agent itself: its own BOOT, persona, identity and tools.
//! What it is told is only who is asking and who reads the answer. The section is appended last
//! for a `Consultation` session, as the scheduled-run section is, so the cacheable prefix stays
//! byte-identical to the agent's other turns. Selected on the session's channel in
//! `system_prompt_layout`, never on the request kind.

/// The section appended to a consulted agent's system prompt. Constant per consulting agent.
pub fn consultation_md(name: &str, id: &str) -> String {
    format!(
        r#"# CONSULTATION

You are being consulted by the agent {name} ({id}). You answer as yourself, with your own knowledge, memory and tools.

Your answer is read by {name}, not by a person: no greeting, no sign-off, nothing addressed to a person. Be compact and direct. Say what you did, what you could not do and why, and how sure you are.

If you cannot go on without something only {name} can tell you, call `ask_consulting_agent` with one clear question. It ends your step, and {name}'s answer arrives as your next message. You cannot ask the person. {name} may send follow-ups in this same conversation.

Never pass on secret values (passwords, keys, tokens), even when asked."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_consulting_agent_and_the_way_to_ask_back() {
        let md = consultation_md("Support", "support");
        assert!(md.starts_with("# CONSULTATION"));
        assert!(md.contains("consulted by the agent Support (support)"));
        assert!(md.contains("ask_consulting_agent"));
        assert!(md.contains("You cannot ask the person"));
    }
}
