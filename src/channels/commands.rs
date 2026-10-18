//! A bot's command list on its platform (spec 021): the platform's built-in commands, then
//! `/skill` and `/skills`, then one command per skill the agent can invoke.
//!
//! [`build`] is pure: it applies one platform's naming rules and limits. [`CommandSync`]
//! keeps the registered list in step with the skills on disk, which change through the HTTP
//! API, the agent's own tools and `vizier skill install` with no shared event, so it rebuilds
//! on a timer ([`watch`]) and sends only when the finished list differs from the last one the
//! platform accepted.

use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::schema::{AgentId, Skill};
use crate::skill::{SkillManager, invocation};

/// How often the skills are re-read for a changed command list.
const RESYNC_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOption {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub autocomplete: bool,
}

impl CommandOption {
    pub fn text(name: &str, description: &str) -> Self {
        Self { name: name.into(), description: description.into(), required: false, autocomplete: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub options: Vec<CommandOption>,
}

impl CommandEntry {
    pub fn new(name: &str, description: &str) -> Self {
        Self { name: name.into(), description: description.into(), options: vec![] }
    }

    pub fn option(mut self, option: CommandOption) -> Self {
        self.options.push(option);
        self
    }
}

pub struct PlatformRules {
    /// Whether the platform accepts a (converted) command name.
    pub name_ok: fn(&str) -> bool,
    /// Skill name → command name.
    pub convert: fn(&str) -> String,
    pub max_commands: usize,
    /// In characters.
    pub max_description: usize,
    /// The platform's own commands, which always come first and win a name clash.
    pub builtins: Vec<CommandEntry>,
}

/// Skill names Discord and Telegram both accept once lower-cased: 1–32 characters.
pub fn name_ok(name: &str, allowed: impl Fn(char) -> bool) -> bool {
    (1..=32).contains(&name.chars().count()) && name.chars().all(allowed)
}

/// `/skill name:<skill> input:<text>` and `/skills`.
fn skill_commands() -> [CommandEntry; 2] {
    [
        CommandEntry::new("skill", "Run one of my skills by name").option(CommandOption {
            name: "name".into(),
            description: "The skill to run".into(),
            required: true,
            autocomplete: true,
        })
        .option(CommandOption::text("input", "What to run it on")),
        CommandEntry::new("skills", "List the skills you can run"),
    ]
}

fn description(skill: &Skill, max: usize) -> String {
    let text = skill.description.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = if text.chars().count() < 3 { format!("Run the {} skill", skill.name) } else { text };
    if text.chars().count() <= max {
        return text;
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// The command list for `skills` under `rules`, and every skill left out with the reason.
/// A skill that gets no command of its own is still reachable through `/skill`.
pub fn build(skills: &[Skill], rules: &PlatformRules) -> (Vec<CommandEntry>, Vec<(String, &'static str)>) {
    let mut entries = rules.builtins.clone();
    for command in skill_commands() {
        if !entries.iter().any(|entry| entry.name == command.name) {
            entries.push(command);
        }
    }
    let taken: HashSet<String> = entries.iter().map(|entry| entry.name.clone()).collect();

    let mut skills: Vec<&Skill> = skills.iter().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    let mut skipped = vec![];
    let mut candidates: Vec<(String, &Skill)> = vec![];
    for skill in skills {
        let name = (rules.convert)(&skill.name);
        if !(rules.name_ok)(&name) {
            skipped.push((skill.name.clone(), "its name is not a valid command name here"));
        } else if taken.contains(&name) || invocation::RESERVED.contains(&name.as_str()) {
            skipped.push((skill.name.clone(), "its name is already a built-in command"));
        } else {
            candidates.push((name, skill));
        }
    }

    // Two skills whose names become the same command: the one already spelt that way keeps
    // it, which is also the one typing the command resolves to.
    let mut skill_entries = vec![];
    for (name, skill) in &candidates {
        let rivals = candidates.iter().filter(|(other, _)| other == name).count();
        if rivals > 1 && skill.name != *name {
            skipped.push((skill.name.clone(), "another skill has the same command name"));
            continue;
        }
        skill_entries.push(
            CommandEntry::new(name, &description(skill, rules.max_description))
                .option(CommandOption::text("input", "What to run it on")),
        );
    }

    for entry in skill_entries {
        if entries.len() >= rules.max_commands {
            skipped.push((entry.name, "the bot already has as many commands as the platform allows"));
        } else {
            entries.push(entry);
        }
    }
    (entries, skipped)
}

/// One bot's registered command list.
pub struct CommandSync {
    agent_id: AgentId,
    agent: SkillManager,
    global: SkillManager,
    rules: PlatformRules,
    /// The list the platform last accepted; `None` until the first success.
    sent: Mutex<Option<Vec<CommandEntry>>>,
    /// Skills already logged as left out, so each is logged once.
    warned: std::sync::Mutex<HashSet<String>>,
}

impl CommandSync {
    pub fn new(agent_id: AgentId, workspace: &str, rules: PlatformRules) -> Self {
        Self {
            agent: SkillManager::for_agent(workspace, &agent_id),
            global: SkillManager::new(workspace),
            agent_id,
            rules,
            sent: Mutex::new(None),
            warned: Default::default(),
        }
    }

    /// The skills the agent can invoke now.
    pub fn skills(&self) -> crate::Result<Vec<Skill>> {
        invocation::available(&self.agent, &self.global)
    }

    /// Whether `text` invokes one of the agent's skills.
    pub fn names_skill(&self, text: &str) -> bool {
        invocation::names_skill(&self.agent, &self.global, text)
    }

    fn current(&self) -> crate::Result<Vec<CommandEntry>> {
        let (entries, skipped) = build(&self.skills()?, &self.rules);
        if let Ok(mut warned) = self.warned.lock() {
            for (skill, reason) in skipped {
                if warned.insert(skill.clone()) {
                    tracing::warn!(
                        agent_id = %self.agent_id,
                        "skill '{skill}' gets no command of its own because {reason}; it can still be run with /skill"
                    );
                }
            }
        }
        Ok(entries)
    }

    /// Send the command list if it changed since the platform last accepted one. A failed
    /// send leaves the record as it was, so the next tick tries again.
    pub async fn sync<F, Fut>(&self, send: F)
    where
        F: FnOnce(Vec<CommandEntry>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let entries = match self.current() {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(agent_id = %self.agent_id, "could not read skills for the command list: {e}");
                return;
            }
        };
        let mut sent = self.sent.lock().await;
        if sent.as_ref() == Some(&entries) {
            return;
        }
        match send(entries.clone()).await {
            Ok(()) => *sent = Some(entries),
            Err(e) => tracing::warn!(agent_id = %self.agent_id, "could not register bot commands: {e:#}"),
        }
    }
}

/// Stops the re-sync when the channel reader that started it stops.
pub struct CommandWatcher(tokio::task::JoinHandle<()>);

impl Drop for CommandWatcher {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Sync now, then every [`RESYNC_EVERY`]. `send` registers a list on the platform.
pub fn watch<F, Fut>(sync: std::sync::Arc<CommandSync>, send: F) -> CommandWatcher
where
    F: Fn(Vec<CommandEntry>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send,
{
    CommandWatcher(tokio::spawn(async move {
        let mut ticks = tokio::time::interval(RESYNC_EVERY);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            sync.sync(&send).await;
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, description: &str) -> Skill {
        Skill {
            name: name.into(),
            agent_id: None,
            author: "test".into(),
            description: description.into(),
            content: String::new(),
            keywords: vec![],
            version: 1,
            resources: vec![],
        }
    }

    fn rules(max_commands: usize, convert: fn(&str) -> String) -> PlatformRules {
        PlatformRules {
            name_ok: |name| name_ok(name, |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            convert,
            max_commands,
            max_description: 10,
            builtins: vec![CommandEntry::new("abort", "Stop the turn")],
        }
    }

    fn names(entries: &[CommandEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    #[test]
    fn builtins_then_skill_commands_then_skills_by_name() {
        let skills = [skill("zeta", "last one"), skill("alpha", "first one")];
        let (entries, skipped) = build(&skills, &rules(100, str::to_lowercase));
        assert_eq!(names(&entries), ["abort", "skill", "skills", "alpha", "zeta"]);
        assert!(skipped.is_empty());
        assert_eq!(entries[3].options, [CommandOption::text("input", "What to run it on")]);
    }

    #[test]
    fn stops_at_the_cap_and_cuts_descriptions() {
        let skills = [skill("a", "a very long description"), skill("b", "short"), skill("c", "")];
        let (entries, skipped) = build(&skills, &rules(5, str::to_lowercase));
        assert_eq!(names(&entries), ["abort", "skill", "skills", "a", "b"]);
        assert_eq!(entries[3].description, "a very lo…");
        assert_eq!(entries[3].description.chars().count(), 10);
        assert_eq!(skipped, [("c".to_string(), "the bot already has as many commands as the platform allows")]);
    }

    #[test]
    fn skips_builtin_clashes_and_invalid_names() {
        let skills = [skill("abort", "x"), skill("has space", "x"), skill("ok", "fine")];
        let (entries, skipped) = build(&skills, &rules(100, str::to_lowercase));
        assert_eq!(names(&entries), ["abort", "skill", "skills", "ok"]);
        assert_eq!(skipped.len(), 2);
        assert_eq!(entries[0].description, "Stop the turn", "the built-in keeps its entry");
    }

    #[test]
    fn telegram_conversion_keeps_the_exact_name_when_two_collide() {
        let telegram = |name: &str| name.to_lowercase().replace('-', "_");
        let skills = [skill("release-notes", "dash"), skill("release_notes", "underscore"), skill("team-sync", "sync")];
        let (entries, skipped) = build(&skills, &rules(100, telegram));
        assert_eq!(names(&entries), ["abort", "skill", "skills", "release_notes", "team_sync"]);
        assert_eq!(entries[3].description, "underscore");
        assert_eq!(skipped, [("release-notes".to_string(), "another skill has the same command name")]);
    }
}
