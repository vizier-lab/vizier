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
         {by} mentioned you. To bring another agent in, write @<agent-id> in your message: it \
         wakes them, and they see what you said here, not your tool calls. They answer you only \
         if they mention you back. Write a mention only when you want that agent to act.",
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
