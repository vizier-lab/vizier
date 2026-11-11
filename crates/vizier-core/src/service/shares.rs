//! Shared chat snapshots (`specs/055-shared-chat`): create and read. The WebUI builds and cleans
//! the snapshot; this layer only checks what is cheap and structural (it parses, the version,
//! the known top-level keys, sizes, the trail flag, the audience shape), authorizes, and stores
//! the received text unchanged. It never rewrites a snapshot, and it never logs content, titles
//! or people (ids only).

use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use super::{ServiceError, Services};
use crate::{
    access::{self, Caller},
    auth::Principal,
    storage::{
        agent::AgentStorage,
        share::{
            Access, Audience, CachePolicy, InsertOutcome, ShareContent, ShareRecord, ShareStorage,
        },
        user::UserStorage,
    },
};

// ---- limits (research R12). The WebUI twin is `webui/app/lib/share/limits.ts`. ----

/// The whole request body, and the stored document.
pub const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;
/// One media item after re-encoding (enforced by the WebUI).
pub const MAX_MEDIA_BYTES: usize = 1024 * 1024;
pub const MAX_MEDIA_ITEMS: usize = 100;
pub const MAX_TITLE_CHARS: usize = 120;
pub const MAX_PEOPLE: usize = 50;
pub const MAX_TURNS: usize = 5_000;
pub const MAX_PARTS: usize = 64;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// Creates per person per hour (the limiter lives on `HTTPState`).
pub const CREATES_PER_HOUR: usize = 10;
/// Unknown or refused reads per address per minute.
pub const MISSES_PER_MINUTE: usize = 30;
/// Active shares per person.
pub const MAX_ACTIVE_PER_PERSON: usize = 200;
/// The only snapshot format this build reads and writes.
pub const FORMAT_VERSION: i64 = 1;

pub const PERMISSION_ALL: &str = "all_agents:share_conversations";
pub const PERMISSION_OWNED: &str = "owned_agents:share_conversations";

/// What `GET /shares/{id}` answers to a caller who has no session: the same for an id that does
/// not exist, one that was revoked, and one that is not public.
pub const SIGN_IN_REQUIRED: &str = "sign_in_required";
/// What a signed-in caller who may not open a share is told, for every reason.
pub const SHARE_UNAVAILABLE: &str = "share_unavailable";

/// The keys a snapshot document may have at its top level.
const KNOWN_KEYS: [&str; 9] = [
    "format_version",
    "generator",
    "created_at",
    "title",
    "source",
    "include_trail",
    "turns",
    "media",
    "rehydration",
];

/// A new share id: 21 URL-safe characters, about 126 bits.
pub fn new_share_id() -> String {
    nanoid::nanoid!(21)
}

/// `^[A-Za-z0-9_-]{21}$`, checked before any query.
pub fn is_valid_share_id(id: &str) -> bool {
    id.len() == 21 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Body of `POST /shares`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    agent_id: String,
    topic_id: String,
    title: String,
    audience: Audience,
    #[serde(default)]
    people: Option<Vec<String>>,
    #[serde(default)]
    include_trail: bool,
    #[serde(default)]
    removals: Option<Value>,
    /// Kept as the received text: it is stored byte for byte.
    snapshot: Box<RawValue>,
}

/// Documentation twin of the `POST /shares` body (the service reads the raw bytes itself).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateShareRequest {
    pub agent_id: String,
    pub topic_id: String,
    /// At most 120 characters.
    pub title: String,
    pub audience: Audience,
    /// Required (1 to 50 existing people) when the audience is `specific`; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub people: Option<Vec<String>>,
    #[serde(default)]
    pub include_trail: bool,
    /// Counts by kind of what was left out (`credentials`, `paths`, `recall`, `reasoning`, `media`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub removals: Option<Value>,
    /// The document of `contracts/snapshot-format.md`, format version 1.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub snapshot: Value,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreatedShare {
    pub share_id: String,
    /// `/share/<id>`, relative to the instance's address.
    pub url_path: String,
    /// Unix milliseconds.
    pub created_at: i64,
}

/// A share as `GET /shares/{id}` returns it. No field depends on who asks.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ShareView {
    pub share_id: String,
    pub title: String,
    pub created_at: i64,
    pub audience: Audience,
    pub include_trail: bool,
    pub format_version: i64,
    /// The stored document, as stored.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub snapshot: Box<RawValue>,
}

/// A readable share and how its response may be cached.
#[derive(Debug, Clone)]
pub struct ShareRead {
    pub view: ShareView,
    pub cache: CachePolicy,
}

fn storage_error(message: &str, e: impl std::fmt::Display) -> ServiceError {
    tracing::error!("{message}: {e}");
    ServiceError::Unavailable(message.to_string())
}

fn invalid(message: impl Into<String>) -> ServiceError {
    ServiceError::invalid(message)
}

fn forbidden() -> ServiceError {
    ServiceError::Forbidden("Forbidden: insufficient permissions".into())
}

