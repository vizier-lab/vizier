//! Skills invoked by name from chat (spec 021): `/<name> [input]`, `/skill <name> [input]`
//! and `/skills`.
//!
//! [`parse`] is pure and knows nothing about which skills exist; [`resolve`] and
//! [`available`] look the name up against an agent's skills and the global ones. Every
//! channel delivers the person's text as `Chat`, so this is used once, in the agent's chat
//! path, plus by the channels that need to know a name before they decide to answer.

use crate::{schema::Skill, skill::SkillManager};

/// Names the commands themselves use. A skill with one of these names is never invoked by
/// name, so `/skill` and `/skills` always mean the commands.
pub const RESERVED: [&str; 2] = ["skill", "skills"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Not a skill command: ordinary chat.
    Plain,
    /// `/skills`: list the skills.
    List,
    /// `/<name> input` or `/skill <name> input`. `name` is lower-cased with any `@bot`
    /// suffix removed; `input` is trimmed and may be empty.
    Invoke { name: String, input: String },
}

/// A resolved invocation, handed to the turn: the skill to follow and the text the person
/// gave it, which is what the model receives as their message.
#[derive(Debug, Clone)]
pub struct Invoked {
    pub skill: Skill,
    pub input: String,
}

impl Invoked {
    /// The model's message when the person gave no input.
    pub const NO_INPUT: &str = "Run the skill.";

    pub fn message(&self) -> &str {
        if self.input.is_empty() { Self::NO_INPUT } else { &self.input }
    }
}

/// `[A-Za-z0-9][A-Za-z0-9_-]*`: what a command-shaped name looks like. A path (`/etc/hosts`)
/// or a lone `/` does not match, so it stays ordinary chat.
fn is_command_name(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Split off the first whitespace-delimited token; the rest is trimmed.
fn split_first(text: &str) -> (&str, &str) {
    match text.split_once(char::is_whitespace) {
        Some((first, rest)) => (first, rest.trim()),
        None => (text, ""),
    }
}

/// The command name with a Telegram-style `@bot` suffix removed.
fn command_token(token: &str) -> &str {
    token.split_once('@').map_or(token, |(name, _)| name)
}

pub fn parse(text: &str) -> Parsed {
    let Some(body) = text.trim().strip_prefix('/') else {
        return Parsed::Plain;
    };
    let (first, rest) = split_first(body);
    let command = command_token(first);
    if !is_command_name(command) {
        return Parsed::Plain;
    }

    match command.to_lowercase().as_str() {
        "skills" => Parsed::List,
        "skill" => {
            let (name, input) = split_first(rest);
            if name.is_empty() {
                // `/skill` alone: show what can be named.
                Parsed::List
            } else if is_command_name(name) {
                Parsed::Invoke { name: name.to_lowercase(), input: input.to_string() }
            } else {
                Parsed::Plain
            }
        }
        name => Parsed::Invoke { name: name.to_string(), input: rest.to_string() },
    }
}

/// Skill-name equality: case-insensitive, with `_` and `-` the same, because Telegram
/// command names cannot contain `-` (`/release_notes` is `release-notes`).
pub fn same_name(a: &str, b: &str) -> bool {
    let fold = |c: char| if c == '_' { '-' } else { c.to_ascii_lowercase() };
    a.len() == b.len() && a.chars().map(fold).eq(b.chars().map(fold))
}

fn is_reserved(name: &str) -> bool {
    RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(name))
}

