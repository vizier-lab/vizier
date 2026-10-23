//! "Since you were away": what happened to the person's agents since their previous visit
//! (spec 024, FR-018a). Each source reads its own table through [`FeedStorage`]; a [`FeedSource`]
//! turns a source's rows into events whose text comes from stored fields alone. The sources are
//! merged newest first and paged with a key-set cursor on `(time, id)`.

use std::collections::HashMap;

use axum::{
    Extension,
    extract::{Query, State},
};
use base64::Engine;
use chrono::{DateTime, Duration, TimeZone, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        auth::AuthenticatedUser,
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{AgentRunState, EventAction, IntegrationEvent, IntegrationFamily, JobKind, JobState, Requester, TaskRunState},
    storage::{
        feed::{FeedStorage, FeedWindow},
        task::TaskStorage,
        user::UserStorage,
        VizierStorage,
    },
};

use super::visible_agents;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;
/// With no previous visit on record, look back this far.
const FALLBACK_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct FeedEvent {
    /// Stable across requests: `task_run:812`, `health:scribe`.
    pub id: String,
    pub at: DateTime<Utc>,
    pub agent_id: String,
    /// `start_failed`, `task_run`, `dream_cycle`, `background_job`, `integration`, `channel_replies`.
    pub kind: &'static str,
    /// `failed` or `default`.
    pub tone: &'static str,
    pub text: String,
    /// Where the WebUI takes the person to see more.
    pub href: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Feed {
    /// Where the feed starts: the previous visit, or 24 hours ago.
    pub since: DateTime<Utc>,
    pub events: Vec<FeedEvent>,
    /// Pass as `before` for the next page; absent on the last one.
    pub next: Option<String>,
}

/// What a source needs to word its events.
pub struct FeedContext<'a> {
    pub storage: &'a VizierStorage,
    pub agent_names: HashMap<String, String>,
    pub usernames: HashMap<String, String>,
}

impl FeedContext<'_> {
    fn agent(&self, id: &str) -> String {
        self.agent_names.get(id).cloned().unwrap_or_else(|| id.to_string())
    }
}

#[async_trait::async_trait]
pub trait FeedSource: Send + Sync {
    /// Up to `w.limit` events newest first, all before `w.before` and after `w.since`.
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent>;
}

fn event(id: String, at: DateTime<Utc>, agent_id: &str, kind: &'static str, failed: bool, text: String, href: String) -> FeedEvent {
    FeedEvent { id, at, agent_id: agent_id.to_string(), kind, tone: if failed { "failed" } else { "default" }, text, href }
}

struct TaskRuns;

#[async_trait::async_trait]
impl FeedSource for TaskRuns {
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent> {
        let mut out = vec![];
        for run in ctx.storage.feed_task_runs(w).await.unwrap_or_default() {
            let title = ctx
                .storage
                .get_task(run.agent_id.clone(), run.task_slug.clone())
                .await
                .ok()
                .flatten()
                .map_or_else(|| run.task_slug.clone(), |t| t.title);
            let (failed, what) = match run.state {
                TaskRunState::Answered => (false, "answered"),
                TaskRunState::NoResponse => (true, "ended without an answer"),
                TaskRunState::Interrupted => (true, "was interrupted"),
                TaskRunState::Running => continue,
            };
            out.push(event(
                format!("task_run:{}", run.id),
                run.at,
                &run.agent_id,
                "task_run",
                failed,
                format!("Task “{title}” {what}."),
                format!("/{}/tasks?task={}&run={}", run.agent_id, run.task_slug, run.id),
            ));
        }
        out
    }
}

struct DreamCycles;

#[async_trait::async_trait]
impl FeedSource for DreamCycles {
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent> {
        ctx.storage
            .feed_dream_cycles(w)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|c| {
                let text = match c.entries {
                    1 => "Dreamed: 1 journal entry.".to_string(),
                    n => format!("Dreamed: {n} journal entries."),
                };
                event(format!("dream:{}", c.cycle_id), c.at, &c.agent_id, "dream_cycle", false, text, format!("/{}/dream?cycle={}", c.agent_id, c.cycle_id))
            })
            .collect()
    }
}

struct BackgroundJobs;

