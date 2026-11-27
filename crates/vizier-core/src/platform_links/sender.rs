//! Who an agent is told sent a platform request.

use serde_json::Value;

use crate::{
    platform_links::{PlatformPerson, PlatformSender, UNREGISTERED_USER},
    schema::{VizierRequest, VizierRequestContent},
    storage::platform_link::PlatformLinkStorage,
};

/// The Vizier username of whoever linked this account, or [`UNREGISTERED_USER`].
///
/// Read afresh every time, so linking and unlinking apply to the very next message. A storage
/// failure answers unlinked, so a message is never dropped and never wrongly attributed.
pub async fn resolve<S>(storage: &S, person: PlatformPerson) -> PlatformSender
where
    S: PlatformLinkStorage + Sync + ?Sized,
{
    let link = match storage.find_link(person.platform, &person.id).await {
        Ok(link) => link,
        Err(error) => {
            tracing::warn!(
                platform = %person.platform,
                "could not look up a platform link, treating the sender as unlinked: {error}"
            );
            None
        }
    };
    let Some(link) = link else {
        return PlatformSender {
            label: UNREGISTERED_USER.to_string(),
            verified: false,
            person,
        };
    };
    // Names are for display only; refresh them when the account shows new ones.
    if (&link.platform_username, &link.platform_display_name)
        != (&person.username, &person.display_name)
        && let Err(error) = storage.touch_link_names(&person).await
    {
        tracing::warn!(platform = %person.platform, "could not refresh a linked account's names: {error}");
    }
    PlatformSender {
        label: link.username,
        verified: true,
        person,
    }
}

/// Mark `request` as sent by `sender`: the label, whether it is proven, and the platform identity
/// in the metadata.
pub fn stamp(request: &mut VizierRequest, sender: &PlatformSender) {
    request.user = sender.label.clone();
    request.verified = Some(sender.verified);
    if !request.metadata.is_object() {
        request.metadata = Value::Object(Default::default());
    }
    if let Value::Object(metadata) = &mut request.metadata {
        metadata.extend(sender.person.metadata_fields());
    }
}

/// [`stamp`] for an event whose platform names no person at all (a Telegram channel post):
/// nobody linked, nothing to put in the metadata.
pub fn stamp_or_unregistered(request: &mut VizierRequest, sender: Option<&PlatformSender>) {
    match sender {
        Some(sender) => stamp(request, sender),
        None => {
            request.user = UNREGISTERED_USER.to_string();
            request.verified = Some(false);
        }
    }
}

/// A chat command (`abort`, `checkpoint`, `lobotomy`) a platform person gave.
pub fn command_request(command: &str, sender: Option<&PlatformSender>) -> VizierRequest {
    let mut request = VizierRequest {
        timestamp: crate::time::now(),
        content: VizierRequestContent::Command(command.to_string()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    stamp_or_unregistered(&mut request, sender);
    request
}

#[cfg(test)]
#[path = "sender_tests.rs"]
mod tests;