/// The document's own checks, with no change to it.
fn check_snapshot(text: &str, include_trail: bool) -> Result<(), ServiceError> {
    let doc: Value = serde_json::from_str(text).map_err(|_| invalid("The snapshot is not valid JSON"))?;
    let Value::Object(doc) = doc else {
        return Err(invalid("The snapshot must be an object"));
    };
    if doc.keys().any(|k| !KNOWN_KEYS.contains(&k.as_str())) {
        return Err(invalid("The snapshot has an unknown top-level key"));
    }
    if doc.get("format_version").and_then(Value::as_i64) != Some(FORMAT_VERSION) {
        return Err(invalid("Unsupported snapshot format_version"));
    }
    if doc.get("rehydration").is_some_and(|v| !v.is_null()) {
        return Err(invalid("rehydration must be null in format version 1"));
    }
    if let Some(title) = doc.get("title") {
        match title.as_str() {
            Some(t) if t.chars().count() <= MAX_TITLE_CHARS => {}
            _ => return Err(invalid("The snapshot title is not a string of at most 120 characters")),
        }
    }
    let turns = doc
        .get("turns")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("The snapshot has no turns"))?;
    if turns.is_empty() {
        return Err(invalid("The snapshot has no turns"));
    }
    if turns.len() > MAX_TURNS {
        return Err(ServiceError::TooLarge("The snapshot has too many turns".into()));
    }
    let mut trail_present = doc.get("include_trail").and_then(Value::as_bool).unwrap_or(false);
    for turn in turns {
        let Some(turn) = turn.as_object() else {
            return Err(invalid("A turn must be an object"));
        };
        if let Some(parts) = turn.get("parts") {
            let parts = parts.as_array().ok_or_else(|| invalid("parts must be a list"))?;
            if parts.len() > MAX_PARTS {
                return Err(ServiceError::TooLarge("A turn has too many parts".into()));
            }
            let too_long = parts.iter().any(|p| {
                p.get("markdown").and_then(Value::as_str).is_some_and(|m| m.len() > MAX_TEXT_BYTES)
            });
            if too_long {
                return Err(ServiceError::TooLarge("A text part is too long".into()));
            }
        }
        if turn.get("trail").and_then(Value::as_array).is_some_and(|t| !t.is_empty()) {
            trail_present = true;
        }
    }
    if let Some(media) = doc.get("media") {
        let media = media.as_object().ok_or_else(|| invalid("media must be an object"))?;
        if media.len() > MAX_MEDIA_ITEMS {
            return Err(ServiceError::TooLarge("The snapshot has too many media items".into()));
        }
    }
    // Only the flag turned off is a contradiction: the document would carry what the person
    // was told would stay out. The document is not edited to fix it.
    if trail_present && !include_trail {
        return Err(ServiceError::Invalid {
            status: 422,
            message: "The snapshot holds trail parts but include_trail is false".into(),
            code: Some("trail_mismatch".into()),
        });
    }
    Ok(())
}

/// Users that must exist and be named once each.
async fn check_people(
    s: &Services,
    audience: Audience,
    people: &Option<Vec<String>>,
) -> Result<Vec<String>, ServiceError> {
    // The audience rule, as data: which audiences take a list. No audience is branched on here.
    let takes_people = Audience::Specific == audience;
    match (takes_people, people) {
        (false, None) => Ok(vec![]),
        (false, Some(_)) => Err(invalid("people is only allowed for specific people")),
        (true, None) => Err(invalid("people is required for specific people")),
        (true, Some(list)) => {
            if list.is_empty() || list.len() > MAX_PEOPLE {
                return Err(invalid("people must name 1 to 50 people"));
            }
            let mut seen = std::collections::BTreeSet::new();
            for id in list {
                if !seen.insert(id.as_str()) {
                    return Err(invalid("people names someone twice"));
                }
                match s.deps().storage.get_user_by_id(id).await {
                    Ok(Some(_)) => {}
                    Ok(None) => return Err(invalid("people names someone who does not exist")),
                    Err(e) => return Err(storage_error("Failed to check people", e)),
                }
            }
            Ok(list.clone())
        }
    }
}

/// Creates a share from the raw request body.
pub async fn create(s: &Services, who: &Principal, body: &[u8]) -> Result<CreatedShare, ServiceError> {
    create_with(s, who, body, &mut new_share_id, MAX_ACTIVE_PER_PERSON).await
}

