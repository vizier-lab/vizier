use chrono::{Local, Utc};
use rig_core::{
    OneOrMany,
    message::{Message, UserContent},
};

use crate::schema::{Memory, Skill};

/// First line of every context block; lets consumers of the raw user message (e.g. dummyplug)
/// tell the injected block apart from the user's own text.
pub const CONTEXT_HEADER: &str = "# Context\n";

/// Per-request context (time, related memories, related skills).
///
/// Prepended to the current user message instead of being sent as system messages, so the
/// system prompts and replayed history stay byte-identical across requests and remain
/// cacheable by the provider. Time is rounded to the minute for the same reason.
pub fn context_md(memory: &[Memory], skills: &[Skill]) -> String {
    let utc_now = Utc::now();
    let local_now = Local::now();

    let mut sections = vec![format!(
        "## Time\n**System Datetime (UTC)**: {}\n**Actual Datetime (Local Timezone)**: {}",
        utc_now.format("%A, %Y-%m-%d %H:%M UTC"),
        local_now.format("%A, %Y-%m-%d %H:%M %:z"),
    )];

    if !memory.is_empty() {
        let summarize_memories = memory
            .iter()
            .map(|memory| {
                format!(
                    "### {}\nslug: **{}**\n**use the slug for more detail of this memory**\n \n---",
                    memory.title, memory.slug
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        sections.push(format!(
            "## Possible Related Memories\nprovided below are memory that could be (but not always) related to user message \n{}",
            summarize_memories
        ));
    }

    if !skills.is_empty() {
        let summarize_skills = skills
            .iter()
            .map(|skill| {
                format!(
                    "### {}\nslug: **{}**\n{}\n**call get_skill_details or use_skill with this slug for more detail**\n---",
                    skill.name, skill.name, skill.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        sections.push(format!(
            "## Possibly Related Skills\nprovided below are skills that could be (but not always) related to the user message\n{}",
            summarize_skills
        ));
    }

    format!("{CONTEXT_HEADER}{}", sections.join("\n\n"))
}

/// Prepend `context` as the first text block of a user message. Non-user messages are returned
/// unchanged.
pub fn with_context(message: Message, context: String) -> Message {
    match message {
        Message::User { content } => {
            let mut contents = OneOrMany::one(UserContent::text(context));
            for item in content {
                contents.push(item);
            }
            Message::User { content: contents }
        }
        other => other,
    }
}
