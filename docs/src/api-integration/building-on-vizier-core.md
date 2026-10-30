# 3.5 Building on vizier-core

`vizier-core` is the engine as a library. The server binary and the [desktop app](../getting-started/desktop-app.md) are both hosts of it; you can write a third. A runnable starting point is `crates/vizier-core/examples/headless.rs`.

## Start an engine

```rust
use vizier_core::prelude::*;

let config = VizierConfig::with_workspace("/path/to/workspace".into());
let runtime = VizierRuntime::start(RuntimeOptions { config }).await?;
```

You name the workspace explicitly. The engine never reads `$HOME` or environment variables to decide where data lives, never exits your process, and takes `{workspace}/.runtime/vizier.lock` so a second engine on the same folder is refused. A stopped subsystem is reported through `runtime.events()`, `runtime.health()` and `wait`.

## Services

`runtime.services()` returns `Services`, the application-service layer: the operations a person can ask of the engine, with their authorization. Every adapter (the HTTP routes, the desktop IPC) goes through the same rules.

```rust
let services = runtime.services();
// Resolve the caller's current role and permissions; they are re-read on each call.
let me = services.principal_for(&user_id, AuthMethod::ApiKey).await?;
```

A `Principal` carries `user_id`, `username`, `role`, `permissions` and the `AuthMethod` (`Jwt`, `ApiKey`, `Desktop`). `AuthMethod::Desktop` can only be produced by the in-process dispatcher in `vizier-http`; `principal_for` refuses to mint it.

Access checks live in `vizier_core::access`: `can_view_agent`, `can_edit_agent`, `can_show_platform_chats` and `require(&principal, permission)`. System roles bypass them.

## Errors

Operations return `ServiceError`:

| Kind | Meaning | HTTP status |
|---|---|---|
| `NotFound` | no such object, or not visible to you | 404 |
| `Unauthorized` | not authenticated | 401 |
| `Forbidden` | authenticated, not allowed | 403 |
| `Invalid { status, message, code }` | bad input or refused request | 400 / 422 |
| `Conflict { message, code, data }` | state conflict | 409 |
| `Gone { code, message }` | removed on purpose | 410 |
| `TooLarge`, `RateLimited`, `Unavailable`, `Internal` | | 413, 429, 503, 500 |

`status()`, `message()` and `code()` give an adapter what it needs to render one in its own format. `From<VizierError>` turns an internal failure into `Internal`.

## Events

- `runtime.events()` — lifecycle: `Started`, subsystem stopped.
- `runtime.session_events()` — everything pushed to conversations: turn frames, background job changes, reactions.
- `runtime.transport()` — send requests to an agent and receive its frames.

## Embedding the HTTP surface

`vizier_http::router(&runtime, &config)` gives you the full REST/WebSocket server as an axum `Router`; `vizier_http::serve` binds it. The desktop app uses `vizier_http::embedded::Dispatcher`, which dispatches requests in-process as one signed-in person with no socket.

> The service layer is being filled in domain by domain (spec 038). Until a domain moves, its operations are still reachable through the HTTP routes.
