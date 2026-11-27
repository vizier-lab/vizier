//! Every reply the bots send for `/connect` (contracts/bot-connect.md).

use crate::{platform_links::connect::ConnectReply, schema::ChannelPlatform};

/// The platform's name as a field name.
pub fn platform_key(platform: ChannelPlatform) -> &'static str {
    match platform {
        ChannelPlatform::Discord => "discord",
        ChannelPlatform::Telegram => "telegram",
    }
}

/// The platform's name as people write it.
pub fn platform_name(platform: ChannelPlatform) -> &'static str {
    match platform {
        ChannelPlatform::Discord => "Discord",
        ChannelPlatform::Telegram => "Telegram",
    }
}

pub fn reply(platform: ChannelPlatform, reply: &ConnectReply) -> String {
    let name = platform_name(platform);
    match reply {
        ConnectReply::Code {
            code,
            link,
            expires_in_minutes,
        } => {
            let how = match link {
                Some(link) => format!(
                    "Link this {name} account to your Vizier user:\n{link}\n\n\
                     Or enter {code} in Vizier under Settings › Profile › Connected accounts."
                ),
                None => format!(
                    "To link this {name} account to your Vizier user, enter {code} in Vizier \
                     under Settings › Profile › Connected accounts."
                ),
            };
            format!(
                "{how}\nThe code works once and expires in {expires_in_minutes} minutes. \
                 Only use a code you asked for yourself."
            )
        }
        ConnectReply::AskInPrivate { bot_link: Some(link) } => {
            format!("Send /connect to me in a direct message: {link}")
        }
        ConnectReply::AskInPrivate { bot_link: None } => {
            "Send /connect to me in a direct message.".to_string()
        }
        ConnectReply::TooMany { retry_after_secs } => {
            let minutes = retry_after_secs.div_ceil(60).max(1);
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("Too many codes. Try again in {minutes} {unit}.")
        }
        ConnectReply::Failed => {
            "Couldn't make a code just now. Send /connect again in a moment.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(link: Option<&str>) -> ConnectReply {
        ConnectReply::Code {
            code: "ABCD-EFGH".into(),
            link: link.map(str::to_string),
            expires_in_minutes: 10,
        }
    }

    #[test]
    fn a_reply_always_carries_the_code_and_where_to_enter_it() {
        for link in [Some("https://v.example/connect#ABCD-EFGH"), None] {
            let text = reply(ChannelPlatform::Discord, &code(link));
            assert!(text.contains("ABCD-EFGH"), "{text}");
            assert!(text.contains("Settings › Profile › Connected accounts"), "{text}");
            assert!(text.contains("expires in 10 minutes"), "{text}");
            assert!(text.contains("Discord"), "{text}");
            assert_eq!(text.contains("https://"), link.is_some(), "{text}");
        }
    }

    #[test]
    fn the_other_replies_carry_no_code() {
        let ask = reply(
            ChannelPlatform::Telegram,
            &ConnectReply::AskInPrivate {
                bot_link: Some("https://t.me/vizier_bot".into()),
            },
        );
        assert_eq!(ask, "Send /connect to me in a direct message: https://t.me/vizier_bot");
        assert_eq!(
            reply(ChannelPlatform::Discord, &ConnectReply::TooMany { retry_after_secs: 61 }),
            "Too many codes. Try again in 2 minutes."
        );
        assert_eq!(
            reply(ChannelPlatform::Discord, &ConnectReply::TooMany { retry_after_secs: 5 }),
            "Too many codes. Try again in 1 minute."
        );
    }
}