#[async_trait::async_trait]
impl FeedSource for BackgroundJobs {
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent> {
        ctx.storage
            .feed_background_jobs(w)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|j| {
                let what = match j.kind {
                    JobKind::Batch => "Background batch",
                    JobKind::Delegation => "Delegation",
                };
                let (failed, outcome) = match j.state {
                    JobState::Reported => (false, "reported back".to_string()),
                    JobState::Cancelled => (false, "was cancelled".to_string()),
                    JobState::Undelivered => (true, "finished, but its report could not be delivered".to_string()),
                    JobState::Interrupted => (true, "was lost when vizier restarted".to_string()),
                    JobState::Running | JobState::Reporting => (false, "is still running".to_string()),
                };
                let reason = j.reason.filter(|r| !r.is_empty()).map(|r| format!(" ({r})")).unwrap_or_default();
                event(format!("job:{}", j.id), j.at, &j.agent_id, "background_job", failed, format!("{what} {outcome}{reason}."), format!("/{}/chat", j.agent_id))
            })
            .collect()
    }
}

struct IntegrationChanges;

fn changed(action: EventAction) -> &'static str {
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

fn integrations_page(family: IntegrationFamily) -> &'static str {
    match family {
        IntegrationFamily::Channel => "channels",
        IntegrationFamily::Addon => "addons",
        IntegrationFamily::Mcp => "mcp",
    }
}

#[async_trait::async_trait]
impl FeedSource for IntegrationChanges {
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent> {
        ctx.storage
            .feed_integration_events(w)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|e: IntegrationEvent| {
                let who = match &e.actor {
                    Requester::User(id) => ctx.usernames.get(id).cloned().unwrap_or_else(|| id.clone()),
                    Requester::Agent(id) => ctx.agent(id),
                };
                let what = match e.action {
                    EventAction::GrantOn | EventAction::GrantOff => format!("{} integrations", e.integration_name),
                    _ => e.integration_name.clone(),
                };
                event(
                    format!("integration:{}", e.id),
                    e.at,
                    &e.agent_id,
                    "integration",
                    false,
                    format!("{who} {} {what}.", changed(e.action)),
                    format!("/{}/integrations/{}", e.agent_id, integrations_page(e.family)),
                )
            })
            .collect()
    }
}

struct ChannelReplies;

#[async_trait::async_trait]
impl FeedSource for ChannelReplies {
    async fn fetch(&self, ctx: &FeedContext<'_>, w: &FeedWindow) -> Vec<FeedEvent> {
        ctx.storage
            .feed_channel_replies(w)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let platform = if r.channel.starts_with("telegram__") { "Telegram" } else { "Discord" };
                let text = match r.replies {
                    1 => format!("Replied once in a {platform} conversation."),
                    n => format!("Replied {n} times in a {platform} conversation."),
                };
                event(
                    format!("reply:{}:{}", r.channel, r.hour),
                    r.at,
                    &r.agent_id,
                    "channel_replies",
                    false,
                    text,
                    format!("/{}/chat?channel={}", r.agent_id, r.channel),
                )
            })
            .collect()
    }
}

/// Newest first; equal times by id, the order the sources' own cursors use.
pub fn merge(batches: Vec<Vec<FeedEvent>>, limit: usize) -> Vec<FeedEvent> {
    let mut all: Vec<FeedEvent> = batches.into_iter().flatten().collect();
    all.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
    all.truncate(limit);
    all
}

/// The cursor after `last`: opaque to the client.
pub fn encode_cursor(last: &FeedEvent) -> String {
    let json = serde_json::json!({ "at": last.at.timestamp_millis(), "id": last.id });
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string())
}

