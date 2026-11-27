# Research: Platform Account Linking

What the code does today, and the decisions this plan takes. Each decision: what, why, what else was weighed.

## Today

- `UserProfile` (`storage/user.rs`) holds `discord_id`, `discord_username`, `telegram_id`, `telegram_username`, `alias`, as one JSON blob in `user_profile.data`. They are typed by hand on `settings-profile.tsx` through `PUT /auth/users/me/profile` (`service/profile.rs`). Nothing checks them.
- The only reader is `system_prompt/user.rs::owner_md`, which prints them into OWNER.md. The agent then compares them with the sender text by eye.
- Every platform request carries a free-text `user`: `@{name} (DiscordId: {id})` (Discord: `user_label`, the interaction path at `discord/mod.rs:892`, the message path at `:1467`, `questions_handlers.rs:58/322`) or `@{name} (TelegramUser: {id})` (Telegram `mod.rs:486/990`, `questions_handlers.rs:23`).
- **Bug found**: Telegram's message path (`telegram/mod.rs:486`) puts `chat_id.0` in the label, not the sender's user id. In a group, every member reads as the group's id. This plan fixes it, since resolving a link needs the real `from.id`.
- **Identity keyed on the label**: `side_chat_limits` is keyed on that same label string (`channels/side_chat.rs:159`, `discord/mod.rs:764`). If unregistered senders all read `Unregistered User`, every stranger would share one rate-limit bucket. The key has to become the platform id.
- Mentions in message bodies are rendered `@alice (DiscordId: 2)` (`discord/mod.rs:1815`). They name people mentioned, not the sender, and are left as they are.
- `config::base_url` already exists (`--base-url`, `VIZIER_BASE_URL`, `vizier.base_url`; desktop: Desktop settings → `engine.rs:181`), checked by `base_url::normalize`, on `deps.config.base_url`. Its doc says nothing reads it yet. This is the first reader.
- Built-in bot commands are per platform `CommandEntry` lists (`discord::builtin_commands`, `telegram_rules().builtins`). `commands::build` already skips a skill whose name clashes with a built-in, so a built-in `connect` cannot be shadowed on a platform.
- Discord already sends ephemeral interaction replies (`MessageFlags::EPHEMERAL`, `discord/mod.rs:771`). Telegram knows `msg.chat.is_private()`.
- The WebUI login honours `?returnTo=` (`lib/returnTo.ts`, `routes/login.tsx`); `RequireAuth` redirects to a bare `/login`.
- `service/` has the HTTP rate limiters on `HTTPState` (`share_misses` and others, `utils::rate_limit::RateLimiter`).
- `ChannelPlatform::{Discord, Telegram}` (`schema`) is the platform type.

## Decisions

### R1. Links get their own table, not the profile blob

**Decision**: a new table `platform_link (platform, platform_user_id, user_id, platform_username, platform_display_name, linked_at)`, primary key `(platform, platform_user_id)`, `UNIQUE (user_id, platform)`, behind a new `PlatformLinkStorage` trait on `VizierStorageProvider`. Implemented once in `SqlStorage` (SQLite init + PostgreSQL migration `0018_platform_links.sql`).

**Why**: the sender lookup goes by platform id on every platform message. That needs an index, and the blob can't be queried. The two uniqueness rules (FR-007) are then enforced by the database in the redeem transaction.

**Alternatives**: keep the profile JSON and scan it (no index, no uniqueness); reuse `integration`-style tables (wrong owner: a link belongs to a person, not an agent).

### R2. Connect codes are stored hashed, in the database

**Decision**: table `platform_connect_code (platform, platform_user_id, code_hash, platform_username, platform_display_name, created_at_ms, expires_at_ms)`, primary key `(platform, platform_user_id)`. Issuing upserts the row, which voids any earlier code (FR-004). Redeeming deletes the row in the same transaction that writes the link, so a code works once. Only `sha256(code)` is kept. Expired rows are purged on issue and by the scheduler's daily retention tick, which already purges notifications.

**Code shape**: 8 characters from a 31-symbol alphabet without `0 O 1 I L` (about 40 bits), shown as `ABCD-EFGH`. Input is case-insensitive and ignores `-` and spaces. With 10 attempts per 10 minutes per person and per address (R5), guessing a live code is negligible.

**Why the database, not memory**: in Fleet the channel reader (channels role) issues the code and the gateway (gateway-http role) redeems it, possibly in different processes. A row also survives a restart within the 10 minutes.

**Alternatives**: a signed, stateless token (cannot be voided by a second `/connect` or made single-use without a table anyway); an in-memory map (breaks across roles and restarts).

### R3. The link carries the code in the fragment

**Decision**: the link is `{base_url}/connect#ABCD-EFGH`. The fragment never reaches the server, so the code stays out of access logs, proxies and `tracing` request spans. The `/connect` page (outside `RequireAuth`) reads the fragment, keeps the code in `sessionStorage`, and clears the fragment from the address bar. If signed out, it goes to `/login?returnTo=/connect` (code not in the URL). After sign-in it reads the code back and shows the confirmation. Typing the code into the profile leads to the same confirmation component.

**No `base_url`** → the reply has the code only (FR-002), plus where to enter it. This is not logged as a problem.

**Alternatives**: `?code=` (reaches server logs); a path segment (same).

### R4. Two calls: preview, then redeem

**Decision**:
- `POST /auth/users/me/connections/preview {code}` checks the code and returns the platform account and any account it would replace. It consumes nothing.
- `POST /auth/users/me/connections {code}` does it all in one transaction: checks the code, deletes it, refuses if the account is linked to someone else, replaces the caller's link for that platform, writes the link and an event.

