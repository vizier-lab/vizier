//! The SSO portal half of the flow: the accounts and roles the signed-in identity may use, and
//! the role credentials they resolve to (`contracts/bedrock-sso-wire.md` §4).
//!
//! Accounts and roles populate the pickers, so a person chooses from what AWS actually assigned
//! them. A typed value is offered **only** when the portal listed nothing at all (FR-007) — that
//! is what [`DiscoveredAccounts::free_text`] / [`DiscoveredRoles::free_text`] say, and they say it
//! once, here, rather than leaving each caller to treat an empty list as permission to type.
//!
//! Role credentials are the short-lived key, secret and session token (§4.3). They live in memory
//! only, in the value that asked for them, and their `Debug` is redacted (FR-009d, research D8.3).

use crate::time::Timestamp;
use std::fmt;


use serde::Deserialize;

use super::client::SsoClient;
use super::error::SsoError;

/// One account the identity is assigned, as the portal names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsoAccount {
    pub account_id: String,
    pub account_name: String,
    pub email_address: Option<String>,
}

/// One role the identity may assume in an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsoRole {
    pub role_name: String,
    pub account_id: String,
}

/// What the account picker may offer: the portal's list, and whether a typed value is allowed.
///
/// `free_text` is true **only** when the list is empty — a non-empty list is the authority, so a
/// person cannot save an account AWS did not assign (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAccounts {
    pub accounts: Vec<SsoAccount>,
    pub free_text: bool,
}

/// The role picker's counterpart of [`DiscoveredAccounts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRoles {
    pub roles: Vec<SsoRole>,
    pub free_text: bool,
}

impl DiscoveredAccounts {
    fn new(accounts: Vec<SsoAccount>) -> Self {
        Self {
            free_text: accounts.is_empty(),
            accounts,
        }
    }
}

impl DiscoveredRoles {
    fn new(roles: Vec<SsoRole>) -> Self {
        Self {
            free_text: roles.is_empty(),
            roles,
        }
    }
}

/// The three halves [`SigV4Credential`](crate::llm::auth::SigV4Credential) signs with, plus their
/// expiry (§4.3). Memory only: nothing in this type can be written by any code path here.
#[derive(Clone, PartialEq, Eq)]
pub struct RoleCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: String,
    pub expires_at: Timestamp,
}

