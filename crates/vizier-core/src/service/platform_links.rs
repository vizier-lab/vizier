//! The signed-in person's connected Discord and Telegram accounts (spec 069,
//! contracts/http-connections.md). Everything here acts only on the caller's own account.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{
    auth::Principal,
    platform_links::{PlatformPerson, code, text::platform_name},
    schema::ChannelPlatform,
    service::{ServiceError, Services, users::fail},
    storage::{
        platform_link::{LinkAction, PlatformLink, PlatformLinkStorage, RedeemOutcome},
        user::{UserProfile, UserStorage},
    },
};

/// How many changes the listing carries.
const EVENTS_SHOWN: u32 = 20;

/// The one answer for a code that is unknown, expired or used (FR-008).
const UNAVAILABLE: &str = "This code has expired or was already used. Send /connect again.";

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConnectedAccount {
    pub platform: ChannelPlatform,
    pub platform_user_id: String,
    pub platform_username: Option<String>,
    pub platform_display_name: Option<String>,
    pub linked_at: DateTime<Local>,
}

impl From<PlatformLink> for ConnectedAccount {
    fn from(link: PlatformLink) -> Self {
        Self {
            platform: link.platform,
            platform_user_id: link.platform_user_id,
            platform_username: link.platform_username,
            platform_display_name: link.platform_display_name,
            linked_at: link.linked_at,
        }
    }
}