/// The skills an agent can invoke: its own, then the global ones it does not override,
/// minus reserved names, sorted by name. Read from disk on every call, so an install or a
/// removal shows on the next message.
pub fn available(agent: &SkillManager, global: &SkillManager) -> crate::Result<Vec<Skill>> {
    let mut skills = agent.list_skills()?;
    for skill in global.list_skills()? {
        if !skills.iter().any(|s| s.name == skill.name) {
            skills.push(skill);
        }
    }
    skills.retain(|skill| !is_reserved(&skill.name));
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// The skill `name` invokes, the agent's own before a global one. A case-insensitive exact
/// match wins over one that only matches with `_`/`-` folded.
pub fn resolve(
    agent: &SkillManager,
    global: &SkillManager,
    name: &str,
) -> crate::Result<Option<Skill>> {
    if is_reserved(name) {
        return Ok(None);
    }
    let mut skills = agent.list_skills()?;
    skills.extend(global.list_skills()?);
    skills.retain(|skill| !is_reserved(&skill.name));

    if let Some(index) = skills.iter().position(|s| s.name.eq_ignore_ascii_case(name)) {
        return Ok(Some(skills.swap_remove(index)));
    }
    Ok(skills.into_iter().find(|s| same_name(&s.name, name)))
}

/// Whether `text` invokes one of this agent's skills. Telegram uses it to let such a message
/// through in a group without a mention.
pub fn names_skill(agent: &SkillManager, global: &SkillManager, text: &str) -> bool {
    match parse(text) {
        Parsed::Invoke { name, .. } => matches!(resolve(agent, global, &name), Ok(Some(_))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(name: &str, input: &str) -> Parsed {
        Parsed::Invoke { name: name.into(), input: input.into() }
    }

    #[test]
    fn parses_invocations() {
        assert_eq!(parse("/release-notes v1.2"), invoke("release-notes", "v1.2"));
        assert_eq!(parse("  /Release-Notes  "), invoke("release-notes", ""));
        assert_eq!(parse("/skill standup hi"), invoke("standup", "hi"));
        assert_eq!(parse("/skill Standup"), invoke("standup", ""));
        assert_eq!(parse("/release_notes@mybot x"), invoke("release_notes", "x"));
        assert_eq!(parse("/notes line one\nline two"), invoke("notes", "line one\nline two"));
    }

    #[test]
    fn parses_the_list() {
        assert_eq!(parse("/skills"), Parsed::List);
        assert_eq!(parse("/skills@bot"), Parsed::List);
        assert_eq!(parse("/SKILLS"), Parsed::List);
        assert_eq!(parse("/skill"), Parsed::List);
    }

    #[test]
    fn leaves_other_text_as_chat() {
        for text in ["/etc/hosts", "/ x", "/", "hello /x", "https://a/b", "", "/skill a/b", "/-x"] {
            assert_eq!(parse(text), Parsed::Plain, "{text:?}");
        }
    }

    #[test]
    fn names_match_across_case_and_separators() {
        assert!(same_name("release_notes", "Release-Notes"));
        assert!(same_name("standup", "STANDUP"));
        assert!(!same_name("release", "release-notes"));
    }

    fn manager(dir: &std::path::Path, scope: &str, names: &[&str]) -> SkillManager {
        let manager = SkillManager::new(dir.join(scope).to_str().unwrap());
        for name in names {
            manager
                .save_skill(&Skill {
                    name: name.to_string(),
                    agent_id: None,
                    author: "test".into(),
                    description: format!("{scope} {name}"),
                    content: format!("{scope} content"),
                    keywords: vec![],
                    version: 1,
                    resources: vec![],
                })
                .unwrap();
        }
        manager
    }

    #[test]
    fn resolves_agent_skills_before_global_ones() {
        let dir = tempfile::tempdir().unwrap();
        let agent = manager(dir.path(), "agent", &["standup", "release_notes", "skills"]);
        let global = manager(dir.path(), "global", &["standup", "release-notes", "triage"]);

        let standup = resolve(&agent, &global, "standup").unwrap().unwrap();
        assert_eq!(standup.description, "agent standup");
        // An exact name beats one that only matches with `_`/`-` folded.
        assert_eq!(resolve(&agent, &global, "release-notes").unwrap().unwrap().description, "global release-notes");
        assert_eq!(resolve(&agent, &global, "triage").unwrap().unwrap().description, "global triage");
        assert!(resolve(&agent, &global, "skills").unwrap().is_none(), "reserved");
        assert!(resolve(&agent, &global, "nope").unwrap().is_none());

        let names: Vec<_> = available(&agent, &global).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["release-notes", "release_notes", "standup", "triage"]);

        assert!(names_skill(&agent, &global, "/triage now"));
        assert!(names_skill(&agent, &global, "/skill triage"));
        assert!(!names_skill(&agent, &global, "/skills"));
        assert!(!names_skill(&agent, &global, "/unknown"));
    }
}
