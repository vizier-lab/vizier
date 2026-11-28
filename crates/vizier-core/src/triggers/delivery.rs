//! Who gets a trigger's results and notices, and how (`specs/071-triggers` FR-046, FR-047, FR-050).

use crate::{
    access::Caller,
    dependencies::VizierDependencies,
    schema::{NotificationTone, Trigger, TriggerRun},
    storage::{agent::AgentStorage, trigger::TriggerStorage, user::UserStorage},
};

use super::{access, run_session};

/// A destination's conversation: one of the recipient's own WebUI conversations, by topic id
/// (FR-046). `None` when the recipient or the conversation is gone, or the reference is not one.
async fn destination_session(
    deps: &VizierDependencies,
    trigger: &Trigger,
    recipient: &str,
) -> Option<crate::schema::VizierSession> {
    use crate::storage::session::SessionStorage;
    let topic = trigger.destination.as_deref()?.strip_prefix("webui:")?.to_string();
    let user = deps.storage.get_user_by_id(recipient).await.ok().flatten()?;
    let session = crate::schema::VizierSession(
        trigger.agent_id.clone(),
        crate::schema::VizierChannelId::HTTP(user.username, crate::agents::tools::webui_channel().to_string()),
        Some(topic),
    );
    deps.storage
        .get_session_detail_by_topic(trigger.agent_id.clone(), session.1.clone(), session.2.clone())
        .await
        .ok()
        .flatten()?;
    Some(session)
}

/// The person results go to: the one who set the trigger up while they may still view it, else
/// the agent's owner. `None` when there is nobody, in which case nothing is delivered, since a row
/// without an audience would show to everyone who can see the agent.
pub async fn recipient(deps: &VizierDependencies, trigger: &Trigger) -> Option<String> {
    let agent = deps.storage.get_agent(&trigger.agent_id).await.ok().flatten()?;
    if let Some(user_id) = &trigger.delivers_to
        && let Ok(Some(user)) = deps.storage.get_user_by_id(user_id).await
        && let Ok(Some(role)) = deps.storage.get_role(&user.role_id).await
    {
        let caller = Caller { user_id, is_system: role.is_system, permissions: &role.permissions };
        if access::can_view(caller, &agent) {
            return Some(user_id.clone());
        }
    }
    agent.owner_id.clone()
}

/// Deliver a run's reply, at most once (FR-046).
pub(super) async fn deliver(deps: &VizierDependencies, trigger: &Trigger, run: &TriggerRun, reply: &str) {
    match deps.storage.claim_delivery(run.id, crate::time::now()).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            tracing::warn!(run = run.id, "could not claim a trigger delivery: {e:#}");
            return;
        }
    }
    let Some(audience) = recipient(deps, trigger).await else {
        tracing::warn!(trigger = %trigger.slug, "a trigger result has nobody to go to; it stays in its run");
        return;
    };
    // The conversation copy, when the trigger names one; a failed send is reported once, and the
    // inbox copy below is made either way.
    if trigger.destination.is_some() {
        let posted = match destination_session(deps, trigger, &audience).await {
            Some(session) => {
                let response = crate::schema::VizierResponse {
                    timestamp: crate::time::now(),
                    content: crate::schema::VizierResponseContent::Message {
                        content: format!("**Trigger \u{201c}{}\u{201d}**\n\n{reply}", trigger.title),
                        stats: None,
                    },
                    ..Default::default()
                };
                crate::agents::steps::record_step(
                    &deps.storage,
                    &deps.transport,
                    &session,
                    crate::schema::SessionHistoryContent::Response(response),
                )
                .await
                .is_some()
            }
            None => false,
        };
        if !posted {
            notice(
                deps,
                trigger,
                "destination_failed",
                format!("Trigger \u{201c}{}\u{201d}: its conversation could not be reached", trigger.title),
                Some("The result is in your inbox and in the run.".into()),
                NotificationTone::Failed,
            )
            .await;
        }
    }
    deps.notifications
        .trigger_result(
            &trigger.agent_id,
            Some(audience),
            &trigger.slug,
            &trigger.title,
            run.id,
            run_session(run),
            reply,
            crate::time::now(),
        )
        .await;
}

