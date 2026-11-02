# Contract: Bedrock SSO wire behaviour

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](../plan.md) | **Research**: [research.md](../research.md) §D4, D5, D11

What Vizier sends to AWS and what it does with the answers. Implemented entirely over the existing `reqwest` client (`VizierDependencies.http`) — no `aws-sdk-*`, no CLI, no file under `~/.aws`.

---

## §1 Endpoints, derived from the **SSO region**

| Purpose | Method and URL |
|---|---|
| client registration (once per profile) | `POST https://oidc.{sso_region}.amazonaws.com/client/register` |
| device authorization | `POST https://oidc.{sso_region}.amazonaws.com/device_authorization` |
| token polling | `POST https://oidc.{sso_region}.amazonaws.com/token` |
| assigned accounts | `GET https://portal.sso.{sso_region}.amazonaws.com/assignment/accounts` |
| roles in an account | `GET https://portal.sso.{sso_region}.amazonaws.com/assignment/accounts/{account_id}/roles` |
| role credentials | `GET https://portal.sso.{sso_region}.amazonaws.com/federation/credentials?account_id={id}&role_name={name}` |
| Bedrock runtime (unchanged) | `https://bedrock-runtime.{bedrock_region}.amazonaws.com/model/{encoded}/converse` |

`{sso_region}` is the connection's `sso_region`; `{bedrock_region}` is the connection's `region`. **They are never interchangeable** (FR-010); every message about a region says which one it means.

---

## §2 Client registration

```http
POST /client/register
Content-Type: application/json

{
  "clientName": "vizier",
  "clientType": "public",
  "scopes": ["sso:account:access"]
}
```
Response used: `clientId`, `clientSecret`, `clientSecretExpiresAt`.

- Stored in the session envelope (secret, D8), keyed by profile.
- Reused on every re-sign-in for that profile; re-registered only when `clientSecretExpiresAt` has passed.
- Failure here is `SsoError::Unavailable` (the region is wrong, or the endpoint is unreachable), with the region named.

---

## §3 Device authorization and polling

```http
POST /device_authorization
Content-Type: application/x-www-form-urlencoded

client_id=<clientId>&client_secret=<clientSecret>&start_url=<start_url>
```
Response used: `deviceCode`, `userCode`, `verificationUri`, `verificationUriComplete`, `expiresIn`, `interval`.

Vizier returns to the caller **only** `user_code`, `verification_uri(_complete)` and `expires_at`; `device_code` stays in the process (FR-009b, FR-035).

Polling:
```http
POST /token
Content-Type: application/x-www-form-urlencoded

client_id=…&client_secret=…&grant_type=urn:ietf:params:oauth:grant-type:device_code&device_code=…
```

| Response | Meaning | Action |
|---|---|---|
| `200 { accessToken, refreshToken?, expiresIn }` | approved | store the session, continue to §4 |
| `400 { error: "authorization_pending" }` | not yet | wait `interval`, poll again |
| `400 { error: "slow_down" }` | polling too fast | `interval += 5`, poll again |
| `400 { error: "access_denied" }` | the person refused | `SsoError::SignInDenied` |
| `400 { error: "expired_token" }` | the code died | `SsoError::CodeExpired` |
| `5xx` / transport | AWS or the network | one retry, then `SsoError::Unavailable` |

The poll loop is bounded by `expiresIn` minus a 5-second guard, and by an overall 10-minute wall clock; both produce `CodeExpired` rather than hanging (FR-006, SC-002).

---

## §4 Accounts, roles, credentials

```http
GET /assignment/accounts
Authorization: Bearer <accessToken>
→ { "accountList": [ { "accountId": "123456789012", "accountName": "…", "emailAddress": "…" } ] }

GET /assignment/accounts/{accountId}/roles
→ { "roleList": [ { "roleName": "BedrockOperator", "accountId": "123456789012" } ] }

GET /federation/credentials?account_id=…&role_name=…
→ { "roleCredentials": { "accessKeyId": "ASIA…", "secretAccessKey": "…", "sessionToken": "…", "expiration": 1730000000000 } }
```

- §4.1/§4.2 populate the pickers; a free-text fallback exists only when the portal returns an empty list (FR-007).
- §4.3's three values become `RoleCredentials` in memory, never written (FR-009d).
- The `Bearer` token is the session's `accessToken`; an expired one is refreshed with the `refreshToken` through the same `/token` endpoint (`grant_type=refresh_token`) before §4 is attempted.

---

