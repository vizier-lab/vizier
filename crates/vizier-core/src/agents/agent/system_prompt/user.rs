use crate::{
    platform_links::text::platform_key,
    storage::{platform_link::PlatformLink, user::UserProfile},
};

/// The owner as the agent knows them. Platform accounts appear only when the owner proved them
/// (spec 069); ids typed by hand are never printed, so a stranger cannot pass as the owner.
pub fn owner_md(username: &str, profile: Option<&UserProfile>, links: &[PlatformLink]) -> String {
    let mut parts = vec![format!("vizier_username: {username:?}")];

    for link in links {
        let name = link
            .platform_username
            .as_deref()
            .or(link.platform_display_name.as_deref());
        let id = &link.platform_user_id;
        let platform = platform_key(link.platform);
        parts.push(match name {
            Some(name) => format!("{platform}: {name:?} (id {id:?}), verified link"),
            None => format!("{platform}: id {id:?}, verified link"),
        });
    }
    if let Some(profile) = profile
        && !profile.alias.is_empty()
    {
        parts.push(format!("alias: {:?}", profile.alias));
    }

    format!(
        r#"# OWNER.md -- Your Owner

Below is the data of your owner:
{},

This is the user who created and owns you. Refer to this for owner-specific context.
"#,
        parts.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ChannelPlatform;

    fn profile() -> UserProfile {
        UserProfile {
            user_id: "u1".into(),
            discord_id: Some("999".into()),
            discord_username: Some("typed_by_hand".into()),
            telegram_id: None,
            telegram_username: None,
            alias: vec!["Rin".into()],
        }
    }

    fn link(platform: ChannelPlatform, id: &str, handle: Option<&str>) -> PlatformLink {
        PlatformLink {
            platform,
            platform_user_id: id.into(),
            user_id: "u1".into(),
            username: "rin".into(),
            platform_username: handle.map(str::to_string),
            platform_display_name: None,
            linked_at: crate::time::now(),
        }
    }

    #[test]
    fn only_proven_accounts_are_listed() {
        let text = owner_md(
            "rin",
            Some(&profile()),
            &[
                link(ChannelPlatform::Discord, "381926457021", Some("rin_dev")),
                link(ChannelPlatform::Telegram, "55120", None),
            ],
        );
        assert!(text.contains("vizier_username: \"rin\""), "{text}");
        assert!(
            text.contains("discord: \"rin_dev\" (id \"381926457021\"), verified link"),
            "{text}"
        );
        assert!(text.contains("telegram: id \"55120\", verified link"), "{text}");
        assert!(text.contains("alias: [\"Rin\"]"), "{text}");
        assert!(!text.contains("999") && !text.contains("typed_by_hand"), "{text}");
    }

    #[test]
    fn a_hand_typed_id_without_a_link_is_not_printed() {
        let text = owner_md("rin", Some(&profile()), &[]);
        assert!(!text.contains("verified link"), "{text}");
        assert!(!text.contains("999"), "{text}");
    }
}