async fn create_with(
    s: &Services,
    who: &Principal,
    body: &[u8],
    next_id: &mut (dyn FnMut() -> String + Send),
    max_active: usize,
) -> Result<CreatedShare, ServiceError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(ServiceError::TooLarge("The snapshot is too large to share".into()));
    }
    let req: CreateBody = serde_json::from_slice(body).map_err(|e| {
        tracing::debug!("share create: body rejected ({:?})", e.classify());
        invalid("Invalid request body")
    })?;

    // Permission first: system, the all tier for an agent the person can see, or the owned tier
    // for an agent they own. An agent that is not there is refused the same way.
    let agent = s
        .deps()
        .storage
        .get_agent(&req.agent_id)
        .await
        .map_err(|e| storage_error("Failed to read the agent", e))?;
    let allowed = agent.as_ref().is_some_and(|agent| {
        Caller::from(who).manages(agent, PERMISSION_ALL, PERMISSION_OWNED)
            && (who.role.is_system || access::can_view_agent(who, agent))
    });
    if !allowed {
        return Err(forbidden());
    }

    if req.title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid("The title is longer than 120 characters"));
    }
    if req.topic_id.trim().is_empty() {
        return Err(invalid("topic_id is required"));
    }
    let removals = match &req.removals {
        None => "{}".to_string(),
        Some(v @ Value::Object(_)) => v.to_string(),
        Some(_) => return Err(invalid("removals must be an object")),
    };
    let snapshot_json = req.snapshot.get();
    if snapshot_json.len() > MAX_BODY_BYTES {
        return Err(ServiceError::TooLarge("The snapshot is too large to share".into()));
    }
    check_snapshot(snapshot_json, req.include_trail)?;
    let mut people = check_people(s, req.audience, &req.people).await?;
    // The author is always allowed; the list passed to `Audience::allows` includes them.
    people.sort();

    let created_at = crate::time::now().timestamp_millis();
    let mut share = ShareContent {
        record: ShareRecord {
            share_id: String::new(),
            creator_id: who.user_id.clone(),
            agent_id: req.agent_id.clone(),
            topic_id: req.topic_id.clone(),
            title: req.title.clone(),
            audience: req.audience,
            include_trail: req.include_trail,
            format_version: FORMAT_VERSION,
            size_bytes: snapshot_json.len() as i64,
            created_at,
            revoked_at: None,
            revoked_by: None,
            removals_json: removals,
        },
        snapshot_json: snapshot_json.to_string(),
        people,
    };

    // A clash on the id is retried once with a new one.
    for attempt in 0..2 {
        share.record.share_id = next_id();
        let outcome = s
            .deps()
            .storage
            .insert_share(&share, max_active)
            .await
            .map_err(|e| storage_error("Failed to store the share", e))?;
        match outcome {
            InsertOutcome::Inserted => {
                tracing::info!(
                    share_id = %share.record.share_id,
                    creator_id = %who.user_id,
                    agent_id = %req.agent_id,
                    audience = share.record.audience.name(),
                    size_bytes = share.record.size_bytes,
                    "share created"
                );
                return Ok(CreatedShare {
                    url_path: format!("/share/{}", share.record.share_id),
                    share_id: share.record.share_id,
                    created_at,
                });
            }
            InsertOutcome::CapReached => {
                return Err(ServiceError::RateLimited(
                    "You have reached the limit of active shares; revoke one first".into(),
                ));
            }
            InsertOutcome::IdTaken if attempt == 0 => continue,
            InsertOutcome::IdTaken => break,
        }
    }
    Err(ServiceError::Internal("Failed to allocate a share id".into()))
}

/// Opens a share for `viewer` (`None` when there is no session).
///
/// Anything that is not an active share the viewer may open is the same answer, whatever the
/// reason: without a session `Unauthorized(SIGN_IN_REQUIRED)`, signed in
/// `NotFound(SHARE_UNAVAILABLE)`. The audience decides only through `Audience::allows`.
pub async fn read(
    s: &Services,
    viewer: Option<&Principal>,
    share_id: &str,
) -> Result<ShareRead, ServiceError> {
    let refused = || match viewer {
        None => ServiceError::Unauthorized(SIGN_IN_REQUIRED.into()),
        Some(_) => ServiceError::NotFound(SHARE_UNAVAILABLE.into()),
    };
    if !is_valid_share_id(share_id) {
        return Err(refused());
    }
    let found = s
        .deps()
        .storage
        .get_share(share_id)
        .await
        .map_err(|e| storage_error("Failed to read the share", e))?;
    let Some(content) = found.filter(|c| c.record.revoked_at.is_none()) else {
        return Err(refused());
    };
    let audience = content.record.audience;
    let mut named = content.people.clone();
    named.push(content.record.creator_id.clone());
    match audience.allows(viewer, &named) {
        Access::Allowed => {}
        Access::NeedSignIn => return Err(ServiceError::Unauthorized(SIGN_IN_REQUIRED.into())),
        Access::Denied => return Err(ServiceError::NotFound(SHARE_UNAVAILABLE.into())),
    }
    let snapshot = RawValue::from_string(content.snapshot_json)
        .map_err(|e| ServiceError::Internal(format!("The stored share is damaged: {}", e.classify() as u8)))?;
    let record = content.record;
    Ok(ShareRead {
        cache: audience.cache_policy(),
        view: ShareView {
            share_id: record.share_id,
            title: record.title,
            created_at: record.created_at,
            audience,
            include_trail: record.include_trail,
            format_version: record.format_version,
            snapshot,
        },
    })
}

/// A matching user for naming in `specific` audience (`GET /shares/people?q=`).
/// Returns only `user_id` and `username` (no email or role leaks).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PersonMatch {
    pub user_id: String,
    pub username: String,
}

