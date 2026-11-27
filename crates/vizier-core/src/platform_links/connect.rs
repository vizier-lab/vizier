//! Issuing a connect code to a platform account that asked for one.

use crate::{
    dependencies::VizierDependencies,
    platform_links::{PlatformPerson, code},
    storage::platform_link::PlatformLinkStorage,
};

/// How many codes one platform account may ask for per [`ISSUE_WINDOW`].
pub const ISSUES_PER_WINDOW: usize = 5;
pub const ISSUE_WINDOW: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// What to answer `/connect` with. [`crate::platform_links::text::reply`] words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectReply {
    Code {
        /// `ABCD-EFGH`.
        code: String,
        /// The one-click link, when the operator told Vizier its public address.
        link: Option<String>,
        expires_in_minutes: u32,
    },
    /// Asked where others can read the answer, so no code was made.
    AskInPrivate { bot_link: Option<String> },
    TooMany { retry_after_secs: u64 },
    Failed,
}

/// The one-click link carrying `code`. The code is in the fragment, which a browser never sends,
/// so it stays out of server and proxy logs.
fn link(base_url: &str, code: &str) -> String {
    format!("{base_url}/connect#{code}")
}

/// Answer `/connect` from `person`. `private` says only they can read the answer; where that is
/// not so, nothing is issued.
pub async fn issue(
    deps: &VizierDependencies,
    person: &PlatformPerson,
    private: bool,
    bot_link: Option<String>,
) -> ConnectReply {
    if !private {
        return ConnectReply::AskInPrivate { bot_link };
    }
    let key = person.key();
    if !deps.connect_issues.check(&key) {
        let wait = deps.connect_issues.retry_after(&key).unwrap_or(ISSUE_WINDOW);
        return ConnectReply::TooMany {
            retry_after_secs: wait.as_secs().max(1),
        };
    }
    match store_new_code(deps, person).await {
        Ok(code) => {
            // Never the code itself.
            tracing::info!(platform = %person.platform, "connect code issued");
            let code = code::display(&code);
            ConnectReply::Code {
                link: deps.config.base_url.as_deref().map(|base| link(base, &code)),
                code,
                expires_in_minutes: (code::LIFETIME_MS / 60_000) as u32,
            }
        }
        Err(error) => {
            tracing::warn!(platform = %person.platform, "could not issue a connect code: {error}");
            ConnectReply::Failed
        }
    }
}

async fn store_new_code(
    deps: &VizierDependencies,
    person: &PlatformPerson,
) -> anyhow::Result<String> {
    let code = code::generate()?;
    let now_ms = crate::time::now().timestamp_millis();
    deps.storage
        .issue_connect_code(person, &code::hash(&code), now_ms, now_ms + code::LIFETIME_MS)
        .await?;
    Ok(code)
}
