//! The one place notification wording is written (`specs/029-webui-notifications/
//! contracts/notification-sources.md`). Every sentence is built from stored facts — a task
//! title, a journal count, a job kind and state, an integration name and action, a reply count —
//! never from model output, a secret or an integration setting value.

use crate::schema::{
    EventAction, IntegrationFamily, NotificationTone, TaskRunState,
};

/// `{who} added {what}.` and friends, as the old feed worded it.
pub fn changed(action: EventAction) -> &'static str {
    match action {
        EventAction::Created => "added",
        EventAction::Edited => "changed",
        EventAction::Enabled => "switched on",
        EventAction::Disabled => "switched off",
        EventAction::Deleted => "deleted",
        EventAction::Unlinked => "unlinked",
        EventAction::GrantOn => "let the agent manage its own",
        EventAction::GrantOff => "stopped the agent managing its own",
    }
}

/// The integrations page a family lives on.
pub fn integrations_page(family: IntegrationFamily) -> &'static str {
    match family {
        IntegrationFamily::Channel => "channels",
        IntegrationFamily::Addon => "addons",
        IntegrationFamily::Mcp => "mcp",
    }
}

pub fn integration_changed(actor: &str, action: EventAction, name: &str) -> String {
    let what = match action {
        EventAction::GrantOn | EventAction::GrantOff => format!("{name} integrations"),
        _ => name.to_string(),
    };
    format!("{actor} {} {what}.", changed(action))
}

/// A self-management grant switched on or off: `{who} let the agent manage its own mcp
/// integrations.`
pub fn integration_grant(actor: &str, family: IntegrationFamily, on: bool) -> String {
    let verb = if on {
        "let the agent manage its own"
    } else {
        "stopped the agent managing its own"
    };
    format!("{actor} {verb} {} integrations.", family.as_str())
}

/// A task run that ended, or `None` while it is still running.
pub fn task_run(title: &str, state: TaskRunState) -> Option<(String, NotificationTone)> {
    let (what, tone) = match state {
        TaskRunState::Answered => ("answered", NotificationTone::Default),
        TaskRunState::NoResponse => ("ended without an answer", NotificationTone::Failed),
        TaskRunState::Interrupted => ("was interrupted", NotificationTone::Failed),
        TaskRunState::Running => return None,
    };
    Some((format!("Task “{title}” {what}."), tone))
}

pub fn dream(entries: u64) -> String {
    match entries {
        1 => "Dreamed: 1 journal entry.".to_string(),
        n => format!("Dreamed: {n} journal entries."),
    }
}

/// A detached script reached a final state (spec 058). A cancelled one says nothing: the agent
/// cancelled it and is told so in its own turn.
pub fn background_work(
    intent: &str,
    state: crate::schema::DetachedScriptState,
) -> Option<(String, NotificationTone)> {
    use crate::schema::DetachedScriptState as S;
    let (outcome, tone) = match state {
        S::Succeeded => ("finished", NotificationTone::Success),
        S::Failed => ("failed", NotificationTone::Failed),
        S::TimedOut => ("ran out of time", NotificationTone::Failed),
        S::Interrupted => ("was interrupted by a restart", NotificationTone::Failed),
        S::Running | S::Cancelled => return None,
    };
    let intent = intent.trim().trim_end_matches('.');
    let what = if intent.is_empty() {
        "Background work".to_string()
    } else {
        let mut chars = intent.chars();
        let first: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        format!("{first}{}", chars.as_str())
    };
    Some((format!("{what}: {outcome}."), tone))
}

/// A WebUI conversation's reply, for the person whose conversation it is.
pub fn chat_replied() -> &'static str {
    "Replied in your conversation."
}

pub fn agent_ready(name: &str) -> String {
    format!("{name} is ready.")
}