/// Search enabled users by username prefix for the specific audience picker.
/// Requires a share permission (`PERMISSION_ALL` or `PERMISSION_OWNED`), or system role.
/// Returns at most 20 matches.
pub async fn search_people(
    s: &Services,
    who: &Principal,
    q: &str,
) -> Result<Vec<PersonMatch>, ServiceError> {
    let has_share_perm = who.role.is_system
        || who.role.permissions.iter().any(|p| p == PERMISSION_ALL || p == PERMISSION_OWNED);
    if !has_share_perm {
        return Err(forbidden());
    }
    let query = q.trim();
    if query.chars().count() < 2 {
        return Err(invalid("Search query must be at least 2 characters"));
    }
    let users = s
        .deps()
        .storage
        .list_users()
        .await
        .map_err(|e| storage_error("Failed to list users", e))?;
    let q_lower = query.to_lowercase();
    let mut matches: Vec<PersonMatch> = users
        .into_iter()
        .filter(|u| u.username.to_lowercase().starts_with(&q_lower))
        .map(|u| PersonMatch {
            user_id: u.user_id,
            username: u.username,
        })
        .collect();
    matches.sort_by(|a, b| a.username.cmp(&b.username));
    matches.truncate(20);
    Ok(matches)
}

/// A share item in a list (`GET /shares`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ShareListItem {
    pub share_id: String,
    pub title: String,
    pub agent_id: String,
    pub agent_name: String,
    pub topic_id: String,
    pub audience: Audience,
    pub people_count: usize,
    pub include_trail: bool,
    pub size_bytes: i64,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub removals: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<String>,
}

/// List the caller's shares, newest first (`GET /shares`).
pub async fn list_mine(
    s: &Services,
    who: &Principal,
    before: Option<i64>,
    limit: usize,
) -> Result<Vec<ShareListItem>, ServiceError> {
    let storage = &s.deps().storage;
    let limit = limit.clamp(1, 100);
    let summaries = storage
        .list_shares_by_creator(&who.user_id, before, limit)
        .await
        .map_err(|e| storage_error("Failed to list shares", e))?;

    let mut items = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let r = summary.record;
        let agent = storage.get_agent(&r.agent_id).await.ok().flatten();
        let agent_name = agent.map(|a| a.name).unwrap_or_else(|| r.agent_id.clone());
        let removals = serde_json::from_str(&r.removals_json).ok();
        items.push(ShareListItem {
            share_id: r.share_id,
            title: r.title,
            agent_id: r.agent_id,
            agent_name,
            topic_id: r.topic_id,
            audience: r.audience,
            people_count: summary.people_count,
            include_trail: r.include_trail,
            size_bytes: r.size_bytes,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            removals,
            creator_name: None,
        });
    }
    Ok(items)
}

/// List all shares across the instance (`GET /shares?all=true`). Requires admin/system.
pub async fn list_all(
    s: &Services,
    who: &Principal,
    before: Option<i64>,
    limit: usize,
) -> Result<Vec<ShareListItem>, ServiceError> {
    let is_admin = who.role.is_system || access::can(who, PERMISSION_ALL);
    if !is_admin {
        return Err(forbidden());
    }
    let storage = &s.deps().storage;
    let limit = limit.clamp(1, 100);
    let summaries = storage
        .list_all_shares(before, limit)
        .await
        .map_err(|e| storage_error("Failed to list all shares", e))?;

    let mut items = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let r = summary.record;
        let agent = storage.get_agent(&r.agent_id).await.ok().flatten();
        let agent_name = agent.map(|a| a.name).unwrap_or_else(|| r.agent_id.clone());
        let removals = serde_json::from_str(&r.removals_json).ok();
        let creator = storage.get_user_by_id(&r.creator_id).await.ok().flatten();
        let creator_name = Some(creator.map(|u| u.username).unwrap_or_else(|| r.creator_id.clone()));
        items.push(ShareListItem {
            share_id: r.share_id,
            title: r.title,
            agent_id: r.agent_id,
            agent_name,
            topic_id: r.topic_id,
            audience: r.audience,
            people_count: summary.people_count,
            include_trail: r.include_trail,
            size_bytes: r.size_bytes,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            removals,
            creator_name,
        });
    }
    Ok(items)
}

