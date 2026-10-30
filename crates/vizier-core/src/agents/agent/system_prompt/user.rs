use crate::storage::user::UserProfile;

pub fn owner_md(profile: &UserProfile) -> String {
    let mut parts = Vec::new();

    if let Some(ref discord_id) = profile.discord_id {
        if !discord_id.is_empty() {
            parts.push(format!("discord_id: \"{}\"", discord_id));
        }
    }
    if let Some(ref discord_username) = profile.discord_username {
        if !discord_username.is_empty() {
            parts.push(format!("discord_username: \"{}\"", discord_username));
        }
    }
    if let Some(ref telegram_id) = profile.telegram_id {
        if !telegram_id.is_empty() {
            parts.push(format!("telegram_id: \"{}\"", telegram_id));
        }
    }
    if let Some(ref telegram_username) = profile.telegram_username {
        if !telegram_username.is_empty() {
            parts.push(format!("telegram_username: \"{}\"", telegram_username));
        }
    }
    if !profile.alias.is_empty() {
        parts.push(format!("alias: {:?}", profile.alias));
    }

    let data = if parts.is_empty() {
        "No additional profile data available.".to_string()
    } else {
        parts.join("\n")
    };

    format!(
        r#"# OWNER.md -- Your Owner

Below is the data of your owner:
{},

This is the user who created and owns you. Refer to this for owner-specific context.
"#,
        data
    )
}