impl fmt::Debug for RoleCredentials {
    /// Prints the expiry and nothing a signature could use (research D8.3, FR-035).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoleCredentials")
            .field("access_key_id", &"[redacted]")
            .field("secret_access_key", &"[redacted]")
            .field("session_token", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// The portal of one SSO region, over the same [`SsoClient`] the flow registered with.
#[derive(Debug, Clone)]
pub struct SsoPortal {
    client: SsoClient,
}

impl SsoPortal {
    pub fn new(client: SsoClient) -> Self {
        Self { client }
    }

    /// §4.1: the accounts assigned to the session's identity.
    pub async fn accounts(&self, access_token: &str) -> Result<DiscoveredAccounts, SsoError> {
        let url = format!(
            "{}/assignment/accounts",
            self.client.endpoints().portal
        );
        let (status, body) = self
            .client
            .send_with_retry(&url, |url| {
                self.client
                    .http
                    .get(url)
                    .bearer_auth(access_token)
            })
            .await?;
        if status != 200 {
            return Err(self.refused(status, "list the accounts assigned to this identity"));
        }
        let wire: AccountsWire = serde_json::from_str(&body)
            .map_err(|_| self.client.unavailable("the accounts list answered an unexpected shape"))?;
        let accounts = wire
            .account_list
            .unwrap_or_default()
            .into_iter()
            .map(|account| SsoAccount {
                account_id: account.account_id,
                account_name: account.account_name.unwrap_or_default(),
                email_address: account.email_address,
            })
            .collect();
        Ok(DiscoveredAccounts::new(accounts))
    }

    /// §4.2: the roles this identity may assume in `account_id`.
    pub async fn roles(
        &self,
        access_token: &str,
        account_id: &str,
    ) -> Result<DiscoveredRoles, SsoError> {
        let url = format!(
            "{}/assignment/accounts/{account_id}/roles",
            self.client.endpoints().portal
        );
        let (status, body) = self
            .client
            .send_with_retry(&url, |url| {
                self.client.http.get(url).bearer_auth(access_token)
            })
            .await?;
        if status != 200 {
            return Err(self.refused(
                status,
                &format!("list the roles in account {account_id}"),
            ));
        }
        let wire: RolesWire = serde_json::from_str(&body)
            .map_err(|_| self.client.unavailable("the roles list answered an unexpected shape"))?;
        let roles = wire
            .role_list
            .unwrap_or_default()
            .into_iter()
            .map(|role| SsoRole {
                role_name: role.role_name,
                account_id: role.account_id.unwrap_or_else(|| account_id.to_string()),
            })
            .collect();
        Ok(DiscoveredRoles::new(roles))
    }

    /// §4.3: exchange the session for the role's credentials.
    pub async fn role_credentials(
        &self,
        access_token: &str,
        account_id: &str,
        role_name: &str,
    ) -> Result<RoleCredentials, SsoError> {
        let url = format!(
            "{}/federation/credentials",
            self.client.endpoints().portal
        );
        let (status, body) = self
            .client
            .send_with_retry(&url, |url| {
                self.client
                    .http
                    .get(url)
                    .query(&[("account_id", account_id), ("role_name", role_name)])
                    .bearer_auth(access_token)
            })
            .await?;
        if status == 403 || status == 404 {
            tracing::warn!(
                region = %self.client.region(),
                status,
                account_id,
                role_name,
                "the identity is not assigned this role"
            );
            return Err(SsoError::RoleNotAssigned {
                account_id: account_id.to_string(),
                role_name: role_name.to_string(),
            });
        }
        if status != 200 {
            return Err(self.refused(
                status,
                &format!("read the credentials of role {role_name}"),
            ));
        }
        let wire: CredentialsWire = serde_json::from_str(&body).map_err(|_| {
            self.client
                .unavailable("the role credentials answered an unexpected shape")
        })?;
        let expires_at = crate::time::from_millis_opt(wire.role_credentials.expiration)
            .ok_or_else(|| {
                self.client
                    .unavailable("the role credentials carried an unreadable expiry")
            })?;
        Ok(RoleCredentials {
            access_key_id: wire.role_credentials.access_key_id,
            secret_access_key: wire.role_credentials.secret_access_key,
            session_token: wire.role_credentials.session_token,
            expires_at,
        })
    }

    /// §4's shared failure table: an expired token says so, a missing permission names the action,
    /// throttling keeps its own cause, and anything else is `Unavailable`.
    fn refused(&self, status: u16, action: &str) -> SsoError {
        tracing::warn!(
            region = %self.client.region(),
            status,
            action,
            "the sso portal refused the request"
        );
        match status {
            401 => SsoError::SessionExpired,
            403 => SsoError::PermissionMissing {
                action: action.to_string(),
            },
            429 => SsoError::Throttled,
            other => self
                .client
                .unavailable(format!("the SSO portal answered HTTP {other}")),
        }
    }
}

// ---------------------------------------------------------------- wire shapes

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountsWire {
    #[serde(default)]
    account_list: Option<Vec<AccountWire>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountWire {
    account_id: String,
    #[serde(default)]
    account_name: Option<String>,
    #[serde(default)]
    email_address: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RolesWire {
    #[serde(default)]
    role_list: Option<Vec<RoleWire>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleWire {
    role_name: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsWire {
    role_credentials: CredentialsBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsBody {
    access_key_id: String,
    secret_access_key: String,
    session_token: String,
    /// Epoch milliseconds (§4).
    expiration: i64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm::sso::tests_support::{Reply, Stub};

    fn portal(base: &str) -> SsoPortal {
        SsoPortal::new(SsoClient::at(reqwest::Client::new(), base))
    }

    #[tokio::test]
    async fn the_portal_lists_what_the_identity_was_assigned() {
        let stub = Stub::start_portal(
            Reply::ok(json!({"accountList": [
                {"accountId": "123456789012", "accountName": "Acme", "emailAddress": "a@acme.test"},
                {"accountId": "210987654321", "accountName": "Acme Lab"}
            ]})),
            Reply::ok(json!({"roleList": [
                {"roleName": "BedrockOperator", "accountId": "123456789012"}
            ]})),
            Reply::ok(json!({"roleCredentials": {
                "accessKeyId": "ASIAEXAMPLEKEYID",
                "secretAccessKey": "secret-access-value",
                "sessionToken": "session-token-value",
                "expiration": 4102444800000_i64
            }})),
        )
        .await;
        let portal = portal(stub.base());

        let accounts = portal.accounts("access-token-value").await.unwrap();
        assert_eq!(accounts.accounts.len(), 2);
        assert_eq!(accounts.accounts[1].account_name, "Acme Lab");
        assert!(!accounts.free_text);

        let roles = portal
            .roles("access-token-value", "123456789012")
            .await
            .unwrap();
        assert_eq!(roles.roles[0].role_name, "BedrockOperator");
        assert!(!roles.free_text);

        let credentials = portal
            .role_credentials("access-token-value", "123456789012", "BedrockOperator")
            .await
            .unwrap();
        assert_eq!(credentials.access_key_id, "ASIAEXAMPLEKEYID");
        assert_eq!(credentials.expires_at.timestamp(), 4102444800);

        // §4's three paths, and the query the credentials call carries.
        let seen = stub.seen();
        assert!(seen.iter().any(|(path, _)| path == "/assignment/accounts"));
        assert!(seen
            .iter()
            .any(|(path, _)| path == "/assignment/accounts/123456789012/roles"));
        assert!(seen.iter().any(|(path, _)| path
            == "/federation/credentials"));
    }

    #[tokio::test]
    async fn an_empty_list_is_the_only_case_that_allows_a_typed_value() {
        let empty = Stub::start_portal(
            Reply::ok(json!({"accountList": []})),
            Reply::ok(json!({"roleList": []})),
            Reply::ok(json!({})),
        )
        .await;
        let empty_portal = portal(empty.base());

        let accounts = empty_portal.accounts("access-token-value").await.unwrap();
        assert!(accounts.accounts.is_empty());
        assert!(accounts.free_text, "an empty list is the free-text case");

        let roles = empty_portal
            .roles("access-token-value", "123456789012")
            .await
            .unwrap();
        assert!(roles.free_text);

        // A non-empty list is the authority, even when it holds one entry.
        let listed = Stub::start_portal(
            Reply::ok(json!({"accountList": [{"accountId": "123456789012"}]})),
            Reply::ok(json!({"roleList": [{"roleName": "BedrockOperator"}]})),
            Reply::ok(json!({})),
        )
        .await;
        let listed_portal = portal(listed.base());

        let accounts = listed_portal.accounts("access-token-value").await.unwrap();
        assert!(!accounts.free_text);
        let roles = listed_portal
            .roles("access-token-value", "123456789012")
            .await
            .unwrap();
        assert!(!roles.free_text);
    }

    #[tokio::test]
    async fn an_expired_token_and_a_missing_permission_keep_their_own_causes() {
        let stub = Stub::start_portal(
            Reply::refusing(401),
            Reply::refusing(403),
            Reply::refusing(404),
        )
        .await;
        let portal = portal(stub.base());

        assert_eq!(
            portal.accounts("access-token-value").await.unwrap_err(),
            SsoError::SessionExpired
        );
        assert!(matches!(
            portal
                .roles("access-token-value", "123456789012")
                .await
                .unwrap_err(),
            SsoError::PermissionMissing { .. }
        ));
        assert_eq!(
            portal
                .role_credentials("access-token-value", "123456789012", "BedrockOperator")
                .await
                .unwrap_err(),
            SsoError::RoleNotAssigned {
                account_id: "123456789012".into(),
                role_name: "BedrockOperator".into(),
            }
        );
    }

    #[test]
    fn role_credentials_can_never_be_printed() {
        let credentials = RoleCredentials {
            access_key_id: "ASIAEXAMPLEKEYID".into(),
            secret_access_key: "secret-access-value".into(),
            session_token: "session-token-value".into(),
            expires_at: crate::time::now(),
        };
        let printed = format!("{credentials:?}");
        for poison in [
            "ASIAEXAMPLEKEYID",
            "secret-access-value",
            "session-token-value",
        ] {
            assert!(!printed.contains(poison), "{printed}");
        }
    }
}