/// Revoke a share (`DELETE /shares/{share_id}`).
pub async fn revoke(
    s: &Services,
    who: &Principal,
    share_id: &str,
) -> Result<(), ServiceError> {
    if !is_valid_share_id(share_id) {
        return Err(ServiceError::NotFound(SHARE_UNAVAILABLE.into()));
    }
    let storage = &s.deps().storage;
    let share = match storage.get_share(share_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return Err(ServiceError::NotFound(SHARE_UNAVAILABLE.into())),
        Err(e) => return Err(storage_error("Failed to read share", e)),
    };
    let is_admin = who.role.is_system || access::can(who, PERMISSION_ALL);
    let is_author = share.record.creator_id == who.user_id;
    if !is_author && !is_admin {
        return Err(ServiceError::NotFound(SHARE_UNAVAILABLE.into()));
    }
    if share.record.revoked_at.is_some() {
        return Ok(());
    }
    let now = crate::time::now().timestamp_millis();
    let _ = storage
        .revoke_share(share_id, &who.user_id, now)
        .await
        .map_err(|e| storage_error("Failed to revoke share", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_have_the_shape_the_validator_wants() {
        for _ in 0..50 {
            assert!(is_valid_share_id(&new_share_id()));
        }
    }

    #[test]
    fn the_id_validator_rejects_everything_else() {
        let good = "abcdefghijklmnopqrstu";
        assert_eq!(good.len(), 21);
        assert!(is_valid_share_id(good));
        assert!(is_valid_share_id("A-Z_a-z0-9_-_-_-_-_-"[..].repeat(2).get(..21).unwrap()));
        assert!(!is_valid_share_id(&good[..20]), "20 chars");
        assert!(!is_valid_share_id(&format!("{good}x")), "22 chars");
        assert!(!is_valid_share_id(""));
        assert!(!is_valid_share_id("../../../etc/passwd/x"), "traversal");
        assert!(!is_valid_share_id("abcdefghijklmnopq%00"), "percent-encoded NUL");
        assert!(!is_valid_share_id("abcdefghijklmnopqrs\0\0"), "NUL");
        // 21 bytes, but not 21 ASCII characters: Cyrillic "а" looks like Latin "a".
        assert!(!is_valid_share_id("аbcdefghijklmnopqrs"), "look-alike, 20 bytes + 1");
        assert!(!is_valid_share_id("аbcdefghijklmnopqrst"), "look-alike");
        assert!(!is_valid_share_id("abcdefghijklmnopqr é"), "space and accent");
        assert!(!is_valid_share_id("abcdefghijklmnopqrs.u"), "dot");
        assert!(!is_valid_share_id("abcdefghijklmnopqrs/u"), "slash");
    }

    fn doc(edit: impl FnOnce(&mut Value)) -> String {
        let mut v = serde_json::json!({
            "format_version": 1, "title": "t", "include_trail": false,
            "turns": [{"id": "t1", "role": "person", "parts": [{"kind": "text", "markdown": "hi"}]}],
            "media": {}
        });
        edit(&mut v);
        v.to_string()
    }

    #[test]
    fn a_plain_document_passes_and_the_structural_rules_hold() {
        assert!(check_snapshot(&doc(|_| {}), false).is_ok());
        assert!(check_snapshot("{", false).is_err());
        assert!(check_snapshot("[]", false).is_err());
        assert!(check_snapshot(&doc(|v| v["format_version"] = 2.into()), false).is_err());
        assert!(check_snapshot(&doc(|v| v["extra"] = 1.into()), false).is_err());
        assert!(check_snapshot(&doc(|v| v["turns"] = serde_json::json!([])), false).is_err());
        assert!(check_snapshot(&doc(|v| v["rehydration"] = 1.into()), false).is_err());
        assert!(check_snapshot(&doc(|v| v["title"] = "x".repeat(121).into()), false).is_err());
        let long = "x".repeat(MAX_TEXT_BYTES + 1);
        assert!(matches!(
            check_snapshot(&doc(|v| v["turns"][0]["parts"][0]["markdown"] = long.into()), false),
            Err(ServiceError::TooLarge(_))
        ));
    }

    #[test]
    fn trail_parts_with_the_flag_off_are_refused_and_never_rewritten() {
        let with_trail = doc(|v| v["turns"][0]["trail"] = serde_json::json!([{"kind": "tool_call", "name": "x"}]));
        let err = check_snapshot(&with_trail, false).unwrap_err();
        assert_eq!((err.status(), err.code()), (422, Some("trail_mismatch")));
        assert!(check_snapshot(&with_trail, true).is_ok());
        // An empty trail is not trail.
        assert!(check_snapshot(&doc(|v| v["turns"][0]["trail"] = serde_json::json!([])), false).is_ok());
        // The document says it has a trail while the request says it has not.
        assert_eq!(check_snapshot(&doc(|v| v["include_trail"] = true.into()), false).unwrap_err().status(), 422);
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use crate::{auth::AuthMethod, config::VizierConfig, storage::user::Role};

    struct World {
        s: Services,
        owner: Principal,
        other: Principal,
        sharer_all: Principal,
        plain: Principal,
        root: Principal,
        _dir: tempfile::TempDir,
    }

    async fn person(s: &Services, name: &str, role: &Role) -> Principal {
        s.deps().storage.create_user(name, "hash", &role.role_id).await.unwrap();
        let user = s.deps().storage.get_user(name).await.unwrap().unwrap();
        Principal {
            user_id: user.user_id,
            username: name.into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Jwt,
        }
    }

    async fn world() -> World {
        let dir = tempfile::tempdir().unwrap();
        let deps = crate::dependencies::VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let s = Services::new(deps);
        let st = &s.deps().storage;
        let owned = st
            .create_role("owned", vec!["owned_agents:view".into(), PERMISSION_OWNED.into()], false)
            .await
            .unwrap();
        let all = st
            .create_role("all", vec!["all_agents:view".into(), PERMISSION_ALL.into()], false)
            .await
            .unwrap();
        let none = st.create_role("none", vec!["all_agents:view".into()], false).await.unwrap();
        let system = st.create_role("system", vec![], true).await.unwrap();
        let owner = person(&s, "owner", &owned).await;
        let other = person(&s, "other", &owned).await;
        let sharer_all = person(&s, "sharer", &all).await;
        let plain = person(&s, "plain", &none).await;
        let root = person(&s, "root", &system).await;
        let mut a = crate::tests_support::agent_config("dp-a");
        a.owner_id = Some(owner.user_id.clone());
        st.create_agent("dp-a", &a).await.unwrap();
        let mut b = crate::tests_support::agent_config("dp-b");
        b.owner_id = Some(plain.user_id.clone());
        st.create_agent("dp-b", &b).await.unwrap();
        World { s, owner, other, sharer_all, plain, root, _dir: dir }
    }

    fn body(agent: &str, audience: &str, people: Option<Vec<String>>) -> Vec<u8> {
        let mut v = serde_json::json!({
            "agent_id": agent, "topic_id": "t1", "title": "A chat", "audience": audience,
            "include_trail": false,
            "removals": {"credentials": 1},
            // Spacing and key order that a re-serialisation would change.
            "snapshot": 1
        });
        if let Some(p) = people {
            v["people"] = p.into();
        }
        let text = v.to_string().replace(
            "\"snapshot\":1",
            "\"snapshot\":{ \"turns\" : [ {\"role\":\"person\",\"id\":\"t1\",\"parts\":[{\"markdown\":\"hi é\\u0000\",\"kind\":\"text\"}]} ], \"format_version\":1,\"title\":\"A chat\" }",
        );
        text.into_bytes()
    }

    const POSTED_SNAPSHOT: &str = "{ \"turns\" : [ {\"role\":\"person\",\"id\":\"t1\",\"parts\":[{\"markdown\":\"hi é\\u0000\",\"kind\":\"text\"}]} ], \"format_version\":1,\"title\":\"A chat\" }";

    #[tokio::test]
    async fn the_owner_creates_and_the_stored_bytes_come_back_unchanged() {
        let w = world().await;
        let made = create(&w.s, &w.owner, &body("dp-a", "public", None)).await.unwrap();
        assert!(is_valid_share_id(&made.share_id));
        assert_eq!(made.url_path, format!("/share/{}", made.share_id));
        let got = read(&w.s, None, &made.share_id).await.unwrap();
        assert_eq!(got.view.snapshot.get(), POSTED_SNAPSHOT);
        assert_eq!(got.cache, CachePolicy::Shared);
        assert_eq!(got.view.title, "A chat");
    }

    #[tokio::test]
    async fn permission_is_the_owned_tier_for_own_agents_the_all_tier_or_system() {
        let w = world().await;
        // No permission at all, even on an agent they own.
        let err = create(&w.s, &w.plain, &body("dp-b", "public", None)).await.unwrap_err();
        assert_eq!(err.status(), 403);
        // The owned tier does not reach an agent they do not own.
        let err = create(&w.s, &w.other, &body("dp-a", "public", None)).await.unwrap_err();
        assert_eq!(err.status(), 403);
        // The all tier reaches both; system bypasses.
        assert!(create(&w.s, &w.sharer_all, &body("dp-a", "public", None)).await.is_ok());
        assert!(create(&w.s, &w.sharer_all, &body("dp-b", "public", None)).await.is_ok());
        assert!(create(&w.s, &w.root, &body("dp-a", "public", None)).await.is_ok());
        // An agent that is not there is refused like a forbidden one.
        let err = create(&w.s, &w.root, &body("missing", "public", None)).await.unwrap_err();
        assert_eq!(err.status(), 403);
    }

    #[tokio::test]
    async fn a_clash_on_the_id_is_retried_once() {
        let w = world().await;
        let fixed = "AAAAAAAAAAAAAAAAAAAAA".to_string();
        let first = create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut || fixed.clone(), 200)
            .await
            .unwrap();
        assert_eq!(first.share_id, fixed);
        let mut calls = vec![fixed.clone(), "BBBBBBBBBBBBBBBBBBBBB".to_string()].into_iter();
        let second = create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut || calls.next().unwrap(), 200)
            .await
            .unwrap();
        assert_eq!(second.share_id, "BBBBBBBBBBBBBBBBBBBBB");
        // Two clashes in a row is a failure, not a loop.
        let err = create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut || fixed.clone(), 200)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 500);
    }

    #[tokio::test]
    async fn the_active_cap_refuses_and_a_revoked_share_frees_a_place() {
        let w = world().await;
        let a = create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut new_share_id, 2).await.unwrap();
        create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut new_share_id, 2).await.unwrap();
        let err = create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut new_share_id, 2).await.unwrap_err();
        assert_eq!(err.status(), 429);
        assert!(w.s.deps().storage.revoke_share(&a.share_id, &w.owner.user_id, 1).await.unwrap());
        assert!(create_with(&w.s, &w.owner, &body("dp-a", "public", None), &mut new_share_id, 2).await.is_ok());
    }

    #[tokio::test]
    async fn bad_bodies_are_400_and_too_large_is_413() {
        let w = world().await;
        for bad in [&b"{"[..], b"[]", b"{\"agent_id\":\"dp-a\"}"] {
            assert_eq!(create(&w.s, &w.owner, bad).await.unwrap_err().status(), 400);
        }
        let mut v: Value = serde_json::from_slice(&body("dp-a", "public", None)).unwrap();
        v["title"] = "x".repeat(121).into();
        assert_eq!(create(&w.s, &w.owner, v.to_string().as_bytes()).await.unwrap_err().status(), 400);
        let mut v: Value = serde_json::from_slice(&body("dp-a", "public", None)).unwrap();
        v["surprise"] = 1.into();
        assert_eq!(create(&w.s, &w.owner, v.to_string().as_bytes()).await.unwrap_err().status(), 400);
        let mut v: Value = serde_json::from_slice(&body("dp-a", "everyone", None)).unwrap();
        v["audience"] = "everyone".into();
        assert_eq!(create(&w.s, &w.owner, v.to_string().as_bytes()).await.unwrap_err().status(), 400);
        let huge = vec![b' '; MAX_BODY_BYTES + 1];
        assert_eq!(create(&w.s, &w.owner, &huge).await.unwrap_err().status(), 413);
    }

    /// T047: the flag is off but the document carries trail parts.
    #[tokio::test]
    async fn trail_without_the_flag_is_422() {
        let w = world().await;
        let mut v: Value = serde_json::from_slice(&body("dp-a", "public", None)).unwrap();
        v["snapshot"]["turns"][0]["trail"] = serde_json::json!([{"kind": "tool_call", "name": "x", "arguments": {}}]);
        let err = create(&w.s, &w.owner, v.to_string().as_bytes()).await.unwrap_err();
        assert_eq!((err.status(), err.code()), (422, Some("trail_mismatch")));
        v["include_trail"] = true.into();
        assert!(create(&w.s, &w.owner, v.to_string().as_bytes()).await.is_ok());
        assert_eq!(w.s.deps().storage.count_active_by_creator(&w.owner.user_id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn people_rules_follow_the_audience() {
        let w = world().await;
        let named = vec![w.other.user_id.clone()];
        // Required for specific, forbidden otherwise.
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "specific", None)).await.unwrap_err().status(), 400);
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "specific", Some(vec![]))).await.unwrap_err().status(), 400);
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "public", Some(named.clone()))).await.unwrap_err().status(), 400);
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "signed_in", Some(named.clone()))).await.unwrap_err().status(), 400);
        // Unknown and duplicate.
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "specific", Some(vec!["ghost".into()]))).await.unwrap_err().status(), 400);
        let dup = vec![w.other.user_id.clone(), w.other.user_id.clone()];
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "specific", Some(dup))).await.unwrap_err().status(), 400);
        // Too many.
        let many: Vec<String> = (0..51).map(|i| format!("u{i}")).collect();
        assert_eq!(create(&w.s, &w.owner, &body("dp-a", "specific", Some(many))).await.unwrap_err().status(), 400);
        // Fine, and the author may be in the list.
        let both = vec![w.other.user_id.clone(), w.owner.user_id.clone()];
        assert!(create(&w.s, &w.owner, &body("dp-a", "specific", Some(both))).await.is_ok());
    }

    #[tokio::test]
    async fn reading_follows_the_audience_and_every_refusal_looks_alike() {
        let w = world().await;
        let public = create(&w.s, &w.owner, &body("dp-a", "public", None)).await.unwrap().share_id;
        let signed = create(&w.s, &w.owner, &body("dp-a", "signed_in", None)).await.unwrap().share_id;
        let named = vec![w.other.user_id.clone()];
        let specific = create(&w.s, &w.owner, &body("dp-a", "specific", Some(named))).await.unwrap().share_id;

        let outcome = |r: Result<ShareRead, ServiceError>| match r {
            Ok(r) => format!("{}", r.cache.cache_control()),
            Err(e) => format!("{} {}", e.status(), e.message()),
        };
        // public: everyone. signed_in: any session. specific: named and the author.
        assert!(outcome(read(&w.s, None, &public).await).starts_with("public"));
        assert_eq!(outcome(read(&w.s, None, &signed).await), "401 sign_in_required");
        assert_eq!(outcome(read(&w.s, None, &specific).await), "401 sign_in_required");
        assert!(outcome(read(&w.s, Some(&w.plain), &signed).await).starts_with("private"));
        assert_eq!(outcome(read(&w.s, Some(&w.plain), &specific).await), "404 share_unavailable");
        assert!(outcome(read(&w.s, Some(&w.other), &specific).await).starts_with("private"));
        assert!(outcome(read(&w.s, Some(&w.owner), &specific).await).starts_with("private"));
        // Unknown, malformed and revoked: the same as a restricted one.
        let unknown = "ZZZZZZZZZZZZZZZZZZZZZ";
        assert_eq!(outcome(read(&w.s, None, unknown).await), "401 sign_in_required");
        assert_eq!(outcome(read(&w.s, None, "../x").await), "401 sign_in_required");
        assert_eq!(outcome(read(&w.s, Some(&w.owner), unknown).await), "404 share_unavailable");
        assert_eq!(outcome(read(&w.s, Some(&w.owner), "%00").await), "404 share_unavailable");
        assert!(w.s.deps().storage.revoke_share(&public, &w.owner.user_id, 5).await.unwrap());
        assert_eq!(outcome(read(&w.s, None, &public).await), "401 sign_in_required");
        assert_eq!(outcome(read(&w.s, Some(&w.owner), &public).await), "404 share_unavailable");
    }

    /// A person named on a link who is later removed from the instance opens nothing; the id
    /// left in the list is never matched by anyone else.
    #[tokio::test]
    async fn a_removed_named_person_is_refused() {
        let w = world().await;
        let named = vec![w.other.user_id.clone()];
        let id = create(&w.s, &w.owner, &body("dp-a", "specific", Some(named))).await.unwrap().share_id;
        w.s.deps().storage.delete_user(&w.other.user_id).await.unwrap();
        // The deleted person cannot authenticate; anyone else is refused.
        assert_eq!(read(&w.s, Some(&w.plain), &id).await.unwrap_err().status(), 404);
        assert_eq!(read(&w.s, None, &id).await.unwrap_err().status(), 401);
        assert!(read(&w.s, Some(&w.owner), &id).await.is_ok());
    }

    #[tokio::test]
    async fn search_people_filters_prefix_and_requires_permission() {
        let w = world().await;
        // query too short: 400
        assert_eq!(search_people(&w.s, &w.owner, "a").await.unwrap_err().status(), 400);
        // without permission: 403
        assert_eq!(search_people(&w.s, &w.plain, "al").await.unwrap_err().status(), 403);
        // with permission: prefix match
        let found = search_people(&w.s, &w.owner, "al").await.unwrap();
        assert!(found.iter().any(|u| u.username == "alice"));
        // system role is allowed
        let root = w.s.deps().storage.create_role("system-search", vec![], true).await.unwrap();
        let root_user = w.s.deps().storage.create_user("root-search", "", &root.role_id).await.unwrap();
        let root_principal = Principal {
            user_id: root_user.user_id,
            username: root_user.username,
            role: root,
            method: crate::auth::AuthMethod::Password,
        };
        assert!(search_people(&w.s, &root_principal, "al").await.is_ok());
    }

    #[tokio::test]
    async fn list_and_revoke_behavior_and_user_deletion_cascade() {
        let w = world().await;
        let s1 = create(&w.s, &w.owner, &body("dp-a", "public", None)).await.unwrap().share_id;
        let s2 = create(&w.s, &w.owner, &body("dp-a", "signed_in", None)).await.unwrap().share_id;

        // list_mine returns owner's shares newest first
        let mine = list_mine(&w.s, &w.owner, None, 50).await.unwrap();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].share_id, s2);
        assert_eq!(mine[1].share_id, s1);
        assert!(mine[0].creator_name.is_none());

        // other person's list_mine is empty
        let other_mine = list_mine(&w.s, &w.other, None, 50).await.unwrap();
        assert!(other_mine.is_empty());

        // non-admin list_all is 403 forbidden
        assert_eq!(list_all(&w.s, &w.plain, None, 50).await.unwrap_err().status(), 403);

        // system role list_all succeeds and includes creator_name
        let root_role = w.s.deps().storage.create_role("system-admin", vec![], true).await.unwrap();
        let root_user = w.s.deps().storage.create_user("root-admin", "", &root_role.role_id).await.unwrap();
        let root_p = Principal {
            user_id: root_user.user_id,
            username: root_user.username,
            role: root_role,
            method: crate::auth::AuthMethod::Password,
        };
        let all = list_all(&w.s, &root_p, None, 50).await.unwrap();
        assert!(all.len() >= 2);
        assert!(all.iter().any(|i| i.share_id == s1 && i.creator_name.as_deref() == Some("alice")));

        // revoke by unauthorized user returns 404
        assert_eq!(revoke(&w.s, &w.plain, &s1).await.unwrap_err().status(), 404);

        // revoke by author succeeds
        assert!(revoke(&w.s, &w.owner, &s1).await.is_ok());
        // second revoke is 204 no-op
        assert!(revoke(&w.s, &w.owner, &s1).await.is_ok());

        // read revoked is refused
        assert_eq!(read(&w.s, Some(&w.owner), &s1).await.unwrap_err().status(), 404);

        // revoked shares do not count toward cap
        let active = w.s.deps().storage.count_active_by_creator(&w.owner.user_id).await.unwrap();
        assert_eq!(active, 1);

        // user deletion deletes their shares and audience rows
        let deleted = w.s.deps().storage.delete_shares_by_creator(&w.owner.user_id).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(w.s.deps().storage.count_active_by_creator(&w.owner.user_id).await.unwrap(), 0);
    }
}