pub fn decode_cursor(cursor: &str) -> Option<(DateTime<Utc>, String)> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let at = Utc.timestamp_millis_opt(value.get("at")?.as_i64()?).single()?;
    Some((at, value.get("id")?.as_str()?.to_string()))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct FeedQuery {
    /// The `next` of the previous page.
    pub before: Option<String>,
    /// 1–50; default 20.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/overview/feed",
    params(FeedQuery),
    responses(
        (status = 200, description = "What happened since the caller's previous visit (else the last 24 hours), newest first, with start failures pinned to the top of the first page", body = APIResponse<Feed>),
        (status = 400, description = "A cursor that isn't one", body = APIResponse<String>)
    )
)]
pub async fn feed(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(query): Query<FeedQuery>,
) -> models::response::Response<Feed> {
    let agents = match visible_agents(&state, &user).await {
        Ok(a) => a,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let before = match query.before.as_deref().map(decode_cursor) {
        Some(Some(cursor)) => Some(cursor),
        Some(None) => return err_response(StatusCode::BAD_REQUEST, "malformed cursor".into()),
        None => None,
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let now = Utc::now();
    let visit = state.storage.get_visit(&user.user_id).await.unwrap_or_default();
    let since = visit.previous_visit_at.unwrap_or(now - Duration::hours(FALLBACK_HOURS));

    let ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();
    let ctx = FeedContext {
        storage: &state.storage,
        agent_names: agents.iter().map(|(id, c)| (id.clone(), c.name.clone())).collect(),
        usernames: state
            .storage
            .list_users()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|u| (u.user_id, u.username))
            .collect(),
    };
    // One more than a page, to know whether there is another.
    let window = FeedWindow { agents: ids, since, before: before.clone(), limit: limit + 1 };
    let sources: Vec<Box<dyn FeedSource>> = vec![
        Box::new(TaskRuns),
        Box::new(DreamCycles),
        Box::new(BackgroundJobs),
        Box::new(IntegrationChanges),
        Box::new(ChannelReplies),
    ];
    let mut batches = vec![];
    for source in &sources {
        batches.push(source.fetch(&ctx, &window).await);
    }
    let mut events = merge(batches, limit + 1);
    let next = (events.len() > limit).then(|| {
        events.truncate(limit);
        encode_cursor(&events[limit - 1])
    });

    // Agents that can't start are news until they can, however long ago it began: pinned to
    // the first page, never counted against paging.
    if before.is_none() {
        let mut pinned: Vec<FeedEvent> = state
            .transport
            .agent_health()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|h| h.state == AgentRunState::Failed && ctx.agent_names.contains_key(&h.agent_id))
            .map(|h| {
                let retry = h
                    .next_retry_at
                    .map(|t| format!(" Retrying in {}s.", (t - now).num_seconds().max(0)))
                    .unwrap_or_default();
                let error = h.last_error.unwrap_or_else(|| "unknown error".into());
                event(format!("health:{}", h.agent_id), now, &h.agent_id, "start_failed", true, format!("Failed to start: {error}.{retry}"), format!("/{}/config", h.agent_id))
            })
            .collect();
        pinned.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        pinned.extend(events);
        events = pinned;
    }

    api_response(StatusCode::OK, Feed { since, events, next })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str, minute: i64) -> FeedEvent {
        let at = Utc.timestamp_millis_opt(1_700_000_000_000 + minute * 60_000).single().unwrap();
        event(id.into(), at, "a", "task_run", false, id.into(), "/".into())
    }

    #[test]
    fn sources_merge_newest_first_with_ids_breaking_ties() {
        let merged = merge(vec![vec![ev("task_run:2", 5), ev("task_run:1", 1)], vec![ev("job:x", 5), ev("dream:c", 3)]], 10);
        let ids: Vec<&str> = merged.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["task_run:2", "job:x", "dream:c", "task_run:1"]);
    }

    #[test]
    fn merging_keeps_only_a_page() {
        let merged = merge(vec![vec![ev("a:3", 3), ev("a:2", 2)], vec![ev("b:1", 1)]], 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "a:2");
    }

    #[test]
    fn a_cursor_round_trips_and_a_bad_one_is_refused() {
        let last = ev("task_run:812", 4);
        let (at, id) = decode_cursor(&encode_cursor(&last)).expect("round trip");
        assert_eq!((at, id.as_str()), (last.at, "task_run:812"));
        assert!(decode_cursor("not a cursor").is_none());
        assert!(decode_cursor(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("{\"at\":1}")).is_none());
    }

    #[test]
    fn the_next_page_starts_after_the_cursor_even_with_tied_times() {
        // The sources compare `(at, id) < cursor`: the same rule applied to the merged order.
        let all = merge(vec![vec![ev("a:2", 5), ev("a:1", 5), ev("a:0", 5)]], 10);
        let page = &all[..2];
        let (at, id) = decode_cursor(&encode_cursor(&page[1])).unwrap();
        let rest: Vec<&str> = all.iter().filter(|e| (e.at, e.id.as_str()) < (at, id.as_str())).map(|e| e.id.as_str()).collect();
        assert_eq!(rest, ["a:0"]);
    }
}