/// An id or username typed by hand before accounts could be proven. It identifies nobody (#82).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UnverifiedAccount {
    pub platform: ChannelPlatform,
    pub platform_user_id: Option<String>,
    pub platform_username: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConnectionChange {
    pub platform: ChannelPlatform,
    pub platform_user_id: String,
    pub action: LinkAction,
    pub at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Connections {
    pub links: Vec<ConnectedAccount>,
    pub unverified: Vec<UnverifiedAccount>,
    /// Newest first.
    pub events: Vec<ConnectionChange>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConnectCodeRequest {
    /// As typed: case, spaces and dashes don't matter.
    pub code: String,
}

/// What confirming a code would do. Nothing has changed yet.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConnectPreview {
    pub platform: ChannelPlatform,
    pub platform_user_id: String,
    pub platform_username: Option<String>,
    pub platform_display_name: Option<String>,
    /// The Vizier account the platform account would be connected to.
    pub vizier_username: String,
    /// The caller's other account on this platform, which confirming would unlink.
    pub replaces: Option<ConnectedAccount>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConnectResult {
    pub link: ConnectedAccount,
    pub replaced: Option<ConnectedAccount>,
}

fn unavailable() -> ServiceError {
    ServiceError::Invalid {
        status: 404,
        message: UNAVAILABLE.into(),
        code: Some("connect_code_unavailable".into()),
    }
}

/// Never says who the other person is.
fn linked_elsewhere(platform: ChannelPlatform) -> ServiceError {
    ServiceError::Conflict {
        message: format!(
            "This {} account is already connected to another Vizier user.",
            platform_name(platform)
        ),
        code: Some("platform_linked_elsewhere".into()),
        data: None,
    }
}

fn now_ms() -> i64 {
    crate::time::now().timestamp_millis()
}

/// The hash a typed code is looked up by. What cannot be a code reads like a wrong one.
fn hash_of(input: &str) -> Result<String, ServiceError> {
    code::normalize(input)
        .map(|code| code::hash(&code))
        .ok_or_else(unavailable)
}

fn hand_typed(profile: &UserProfile) -> Vec<UnverifiedAccount> {
    [
        (ChannelPlatform::Discord, &profile.discord_id, &profile.discord_username),
        (ChannelPlatform::Telegram, &profile.telegram_id, &profile.telegram_username),
    ]
    .into_iter()
    .filter_map(|(platform, id, username)| {
        let set = |value: &Option<String>| value.clone().filter(|v| !v.trim().is_empty());
        let (platform_user_id, platform_username) = (set(id), set(username));
        (platform_user_id.is_some() || platform_username.is_some()).then_some(UnverifiedAccount {
            platform,
            platform_user_id,
            platform_username,
        })
    })
    .collect()
}

pub async fn list(services: &Services, principal: &Principal) -> Result<Connections, ServiceError> {
    let storage = &services.deps().storage;
    let failed = |e| fail("Failed to list connected accounts", e);
    let links = storage.list_links(&principal.user_id).await.map_err(failed)?;
    let profile = storage
        .get_user_profile(&principal.user_id)
        .await
        .map_err(failed)?;
    let events = storage
        .list_link_events(&principal.user_id, EVENTS_SHOWN)
        .await
        .map_err(failed)?;
    Ok(Connections {
        links: links.into_iter().map(ConnectedAccount::from).collect(),
        unverified: profile.as_ref().map(hand_typed).unwrap_or_default(),
        events: events
            .into_iter()
            .map(|event| ConnectionChange {
                platform: event.platform,
                platform_user_id: event.platform_user_id,
                action: event.action,
                at: event.at,
            })
            .collect(),
    })
}

/// The account a code belongs to, and what confirming it would replace. Consumes nothing.
pub async fn preview(
    services: &Services,
    principal: &Principal,
    input: &str,
) -> Result<ConnectPreview, ServiceError> {
    let hash = hash_of(input)?;
    let storage = &services.deps().storage;
    let failed = |e| fail("Failed to read the code", e);
    let Some(person) = storage
        .peek_connect_code(&hash, now_ms())
        .await
        .map_err(failed)?
    else {
        return Err(unavailable());
    };
    let holder = storage
        .find_link(person.platform, &person.id)
        .await
        .map_err(failed)?;
    if holder.is_some_and(|link| link.user_id != principal.user_id) {
        return Err(linked_elsewhere(person.platform));
    }
    let replaces = storage
        .list_links(&principal.user_id)
        .await
        .map_err(failed)?
        .into_iter()
        .find(|link| link.platform == person.platform && link.platform_user_id != person.id)
        .map(ConnectedAccount::from);
    let PlatformPerson {
        platform,
        id,
        username,
        display_name,
    } = person;
    Ok(ConnectPreview {
        platform,
        platform_user_id: id,
        platform_username: username,
        platform_display_name: display_name,
        vizier_username: principal.username.clone(),
        replaces,
    })
}

/// Take the code and connect its account to the caller.
pub async fn redeem(
    services: &Services,
    principal: &Principal,
    input: &str,
) -> Result<ConnectResult, ServiceError> {
    let hash = hash_of(input)?;
    let storage = &services.deps().storage;
    let failed = |e| fail("Failed to connect the account", e);
    // Read first only to name the platform in a refusal; the transaction decides.
    let platform = storage
        .peek_connect_code(&hash, now_ms())
        .await
        .map_err(failed)?
        .map(|person| person.platform);
    let outcome = storage
        .redeem_connect_code(&hash, &principal.user_id, now_ms())
        .await
        .map_err(failed)?;
    let (link, replaced) = match outcome {
        RedeemOutcome::Linked(link) => (link, None),
        RedeemOutcome::Replaced { link, previous } => (link, Some(previous)),
        RedeemOutcome::Unavailable => return Err(unavailable()),
        RedeemOutcome::LinkedElsewhere => {
            return Err(platform.map_or_else(unavailable, linked_elsewhere));
        }
    };
    tracing::info!(platform = %link.platform, "platform account connected");
    Ok(ConnectResult {
        link: link.into(),
        replaced: replaced.map(ConnectedAccount::from),
    })
}

pub async fn unlink(
    services: &Services,
    principal: &Principal,
    platform: ChannelPlatform,
) -> Result<(), ServiceError> {
    let unlinked = services
        .deps()
        .storage
        .unlink_platform(&principal.user_id, platform)
        .await
        .map_err(|e| fail("Failed to unlink the account", e))?;
    if !unlinked {
        return Err(ServiceError::Invalid {
            status: 404,
            message: format!("No {} account is connected.", platform_name(platform)),
            code: Some("not_linked".into()),
        });
    }
    tracing::info!(platform = %platform, "platform account unlinked");
    Ok(())
}

/// Remove what was typed by hand for `platform` (#82). Nothing to remove is not an error.
pub async fn clear_unverified(
    services: &Services,
    principal: &Principal,
    platform: ChannelPlatform,
) -> Result<(), ServiceError> {
    let storage = &services.deps().storage;
    let failed = |e| fail("Failed to remove the unverified account", e);
    let Some(mut profile) = storage
        .get_user_profile(&principal.user_id)
        .await
        .map_err(failed)?
    else {
        return Ok(());
    };
    if profile.clear_platform(platform) {
        storage
            .upsert_user_profile(&principal.user_id, &profile)
            .await
            .map_err(failed)?;
    }
    Ok(())
}
