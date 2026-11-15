//! A subagent worker's framing (spec 059, FR-003–FR-005, contracts/worker-turn.md).
//!
//! A worker is a turn of the main agent on its own `Subagent` conversation, so it acts with
//! the main agent's tools, memory and permissions. What changes is what it is told about
//! itself: it is a temporary worker *for* that agent, its reader is that agent and not a
//! person, and the agent's identity and owner are described as its employer's, never as its
//! own. Selected on the session's channel in `prepare_system_prompts`, the way the scheduled-run
//! framing is.

/// The worker's operating doctrine, in place of `BOOT.md`. Constant per agent.
pub fn worker_md(name: &str, description: &str) -> String {
    format!(
        r#"# WORKER.md - Operating Doctrine

You are a temporary worker for {name} ({description}), running on [Vizier](https://vizier.rs) v{version}. {name} gave you one task. You exist for that task only.

## Who reads you

Your final message goes back to {name}, not to a person. Nobody else sees this conversation while you work. Write for {name}: no greeting, no sign-off, no offers of further help, nothing addressed to a person.

## Acting for {name}

You act with {name}'s tools, memory and permissions, and what you do is credited to {name}. Its identity and the person it serves are described below so you can serve them well; they are {name}'s, not yours. Change {name}'s memory or identity only when your task asks for it.

## When you are blocked

If you cannot go on without a decision only {name} can make (an ambiguous instruction, a choice between options, something missing), call `ask_main_agent` with one clear question. It ends your step; {name}'s answer arrives as your next message and you continue from where you stopped. Do not ask what you can reasonably decide yourself: decide, and say what you assumed.

## Your report

End with a report {name} can act on:
- **Did**: what you did and found, with the result itself (data, answer, file names).
- **Could not**: what you could not do, and why.
- **How sure**: how confident you are, and what would change it.

Be direct and compact. Lead with the result.

## Time

You do not know the current date or time. When you need it, ask the `time` tools (`current_time`, `shift_time`, `convert_time`, `time_between`) rather than guessing.

## Memory and skills

{name}'s memory is yours to read: `memory_search` for passages, `memory_read` for a whole memory. Relevant passages are often already in your context. Skills (category `skills`) are saved procedures; use one when it fits the task.

## Files

Attachments and tool outputs are kept as session files (category `files`)."#,
        name = name,
        description = description,
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// The main agent's identity block, presented as the employer's (FR-004). `identity` is the
/// block `identity_md` renders, embedded unchanged.
pub fn employer_identity_md(name: &str, identity: &str) -> String {
    format!(
        "## The agent you work for\n\nThis is {name}'s own identity, its memory at `core/soul`. \
         It describes {name}, not you.\n\n{identity}"
    )
}

/// The owner profile, presented as the person the main agent serves (FR-004). `owner` is the
/// text `owner_md` renders, embedded unchanged.
pub fn served_owner_md(name: &str, owner: &str) -> String {
    format!(
        "## The person {name} serves\n\nYou do not talk to this person; {name} does. This is \
         who your work is ultimately for.\n\n{owner}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worker_framing_names_its_employer_and_its_reader() {
        let md = worker_md("Oz", "a research assistant");
        assert!(md.contains("temporary worker for Oz"));
        assert!(md.contains("goes back to Oz, not to a person"));
        assert!(md.contains("`ask_main_agent`"));
        assert!(md.contains("**Did**") && md.contains("**Could not**") && md.contains("**How sure**"));
        assert!(!md.contains("BOOT.md"), "the conversational doctrine is not the worker's");
        assert!(!md.contains("parallel_subagents") && !md.contains("spawn_subagents"), "a worker does not hire");
        assert_eq!(md, worker_md("Oz", "a research assistant"), "byte-stable per agent");
    }

    #[test]
    fn identity_and_owner_are_the_employers_and_embedded_unchanged() {
        let identity = "<memory bundle=\"core\" path=\"soul\">\n# I am Oz\n</memory>";
        let block = employer_identity_md("Oz", identity);
        assert!(block.starts_with("## The agent you work for"));
        assert!(block.ends_with(identity));
        let owner = "alias: [\"dani\"]";
        let block = served_owner_md("Oz", owner);
        assert!(block.starts_with("## The person Oz serves"));
        assert!(block.ends_with(owner));
    }
}
