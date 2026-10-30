# Data Model: Extract vizier-core and split the workspace

This feature adds **no storage tables, columns or migrations**. The "entities" are the
in-memory types that form the new boundaries. Persisted data, config schema and API payloads
are unchanged (FR-018).

## VizierRuntime (new, `vizier-core`)

The handle a host holds for one running engine on one workspace.

| Field / method | Meaning |
|---|---|
| `start(RuntimeOptions) -> Result<VizierRuntime>` | Takes the workspace lock (R14), opens storage, runs startup migrations, registers agents, recovers background jobs, spawns the core subsystems on the current tokio runtime. |
| `events() -> broadcast::Receiver<RuntimeEvent>` | Lifecycle events. |
| `session_events() -> broadcast::Receiver<SessionEvent>` | The existing per-session frames (`VizierTransport::subscribe_session_events`). |
| `health() -> HealthReport` | Subsystems' state plus the existing per-agent health. |
| `wait(&mut self) -> RuntimeExit` | Resolves when exit is requested or a subsystem stops. |
| `shutdown(self) -> Result<()>` | Aborts subsystems the way the requested-exit path does today (turns aborted and resumable, per spec 023), releases the lock. |
| `transport() -> &VizierTransport` | Send requests and agent commands; stable tier (research R3). |
| `deps() -> &VizierDependencies` | **Transitional** (`#[doc(hidden)]`), for `vizier-http` and the binary until phase 5. |

**Lifecycle**: `Starting → Running → (Stopping) → Stopped`. A subsystem stop moves to
`Degraded { stopped: Vec<SubsystemName> }` and emits `SubsystemStopped`; the runtime does not
restart subsystems itself (unchanged from today, where the process is restarted whole).

**Invariants**: at most one `VizierRuntime` per workspace across processes (lock); dropping
without `shutdown` aborts subsystems (same as `shutdown`, minus awaiting).

## RuntimeOptions (new)

| Field | Type | Rule |
|---|---|---|
| `config` | `VizierConfig` | Must carry an explicit, non-empty `workspace`; core never derives it from env/home (FR-005). |
| `workers` | — | Not here: the host owns its tokio runtime. |

## RuntimeEvent (new, `#[non_exhaustive]`)

| Variant | Emitted when |
|---|---|
| `Started` | All core subsystems spawned. |
| `SubsystemStopped { name: &'static str, outcome: String }` | A subsystem task returned, errored or panicked. |
| `ExitRequested` | The transport's existing exit signal fired (e.g. `vizier shutdown`). |

## RuntimeExit (new)

`Requested` | `SubsystemStopped { name, outcome }`. The binary maps them to exit codes 0 / 1.

## HealthReport (new)

`subsystems: Vec<(name, SubsystemState)>` with `SubsystemState = Running | Stopped { outcome }`,
plus `agents` = today's `AgentHealthStatus` list (incl. `channels`). Read-only snapshot.

## Identity (moved + renamed from `channels::http::auth::AuthenticatedUser`, `vizier_core::auth`)

| Field | Type |
|---|---|
| `user_id` | `String` |
| `username` | `String` |
| `role` | `Role` (existing, `storage::user`) |
| `permissions` | `Vec<String>` |
| `method` | `AuthMethod` |

`AuthMethod` (`#[non_exhaustive]`): `Jwt`, `ApiKey`. (`Desktop` is phase 7.)
Relationships: `Caller<'a>: From<&'a Identity>`; `RevisionOrigin` built from `&Identity`.
Produced only by an adapter (`vizier-http`'s middleware today).

## Client outbound channel (changed)

`ClientConnection.outbound: Mutex<Option<mpsc::Sender<serde_json::Value>>>` (was
`mpsc::Sender<axum::extract::ws::Message>`). Each value is one complete client frame, the same
JSON as today's text frame. The WebSocket writer serialises it to `Message::Text`.

## RefreshStatus (moved, `vizier_core::catalogue`)

Fields unchanged: `enabled`, `ttl`, `in_progress`, `last_error`, `failed_at`. `ToSchema` only
with feature `utoipa`.

## RateLimiter (moved, `vizier_core::utils::rate_limit`)

Unchanged API (`new(max, window)`, hit/check). Used by `side_chat_limits` (core) and the
HTTP state's limiters (`vizier-http`).

## Package (build-level entity)

| Package | Depends on | Forbidden dependencies (normal) |
|---|---|---|
| `vizier-derive` | `syn`, `quote`, `proc-macro2` | — |
| `vizier-core` | `vizier-derive` + engine crates | `axum*`, `tower-http`, `utoipa-axum`, `utoipa-swagger-ui`, `clap`, `inquire`, `termimad`, `daemonize`, `dotenvy`, `opentelemetry-otlp` |
| `vizier-http` | `vizier-core` (`utoipa`) + `axum`, `axum-extra`, `tower-http`, `utoipa-*`, `rust-embed`, `jsonwebtoken`, `argon2` | `clap`, `inquire`, `termimad`, `daemonize` |
| `vizier` | `vizier-core` (`full`), `vizier-http` + `clap`, `inquire`, `termimad`, `daemonize`, `dotenvy`, `tracing-subscriber`, `opentelemetry-otlp` | — |