/// A notice about the trigger itself, at most once per hour for each cause (FR-047).
pub(super) async fn notice(
    deps: &VizierDependencies,
    trigger: &Trigger,
    cause: &str,
    summary: String,
    detail: Option<String>,
    tone: NotificationTone,
) {
    let Some(audience) = recipient(deps, trigger).await else {
        return;
    };
    let now = crate::time::now();
    let hour = now.timestamp() / 3600;
    deps.notifications
        .trigger_notice(
            &trigger.agent_id,
            Some(audience),
            format!("trigger_notice:{}:{cause}:{hour}", trigger.id),
            &trigger.slug,
            &trigger.title,
            summary,
            detail,
            tone,
            now,
        )
        .await;
}

/// Send each trigger's summary when its window has passed (FR-050). Built without a model.
pub(super) async fn send_due_summaries(deps: &VizierDependencies) {
    let triggers = match deps.storage.list_triggers(None).await {
        Ok(triggers) => triggers,
        Err(e) => {
            tracing::warn!("could not list triggers for their summaries: {e}");
            return;
        }
    };
    let now = crate::time::now();
    for trigger in triggers {
        let Some(hours) = trigger.summary_hours.filter(|h| *h > 0) else {
            continue;
        };
        let since = trigger.last_summary_at.unwrap_or(trigger.created_at);
        if now - since < chrono::Duration::hours(hours as i64) {
            continue;
        }
        let items = match deps.storage.summary_items(&trigger.id).await {
            Ok(items) => items,
            Err(e) => {
                tracing::warn!(trigger = %trigger.slug, "could not read a summary: {e}");
                continue;
            }
        };
        if !items.unimportant.is_empty() || !items.quiet_runs.is_empty() {
            let detail = summary_text(&items);
            let key = format!("trigger_summary:{}:{}", trigger.id, now.timestamp() / 60);
            if let Some(audience) = recipient(deps, &trigger).await {
                deps.notifications
                    .trigger_notice(
                        &trigger.agent_id,
                        Some(audience),
                        key,
                        &trigger.slug,
                        &trigger.title,
                        crate::notifications::text::trigger_summary(
                            &trigger.title,
                            items.unimportant.iter().map(|e| e.count.max(1) as usize).sum(),
                            items.quiet_runs.len(),
                        ),
                        Some(detail),
                        NotificationTone::Default,
                        now,
                    )
                    .await;
            }
            let events: Vec<i64> = items.unimportant.iter().map(|e| e.id).collect();
            let runs: Vec<i64> = items.quiet_runs.iter().map(|r| r.id).collect();
            if let Err(e) = deps.storage.mark_summarised(&trigger.id, &events, &runs, now).await {
                tracing::warn!(trigger = %trigger.slug, "could not mark a summary sent: {e}");
            }
        }
        if let Err(e) = deps.storage.note_trigger_summarised(&trigger.id, now).await {
            tracing::warn!(trigger = %trigger.slug, "could not record a summary time: {e}");
        }
    }
}

/// The summary's lines: the quiet runs' notes, then the unimportant events' reasons, at most 50.
pub(super) fn summary_text(items: &crate::storage::trigger::SummaryItems) -> String {
    const MAX_LINES: usize = 50;
    let mut lines: Vec<String> = items
        .quiet_runs
        .iter()
        .filter_map(|run| run.quiet_note.as_ref().map(|note| format!("Nothing to report: {note}")))
        .chain(items.unimportant.iter().map(|event| {
            format!("Not important: {}", event.reason.as_deref().unwrap_or("no reason given"))
        }))
        .collect();
    let extra = lines.len().saturating_sub(MAX_LINES);
    lines.truncate(MAX_LINES);
    if extra > 0 {
        lines.push(format!("and {extra} more"));
    }
    lines.join("\n")
}