## §5 Renewal, cache and invalidation

| Situation | Behaviour |
|---|---|
| cached `RoleCredentials.expires_at - now > 5 min` | use them; **zero** HTTP calls (FR-013) |
| within 5 min of expiry | one renewal, under the per-connection mutex (FR-014) |
| the access token itself expired | one refresh-token call, then `GetRoleCredentials` |
| refresh token rejected / no refresh token | `SsoError::SessionExpired` (FR-016) |
| an invoke answered `401`/`ExpiredToken` | `CredentialSource::invalidate()` clears the cache, the existing `complete_once` retry (`crates/vizier-core/src/llm/model.rs:141-148`) renews once and retries once; a second failure ends the turn (FR-015) |
| session row unreadable | `SsoError::SessionUnreadable` before any AWS call (FR-009f) |

Renewal writes the refreshed session back with compare-and-swap on `revision` (§2 of [data-model.md](../data-model.md)); a lost race re-reads and uses the winner's session instead of overwriting it.

---

## §6 Inference-profile ARNs

Grammar, exactly — AWS's own pattern for an inference profile, both resource types:

```text
arn:{partition}:bedrock:{region}:{account}:{resource}/{profile-id}
resource := inference-profile | application-inference-profile
```

| Segment | Rule | Failure message |
|---|---|---|
| literal prefix | `arn:` | `an inference profile ARN starts with "arn:"` |
| `partition` | `aws`, `aws-cn`, `aws-us-gov`, `aws-iso`, `aws-iso-b` | `unsupported ARN partition "…": expected aws, aws-cn, aws-us-gov, aws-iso or aws-iso-b` |
| `service` | `bedrock` | `"…" is not a Bedrock ARN: the service segment must be "bedrock"` |
| `region` | `valid_region` (`crates/vizier-core/src/llm/bedrock.rs:67`) — already covers `us-iso-east-1` and `us-isob-east-1` | `"…" is not an AWS region` |
| `account` | 12 digits | `the ARN's account segment must be 12 digits` |
| `resource` | **`inference-profile` or `application-inference-profile`**, compared as a whole segment | `an inference profile ARN names "inference-profile" or "application-inference-profile", not "…"` |
| `profile-id` | non-empty, `[A-Za-z0-9.:-]+`, no `/`, no whitespace | `the ARN names no profile id` |

**Both forms are accepted on equal footing** (FR-023, SC-008): this feature exists for the **application** form, and the system form is what a person who does not use application profiles already saves. Neither is refused for being the other, and neither is rewritten — the ARN is sent exactly as saved (FR-027).

```
arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123def456   # accepted
arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-sonnet-4-5-20250929-v1:0   # accepted
arn:aws-us-gov:bedrock:us-gov-west-1:123456789012:application-inference-profile/x1:0   # accepted
```

Whitespace: `trim()` once, then parse; a value whose trimmed form differs from the input is **accepted** (a table copy), but a value containing an internal space fails the segment rules. A console URL fails the literal prefix; a foundation-model ARN fails the `resource` rule. Nothing is stored when any rule fails (FR-023, SC-008).

**The wider grammar does not widen what may use an ARN.** Both resource types are chat resources; neither may be offered or accepted for embedding or image generation (`contracts/media-and-test.md` §2, FR-033a).

Region comparison (FR-024): `arn.region == credentials.region`. Checked at save **and** at first use, because an entry can outlive a region change; the message names both values. The resource type takes no part in it, and **the pricing pairing takes no part in it either** (FR-034a.6, SC-022): picking a model never changes whether an ARN is accepted, and an ARN in one region paired with a model of another family is accepted, because the pairing prices and does not judge.

Sent to the runtime **unchanged**: `llm::bedrock::model_path(arn, "converse")` percent-encodes it into one path segment (`crates/vizier-core/src/llm/bedrock.rs:19-36`), so `:` and `/` in the ARN are encoded rather than reinterpreted (FR-027).

## §7 What is never sent back up

| Value | Where it may exist | Where it must not appear |
|---|---|---|
| `device_code` | process memory during a pending sign-in | any API response, log, span |
| `access_token`, `refresh_token` | session envelope + memory | same |
| `client_secret` | session envelope + memory | same |
| `accessKeyId`/`secretAccessKey`/`sessionToken` (role) | memory only | same, and no file |
| session JSON | encrypted blob in SQLite | plaintext anywhere |

Verified by quickstart §4 (grep the workspace DB, the log, and every response body).
