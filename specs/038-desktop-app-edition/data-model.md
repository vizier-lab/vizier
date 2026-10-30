# Data Model: Vizier Desktop App Edition

**No SQLite tables, columns or migrations are added.** New state is either in memory, in the
app's own config file, or one generated key file in the workspace's `.runtime/`. Nothing is
added to #82 (no migration, shim or legacy type).

## Principal (core, `auth.rs`) — replaces `Identity` as the owned value

| Field | Type | Notes |
|---|---|---|
| `user_id` | `String` | a real `user` row |
| `username` | `String` | keys HTTP/desktop conversations: `VizierChannelId::HTTP(username, channel)` |
| `role` | `Role` | `is_system` bypasses permission checks |
| `permissions` | `Vec<String>` | resolved from the role **per request** |
| `method` | `AuthMethod` | `Jwt` \| `ApiKey` \| **`Desktop`** (new) |

`Identity` stays as a type alias for one release cycle inside the workspace only (it is in
the stable tier's `public-api.txt`; nothing is published yet, so the rename is free).
`access::Caller` borrows from `Principal` as it did from `Identity`.
`RevisionOrigin` records `Desktop` like the other methods.

**Rule**: a `Principal` with `method: Desktop` is only ever constructed by
`vizier_http::embedded::Dispatcher` (research R4).

## ServiceError (core, `service/error.rs`)

| Kind | HTTP status | Carries |
|---|---|---|
| `NotFound` | 404 | message |
| `Unauthorized` | 401 | message |
| `Forbidden` | 403 | message (today's `"Access denied"` etc., verbatim) |
| `Invalid` | 400 or 422 (field `status`) | message, optional `code` |
| `Conflict` | 409 | message, `code`, optional `data` (e.g. `stale_revision` + current revision) |
| `Gone` | 410 | `code` (e.g. `attach_removed`) |
| `TooLarge` | 413 | message |
| `RateLimited` | 429 | message |
| `Unavailable` | 503 | message (agent unavailable, provider down) |
| `Internal` | 500 | message |

`From<VizierError> for ServiceError` → `Internal`. Envelope mapping is in
[contracts/service-layer.md](contracts/service-layer.md).

## AuthMode (vizier-http, on `HTTPState`)

`Network` (default; JWT/API key, today's behaviour) | `Desktop { user_id }` (only from
`embedded::Dispatcher::new`).

## HTTPChannelConfig (core) — one new optional field

| Field | Type | Default | Notes |
|---|---|---|---|
| `cors_origins` | `Option<Vec<String>>` | `None` | `None` = today's `allow_origin(Any)` (server unchanged); the app passes the served origins |

## User (existing row) — one new reading of an existing column

`password_hash == ""` means **no password**: login refuses it before argon2 runs, and a
desktop principal may set a first password without a current one. Created only by the desktop
first-run flow. `has_password` is exposed on `GET /auth/users/me` (additive field).

## TopicConnection (vizier-http, `topic.rs`) — extracted from `handle_socket`

| Part | Meaning |
|---|---|
| `open(state, principal, session, limits, out: flume::Sender<Value>)` | Authorizes (view agent), registers the `ClientConnection`, subscribes to session events, delivers deferred client calls (claims via `DeferredClaims`) |
| `handle_inbound(Value)` | The frames the socket accepts today (chat, client tool answers/declarations, client state saves, client events, abort, …) |
| `close()` | Drops the subscription; a pending client call follows today's drop rules (deferred or cancelled) |

**Lifecycle**: `Open → Closed`. Idle timeout stays with the WebSocket pump; the app has no idle
timeout (the window owns the lifetime).

## Desktop app config (`{app_config_dir}/desktop.json`)

| Field | Type | Default | Notes |
|---|---|---|---|
| `workspace` | path | `{app_data_dir}/workspace` | the folder the engine opens |
| `bindings` | map canonical workspace path → `user_id` | `{}` | which user the app signs in as, per workspace |
| `close_to_tray` | bool | `true` | close hides the window and keeps the engine running |
| `web_access.enabled` | bool | `false` | FR-017 |
| `web_access.allow_network` | bool | `false` | bind `0.0.0.0` instead of `127.0.0.1` (FR-019) |
| `web_access.port` | u16 | `9999` | |
| `updates.check_on_start` | bool | `true` | |

Written atomically (temp file + rename). Unknown fields are kept on rewrite.

`{app_data_dir}` is the platform app-data folder for identifier `rs.vizier.app`
(`~/.local/share/rs.vizier.app`, `~/Library/Application Support/rs.vizier.app`).

## Workspace files added by the app

| File | Created when | Notes |
|---|---|---|
| `{workspace}/.runtime/web-access.key` | web access first enabled | 32 random bytes; the JWT secret for the app's listener; mode 0600 |

The engine's own `{workspace}/.runtime/vizier.lock` and `encryption.key` behave as in 037/031.

## Desktop app state (in memory, Rust shell)

```text
Starting ──► Locked{holder}           (workspace held by another vizier: FR-003)
   │
   ├──► NeedsOwner                     (no users)            ──setup──► Ready
   ├──► ChooseOwner{candidates}        (several system users) ──choose─► Ready
   ├──► Ready{user}
   └──► Failed{error}                  (storage/key/migration error)
Ready ──SubsystemStopped──► Degraded{stopped} ──restart──► Starting
```

`desktop_state` returns this to the WebUI; changes are pushed as a `desktop://state` event.

## Request envelope over IPC

The `api` command returns `{status: u16, body: <the server's JSON envelope>}` so the WebUI's
`ApiError` logic is shared byte for byte. See [contracts/ipc-protocol.md](contracts/ipc-protocol.md).