pub fn agent_start_failed(error: &str, retry_secs: Option<i64>) -> String {
    let retry = retry_secs
        .map(|s| format!(" Retrying in {}s.", s.max(0)))
        .unwrap_or_default();
    let error = error.trim_end_matches('.');
    format!("Failed to start: {error}.{retry}")
}

pub fn task_href(agent_id: &str, slug: &str, run_id: i64) -> String {
    format!("/{agent_id}/tasks?task={slug}&run={run_id}")
}

pub fn dream_href(agent_id: &str, cycle_id: &str) -> String {
    format!("/{agent_id}/dream?cycle={cycle_id}")
}

pub fn chat_href(agent_id: &str) -> String {
    format!("/{agent_id}/chat")
}

pub fn config_href(agent_id: &str) -> String {
    format!("/{agent_id}/config")
}

pub fn integrations_href(agent_id: &str, family: IntegrationFamily) -> String {
    format!("/{agent_id}/integrations/{}", integrations_page(family))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_runs_say_how_they_ended_and_only_running_is_silent() {
        assert_eq!(
            task_run("Morning digest", TaskRunState::Answered),
            Some((
                "Task “Morning digest” answered.".into(),
                NotificationTone::Default
            ))
        );
        assert_eq!(
            task_run("Morning digest", TaskRunState::NoResponse),
            Some((
                "Task “Morning digest” ended without an answer.".into(),
                NotificationTone::Failed
            ))
        );
        assert_eq!(
            task_run("Morning digest", TaskRunState::Interrupted),
            Some((
                "Task “Morning digest” was interrupted.".into(),
                NotificationTone::Failed
            ))
        );
        assert_eq!(task_run("Morning digest", TaskRunState::Running), None);
    }

    #[test]
    fn dreams_are_singular_or_plural() {
        assert_eq!(dream(1), "Dreamed: 1 journal entry.");
        assert_eq!(dream(4), "Dreamed: 4 journal entries.");
    }

    #[test]
    fn background_work_speaks_once_it_is_over_and_not_when_cancelled() {
        use crate::schema::DetachedScriptState as S;
        assert_eq!(
            background_work("check the vendor pages.", S::Succeeded),
            Some(("Check the vendor pages: finished.".into(), NotificationTone::Success))
        );
        assert_eq!(background_work("", S::Failed).unwrap().0, "Background work: failed.");
        assert!(background_work("x", S::Cancelled).is_none());
        assert!(background_work("x", S::Running).is_none());
    }

    #[test]
    fn integration_wording_reads_as_a_sentence() {
        assert_eq!(
            integration_changed("ana", EventAction::Created, "Work Discord"),
            "ana added Work Discord."
        );
        assert_eq!(
            integration_changed("scribe", EventAction::GrantOn, "mcp"),
            "scribe let the agent manage its own mcp integrations."
        );
        assert_eq!(
            integration_grant("ana", IntegrationFamily::Mcp, false),
            "ana stopped the agent managing its own mcp integrations."
        );
    }

    #[test]
    fn start_failure_keeps_the_error_and_optional_retry() {
        assert_eq!(
            agent_start_failed("bad key", Some(2)),
            "Failed to start: bad key. Retrying in 2s."
        );
        assert_eq!(
            agent_start_failed("bad key.", None),
            "Failed to start: bad key."
        );
    }

    #[test]
    fn hrefs_point_at_their_destinations() {
        assert_eq!(task_href("a", "brief", 812), "/a/tasks?task=brief&run=812");
        assert_eq!(dream_href("a", "c1"), "/a/dream?cycle=c1");
        assert_eq!(integrations_href("a", IntegrationFamily::Addon), "/a/integrations/addons");
        assert_eq!(config_href("a"), "/a/config");
    }

    /// A reply notifies with a fixed sentence, built from nothing the turn handled: a notification
    /// has no route by which a supplied value could reach it (US5 #4).
    #[test]
    fn a_chat_reply_notification_carries_no_content() {
        assert_eq!(chat_replied(), "Replied in your conversation.");
    }
}
