//! Skills and commands in a Workspace (`specs/053-workspace-channel/research.md` R19).
//!
//! The format is `/<command> @<agent-id> [input]`: the command first, then who it is for. Pure
//! and server-side, so the API and every client agree. Only the message's start is read; anything
//! that is not exactly this form is plain text (and a plain mention elsewhere still wakes).

/// What a message starting with `/` means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Not a command. The caller treats the message as text.
    Plain,
    /// `/name @a input` or `/skill name @a input`: run the skill on each agent.
    Skill { name: String, agents: Vec<String>, input: String },
    /// `/skills @a`: list the agent's skills.
    Skills { agents: Vec<String> },
    /// `/abort @a`: stop that agent's turn here.
    Abort { agents: Vec<String> },
    /// `/abort` alone: the same as Stop all.
    StopAll,
    /// `/checkpoint @a`.
    Checkpoint { agents: Vec<String> },
    /// `/lobotomy @a`.
    Lobotomy { agents: Vec<String> },
    /// `/btw` and `/clear-files`: not available in a Workspace.
    NotInWorkspace(&'static str),
}

/// The refusal shown for a side chat or clearing files.
pub const NOT_IN_WORKSPACE: &str =
    "Side chats and clearing files are not available in a Workspace.";

/// Reads a message. `agents` are the handles of the member agents.
pub fn parse(text: &str, agents: &[&str]) -> Parsed {
    let Some(rest) = text.trim_start().strip_prefix('/') else {
        return Parsed::Plain;
    };
    let (word, mut rest) = split_word(rest);
    if word.is_empty() {
        return Parsed::Plain;
    }
    let lower = word.to_ascii_lowercase();
    if matches!(lower.as_str(), "btw" | "clear-files" | "clear_files") {
        return Parsed::NotInWorkspace(NOT_IN_WORKSPACE);
    }

    let name = if lower == "skill" {
        let (name, after) = split_word(rest);
        if name.is_empty() {
            return Parsed::Plain;
        }
        rest = after;
        name.to_string()
    } else {
        word.to_string()
    };

    let (targets, input) = leading_mentions(rest, agents);
    let lower_name = name.to_ascii_lowercase();
    if targets.is_empty() {
        // `/abort` alone is Stop all. Anything else without a target is ordinary text.
        return if lower == "abort" && input.is_empty() { Parsed::StopAll } else { Parsed::Plain };
    }
    match (lower == "skill", lower_name.as_str()) {
        (false, "abort") => Parsed::Abort { agents: targets },
        (false, "checkpoint") => Parsed::Checkpoint { agents: targets },
        (false, "lobotomy") => Parsed::Lobotomy { agents: targets },
        (false, "skills") => Parsed::Skills { agents: targets },
        _ => Parsed::Skill { name, agents: targets, input },
    }
}

/// The text an agent receives for a command: `/name input`, `/skills`, `/abort`, … with the
/// mentions taken out. `None` when the message is not a command that names agents.
pub fn command_text(parsed: &Parsed) -> Option<String> {
    match parsed {
        Parsed::Skill { name, input, .. } => {
            Some(if input.is_empty() { format!("/{name}") } else { format!("/{name} {input}") })
        }
        Parsed::Skills { .. } => Some("/skills".into()),
        Parsed::Abort { .. } => Some("/abort".into()),
        Parsed::Checkpoint { .. } => Some("/checkpoint".into()),
        Parsed::Lobotomy { .. } => Some("/lobotomy".into()),
        Parsed::Plain | Parsed::StopAll | Parsed::NotInWorkspace(_) => None,
    }
}

/// The agents a command names.
pub fn targets(parsed: &Parsed) -> &[String] {
    match parsed {
        Parsed::Skill { agents, .. }
        | Parsed::Skills { agents }
        | Parsed::Abort { agents }
        | Parsed::Checkpoint { agents }
        | Parsed::Lobotomy { agents } => agents,
        _ => &[],
    }
}

/// What one woken agent is sent, and whether it is plain text. A command goes only to the agents
/// it names, as `/name input`; every other mention gets the message as written with the mentions
/// at its very start removed (as Discord's `command_text` does). Plain text must never run a
/// skill, so the caller marks the request (research R19).
pub fn request_for(body: &str, target: &str, agents: &[&str]) -> (String, bool) {
    let parsed = parse(body, agents);
    if let Some(text) = command_text(&parsed)
        && targets(&parsed).iter().any(|t| t.eq_ignore_ascii_case(target))
    {
        return (text, false);
    }
    let (_, rest) = leading_mentions(body.trim_start(), agents);
    let text = if rest.is_empty() { body.trim().to_string() } else { rest };
    (text, true)
}

fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.find(char::is_whitespace) {
        Some(i) => (&text[..i], &text[i..]),
        None => (text, ""),
    }
}