Both share one "expired or already used" answer for unknown, expired and consumed codes (FR-008). `GET /auth/users/me/connections` lists links plus legacy unverified values. `DELETE /auth/users/me/connections/{platform}` unlinks. The service functions live in `service/platform_links.rs`, and the desktop reaches the same routes through the dispatcher.

**Why**: the confirmation (FR-006) must show what will be linked before anything changes. A preview that consumed the code would leave a reload or a cancel with a dead code.

### R5. Rate limits

**Decision**: two `RateLimiter`s on `HTTPState`, `connect_attempts_user` and `connect_attempts_addr`, 10 per 10 minutes each, applied to preview and redeem. A third on `VizierDependencies`, `connect_issues`, allows 5 `/connect` per 10 minutes per platform account, so a bot can't be used to spam code rows.

### R6. Sender resolution is one core function

**Decision**: `platform_links::sender::resolve(deps, PlatformPerson { platform, id, username, display_name }) -> PlatformSender`. It does one indexed read of `platform_link`, then the Vizier username from `user` (one join). It returns `{ label, verified, metadata }`:
- `label` is the Vizier username, or the constant `UNREGISTERED_USER = "Unregistered User"`.
- `verified` is `true` for a link, `false` otherwise.
- `metadata` is the `platform`, `platform_user_id`, `platform_username` and `platform_display_name` fields, merged into the request's existing metadata.

No cache, so a link or unlink applies on the very next message (FR-009, SC-005). On a storage error it answers unregistered and logs a warning, so a message is never dropped and never wrongly attributed.

Every place that builds a platform sender calls it: the Discord message and interaction paths, `user_label` callers, the question handlers on both platforms, the Telegram message and callback paths, and side chats. `user_label`, `question_answerer` and the inline `format!`s are removed. That is the single home Principle II asks for.

`last seen` username/display name is refreshed on the link row on resolve when it differs. That is a write only on change.

### R7. What the agent reads

**Decision**: `VizierRequest` gains `verified: Option<bool>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`, journaled like the rest). `generate_frontmatter` emits `verified` right after `sender` when it is `Some`, in every arm that already emits `sender: self.user`. Web and agent requests leave it `None`, so their frontmatter is byte-identical. Platform fields ride in `metadata` (FR-016). Old history rows have no `verified` and render as before (FR-019).

`owner_md` takes the owner's links (`list_links(owner_id)`) instead of the four profile fields. Its lines become `discord: <username> (id <id>), verified link`. Legacy hand-typed values are not printed (FR-017). `VizierAgent` loads them with the profile, at the same place (`agents/agent/mod.rs:200`).

BOOT.md gets one sentence: a `verified: true` sender is a Vizier user proven by link; `Unregistered User` is someone with no link, and `metadata` says who they are on the platform.

### R8. Rate-limit and identity keys move to the platform id

**Decision**: `side_chat_limits` and any other per-person key on platforms use `"{platform}:{platform_user_id}"`, not the label (see Today).

### R9. The `/connect` command

**Decision**: a `connect` `CommandEntry` in both built-in lists ("link this account to your Vizier user").
- **Discord**: handled in the interaction match beside `abort`/`btw`, always answered ephemerally, in a guild or a DM.
- **Telegram**: `is_command(&text, "connect", …)` before the chat path. In a private chat it replies there. In a group it replies "send /connect to me in a direct message" with a `t.me/<bot>` link and posts no code. The issuing and the reply text live in `platform_links::connect::issue(deps, person) -> ConnectReply`, and wording in `platform_links/text.rs`, so the two readers only send.
- **Typed `/connect` as plain Discord text** is not intercepted: Discord sends it as a slash command when the person picks it, and plain text in a guild could be read by others.

### R10. Legacy hand-typed values

**Decision**: `UserProfile` keeps its four fields, read-only, on the wire as `unverified: { discord_id, discord_username, telegram_id, telegram_username }` in the connections listing.
- `PUT /auth/users/me/profile` refuses non-null values for them with `422 connect_required`, and accepts `null` to clear them (FR-014 "removable").
- A successful link of a platform clears that platform's legacy pair in the same transaction.
- Nothing reads them for identity.

The refusal, the fields and the clearing are added to #82.

### R11. Deleting a person

**Decision**: `delete_user`'s existing transaction also deletes the person's `platform_link` rows and their events. Codes are not tied to a Vizier user until redeemed, so nothing else is needed (FR-013).

### R12. Audit

**Decision**: a `platform_link_event (id, user_id, platform, platform_user_id, action, at)` table, with `action` = `linked | replaced | unlinked`, written in the same transaction as each change. It is listed with the connections, as the person's own history (FR-010). No code is stored or logged, and `tracing` lines name the platform and the action only.

### R13. Testing offline

The Discord and Telegram readers can't run without a live platform. So:
- The pure parts are unit-tested: code generation and normalisation, `resolve`, the frontmatter, `owner_md`, the storage contract.
- The HTTP flow is tested with `vizier-http` tests (preview, redeem, refusals, rate limits, unlink, user deletion).
- The agent-observable change is checked end to end through dummyplug, with a new **§9 `sender`** command (in the `dummyplug` submodule, matching branch). It echoes the frontmatter of the newest user message, so a quickstart can show exactly what the agent read.
- A live Discord and Telegram pass is scripted in `quickstart.md` and marked live.
