//! The conversation picker's list (spec 024, FR-018e): every conversation the person could open
//! with an agent, across the WebUI, Discord and Telegram, newest first, in key-set pages.

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::get,
};
use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};
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
    storage::{
        agent::AgentStorage,
        session::{ConversationRow, SessionStorage},
    },
    usage::{NameBook, slug_place},
};

use super::{user_can_show_platform_chats, user_can_view_agent};

pub fn conversations() -> Router<HTTPState> {
    Router::new().route("/", get(list))
}

const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Webui,
    Discord,
    Telegram,
}

/// Which kind of conversation a channel slug is, and whether the person may open it: a WebUI
/// conversation is only ever its owner's.
fn kind_of(channel: &str, username: &str) -> Option<ConversationKind> {
    if channel.starts_with("discord__") {
        Some(ConversationKind::Discord)
    } else if channel.starts_with("telegram__") {
        Some(ConversationKind::Telegram)
    } else if !username.is_empty() && channel.starts_with(&format!("http__{username}__")) {
        Some(ConversationKind::Webui)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ConversationItem {
    /// The channel slug. A WebUI conversation is the one topic `latest_topic` of its channel.
    pub channel: String,
    pub kind: ConversationKind,
    pub title: String,
    /// Under the title: "topic 3 of 5", or the server a Discord channel is in.
    pub place: String,
    pub topics: u64,
    pub last_activity: DateTime<Utc>,
    /// The WebUI topic to open, or a Discord or Telegram conversation's newest; empty without topics.
    pub latest_topic: String,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct ConversationCounts {
    pub all: usize,
    pub webui: usize,
    pub discord: usize,
    pub telegram: usize,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ConversationPage {
    pub items: Vec<ConversationItem>,
    /// Matches per kind after `q`, ignoring `channel`.
    pub counts: ConversationCounts,
    /// Matches after `q` and `channel`, before paging.
    pub total: usize,
    /// Pass as `cursor` for the next page.
    pub next: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ConversationQuery {
    /// Matches titles, and message text within the agent's newest 5,000 messages.
    pub q: Option<String>,
    pub channel: Option<ConversationKind>,
    /// The `next` of the previous page.
    pub cursor: Option<String>,
    /// 1–100; default 25.
    pub limit: Option<usize>,
}

fn encode_cursor(row: &ConversationRow) -> String {
    let json = serde_json::json!({ "at": row.last_activity.timestamp_millis(), "channel": row.key() });
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string())
}

fn decode_cursor(cursor: &str) -> Option<(DateTime<Utc>, String)> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some((
        Utc.timestamp_millis_opt(value.get("at")?.as_i64()?).single()?,
        value.get("channel")?.as_str()?.to_string(),
    ))
}

/// The rows after `cursor` in `(last_activity, channel)` order, newest first, up to `limit`, and
/// whether there are more.
fn page_of(rows: &[ConversationRow], cursor: Option<&(DateTime<Utc>, String)>, limit: usize) -> (Vec<ConversationRow>, bool) {
    let mut after: Vec<&ConversationRow> = rows
        .iter()
        .filter(|r| cursor.is_none_or(|(at, channel)| (r.last_activity, r.key().as_str()) < (*at, channel.as_str())))
        .collect();
    let more = after.len() > limit;
    after.truncate(limit);
    (after.into_iter().cloned().collect(), more)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/conversations",
    params(("agent_id" = String, Path, description = "Agent ID"), ConversationQuery),
    description = "The agent's conversations on the WebUI (the caller's own), Discord and Telegram, newest first. `q` searches titles and the text of the agent's newest 5,000 messages only, so an older match may not be found.",
    responses(
        (status = 200, description = "One page of conversations", body = APIResponse<ConversationPage>),
        (status = 400, description = "A cursor that isn't one", body = APIResponse<String>),
        (status = 403, description = "The caller can't see this agent"),
        (status = 404, description = "No such agent")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(agent_id): Path<String>,
    Query(query): Query<ConversationQuery>,
) -> models::response::Response<ConversationPage> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) if user_can_view_agent(&user, &config) => config,
        Ok(Some(_)) => return err_response(StatusCode::FORBIDDEN, "Access denied".into()),
        Ok(None) => return err_response(StatusCode::NOT_FOUND, "agent not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let cursor = match query.cursor.as_deref().map(decode_cursor) {
        Some(Some(c)) => Some(c),
        Some(None) => return err_response(StatusCode::BAD_REQUEST, "malformed cursor".into()),
        None => None,
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let rows = match state.storage.conversation_rows(&agent_id, query.q.as_deref()).await {
        Ok(rows) => rows,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let show_discord = user_can_show_platform_chats(&user, &config, "discord");
    let show_telegram = user_can_show_platform_chats(&user, &config, "telegram");
    let rows: Vec<(ConversationKind, ConversationRow)> = rows
        .into_iter()
        .filter_map(|r| Some((kind_of(&r.channel, &user.username)?, r)))
        .filter(|(kind, _)| match kind {
            ConversationKind::Webui => true,
            ConversationKind::Discord => show_discord,
            ConversationKind::Telegram => show_telegram,
        })
        .collect();

    let mut counts = ConversationCounts { all: rows.len(), ..Default::default() };
    for (kind, _) in &rows {
        match kind {
            ConversationKind::Webui => counts.webui += 1,
            ConversationKind::Discord => counts.discord += 1,
            ConversationKind::Telegram => counts.telegram += 1,
        }
    }
    let wanted: Vec<ConversationRow> = rows
        .iter()
        .filter(|(kind, _)| query.channel.is_none_or(|c| c == *kind))
        .map(|(_, r)| r.clone())
        .collect();
    let total = wanted.len();
    let (page, more) = page_of(&wanted, cursor.as_ref(), limit);

    // Names for this page's Discord and Telegram conversations, looked up through the agent's
    // own connections and cached; what can't be found keeps its id.
    let names = NameBook::resolve(
        &state.storage,
        &state.deps.integration_kinds,
        &agent_id,
        page.iter().filter_map(|r| slug_place(&r.channel)).collect::<Vec<_>>().iter().map(String::as_str),
    )
    .await;

    let items = page
        .iter()
        .filter_map(|r| {
            let kind = kind_of(&r.channel, &user.username)?;
            let (title, place) = match kind {
                ConversationKind::Webui => (
                    r.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "Untitled".into()),
                    if r.topics > 1 { format!("topic {} of {}", r.latest_ordinal, r.topics) } else { "1 topic".into() },
                ),
                ConversationKind::Discord | ConversationKind::Telegram => {
                    let id = r.channel.split_once("__").map_or("", |(_, id)| id);
                    let (noun, platform) = if kind == ConversationKind::Discord { ("Discord channel", "Discord") } else { ("Telegram chat", "Telegram") };
                    let found = slug_place(&r.channel).and_then(|p| names.names(&p).cloned());
                    let title = found.as_ref().and_then(|n| n.conversation.clone()).unwrap_or_else(|| format!("{noun} {id}"));
                    let place = found.and_then(|n| n.space).unwrap_or_else(|| platform.to_string());
                    (title, place)
                }
            };
            Some(ConversationItem {
                channel: r.channel.clone(),
                kind,
                title,
                place,
                topics: r.topics,
                last_activity: r.last_activity,
                latest_topic: r.latest_topic.clone(),
            })
        })
        .collect();
    let next = more.then(|| page.last().map(encode_cursor)).flatten();

    api_response(StatusCode::OK, ConversationPage { items, counts, total, next })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(channel: &str, ms: i64) -> ConversationRow {
        ConversationRow {
            channel: channel.into(),
            topics: 1,
            last_activity: Utc.timestamp_millis_opt(ms).single().unwrap(),
            latest_topic: String::new(),
            latest_ordinal: 1,
            title: None,
        }
    }

    #[test]
    fn a_webui_conversation_is_only_its_owners() {
        assert_eq!(kind_of("http__ana__vizier-webui", "ana"), Some(ConversationKind::Webui));
        assert_eq!(kind_of("http__rio__vizier-webui", "ana"), None);
        assert_eq!(kind_of("http__anabel__x", "ana"), None, "a longer username is not a prefix match");
        assert_eq!(kind_of("http__ana__x", ""), None, "an API key has no WebUI conversations");
        assert_eq!(kind_of("discord__1", "ana"), Some(ConversationKind::Discord));
        assert_eq!(kind_of("telegram__-9", "ana"), Some(ConversationKind::Telegram));
        assert_eq!(kind_of("task__x__1", "ana"), None);
    }

    #[test]
    fn pages_continue_after_the_cursor_without_skipping_ties() {
        // Newest first; two share an instant and are ordered by channel, descending.
        let all = vec![row("d__3", 300), row("d__2", 200), row("d__1", 200), row("d__0", 100)];
        let (first, more) = page_of(&all, None, 2);
        assert_eq!(first.iter().map(|r| r.channel.as_str()).collect::<Vec<_>>(), ["d__3", "d__2"]);
        assert!(more);
        let cursor = decode_cursor(&encode_cursor(first.last().unwrap())).unwrap();
        let (second, more) = page_of(&all, Some(&cursor), 2);
        assert_eq!(second.iter().map(|r| r.channel.as_str()).collect::<Vec<_>>(), ["d__1", "d__0"]);
        assert!(!more);
    }

    #[test]
    fn a_bad_cursor_is_none() {
        assert!(decode_cursor("???").is_none());
    }
}
