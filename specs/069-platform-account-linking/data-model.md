# Data Model: Platform Account Linking

## Tables

### `platform_link`: a proven link

| Column | Type | Notes |
|---|---|---|
| `platform` | text | `discord` \| `telegram` (`ChannelPlatform`) |
| `platform_user_id` | text | the platform's user id (Discord snowflake, Telegram `from.id`), as text |
| `user_id` | text | Vizier user, `REFERENCES "user"(user_id) ON DELETE CASCADE` |
| `platform_username` | text null | handle (`rin_dev`); Telegram may have none |
| `platform_display_name` | text null | display/global name (`Rin`) |
| `linked_at` | text | `time::stored` (UTC, sortable) |

- `PRIMARY KEY (platform, platform_user_id)` means one Vizier user per platform account (FR-007).
- `UNIQUE (user_id, platform)` means one account per platform per user (FR-007).
- Lookup on every platform message uses the primary key.
- Username and display name are refreshed when a message shows new ones. They are for display only; identity is `platform_user_id` alone.

### `platform_connect_code`: an issued, unredeemed code

| Column | Type | Notes |
|---|---|---|
| `platform` | text | |
| `platform_user_id` | text | |
| `code_hash` | text | hex `sha256(normalised code)`; `UNIQUE` |
| `platform_username` | text null | as seen when `/connect` was sent |
| `platform_display_name` | text null | |
| `created_at_ms` | integer | |
| `expires_at_ms` | integer | `created_at_ms + 600_000` |

- `PRIMARY KEY (platform, platform_user_id)`, so a new `/connect` replaces the old code (FR-004).
- Redeem deletes the row in the link transaction, so a code works once.
- Expired rows are purged on issue (that account's row) and by the daily retention tick.

### `platform_link_event`: history of changes

| Column | Type | Notes |
|---|---|---|
| `id` | text | uuid |
| `user_id` | text | `ON DELETE CASCADE` |
| `platform` | text | |
| `platform_user_id` | text | |
| `action` | text | `linked` \| `replaced` \| `unlinked` |
| `at` | text | `time::stored` |

## Types (core)

```rust
pub const UNREGISTERED_USER: &str = "Unregistered User";

/// Who a platform says sent something. Built by the reader from the native event.
pub struct PlatformPerson {
    pub platform: ChannelPlatform,
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
}

/// What a request is stamped with.
pub struct PlatformSender {
    pub label: String,          // Vizier username, or UNREGISTERED_USER
    pub verified: bool,
    pub person: PlatformPerson, // rendered into metadata
}

pub struct PlatformLink { platform, platform_user_id, user_id, username: String /* Vizier */,
                          platform_username, platform_display_name, linked_at: Timestamp }

pub enum ConnectReply {
    Code { code: String, link: Option<String>, expires_in_minutes: u32 },
    AskInPrivate { bot_link: Option<String> },  // Telegram group
    TooMany { retry_after_secs: u64 },
}

pub struct ConnectPreview { link: PlatformPersonView, replaces: Option<PlatformPersonView>,
                            vizier_username: String }
```

`VizierRequest` gains `verified: Option<bool>` (serde default `None`, skipped when `None`). `metadata` gains `platform`, `platform_user_id`, `platform_username`, `platform_display_name` on every platform request.

## Storage trait

```rust
#[async_trait]
pub trait PlatformLinkStorage {
    async fn issue_connect_code(&self, person: &PlatformPerson, code_hash: &str, now_ms: i64, expires_ms: i64) -> Result<()>;
    async fn peek_connect_code(&self, code_hash: &str, now_ms: i64) -> Result<Option<PlatformPerson>>;
    /// One transaction: take the code, refuse if linked to another user, replace the
    /// caller's link for that platform, clear legacy profile values for it, write the event.
    async fn redeem_connect_code(&self, code_hash: &str, user_id: &str, now_ms: i64) -> Result<RedeemOutcome>;
    async fn find_link(&self, platform: ChannelPlatform, platform_user_id: &str) -> Result<Option<PlatformLink>>;
    async fn list_links(&self, user_id: &str) -> Result<Vec<PlatformLink>>;
    async fn unlink(&self, user_id: &str, platform: ChannelPlatform) -> Result<bool>;
    async fn touch_link_names(&self, person: &PlatformPerson) -> Result<()>;
    async fn list_link_events(&self, user_id: &str, limit: u32) -> Result<Vec<PlatformLinkEvent>>;
    async fn purge_expired_connect_codes(&self, now_ms: i64) -> Result<u64>;
}

pub enum RedeemOutcome { Linked(PlatformLink), Replaced { link: PlatformLink, previous: PlatformLink },
                         Unavailable /* unknown, expired or used */, LinkedElsewhere }
```

## Rules

| Rule | Where |
|---|---|
| A code is 8 symbols from `23456789ABCDEFGHJKMNPQRSTUVWXYZ`, shown `XXXX-XXXX`; input is upper-cased with `-` and whitespace removed | `platform_links::code` |
| Only `sha256` of the normalised code is stored; no code in logs, events, URLs the server sees, or errors | `code`, `connect`, `service` |
| Unknown, expired and used codes give the same `unavailable` answer | `service::platform_links` |
| Redeem refuses `linked_elsewhere` without naming the other user | storage tx |
| Link and unlink are visible to the next `resolve` (no cache) | `sender` |
| `resolve` failure means unregistered, plus a warning; never drops the message | `sender` |
| Legacy profile values identify no one; new non-null values are refused (`422 connect_required`); `null` clears | `service::profile` (#82) |
| Deleting a user deletes their links and events | `delete_user` tx (and FK cascade) |

## State: a connect code

```text
(none) --/connect--> issued --redeem--> (deleted; link written)
   ^                  |  \--expires (10 min) / new /connect--> voided (deleted or replaced)
   \------------------/
```
