//! The one place notification wording is written (`specs/029-webui-notifications/
//! contracts/notification-sources.md`). Every sentence is built from stored facts — a task
//! title, a journal count, a job kind and state, an integration name and action, a reply count —
//! never from model output, a secret or an integration setting value.

use crate::schema::{
    EventAction, IntegrationFamily, JobKind, JobState, NotificationTone, TaskRunState,
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

/// A background job that reached a terminal state, or `None` while it is in flight (`running`
/// and `reporting` are never notified).
pub fn background_job(
    kind: JobKind,
    state: JobState,
    reason: Option<&str>,
) -> Option<(String, NotificationTone)> {
    let what = match kind {
        JobKind::Batch => "Background batch",
        JobKind::Delegation => "Delegation",
    };
    let (outcome, tone) = match state {
        JobState::Reported => ("reported back", NotificationTone::Default),
        JobState::Cancelled => ("was cancelled", NotificationTone::Default),
        JobState::Undelivered => (
            "finished, but its report could not be delivered",
            NotificationTone::Failed,
        ),
        JobState::Interrupted => ("was lost when vizier restarted", NotificationTone::Failed),
        JobState::Running | JobState::Reporting => return None,
    };
    let reason = reason
        .filter(|r| !r.is_empty())
        .map(|r| format!(" ({r})"))
        .unwrap_or_default();
    Some((format!("{what} {outcome}{reason}."), tone))
}

/// `Replied once in a Discord conversation.` / `Replied 3 times in a Telegram conversation.`
pub fn channel_replies(channel: &str, replies: u64) -> String {
    let platform = if channel.starts_with("telegram__") {
        "Telegram"
    } else {
        "Discord"
    };
    match replies {
        1 => format!("Replied once in a {platform} conversation."),
        n => format!("Replied {n} times in a {platform} conversation."),
    }
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

pub fn channel_href(agent_id: &str, channel: &str) -> String {
    format!("/{agent_id}/chat?channel={channel}")
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
    fn background_jobs_only_speak_when_they_are_over() {
        assert_eq!(
            background_job(JobKind::Batch, JobState::Reported, None),
            Some((
                "Background batch reported back.".into(),
                NotificationTone::Default
            ))
        );
        assert_eq!(
            background_job(JobKind::Delegation, JobState::Cancelled, Some("user")),
            Some((
                "Delegation was cancelled (user).".into(),
                NotificationTone::Default
            ))
        );
        assert_eq!(
            background_job(JobKind::Delegation, JobState::Undelivered, None),
            Some((
                "Delegation finished, but its report could not be delivered.".into(),
                NotificationTone::Failed
            ))
        );
        assert_eq!(
            background_job(JobKind::Batch, JobState::Interrupted, None),
            Some((
                "Background batch was lost when vizier restarted.".into(),
                NotificationTone::Failed
            ))
        );
        assert_eq!(background_job(JobKind::Batch, JobState::Running, None), None);
        assert_eq!(
            background_job(JobKind::Batch, JobState::Reporting, None),
            None
        );
    }

    #[test]
    fn channel_replies_are_platform_aware_and_plural() {
        assert_eq!(
            channel_replies("discord__1", 1),
            "Replied once in a Discord conversation."
        );
        assert_eq!(
            channel_replies("telegram__5", 3),
            "Replied 3 times in a Telegram conversation."
        );
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
        assert_eq!(channel_href("a", "discord__1"), "/a/chat?channel=discord__1");
        assert_eq!(integrations_href("a", IntegrationFamily::Addon), "/a/integrations/addons");
        assert_eq!(config_href("a"), "/a/config");
    }
}