/// The member agents named one after another right at `text`'s start, and what follows them.
fn leading_mentions(mut text: &str, agents: &[&str]) -> (Vec<String>, String) {
    let mut found: Vec<String> = Vec::new();
    loop {
        let (token, after) = split_word(text);
        let handle = token.strip_prefix('@').and_then(|h| {
            agents.iter().find(|a| a.eq_ignore_ascii_case(h)).map(|a| (*a).to_string())
        });
        match handle {
            Some(h) => {
                if !found.contains(&h) {
                    found.push(h);
                }
                text = after;
            }
            None => break,
        }
    }
    (found, text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENTS: &[&str] = &["alpha", "beta"];

    fn skill(name: &str, agents: &[&str], input: &str) -> Parsed {
        Parsed::Skill {
            name: name.into(),
            agents: agents.iter().map(|a| a.to_string()).collect(),
            input: input.into(),
        }
    }

    #[test]
    fn a_skill_for_one_agent() {
        assert_eq!(parse("/greet @alpha Rafi", AGENTS), skill("greet", &["alpha"], "Rafi"));
    }

    #[test]
    fn the_skill_keyword_names_it_explicitly() {
        assert_eq!(
            parse("/skill greet @alpha Rafi", AGENTS),
            skill("greet", &["alpha"], "Rafi")
        );
    }

    #[test]
    fn several_agents_each_get_it() {
        assert_eq!(
            parse("/greet @alpha @beta hello there", AGENTS),
            skill("greet", &["alpha", "beta"], "hello there")
        );
    }

    #[test]
    fn a_skill_may_have_no_input() {
        assert_eq!(parse("/greet @beta", AGENTS), skill("greet", &["beta"], ""));
    }

    #[test]
    fn a_skill_that_does_not_exist_still_parses() {
        assert_eq!(parse("/nope @alpha", AGENTS), skill("nope", &["alpha"], ""));
    }

    #[test]
    fn the_listing_and_the_per_agent_commands() {
        assert_eq!(parse("/skills @alpha", AGENTS), Parsed::Skills { agents: vec!["alpha".into()] });
        assert_eq!(parse("/abort @beta", AGENTS), Parsed::Abort { agents: vec!["beta".into()] });
        assert_eq!(parse("/checkpoint @alpha", AGENTS), Parsed::Checkpoint { agents: vec!["alpha".into()] });
        assert_eq!(parse("/lobotomy @alpha", AGENTS), Parsed::Lobotomy { agents: vec!["alpha".into()] });
    }

    #[test]
    fn abort_alone_is_stop_all() {
        assert_eq!(parse("/abort", AGENTS), Parsed::StopAll);
        assert_eq!(parse("  /abort  ", AGENTS), Parsed::StopAll);
    }

    #[test]
    fn side_chats_and_clearing_files_are_refused() {
        for text in ["/btw @alpha hello", "/btw", "/clear-files", "/clear_files @alpha"] {
            assert_eq!(parse(text, AGENTS), Parsed::NotInWorkspace(NOT_IN_WORKSPACE), "{text}");
        }
    }

    #[test]
    fn a_slash_word_with_no_mention_right_after_is_plain_text() {
        assert_eq!(parse("/greet", AGENTS), Parsed::Plain);
        assert_eq!(parse("/etc/hosts is wrong", AGENTS), Parsed::Plain);
        assert_eq!(parse("/greet please @alpha", AGENTS), Parsed::Plain);
        assert_eq!(parse("/checkpoint", AGENTS), Parsed::Plain);
    }

    #[test]
    fn a_mention_first_is_a_plain_mention_not_a_command() {
        assert_eq!(parse("@alpha /greet", AGENTS), Parsed::Plain);
    }

    #[test]
    fn a_person_or_unknown_name_after_the_command_is_not_a_target() {
        assert_eq!(parse("/greet @rafi hi", AGENTS), Parsed::Plain);
        assert_eq!(parse("/greet @alpha @rafi hi", AGENTS), skill("greet", &["alpha"], "@rafi hi"));
    }

    #[test]
    fn not_a_command_at_all() {
        assert_eq!(parse("hello", AGENTS), Parsed::Plain);
        assert_eq!(parse("/", AGENTS), Parsed::Plain);
        assert_eq!(parse("/skill", AGENTS), Parsed::Plain);
    }

    #[test]
    fn a_command_is_sent_only_to_the_agents_it_names_and_everyone_else_gets_plain_text() {
        let both = &["alpha", "beta"];
        assert_eq!(request_for("/greet @alpha Rafi", "alpha", both), ("/greet Rafi".into(), false));
        // beta was named later in the text, so it is a plain mention of the whole message.
        assert_eq!(
            request_for("/greet @alpha tell @beta", "beta", both),
            ("/greet @alpha tell @beta".into(), true)
        );
        assert_eq!(request_for("/skills @alpha", "alpha", both), ("/skills".into(), false));
    }

    #[test]
    fn plain_mentions_lose_only_the_mentions_at_the_start() {
        let both = &["alpha", "beta"];
        assert_eq!(request_for("@alpha @beta tools", "beta", both), ("tools".into(), true));
        assert_eq!(request_for("hey @alpha, look", "alpha", both), ("hey @alpha, look".into(), true));
        // `@a /foo` is plain: the text reaches the agent, and nothing runs.
        assert_eq!(request_for("@alpha /foo", "alpha", both), ("/foo".into(), true));
        // Only mentions: the message is kept rather than sent empty.
        assert_eq!(request_for("@alpha", "alpha", both), ("@alpha".into(), true));
    }

    #[test]
    fn command_names_ignore_case() {
        assert_eq!(parse("/ABORT @alpha", AGENTS), Parsed::Abort { agents: vec!["alpha".into()] });
    }
}
