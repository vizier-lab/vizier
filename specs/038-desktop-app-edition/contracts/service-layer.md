# Contract: `vizier_core::service` (phase 5)

The engine's application-service surface: what both adapters (`vizier-http` and, through it,
the app) and third-party UIs call. Part of the **stable tier** once ported; recorded in
`crates/vizier-core/public-api.txt`.

## Shape

```rust
pub struct Services { /* Arc'd VizierDependencies */ }

impl VizierRuntime {
    pub fn services(&self) -> Services;
}

// one module per domain; every operation looks like this
pub mod service::core {
    pub async fn get(s: &Services, who: &Principal, agent_id: &str)
        -> Result<CoreContent, ServiceError>;
    pub async fn update(s: &Services, who: &Principal, agent_id: &str, input: UpdateCore)
        -> Result<CoreUpdated, ServiceError>;
    // ...
}
```

Rules every operation follows:

1. **Authorize first**, through `service::access` (moved from `vizier-http`):
   `require(who, permission)` (today's `require_permission` layer), `can_view_agent`,
   `can_edit_agent`, `can_show_platform_chats`, and the existing `Caller::manages` for
   integrations/secrets/artifacts. A refused check returns the same message the handler
   returns today.
2. Inputs and outputs are the types the handlers already (de)serialize, moved into
   `vizier_core::schema` where they are not there yet (with `utoipa` derives behind the
   existing `cfg_attr(feature = "utoipa")`).
3. No `axum`, `http` or status codes in core. `ServiceError` is the only error returned.
4. No behaviour change: a handler port is accepted only if the conformance vectors for that
   route group are byte-identical before and after.

## Domains (port order)

| # | Module | Routes today | Notes |
|---|---|---|---|
| 1 | `topics` (history, files, list/detail/delete, promote) | `agents/channel.rs` (8) | with `TopicConnection` (research R5) |
| 2 | `agents` (list, detail, CRUD, health, ping, restart, chat, btw, sharing, usage) | `agents/mod.rs` (14) | `can_view_agent`/`can_edit_agent` move here |
| 3 | `core`, `memory`, `documents` | 4 + 11 + 6 | |
| 4 | `tasks`, `dream`, `jobs`, `conversations` | 7 + 6 + 4 + 1 | |
| 5 | `artifacts`, `secrets`, `integrations` (agent + global), `skills` (agent + global) | 5 + 4 + 7 + 5 + 3 + global | already call core `access` |
| 6 | `users`, `roles`, `api_keys`, `profile`, `setup`, `password` | `auth.rs` (25 routes incl. layers) | argon2 hashing moves to `auth::password`; JWT issue/verify stays in `vizier-http` |
| 7 | `providers`, `custom_providers`, `catalogue`, `usage`, `embedding` | 5 + 6 + 8 + usage + 1 | `settings:providers` layer becomes `require` |
| 8 | `notifications`, `overview`, `files`, `setup_state` | 4 + 2 + 2 + 1 | |

`vizier-http` keeps only: extraction (path/query/JSON/multipart/WebSocket), JWT/API-key
authentication, `ServiceError` → response, OpenAPI, CORS, the embedded WebUI and icons.
When the last domain lands, the `#[doc(hidden)]` adapter re-exports that `vizier-http` no
longer uses are made `pub(crate)`.

## `ServiceError` → HTTP envelope

```rust
impl IntoResponse for ServiceError  // in vizier-http
```

| Kind | Status | Body |
|---|---|---|
| `NotFound(msg)` | 404 | `{status, message: msg, data: null}` |
| `Unauthorized(msg)` | 401 | same shape |
| `Forbidden(msg)` | 403 | same shape |
| `Invalid { status, msg, code }` | 400 / 422 | `{status, message, data: null, code?}` (as today's `422 use_grants_endpoint`) |
| `Conflict { msg, code, data }` | 409 | `{status, message, code, data}` (as today's `409 stale_revision`) |
| `Gone { code, msg }` | 410 | `{status, message, code}` |
| `TooLarge(msg)` | 413 | |
| `RateLimited(msg)` | 429 | |
| `Unavailable(msg)` | 503 | |
| `Internal(msg)` | 500 | |

The exact placement of `code` (top level vs `data.code`) is preserved per route, since the
WebUI reads both; the conformance suite pins it.

## Third-party use (documented in `docs/src/`)

```rust
let runtime = VizierRuntime::start(RuntimeOptions { config }).await?;
let s = runtime.services();
let me = s.principal_for(user_id, AuthMethod::ApiKey).await?; // resolves role + permissions
let agents = service::agents::list(&s, &me, ListQuery::default()).await?;
```

`principal_for` is the only way for an embedder to make a `Principal`; it never accepts
`AuthMethod::Desktop`, which only `vizier_http::embedded::Dispatcher` mints.
