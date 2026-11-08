//! The `## Workspace` section of an agent's per-message context
//! (`specs/053-workspace-channel/research.md` R14). It rides the context block, not the system
//! prompt, so a change of membership never invalidates the prompt cache for the history.

use crate::{
    schema::{MemberKind, VizierChannelId, VizierSession, WorkspaceMember},
    storage::{VizierStorage, workspace::WorkspaceStorage},
};

/// What the section says. Plain data, so the wording can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub workspace: String,
    pub topic: String,
    /// The agent being addressed.
    pub me: String,
    pub people: Vec<String>,
    pub agents: Vec<String>,
    /// Who wrote the message that woke it.
    pub mentioned_by: String,
    /// Unread messages older than the ones copied into its record.
    pub skipped: u64,
}

pub fn render(facts: &Facts) -> String {
    let agents = facts
        .agents
        .iter()
        .map(|a| if *a == facts.me { format!("{a} (you)") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(", ");
    let people = if facts.people.is_empty() { "none".to_string() } else { facts.people.join(", ") };
    let mut text = format!(
        "## Workspace\n\
         You are in the Workspace \"{ws}\", topic \"{topic}\", shared with other agents and people.\n\
         - People: {people}\n\
         - Agents: {agents}\n\n\
         You read everything said here, and you are woken only when someone writes @{me}. \
         {by} mentioned you.\n\n\
         Writing @<agent-id> is not a way of naming an agent. It wakes that agent, right now, \
         with a request to act, and the agent cannot tell that apart from a passing remark: \
         \"@alpha owns the lock\" hands the lock to alpha exactly as much as \"@alpha, take the \
         lock\" does. The agent sees only what you wrote here, not your tool calls or your \
         reasoning.\n\n\
         So mention an agent for one reason: you want it to do something. If you are only \
         talking about an agent, write its name without the @ - \"alpha owns the lock\" tells \
         the room and wakes nobody. If you want it to act, mention it and say what you want \
         done.\n\n\
         An agent answers you only if it mentions you back, so a mention nobody needed leaves \
         a thread with no reply.",
        ws = facts.workspace,
        topic = facts.topic,
        me = facts.me,
        by = facts.mentioned_by,
    );
    if facts.skipped > 0 {
        text.push_str(&format!(
            "\n\n{} older unread messages were left out of what you were given.",
            facts.skipped
        ));
    }
    text
}

/// The section for a turn in a Workspace session, or `None` anywhere else.
pub async fn section(
    storage: &VizierStorage,
    session: &VizierSession,
    metadata: &serde_json::Value,
    mentioned_by: &str,
) -> Option<String> {
    let VizierChannelId::Workspace(workspace_id) = &session.1 else {
        return None;
    };
    let topic_id = session.2.as_deref()?;
    let workspace = storage.get_workspace(workspace_id).await.ok()??;
    let topic = storage.get_topic(workspace_id, topic_id).await.ok()??;
    let members: Vec<WorkspaceMember> = storage.list_members(workspace_id).await.ok()?;
    let by_kind = |kind| {
        members.iter().filter(|m| m.kind == kind).map(|m| m.handle.clone()).collect::<Vec<_>>()
    };
    Some(render(&Facts {
        workspace: workspace.name,
        topic: topic.name,
        me: session.0.clone(),
        people: by_kind(MemberKind::Person),
        agents: by_kind(MemberKind::Agent),
        mentioned_by: mentioned_by.to_string(),
        skipped: metadata.get("workspace_skipped").and_then(|v| v.as_u64()).unwrap_or(0),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            workspace: "ops".into(),
            topic: "daily".into(),
            me: "beta".into(),
            people: vec!["rafi".into(), "maya".into()],
            agents: vec!["alpha".into(), "beta".into()],
            mentioned_by: "alpha".into(),
            skipped: 0,
        }
    }

    #[test]
    fn it_names_the_room_the_members_and_the_rule() {
        let text = render(&facts());
        assert!(text.starts_with("## Workspace\n"));
        assert!(text.contains("\"ops\", topic \"daily\""));
        assert!(text.contains("- People: rafi, maya"));
        assert!(text.contains("- Agents: alpha, beta (you)"));
        assert!(text.contains("woken only when someone writes @beta"));
        assert!(text.contains("alpha mentioned you"));
        assert!(!text.contains("older unread"));
    }

    /// A mention wakes an agent; the agent cannot tell a hand-off from a passing remark. The
    /// rule has to say so, and has to offer the way out, or an agent that merely names another
    /// agent in prose wakes it anyway (`specs/053-workspace-channel/spec.md`, edge case
    /// "an agent named in passing").
    #[test]
    fn it_says_a_mention_is_a_hand_off_and_offers_the_bare_name() {
        let text = render(&facts());
        assert!(
            text.contains("is not a way of naming an agent"),
            "{text}"
        );
        assert!(
            text.contains("cannot tell that apart from a passing remark"),
            "{text}"
        );
        assert!(
            text.contains("write its name without the @"),
            "{text}"
        );
        assert!(text.contains("wakes nobody"), "{text}");
        assert!(
            text.contains("leaves a thread with no reply"),
            "{text}"
        );
    }

    /// The rule is its own paragraph, not the last sentence of a paragraph that also carries
    /// four other facts: buried in prose it is the part a model is most likely to skim.
    #[test]
    fn the_rule_is_a_paragraph_of_its_own() {
        let text = render(&facts());
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let rule = paragraphs
            .iter()
            .find(|p| p.contains("is not a way of naming an agent"))
            .unwrap_or_else(|| panic!("no rule paragraph in {text}"));
        assert!(!rule.contains("- People:"), "{rule}");
        assert!(!rule.contains("woken only when"), "{rule}");
    }

    #[test]
    fn it_says_how_many_older_messages_were_left_out() {
        let text = render(&Facts { skipped: 37, ..facts() });
        assert!(text.contains("37 older unread messages were left out"));
    }

    #[test]
    fn a_workspace_with_no_people_still_reads_properly() {
        let text = render(&Facts { people: vec![], ..facts() });
        assert!(text.contains("- People: none"));
    }
}
