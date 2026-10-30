---

description: "Task list for the desktop app edition (issue #102, phases 5–7)"
---

# Tasks: Vizier Desktop App Edition

**Input**: Design documents from `specs/038-desktop-app-edition/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R18), data-model.md, contracts/ (service-layer, ipc-protocol, webui-transport, release-artifacts), quickstart.md

**Tests**: Requested by the spec: FR-009 (automated parity suite → the conformance suite), SC-010 (automated desktop-identity check), plus the plan's app smoke test and the WebUI transport vitest. Those are the test tasks below; no others are added.

**Organization**: One phase per user story. US1 (the app) is the MVP and comes before US2 (the full service-layer port) even though both are P1: by research R1 the app reaches parity through the shared route table, so it does not wait for every domain to be ported. US2's domain ports can run in parallel with US1, US3 and US4. FR-014 keeps "selecting a notification opens the related screen", with the R11 fallback.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US5 from spec.md
- `webui/…` paths are in the `webui` submodule; commit there first, then bump the pin (design → webui → vizier)

## PR map (plan "Phase sequencing")

| PR | Tasks | Title |
|---|---|---|
| 1 | T001–T018 | `refactor: add the service layer foundation` |
| 2 | T019–T023 | `refactor: run the chat socket through a transport-neutral topic connection` |
| 3 *(webui)* | T024–T034 | `feat: add a transport seam for the desktop app` |
| 4 | T035–T056 (+ webui T049–T053) | `feat: add the desktop app shell` |
| 5–11 | T057–T081 | `refactor: move <domain> into the service layer` (one PR per group) |
| 12 | T082–T094 (+ webui T092) | `feat: add native desktop conveniences` |
| 13 | T095–T105 (+ webui T101–T103) | `feat: add opt-in web access to the desktop app` |
| 14 | T106–T119 | `feat: publish the desktop app on GitHub releases` |
| 15 | T120–T128 | polish (docs, CLAUDE.md, tier narrowing) |

---

## Phase 1: Setup

**Purpose**: Branches and baselines that the parity checks are measured against. No product code.

- [X] T001 Run `just checkout 038-desktop-app-edition` so the workspace and the `design`, `webui`, `landing`, `dummyplug` submodules are on the same branch; confirm with `just status`
- [X] T002 On `master`, build the release binary and save into `target/038-baseline/` (gitignored via `target/`): `GET /openapi.json` (`openapi.json`) and every `--help` output, using the commands of `specs/037-core-extraction/quickstart.md` §1 with `038` in the paths
- [X] T003 Copy `target/037-baseline/workspace/` (or recreate it per 037 quickstart §1: one dummyplug agent, one WebUI conversation, one memory document, one task) to `target/038-baseline/workspace/` for the "open an existing workspace" checks

**Checkpoint**: baselines recorded.

---

## Phase 2: Foundational (blocks every user story)

**Purpose**: The pieces both editions share: identity with a desktop method, one error type, access checks in core, the desktop-mode dispatcher and its guard, the transport-neutral topic connection, and the WebUI transport seam.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Identity, errors and access (PR 1)

- [X] T004 In `crates/vizier-core/src/auth.rs` rename `Identity` to `Principal` (same fields) and add `AuthMethod::Desktop`; keep `pub type Identity = Principal;` for the duration of this feature (removed in T124); update `impl From<&Principal> for Caller` in `crates/vizier-core/src/access.rs` and the `RevisionOrigin` construction in `crates/vizier-core/src/schema/revision.rs` so `Desktop` is recorded like `Jwt`/`ApiKey` (serialized as `"desktop"`)
- [X] T005 Create `crates/vizier-core/src/service/error.rs` with `ServiceError` exactly as data-model.md (kinds `NotFound`, `Unauthorized`, `Forbidden`, `Invalid { status, message, code }`, `Conflict { message, code, data }`, `Gone { code, message }`, `TooLarge`, `RateLimited`, `Unavailable`, `Internal`), `impl From<VizierError>` → `Internal`, `Display`, and a `status() -> u16` method
- [X] T006 Create `crates/vizier-core/src/service/mod.rs` with `pub struct Services` (holds a `VizierDependencies` clone; `Clone`), `pub mod error;`, re-export `ServiceError`, and `Services::principal_for(user_id, method) -> Result<Principal, ServiceError>` (resolves role + permissions from `UserStorage`; returns `Invalid` for `AuthMethod::Desktop`); add `pub mod service;` to `crates/vizier-core/src/lib.rs` (stable tier) and `pub fn services(&self) -> Services` to `VizierRuntime` in `crates/vizier-core/src/runtime.rs`
- [X] T007 Move `user_can_view_agent`, `user_can_edit_agent`, `user_can_show_platform_chats` and `platform_of_slug` from `crates/vizier-http/src/api/v1/agents/mod.rs` into `crates/vizier-core/src/access.rs` as `can_view_agent(&Principal, &AgentConfig)`, `can_edit_agent`, `can_show_platform_chats`, `platform_of_slug`, plus `require(&Principal, permission) -> Result<(), ServiceError>` (system bypass, else `Forbidden("Forbidden: insufficient permissions")`); keep the old names in `vizier-http` as one-line wrappers until each handler is ported; extend the unit tests in `access.rs` with the cases each helper branches on (system, all-permission, owner, `shared_to`, none)
- [X] T008 Move argon2 password hashing/verification from `crates/vizier-http/src/auth/mod.rs` (`AuthService`) into `crates/vizier-core/src/auth/password.rs` (`hash(&str)`, `verify(&str, &str) -> bool`, `has_password(&User) -> bool` = hash non-empty; `verify` returns `false` for an empty hash before calling argon2); `AuthService` calls into it; move the `argon2` dependency line from `crates/vizier-http/Cargo.toml` to `crates/vizier-core/Cargo.toml` (version unchanged, from `[workspace.dependencies]`); convert `auth.rs` into `auth/mod.rs` if needed
- [X] T009 Create `crates/vizier-http/src/error.rs` with `impl IntoResponse for ServiceError` producing the envelope table in contracts/service-layer.md (`code` at top level for `Invalid`/`Conflict`/`Gone`, `data` for `Conflict`); add a unit test per kind asserting status and JSON body
- [X] T010 In `crates/vizier-http/src/auth/middleware.rs` change `require_permission` to call `vizier_core::access::require` (same behaviour), and replace every `crate::auth::Identity` use in `crates/vizier-http/src/**` with `Principal` (sed-level rename, no logic change)

### Desktop mode and the guard (PR 1)

- [X] T011 Add `pub enum AuthMode { Network, Desktop { user_id: String } }` to `crates/vizier-http/src/state.rs` and a field `auth: AuthMode` on `HTTPState`; every existing constructor sets `Network`
- [X] T012 In `require_auth` (`crates/vizier-http/src/auth/middleware.rs`): when `state.auth` is `Desktop { user_id }`, ignore headers/query, resolve that user's role and permissions with `resolve_user_role` on every request and insert `Principal { method: AuthMethod::Desktop, username: <user.username>, .. }` (401 `InvalidCredentials` if the user no longer exists); when `Network`, keep today's code and additionally reject (401) a request whose extensions already hold a `Principal` with `method == Desktop`
- [X] T013 Add `cors_origins: Option<Vec<String>>` (`#[serde(default)]`) to `HTTPChannelConfig` in `crates/vizier-core/src/config/mod.rs`; in `crates/vizier-http/src/lib.rs::router` use `allow_origin(Any)` when `None` (unchanged) and an `AllowOrigin::list` of the parsed origins when `Some`
- [X] T014 Create `crates/vizier-http/src/embedded.rs`: `pub struct Dispatcher` built by `Dispatcher::new(runtime: &VizierRuntime, user_id: String)`, holding a private `Router` built like `router()` but with `HTTPState { auth: AuthMode::Desktop { user_id }, .. }` and **without** the WebUI fallback; one public method `async fn dispatch(&self, req: http::Request<axum::body::Body>) -> http::Response<axum::body::Body>` (via `tower::ServiceExt::oneshot`); no method returns the `Router` or the state; `pub mod embedded;` in `lib.rs`; add `tower` to `crates/vizier-http/Cargo.toml` from the workspace (already in the lockfile via axum; version unchanged)
- [X] T015 Expose `has_password: bool` on the `GET /auth/users/me` response in `crates/vizier-http/src/api/v1/auth.rs` (additive field, from `vizier_core::auth::password::has_password`), and make `POST /auth/login` refuse a user whose hash is empty with the existing invalid-credentials response

### Tests for the foundation (PR 1)

- [X] T016 [P] Create `crates/vizier-http/tests/desktop_principal.rs` (SC-010): start a runtime on a temp workspace (reuse `crates/vizier-http/src/test_support.rs`), create a user, then send requests to the **network** router with no credentials, a valid JWT, an API key, `Authorization: Desktop <user_id>`, `?token=` and a forged extension attempt; assert none results in a handler observing `AuthMethod::Desktop` (probe route: `GET /auth/users/me`, which echoes the method in a test-only build or via revision origin of a CORE write); assert `Dispatcher` has no public way to obtain a `Router` (compile-time: document with a `compile_fail` doctest in `embedded.rs`)
- [X] T017 Create the conformance harness `crates/vizier-http/tests/conformance.rs`: fixture workspace with three users (system role; role with `owned_agents:*` permissions owning agent A; role with no permissions) and two dummyplug agents (A owned by user 2, B owned by user 1); a `Vector { method, path, body, user }` table; for each vector send it through the network router with that user's JWT and through `embedded::Dispatcher::new(.., user)`; assert identical status and identical JSON envelope (ignoring fields listed in a small `volatile` set: timestamps, generated ids); a coverage test that loads the router's OpenAPI document and fails if any `(method, path)` has no vector. Seed it with vectors for `/status`, `/auth/users/me`, `/agents` (list, detail of A and B for every user) and `/agents/{id}/core` (get, put allowed and refused)
- [X] T018 Record the conformance responses on the current handlers as the byte baseline (`target/038-baseline/conformance/*.json`, written when `CONFORMANCE_RECORD=1`) and compare against them when not recording, so every later port proves "no behaviour change"; run `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features` and quickstart §1

### Topic connection (PR 2)

- [X] T019 Create `crates/vizier-http/src/topic.rs` with `pub struct TopicConnection` holding what `handle_socket` (`crates/vizier-http/src/api/v1/agents/channel.rs`) builds today: the session, the `ClientConnection`, the session-event subscription, the deferred-claims handle and the rate limiters; `TopicConnection::open(state: &HTTPState, principal: &Principal, agent_id, channel_id, topic_id, out: flume::Sender<serde_json::Value>) -> Result<Self, ServiceError>` (authorizes with `can_view_agent`, `NotFound` for a missing agent, session `VizierSession(agent, VizierChannelId::HTTP(principal.username, channel), Some(topic))`, delivers deferred calls as today); `async fn handle_inbound(&mut self, frame: serde_json::Value)` containing the per-frame logic moved verbatim from the socket loop; `async fn run_outbound(&mut self)` forwarding session events and client frames to `out`; `close(self)`
- [X] T020 Rewrite `chat` + `handle_socket` in `crates/vizier-http/src/api/v1/agents/channel.rs` as a pump: on upgrade, `TopicConnection::open`, then `select!` over socket messages → `handle_inbound`, the `out` receiver → socket text frames, and the existing idle timeout; on error statuses keep today's 403/404/500 empty-body responses
- [X] T021 Add a `topic` method to `embedded::Dispatcher`: `async fn open_topic(&self, agent_id, channel_id, topic_id, out) -> Result<TopicConnection, ServiceError>` (resolves the desktop principal the same way `require_auth` does in T012)
- [ ] T022 [P] Add conformance-style tests in `crates/vizier-http/tests/topic.rs`: over a WebSocket and over `Dispatcher::open_topic`, send the dummyplug sequence `tools` → `execute_python` → sample JSON → `context` (protocol: `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`) and assert both receive the same frame kinds in the same order (ids/timestamps ignored); assert a second subscriber on the same session receives the turn (spec 023/029 behaviour)
- [ ] T023 Run `node scripts/client-tools-e2e.mjs` against `just run` with a dummyplug agent (client tools, deferred calls) and quickstart §1; fix any difference

### WebUI transport seam (PR 3, webui)

- [X] T024 [P] Create `webui/app/api/transport/index.ts` with the `Transport`, `TopicHandle`, `AuthMode` and `DesktopShell` types from contracts/webui-transport.md, and `export const transport: Transport` chosen once: `'__TAURI_INTERNALS__' in window` → `await import('./tauri')`, else `./http` (top-level await or a `getTransport()` promise initialised in `app/root.tsx` before render)
- [X] T025 Create `webui/app/api/transport/http.ts`: move the fetch logic of `request` from `webui/app/api/client.ts` and `TopicSocket` from `webui/app/api/socket.ts` here unchanged; implement `upload` (FormData fetch), `fetchFile` (bearer header + optional `Range`), `assetUrl` (`/api/v1${path}?token=…` when a token exists), `saveFile` (anchor with `download`), `openFile` (`window.open`), `openTopic` (builds today's `ws(s)://…/chat?token=` URL from `endpoints/chat.ts`), `notify` (no-op), `authMode: 'http'`
- [X] T026 Create `webui/app/api/transport/tauri.ts`: `request` → `invoke('api', bytes, { headers: { 'x-method', 'x-path', 'content-type' } })`, unwrap `{status, body}` into the same `ApiError`/data results as `http.ts` (no 401 redirect: emit the bootstrap state instead); `upload` → `new Response(form)` for bytes + content-type then the same invoke; `fetchFile` → `fetch('vizier-asset://localhost/api/v1' + path, { headers: Range })`; `assetUrl` → `vizier-asset://localhost/api/v1${path}`; `saveFile`/`openFile`/`notify` → `invoke('save_file'|'open_file'|'notify')`; `openTopic` → `new Channel()` + `invoke('topic_open')`, `send` → `invoke('topic_send')`, `close` → `invoke('topic_close')`, re-open after `desktop://state` returns to `ready` (calls `onOpen(true)`); `desktop` = wrappers for every desktop command in contracts/ipc-protocol.md §4 and listeners for §5 events; `authMode: 'desktop'`; add `@tauri-apps/api` and `@tauri-apps/plugin-notification` (exact current 2.x) to `webui/package.json`
- [X] T027 Make `webui/app/api/client.ts` (`request`, `api`, `paged`) and `webui/app/api/socket.ts` (`TopicSocket`) thin delegates to `transport`, keeping their exported names and types so callers do not change
- [X] T028 [P] Migrate the asset call sites to `transport.assetUrl` / `fetchFile` / `saveFile` / `openFile`: `webui/app/screens/shared/FilePreviewModal.tsx`, `webui/app/screens/chat/FilePreviewDialog.tsx`, `webui/app/screens/artifacts/ArtifactsGallery.tsx`, `webui/app/api/endpoints/artifacts.ts`, `webui/app/api/endpoints/memory.ts` (bundle export via `fetchFile`), `webui/app/lib/generativeUi/media.ts` and `webui/app/lib/generativeUi/registry/{anchor,media,canvas2d}.tsx`, `webui/app/lib/trail.ts`
- [X] T029 [P] Migrate the socket call sites: `webui/app/api/endpoints/chat.ts` (no URL building; expose `openTopic` args) and `webui/app/stores/connection.ts` (uses `transport.openTopic`)
- [X] T030 Session bootstrap by auth mode in `webui/app/stores/session.ts`: `http` unchanged; `desktop` skips the token check and `/login` redirect, waits for `transport.desktop.state() === 'ready'`, then loads `GET /auth/users/me`; hide the Sign out control when `authMode === 'desktop'` (find it with `grep -rn "Sign out" webui/app`)
- [X] T031 Add a guard test `webui/app/api/transport/boundary.test.ts` that scans `app/**/*.{ts,tsx}` (excluding `app/api/transport/` and tests) and fails on `fetch(`, `new WebSocket(`, `'/api/v1` or `@tauri-apps/`
- [X] T032 [P] Add `webui/app/api/transport/http.test.ts` and `tauri.test.ts` (mock `invoke`/`Channel`) driven by one shared fixture table: success envelope, raw envelope, error with top-level `code`, error with `data.code`, 401, upload bytes + content-type, `assetUrl` shapes, topic open/send/close/reopen; both must yield the same results for the same fixture
- [ ] T033 Run `cd webui && npm run typecheck && npx vitest run && npm run build`; then `just run` and click through chat (dummyplug `tools`/`execute_python`/sample/`context`), file preview + download, artifacts gallery, generative UI media, memory bundle export: no visible change over HTTP
- [X] T034 Commit in `webui` (`feat: add a transport seam for the desktop app`), bump the `webui` pin in the workspace

**Checkpoint**: server unchanged (conformance + openapi diff only additive `has_password`), desktop dispatcher and topic connection exist, WebUI talks only through `transport`.

---

## Phase 3: User Story 1 - Install and use vizier as a desktop app (Priority: P1) 🎯 MVP

**Goal**: A window that runs the engine in-process, signs in as a real local owner with no login screen, and works for chat, memory, files and settings, with no port open.

**Independent Test**: quickstart §2 and §3 on a clean machine (first-run owner, dummyplug chat with a tool, nothing listening, second launch focuses, existing workspace opens, lock refusal).

### App scaffold

- [X] T035 [US1] Create `app/Cargo.toml` (virtual workspace, `members = ["src-tauri"]`, `resolver = "2"`, `[workspace.package]` version equal to the root `Cargo.toml` version) and `app/src-tauri/Cargo.toml` (`name = "vizier-app"`, `publish = false`, deps: `vizier-core = { path = "../../crates/vizier-core", features = ["full"] }`, `vizier-http = { path = "../../crates/vizier-http" }`, `tauri` 2 with `tray-icon`, `tauri-plugin-single-instance`, `tauri-plugin-dialog`, `tauri-plugin-opener`, `tokio`, `serde`, `serde_json`, `tracing`, `tracing-subscriber`, `flume`, `http`, `axum` (body type only) — exact versions; `tauri-build` build-dep); generate and commit `app/Cargo.lock`
- [X] T036 [P] [US1] Create `app/src-tauri/tauri.conf.json`: `identifier: "rs.vizier.app"`, `productName: "Vizier"`, no `version` (read from Cargo), `build.frontendDist: "../../webui/build/client"`, `build.devUrl: "http://localhost:5173"`, `build.beforeDevCommand`/`beforeBuildCommand` empty (WebUI is built separately), one window `main` (`dragDropEnabled: false`, title "Vizier", 1280×800, min 900×600), `app.security.csp` from research R15, `bundle.targets: ["dmg", "app", "appimage"]`, `bundle.macOS.minimumSystemVersion: "11.0"`, `bundle.linux.appimage.bundleMediaFramework: true`, icons list
- [X] T037 [P] [US1] Create `app/src-tauri/capabilities/main.json` granting only window `main`: `core:default`, `core:event:default`, `core:window:allow-set-focus`, `dialog:allow-open`, `dialog:allow-save`, `opener:allow-open-url`, and the app's own commands; no remote URLs
- [X] T038 [P] [US1] Create `app/src-tauri/icons/` from the brand mark (`design/` assets; `cargo tauri icon <source.png>` to generate all sizes) and `app/src-tauri/build.rs` calling `tauri_build::build()`
- [X] T039 [P] [US1] Add `app-dev` (`cd app && cargo tauri dev`), `app-build` (`cd webui && npm run build && cd ../app && cargo tauri build`) and `app-test` (`cd app && cargo test`) recipes to `Justfile`; add `app/target/` to `.gitignore`

### Engine, settings and state

- [X] T040 [US1] Create `app/src-tauri/src/settings.rs`: `DesktopSettings` per data-model.md (`workspace`, `bindings`, `close_to_tray`, `web_access{enabled, allow_network, port}`, `updates{check_on_start}`) with defaults, load from `{app_config_dir}/desktop.json` (missing → defaults with `workspace = {app_data_dir}/workspace`), save atomically (write temp + rename), preserve unknown fields (`#[serde(flatten)] extra: serde_json::Map`)
- [ ] T041 [US1] Create `app/src-tauri/src/engine.rs`: `Engine` owning the current `VizierRuntime`, `embedded::Dispatcher` (once bound) and the `DesktopState` enum from data-model.md; `start(workspace)` builds `VizierConfig::with_workspace(workspace)` and calls `VizierRuntime::start`, maps the lock error to `Locked { holder }` and other errors to `Failed { error }`; after start, resolves the binding: no users → `NeedsOwner`; binding present and user exists → `Ready`; exactly one user whose role `is_system` → bind + `Ready`; several → `ChooseOwner { candidates }`; bound user missing → `ChooseOwner`; every transition emits `desktop://state`; `shutdown()` calls `VizierRuntime::shutdown`; spawn a task forwarding `RuntimeEvent::SubsystemStopped` to `Degraded { stopped }`
- [ ] T042 [US1] Create `app/src-tauri/src/main.rs`: tracing subscriber (env filter defaults matching `crates/vizier/src/main.rs` per-crate directives, no OTLP), install the rustls crypto provider the same way the binary does, build a tokio multi-thread runtime and hand it to Tauri (`tauri::async_runtime::set`), register plugins (single-instance first, then dialog, opener), manage `Engine` as state, start the engine on the settings' workspace in `setup`, register commands, and on `RunEvent::ExitRequested`/`Exit` await `Engine::shutdown`

### IPC

- [ ] T043 [US1] Create `app/src-tauri/src/ipc.rs` command `api(request: tauri::ipc::Request) -> Result<serde_json::Value, IpcError>`: read `x-method`, `x-path`, `content-type` from invoke headers and the raw body, build `http::Request` for `/api/v1{path}`, `Dispatcher::dispatch`, return `{status, body}` where `body` is the parsed JSON envelope (or `null` for an empty body); a non-JSON response → `{status: 406, body: null}`; not `Ready` → `IpcError { code: "engine_not_ready" }` (contracts/ipc-protocol.md §1)
- [ ] T044 [US1] In `app/src-tauri/src/ipc.rs` register the `vizier-asset` scheme with `register_asynchronous_uri_scheme_protocol`: GET only (else 405), forward path + query + `Range` to the dispatcher, copy status and the headers listed in contracts/ipc-protocol.md §2, add `access-control-allow-origin` for the app origin, stream the body (§2)
- [ ] T045 [US1] In `app/src-tauri/src/ipc.rs` add `topic_open { agent_id, channel_id, topic_id, frames: tauri::ipc::Channel<serde_json::Value> }` (calls `Dispatcher::open_topic`, keeps the `TopicConnection` in a `HashMap<u64, ..>` behind a mutex, spawns `run_outbound` forwarding to `frames.send`), `topic_send { id, frame }` (`handle_inbound`), `topic_close { id }`; close all topics when the window is destroyed (§3)
- [ ] T046 [US1] Create `app/src-tauri/src/desktop.rs` commands `desktop_state`, `desktop_setup_owner { username, password? }` (create a user with the built-in system role through the same storage calls `POST /auth/setup` uses in `crates/vizier-http/src/api/v1/auth.rs`, with `password_hash = ""` when no password, else `vizier_core::auth::password::hash`; then bind in settings and build the `Dispatcher`), `desktop_choose_owner { user_id }` (must be a listed candidate), `desktop_open_workspace { path? }` (folder picker via dialog plugin when `None`; shutdown, save settings, start), `runtime_restart` (§4)
- [ ] T047 [US1] Add a debug-only command `runtime_debug_stop_scheduler` (`#[cfg(debug_assertions)]`) in `app/src-tauri/src/desktop.rs` that aborts one subsystem, for quickstart §3.5
- [ ] T048 [US1] Window behaviour in `app/src-tauri/src/main.rs`: single-instance callback focuses and unminimizes `main`; for this story, closing the last window quits (close-to-tray comes in US3)

### WebUI desktop screens (webui)

- [X] T049 [P] [US1] Create `webui/app/screens/desktop/Bootstrap.tsx` with the five states of contracts/webui-transport.md (starting spinner; locked with holder text and "Open another workspace" → `desktop_open_workspace`; needs-owner form with name and optional password; choose-owner list; failed with error and Restart), composed from `@vizier/design` components, following the dismissal convention for any dialog
- [X] T050 [P] [US1] Create `webui/app/screens/desktop/RuntimeBanner.tsx`: shown in `degraded` with the stopped subsystem names and a Restart button (`runtime_restart`)
- [X] T051 [US1] In `webui/app/root.tsx` (or the top-level layout), when `authMode === 'desktop'` render `Bootstrap` instead of the route outlet until the state is `ready`, and `RuntimeBanner` above the layout when `degraded`; listen for `desktop://state`
- [X] T052 [US1] Run `cd webui && npm run typecheck && npx vitest run && npm run build`
- [X] T053 [US1] Commit in `webui` (`feat: add desktop bootstrap screens`), bump the pin

### Tests and verification

- [ ] T054 [US1] Create `app/src-tauri/tests/smoke.rs`: start `Engine` on a temp workspace without a window, `desktop_setup_owner`, create a dummyplug agent through the `api` path (`POST /agents`), open a topic and drive `tools` → `execute_python` → sample JSON → `context`, assert the frames; then a second `Engine::start` on the same workspace reports `Locked`
- [X] T055 [US1] Add an `app` job to `.github/workflows/ci.yml` on `ubuntu-22.04`: install `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`, build the WebUI (submodules, `design` + `webui` `npm ci`, `npm run build`), `cd app && cargo clippy --all-targets -- -D warnings && cargo test`; triggered only when `app/**`, `crates/**`, `webui` pin or the workflow changes; not required for server-only PRs
- [ ] T056 [US1] Run quickstart §2 and §3 on Linux and macOS with `just app-dev`; record results (including the `ss`/`lsof` no-listener check, SC-004) in the PR description

**Checkpoint**: MVP — the app installs from a local build, signs in automatically, runs dummyplug turns, opens existing workspaces, refuses locked ones, and opens no port.

---

## Phase 4: User Story 2 - Same interface, same rules as the server (Priority: P1)

**Goal**: Every permission check has one home in `vizier_core::service`, handlers are thin, and the parity suite covers every route.

**Independent Test**: `cargo test -p vizier-http --test conformance` passes with the coverage check listing every OpenAPI path, and quickstart §4 (another person's agent refused identically in the app; revisions attributed to the desktop owner with method `desktop`).

Each task below: (1) add conformance vectors for every route in the group (allowed + refused for the three fixture users) and record them on the current handler, (2) create the service module with the domain logic and authorization moved from the handlers, input/output types moved to `crates/vizier-core/src/schema/` where not already there (keep `#[cfg_attr(feature = "utoipa", derive(ToSchema))]`), (3) rewrite the handlers as extract → service → `ServiceError`/envelope, (4) delete the now-unused `user_can_*` wrapper uses, (5) run conformance against the recorded baseline, `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features`.

- [X] T057 [P] [US2] Port topics (history, files list/clear/promote, list, detail, delete) into `crates/vizier-core/src/service/topics.rs` from `crates/vizier-http/src/api/v1/agents/channel.rs`; move `TopicConnection`'s authorization to `service::topics::authorize_topic` and keep the socket pump in `vizier-http`
- [X] T058 [P] [US2] Port agents (list, detail, create, update, delete, health, ping, restart, chat, btw, sharing get/patch, usage, usage/conversation) into `crates/vizier-core/src/service/agents.rs` from `crates/vizier-http/src/api/v1/agents/mod.rs`, including the `AgentListing` filters/sorts (move `models/page.rs`'s `Listing`, `ListQuery`, `Page` into `crates/vizier-core/src/service/page.rs` if the service needs them; leave a re-export in `vizier-http`)
- [X] T059 [P] [US2] Port CORE (get, put with `stale_revision` conflict, history list/revision/diff, rollback) into `crates/vizier-core/src/service/core.rs` from `crates/vizier-http/src/api/v1/agents/core.rs`
- [X] T060 [P] [US2] Port memory (all 11 routes) into `crates/vizier-core/src/service/memory.rs` from `crates/vizier-http/src/api/v1/agents/memory.rs`; bundle export returns bytes + filename, the handler builds the download response
- [ ] T061 [P] [US2] Port documents into `crates/vizier-core/src/service/documents.rs` from `crates/vizier-http/src/api/v1/agents/documents.rs`
- [X] T062 [P] [US2] Port tasks and task runs into `crates/vizier-core/src/service/tasks.rs` from `crates/vizier-http/src/api/v1/agents/task.rs` (requester taken from the principal, as today)
- [X] T063 [P] [US2] Port dream into `crates/vizier-core/src/service/dream.rs` from `crates/vizier-http/src/api/v1/agents/dream.rs`
- [X] T064 [P] [US2] Port background jobs (list, cancel) into `crates/vizier-core/src/service/jobs.rs` from `crates/vizier-http/src/api/v1/agents/jobs.rs`
- [X] T065 [P] [US2] Port conversations into `crates/vizier-core/src/service/conversations.rs` from `crates/vizier-http/src/api/v1/agents/conversations.rs`
- [X] T066 [P] [US2] Port artifacts into `crates/vizier-core/src/service/artifacts.rs` from `crates/vizier-http/src/api/v1/agents/artifacts.rs` (content route returns a byte source + metadata; Range handling stays in `crates/vizier-http/src/api/v1/range.rs`)
- [X] T067 [P] [US2] Port secrets into `crates/vizier-core/src/service/secrets.rs` from `crates/vizier-http/src/api/v1/agents/secrets.rs` (keep using `crates/vizier-core/src/secrets/access.rs`)
- [X] T068 [P] [US2] Port agent integrations and grants into `crates/vizier-core/src/service/integrations.rs` from `crates/vizier-http/src/api/v1/agents/integrations.rs`, and the global `/integrations` routes from `crates/vizier-http/src/api/v1/integrations.rs` (icon route stays an asset handler in `vizier-http`); errors keep `409 platform_in_use_on_agent`/`token_in_use`/`addon_installed`/`name_in_use_on_agent`, `410 attach_removed`, `422 moved_to_integrations`/`use_grants_endpoint`
- [X] T069 [P] [US2] Port agent skills and global skills into `crates/vizier-core/src/service/skills.rs` from `crates/vizier-http/src/api/v1/agents/skills.rs`, `crates/vizier-http/src/api/v1/skills.rs` and `crates/vizier-http/src/api/v1/skill_support.rs`
- [X] T070 [US2] Port users, roles, available permissions, API keys, profile, change password and first-user setup into `crates/vizier-core/src/service/{users,roles,api_keys,profile,setup}.rs` from `crates/vizier-http/src/api/v1/auth.rs`; JWT issue/validate stays in `crates/vizier-http/src/auth/mod.rs`, `login` stays a handler that calls `service::users::verify_credentials` then issues the JWT; `change_password` accepts a missing current password only when the principal's method is `Desktop` and the stored hash is empty (research R10)
- [X] T071 [P] [US2] Port providers into `crates/vizier-core/src/service/providers.rs` from `crates/vizier-http/src/api/v1/providers/mod.rs` (the `settings:providers` route layer becomes `access::require` inside each operation)
- [X] T072 [P] [US2] Port custom providers into `crates/vizier-core/src/service/custom_providers.rs` from `crates/vizier-http/src/api/v1/providers/custom.rs`
- [X] T073 [P] [US2] Port usage into `crates/vizier-core/src/service/usage.rs` from `crates/vizier-http/src/api/v1/providers/usage.rs`
- [X] T074 [P] [US2] Port catalogue into `crates/vizier-core/src/service/catalogue.rs` from `crates/vizier-http/src/api/v1/catalogue.rs` (logo route stays an asset handler; the person-triggered refresh rule is unchanged)
- [X] T075 [P] [US2] Port embedding models into `crates/vizier-core/src/service/embedding.rs` from `crates/vizier-http/src/api/v1/embedding.rs`
- [X] T076 [P] [US2] Port notifications into `crates/vizier-core/src/service/notifications.rs` from `crates/vizier-http/src/api/v1/notifications.rs` (visible-agents whitelist resolved inside the service)
- [X] T077 [P] [US2] Port overview into `crates/vizier-core/src/service/overview.rs` from `crates/vizier-http/src/api/v1/overview/mod.rs`
- [X] T078 [P] [US2] Port session file upload/download into `crates/vizier-core/src/service/files.rs` from `crates/vizier-http/src/api/v1/files.rs` (multipart parsing stays in the handler; the service takes name + content type + bytes)
- [X] T079 [P] [US2] Port setup state into `crates/vizier-core/src/service/setup_state.rs` from `crates/vizier-http/src/api/v1/utils.rs`
- [X] T080 [US2] Remove the `user_can_*` wrappers from `crates/vizier-http/src/api/v1/agents/mod.rs`; assert with `grep -rn "permissions.contains\|is_system\|user_can_" crates/vizier-http/src` that no authorization remains in `vizier-http` except `require_auth` (authentication) and the `require_permission` layer wrapper (which only calls `access::require`)
- [ ] T081 [US2] Run quickstart §1 and §4 against the server and the app; confirm the conformance coverage test lists every OpenAPI path and the `openapi.json` diff against `target/038-baseline/` is only the additive `has_password`

**Checkpoint**: one home for authorization; parity proven route by route.

---

## Phase 5: User Story 3 - Native desktop conveniences (Priority: P2)

**Goal**: Tray, close-to-tray, OS notifications that open the right screen, native save/open, external links in the browser, `vizier://` links.

**Independent Test**: quickstart §5 (window closed, scheduled dummyplug task completes, OS notification appears, selecting it opens the task run; link opens the browser; `vizier://` focuses the right chat).

- [ ] T082 [US3] Add `tauri-plugin-notification` and `tauri-plugin-deep-link` (exact 2.x) to `app/src-tauri/Cargo.toml`, register them in `app/src-tauri/src/main.rs` (deep-link after single-instance; on Linux call `register_all()` at startup so the AppImage registers `vizier://`), add `bundle`/`plugins.deep-link.desktop.schemes: ["vizier"]` to `app/src-tauri/tauri.conf.json`, and the matching permissions to `app/src-tauri/capabilities/main.json`
- [ ] T083 [US3] Create `app/src-tauri/src/tray.rs`: tray icon with menu **Open Vizier** (show + focus `main`) and **Quit** (shutdown engine, exit); left-click shows the window; register in `main.rs`
- [ ] T084 [US3] In `app/src-tauri/src/main.rs` replace T048's close behaviour: on `WindowEvent::CloseRequested`, if `settings.close_to_tray` then `prevent_close()` + hide the window (engine keeps running, topics stay open), else quit through the same path as tray **Quit**
- [ ] T085 [US3] Navigation guard in `app/src-tauri/src/main.rs`: build `main` with `on_navigation` allowing only the app origin (and `devUrl` in debug) and opening anything else with the opener plugin, and a new-window handler doing the same for `target=_blank`/`window.open` (FR-013)
- [ ] T086 [US3] Deep links in `app/src-tauri/src/desktop.rs`: on a `vizier://<route>` URL (from the deep-link plugin or forwarded single-instance args), show + focus `main` and emit `desktop://navigate { href: "/<route>" }`; buffer the first URL until the WebUI signals it is listening (`desktop_ready` command) so a cold start via link works
- [ ] T087 [US3] Command `notify { title, body, href }` in `app/src-tauri/src/desktop.rs`: show the OS notification through `tauri-plugin-notification`; where the platform reports activation, show + focus `main` and emit `desktop://navigate { href }`; record the last `href` so that, when activation is not reported, the next window focus within 60 s emits `desktop://navigate { href: "/notifications" }`-equivalent (opens the panel) (research R11)
- [ ] T088 [US3] Commands `save_file { path, filename }` and `open_file { path, filename }` in `app/src-tauri/src/desktop.rs`: `save_file` opens the dialog plugin's save dialog with `filename` as default, dispatches a GET for `/api/v1{path}` and streams the body to the chosen file; `open_file` writes to `std::env::temp_dir()/vizier/<filename>` then opens it with the opener plugin (R6)
- [ ] T089 [P] [US3] macOS microphone: add `app/src-tauri/Info.plist` with `NSMicrophoneUsageDescription` ("Vizier uses the microphone for voice messages to your agents.") and `app/src-tauri/entitlements.plist` with `com.apple.security.device.audio-input`; reference both from `bundle.macOS` in `tauri.conf.json`
- [X] T090 [US3] WebUI: in desktop mode, listen for `desktop://navigate` and route with React Router's `navigate(href)` (`/notifications` opens the notifications panel through the store's `openPanel`) in `webui/app/root.tsx`, and call `transport.desktop.ready()` once mounted
- [X] T091 [US3] WebUI: in `webui/app/screens/shared/NotificationToasts.tsx` (and the 15 s poll in `webui/app/stores/notifications.ts`), when `authMode === 'desktop'` keep polling while hidden (60 s interval) and, if `document.visibilityState === 'hidden'` or the window is unfocused, call `transport.notify({ title, body, href })` per burst instead of the in-app toast
- [X] T092 [US3] WebUI: in the audio chat control (find with `grep -rn "MediaRecorder" webui/app`), when `MediaRecorder` or `navigator.mediaDevices` is missing show "Audio recording isn't available in this window." instead of failing; run typecheck + vitest + build; commit in `webui` (`feat: wire desktop notifications and navigation`), bump the pin
- [ ] T093 [US3] Extend `app/src-tauri/tests/smoke.rs` with: `save_file` path writes the bytes of an uploaded session file (dialog bypassed through a test hook), and the navigation guard function rejects an external URL
- [ ] T094 [US3] Run quickstart §5 on Linux (AppImage-like `cargo tauri build` output) and macOS; record per-OS whether a notification click navigates or uses the focus fallback

**Checkpoint**: the app behaves like a desktop citizen.

---

## Phase 6: User Story 4 - Optional access from a browser (Priority: P2)

**Goal**: Web access off by default; turning it on requires a password or API key; loopback unless explicitly exposed with a warning; normal login always; desktop identity never accepted over the network.

**Independent Test**: quickstart §6.

- [ ] T095 [US4] Create `app/src-tauri/src/web_access.rs`: `start(engine, settings)` reads or creates `{workspace}/.runtime/web-access.key` (32 random bytes from `ring::rand::SystemRandom` — `ring` is already a core dependency; mode 0600 on Unix), builds `HTTPChannelConfig { port, jwt_secret: hex(key), cors_origins: Some(vec![format!("http://{host}:{port}")]), ..defaults }` with host `127.0.0.1` or `0.0.0.0` (`allow_network`), and spawns `vizier_http::serve(&runtime, config)` as a task the app owns; `stop()` aborts it; a failure emits `desktop://web-access { running: false, error }` and never stops the engine
- [ ] T096 [US4] Check `vizier_http::serve` can bind a chosen host; if it binds `0.0.0.0` unconditionally, add a `host: Option<String>` (`#[serde(default)]`, `None` = today's bind address) to `HTTPChannelConfig` in `crates/vizier-core/src/config/mod.rs` and use it in `crates/vizier-http/src/lib.rs` (server default unchanged)
- [ ] T097 [US4] Commands `desktop_settings_get` / `desktop_settings_set` in `app/src-tauri/src/desktop.rs`: `set` with `web_access.enabled = true` returns `{ code: "password_required" }` unless the bound user `has_password` or has at least one API key (`UserStorage` API-key listing); applying the settings starts/stops/restarts web access; emits `desktop://web-access { running, url }`
- [ ] T098 [US4] Start web access at app startup when `settings.web_access.enabled` (after the engine is `Ready`), and stop it on quit and on `desktop_open_workspace`/`runtime_restart` (restart it afterwards)
- [ ] T099 [US4] Extend `crates/vizier-http/tests/desktop_principal.rs` with a router built through `serve`'s code path with `cors_origins: Some(..)`: a preflight from another origin is refused, from the served origin allowed
- [ ] T100 [US4] Extend `app/src-tauri/tests/smoke.rs`: enabling web access for an owner without password returns `password_required`; after setting a password through the `api` path (`POST /auth/change-password` with no current password, desktop principal), enabling succeeds, `GET http://127.0.0.1:<port>/api/v1/agents` without credentials is 401, and a JWT from `POST /auth/login` works
- [X] T101 [P] [US4] WebUI: create `webui/app/routes/settings-desktop.tsx` (Settings › Desktop): workspace folder with "Open another workspace", close to tray switch, updates "check on start" switch, web access section (toggle; on `password_required` an inline message linking to Settings › Password; port field; "Allow other devices on the network" switch behind a confirm `Dialog` warning that traffic is unencrypted and anyone on the network can reach the sign-in page; current URL when running)
- [X] T102 [US4] WebUI: register the route and add the nav item only when `authMode === 'desktop'` (settings navigation component; find with `grep -rn "settings-password" webui/app`); make Settings › Password allow an empty current password when the user has `has_password === false` and the mode is desktop
- [X] T103 [US4] Run typecheck + vitest + build; commit in `webui` (`feat: add desktop settings and web access`), bump the pin
- [ ] T104 [US4] Run quickstart §6 (loopback, login, live sync between window and browser, CORS, allow-network with warning, toggle off closes the browser session)
- [ ] T105 [US4] Confirm the server edition is unchanged: `cors_origins`/`host` absent → identical behaviour; openapi diff and conformance unchanged

**Checkpoint**: opt-in browser access with the server's login rules.

---

## Phase 7: User Story 5 - Ship and update the app (Priority: P2)

**Goal**: Every GitHub release carries the universal macOS `.dmg` (unsigned until #132), x86_64 and arm64 AppImages, and a signed `latest.json`; installed apps update themselves.

**Independent Test**: quickstart §7 (release dry run produces every artifact; `lipo`/`file` checks; install on both Mac architectures and both Linux architectures; update N → N+1).

- [ ] T106 [US5] Add `tauri-plugin-updater` and `tauri-plugin-process` (exact 2.x) to `app/src-tauri/Cargo.toml`; register them in `main.rs`; in `tauri.conf.json` set `bundle.createUpdaterArtifacts: true`, `plugins.updater.endpoints: ["https://github.com/vizier-lab/vizier/releases/latest/download/latest.json"]` and `plugins.updater.pubkey` (the maintainer's `~/.tauri/vizier.key.pub` contents); in debug builds allow overriding the endpoint with `VIZIER_UPDATER_ENDPOINT`
- [ ] T107 [US5] Commands `update_check` (returns `{ available, version, notes }`) and `update_install` (download, install, `process.relaunch()` after engine shutdown) in `app/src-tauri/src/desktop.rs`; check once at startup when `settings.updates.check_on_start` and emit `desktop://update { version, notes }`
- [X] T108 [US5] WebUI: an update notice (design `Banner` or `Toast` with "Install and restart") on `desktop://update`, and a "Check for updates" button on Settings › Desktop; typecheck + vitest + build; commit in `webui` (`feat: show desktop app updates`), bump the pin
- [X] T109 [US5] In `.github/workflows/release.yml` `prepare-release`, bump `app/Cargo.toml`'s `[workspace.package] version` with the root version and refresh `app/Cargo.lock` (`cd app && cargo update -w`), included in the version-bump commit
- [X] T110 [US5] Add job `build-app` to `.github/workflows/release.yml` (needs `prepare-release`, checkout at the bumped ref with submodules) with matrix: `{ os: macos-latest, args: "--target universal-apple-darwin", rust_targets: "aarch64-apple-darwin,x86_64-apple-darwin" }`, `{ os: ubuntu-22.04 }`, `{ os: ubuntu-22.04-arm }`; steps: Rust with targets, Node 22, build the WebUI once and remove `webui/node_modules` (as the server job), Linux deps (`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libgtk-3-dev` + the server job's list), `cargo install tauri-cli --version '^2' --locked`, `cd app && cargo tauri build $args` with `TAURI_SIGNING_PRIVATE_KEY`/`_PASSWORD` from secrets and the `APPLE_*` secrets passed through only when set (unsigned otherwise, research R14)
- [X] T111 [US5] In `build-app`, verification steps: macOS `lipo -archs app/target/universal-apple-darwin/release/vizier-app` contains `x86_64` and `arm64`; Linux `file` on the AppImage reports `x86-64` / `ARM aarch64`; fail otherwise
- [X] T112 [US5] In `build-app`, rename and collect artifacts per contracts/release-artifacts.md (`Vizier_<v>_universal.dmg`, `Vizier_universal.app.tar.gz` + `.sig`, `Vizier_<v>_amd64.AppImage` + `.sig`, `Vizier_<v>_aarch64.AppImage` + `.sig`), write `.sha256` for the `.dmg` and AppImages, upload with `actions/upload-artifact@v4` (name `vizier-app-<os>`)
- [X] T113 [US5] Create `scripts/app-latest-json.mjs` (node, no deps): given the version, notes and the downloaded artifact directory, writes `latest.json` per contracts/release-artifacts.md (`darwin-aarch64` and `darwin-x86_64` both → the universal `.app.tar.gz` and its `.sig`; `linux-x86_64`, `linux-aarch64` → the AppImages) with GitHub release download URLs; add `scripts/app-latest-json.test.mjs` (covered by `just test-scripts`)
- [X] T114 [US5] In `create-release`, extend the artifact flatten `find` to include `*.dmg`, `*.AppImage`, `*.app.tar.gz`, `*.sig`; run `scripts/app-latest-json.mjs` to produce `release-assets/latest.json`; add a "Desktop app" section to the release body listing the three downloads and the macOS **Open Anyway** first-launch step (while #132 is open); for non-dry runs fail if the `.sig` files are missing (update key secret absent)
- [X] T115 [US5] Confirm `release-and-publish.yml`, `publish.yml` and `docker-publish.yml` need no change (the app is not published to crates.io: `publish = false`); the server `build` job and its assets are untouched
- [X] T116 [US5] Add a `policy` step to `.github/workflows/ci.yml` comparing the `version` of `fastembed`, `ort`, `ort-sys`, `whisper-rs`, `any-tts` between `Cargo.lock` and `app/Cargo.lock` (fail on any difference) (FR-023, research R13)
- [ ] T117 [US5] Run `release.yml` with `dry_run: true` from the branch; check every artifact of contracts/release-artifacts.md is produced and the server's four tarballs are unchanged in name and content layout
- [ ] T118 [US5] Install-test per quickstart §7.3–§7.4: the `.dmg` on an Apple Silicon and an Intel Mac (macOS 11+, Open Anyway path), each AppImage on Ubuntu 22.04 x86_64 and arm64
- [ ] T119 [US5] Update rehearsal per quickstart §7.5: install a build of version N, serve N+1's `latest.json` and artifacts from a test release via `VIZIER_UPDATER_ENDPOINT` (debug build), confirm offer → install → relaunch on all four platform/arch combinations

**Checkpoint**: the app ships on GitHub releases and updates itself.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T120 [P] Write `docs/src/getting-started/desktop-app.md`: downloads per platform, macOS Open Anyway step (until #132), AppImage `chmod +x`, first run (owner), where data lives, opening an existing workspace, the lock message, close-to-tray, updates; link it from `docs/src/SUMMARY.md`
- [X] T121 [P] Write `docs/src/configuration/desktop-web-access.md`: off by default, password requirement, loopback vs other devices, plain HTTP on a trusted network only, the security boundary (OS account = workspace boundary; desktop access equals filesystem access); link from `SUMMARY.md`
- [X] T122 [P] Write `docs/src/api-integration/building-on-vizier-core.md`: `VizierRuntime`, `services()`, `principal_for`, one example per common operation, `ServiceError`, events; extend `crates/vizier-core/examples/headless.rs` to call one service operation
- [ ] T123 Narrow the `#[doc(hidden)]` adapter tier in `crates/vizier-core/src/lib.rs`: make `pub(crate)` every module/item `vizier-http`, `vizier` and `app/` no longer use (check with `cargo build --workspace` and `cd app && cargo build`); refresh `crates/vizier-core/public-api.txt` with `service`
- [ ] T124 Remove the `Identity` alias from `crates/vizier-core/src/auth.rs` and any remaining uses
- [X] T125 Update `CLAUDE.md`: workspace layout table (add `app/`), process startup (the app as a host of `VizierRuntime`), the service layer (`vizier_core::service`, `ServiceError`, `Principal`, `AuthMethod::Desktop`, where authorization lives), `embedded::Dispatcher` and its guard, `TopicConnection`, the WebUI transport seam and its boundary test, the `app-*` Justfile recipes, the release job
- [X] T126 Add the desktop app to the README's install section and the release notes template in `release.yml` (already in T114) — no other docs
- [ ] T127 Run the full quickstart (§1–§8) one last time on the final branch, plus `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features`, `cd webui && npm run typecheck && npx vitest run`, `cd app && cargo test`
- [ ] T128 Comment progress on #102 (phases 5–7 done, phase 8 tracked separately, macOS signing in #132) once merged

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: after Setup. T004–T018 (PR 1) → T019–T023 (PR 2). T024–T034 (webui PR 3) needs only T015 (`has_password`) and can otherwise run in parallel with PR 1–2.
- **US1 (Phase 3)**: after Phase 2 complete. MVP.
- **US2 (Phase 4)**: after PR 1 (T004–T018); T057 also after PR 2. Independent of US1, US3, US4; its tasks are mostly [P] (different domain files) but each must merge through the conformance gate.
- **US3 (Phase 5)**: after US1 (needs the app shell).
- **US4 (Phase 6)**: after US1. T102's password change for desktop needs T070 (US2 users port) **or** the equivalent change applied directly in `crates/vizier-http/src/api/v1/auth.rs` if US4 lands first.
- **US5 (Phase 7)**: after US1; best after US3 (tray, deep links are part of what ships).
- **Polish (Phase 8)**: after all stories; T123–T124 after all of US2.

### Within each story

- Rust before the WebUI pieces that call it; WebUI commits before the workspace pin bump.
- In US2, conformance vectors are recorded on the old handler **before** the port.

### Parallel opportunities

- PR 3 (WebUI seam) in parallel with PR 1–2.
- After PR 1: the 23 domain ports T057–T079 (except T070, which touches shared auth code) in parallel, by different people/agents, each on its own file pair.
- Within US1: T036, T037, T038, T039 together; T049 and T050 together.
- Docs T120–T122 together.

## Parallel Example: User Story 2

```text
Task: "T059 [US2] Port CORE into crates/vizier-core/src/service/core.rs from crates/vizier-http/src/api/v1/agents/core.rs"
Task: "T060 [US2] Port memory into crates/vizier-core/src/service/memory.rs from crates/vizier-http/src/api/v1/agents/memory.rs"
Task: "T062 [US2] Port tasks into crates/vizier-core/src/service/tasks.rs from crates/vizier-http/src/api/v1/agents/task.rs"
Task: "T071 [US2] Port providers into crates/vizier-core/src/service/providers.rs from crates/vizier-http/src/api/v1/providers/mod.rs"
```

## Parallel Example: User Story 1

```text
Task: "T036 [US1] Create app/src-tauri/tauri.conf.json"
Task: "T037 [US1] Create app/src-tauri/capabilities/main.json"
Task: "T038 [US1] Create app/src-tauri/icons/ and build.rs"
Task: "T049 [US1] Create webui/app/screens/desktop/Bootstrap.tsx"
Task: "T050 [US1] Create webui/app/screens/desktop/RuntimeBanner.tsx"
```

## Implementation Strategy

### MVP first (User Story 1)

1. Phase 1 → Phase 2 (PR 1, 2, 3).
2. Phase 3 (US1): a local `just app-build` gives a working app with parity through the shared route table.
3. **Stop and validate**: quickstart §2–§3 on Linux and macOS.

### Incremental delivery

1. US1 → usable app from local builds.
2. US2 domain ports land continuously (parallel); each PR is behaviour-neutral.
3. US3 → desktop conveniences; US4 → web access; US5 → public release with updates.
4. Polish: docs, tier narrowing, CLAUDE.md.

### Commits

Conventional commits everywhere; webui commits first, then the pin bump in the workspace (`just commit "<message>"` commits submodules and the workspace together). PR titles per the PR map.
