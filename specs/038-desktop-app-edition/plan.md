# Implementation Plan: Vizier Desktop App Edition

**Branch**: `038-desktop-app-edition` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/038-desktop-app-edition/spec.md` (issue #102, phases 5–7; phase 8 is a separate issue)

## Summary

Ship a Tauri 2 desktop app (`app/`) that runs `VizierRuntime` in-process and shows the same
`webui/build/client` as the server, signed in automatically as a real local owner, with no
port open by default. Three pieces, in this order:

| Phase | What lands | Where |
|---|---|---|
| 5 Service layer | `vizier_core::service` with `ServiceError` and `Principal`; every permission check moves next to the domain logic; `vizier-http` handlers become extract → service → envelope; `AuthMethod::Desktop`; `TopicConnection` extracted from the chat WebSocket; `embedded::Dispatcher` | `crates/vizier-core`, `crates/vizier-http` |
| 6 WebUI transport | `app/api/transport/{index,http,tauri}.ts`; the twelve URL/token call sites migrated; desktop bootstrap, runtime banner and Settings › Desktop screens | `webui` submodule |
| 7 Desktop shell | `app/` Cargo workspace; IPC (`api` command, `vizier-asset://` scheme, `topic_*` over `ipc::Channel`, desktop commands); tray, single instance, deep links, notifications, dialogs, opener, updater; opt-in web access; release job for the universal `.dmg` and x86_64 + arm64 AppImages | `app/`, `.github/workflows/release.yml`, `docs/` |

The key design decision (research R1): the window reaches the engine through **one** generic
IPC command that dispatches requests in memory through the same axum route table the server
mounts, in a desktop auth mode that can never be bound to a listener. Parity with the server
is structural rather than maintained by hand, and no port is opened. Phase 5 gives that route
table one home for authorization and gives third-party UIs a typed surface.

## Technical Context

**Language/Version**: Rust edition 2024, toolchain 1.99 (as 037); TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: existing set unchanged in `crates/` (research R13, FR-023); `argon2`
moves from `vizier-http` to `vizier-core` (`auth::password`, R2). New, in `app/src-tauri`
only: `tauri` 2 (features `tray-icon`), `tauri-build`, `tauri-plugin-{single-instance,
deep-link, opener, notification, dialog, updater, process}`; `tower` (already transitive via
axum) for `oneshot`. New in `webui`: `@tauri-apps/api` 2, `@tauri-apps/plugin-*` JS bindings
used by `tauri.ts` (dynamic import only). Dev tool: `tauri-cli` 2.

**Storage**: embedded SQLite + markdown documents, **no schema change** (data-model.md). New
files: `{app_config_dir}/desktop.json`, `{workspace}/.runtime/web-access.key`.

**Testing**: `cargo test --workspace`, `cargo clippy --workspace`,
`cargo test -p vizier-core --no-default-features`; new `crates/vizier-http/tests/conformance.rs`
(network router vs embedded dispatcher, every OpenAPI path covered) and
`tests/desktop_principal.rs`; `cd webui && npm run typecheck && npx vitest run` (both
transports); `app/src-tauri/tests/smoke.rs` on desktop runners; end-to-end: dummyplug steps in
[quickstart.md](quickstart.md)

**Target Platform**: app: macOS 11+ (universal2), Linux x86_64 and arm64 (AppImage, glibc ≥
2.35). Server edition targets unchanged. Windows out of scope.

**Project Type**: Rust library + web adapter (`vizier-core`, `vizier-http`), web frontend
(`webui`), desktop app (`app/`)

**Performance Goals**: first streamed frame in the window within 1 s of the model's first
output (SC-005; IPC adds sub-millisecond overhead per frame); app cold start to `ready` under
5 s on a workspace with 10 agents

**Constraints**: no listening socket by default (SC-004); server's OpenAPI document changes
only additively (`has_password`), its `--help`, packages and Docker image not at all
(FR-022); dependency pins identical in both lockfiles (FR-023); app workspace not part of the
root `cargo test --workspace` (no GTK/WebKit for server builds)

**Scale/Scope**: 118 routes / ≈ 100 permission checks ported into ≈ 25 service modules
(≈ 16k lines in `vizier-http` touched, much of it moved); ≈ 260-line WebSocket body extracted;
≈ 15 WebUI files migrated + 3 new screens; new `app/` (≈ 1.5k lines Rust, config, icons);
one new release job

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment | Status |
|---|---|---|
| I. Lean by Default | One IPC command instead of 118 (R1). Plugins are the standard Tauri answer to each spec requirement, no others (R9). No core event for notifications, the existing poll is reused (R11). No `ClientSink`-style trait; `TopicConnection` is one concrete type with two pumps (R5). `Services` is a struct, not a trait. | ✅ |
| II. Trait-based extensibility | The two transports are two implementations of one `Transport` interface; screens never branch on edition except the three desktop-only screens and the Sign-out visibility (driven by `authMode`, not scattered checks). Permission rules move to one place instead of being duplicated per adapter. | ✅ |
| III. Self-contained runtime | The app embeds the engine, SQLite and the UI; no external service; network only for providers, as today. The server binary is unchanged. | ✅ |
| IV. Portability | Platform paths through Tauri's path API (`app_data_dir`/`app_config_dir`); no `cfg` in core; platform specifics live in `app/` only. `Cross.toml` musl target unaffected (no new core deps besides `argon2`, already in the tree). | ✅ |
| V. Unified errors & observability | `ServiceError` is the one boundary error, converted from `VizierError`; logging via `tracing` in the app too (the app installs the same subscriber setup as the binary, without OTLP unless configured). | ✅ |
| Conventional commits / PR titles | All PRs `feat:` or `refactor:` as listed below. | ✅ |
| Matching submodule branches via `just` | `just checkout 038-desktop-app-edition` (R18); webui PRs linked and merged with `/cascade-merge`. | ✅ |
| clippy + test + WebUI typecheck | In CI; the app gets its own job on desktop runners (not required for server PRs). | ✅ |
| Manual verification of startup/build changes | quickstart §2, §3, §7 run the app and a release dry run. | ✅ |
| E2E through dummyplug | quickstart §1–§6 are dummyplug steps; the app smoke test drives a dummyplug turn through the real commands. | ✅ |
| #82 tracking | No migration, shim or legacy type added. `Identity` → `Principal` alias removed in the same feature, before any publish. | ✅ |

**Post-design re-check**: all ✅. One recorded deviation from the issue (not from the
constitution): IPC dispatches through the route table instead of typed per-operation commands
(Complexity Tracking).

## Project Structure

### Documentation (this feature)

```text
specs/038-desktop-app-edition/
├── plan.md              # this file
├── research.md          # R1–R18
├── data-model.md        # Principal, ServiceError, AuthMode, desktop.json, app state machine
├── quickstart.md        # dummyplug + platform verification
├── contracts/
│   ├── service-layer.md       # vizier_core::service surface and error mapping
│   ├── ipc-protocol.md        # window ⇄ shell commands, scheme, events
│   ├── webui-transport.md     # Transport interface and call sites
│   └── release-artifacts.md   # what each GitHub release carries for the app
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── vizier-core/src/
│   ├── auth.rs                 # Principal (was Identity), AuthMethod::Desktop
│   ├── auth/password.rs        # argon2 hash/verify, empty hash = no password (moved from vizier-http)
│   ├── access.rs               # + can_view_agent, can_edit_agent, can_show_platform_chats, require
│   ├── config/mod.rs           # HTTPChannelConfig.cors_origins
│   ├── runtime.rs              # + services()
│   └── service/                # NEW: mod.rs (Services, principal_for), error.rs (ServiceError),
│                               # agents, core, memory, documents, tasks, dream, jobs, conversations,
│                               # topics, artifacts, secrets, integrations, skills, users, roles,
│                               # api_keys, profile, setup, providers, custom_providers, catalogue,
│                               # usage, embedding, notifications, overview, files, setup_state
├── vizier-http/src/
│   ├── state.rs                # + auth: AuthMode
│   ├── auth/middleware.rs      # AuthMode-aware require_auth; require_permission → service::access
│   ├── error.rs                # NEW: impl IntoResponse for ServiceError
│   ├── topic.rs                # NEW: TopicConnection (from api/v1/agents/channel.rs::handle_socket)
│   ├── embedded.rs             # NEW: Dispatcher { dispatch(Request) -> Response }, Desktop mode only
│   ├── lib.rs                  # CORS from cors_origins
│   └── api/v1/**               # handlers thinned onto services
│   └── tests/{conformance,desktop_principal}.rs
app/                            # NEW, separate Cargo workspace (publish = false)
├── Cargo.toml / Cargo.lock
└── src-tauri/
    ├── Cargo.toml              # vizier-core {full}, vizier-http, tauri 2 + plugins
    ├── build.rs                # tauri-build
    ├── tauri.conf.json         # identifier rs.vizier.app, frontendDist ../../webui/build/client,
    │                           # devUrl :5173, CSP, bundle (dmg, appimage w/ media framework),
    │                           # updater pubkey + endpoint, minimumSystemVersion 11.0, dragDropEnabled false
    ├── capabilities/main.json  # main window only
    ├── Info.plist / entitlements.plist   # microphone
    ├── icons/
    ├── src/
    │   ├── main.rs             # logging, tokio runtime, builder, plugins (single-instance first)
    │   ├── engine.rs           # VizierRuntime lifecycle, desktop state machine, restart
    │   ├── settings.rs         # desktop.json load/save, bindings
    │   ├── ipc.rs              # api command, vizier-asset scheme, topic_* commands
    │   ├── desktop.rs          # desktop_* commands, notify, save/open file, deep links, navigation guard
    │   ├── tray.rs
    │   └── web_access.rs       # vizier_http::serve on demand, key file, password gate
    └── tests/smoke.rs
webui/ (submodule)
└── app/
    ├── api/transport/{index,http,tauri}.ts (+ tests)
    ├── api/client.ts, api/socket.ts        # delegate to transport
    ├── screens/desktop/{Bootstrap,RuntimeBanner}.tsx
    └── routes/settings-desktop.tsx
.github/workflows/
├── ci.yml                      # + app job (ubuntu-22.04, builds app, runs smoke test), + lockfile pin check in policy
└── release.yml                 # + build-app matrix (macos universal, ubuntu-22.04, ubuntu-22.04-arm), latest.json, notes
docs/src/                       # app install, security boundary, web access, building a UI on vizier-core (FR-024)
Justfile                        # app-dev, app-build recipes
```

**Structure Decision**: issue #102's layout. The app is a separate workspace under `app/`
(R13); the transport seam lives in the WebUI submodule; engine-side changes are confined to
`vizier-core` (services, auth) and `vizier-http` (thin handlers, dispatcher, topic
connection).

## Phase sequencing and PR plan

Workspace PRs unless marked *(webui)*. Each runs quickstart §1; app PRs also §2 onward.

1. **`refactor: add the service layer foundation`**: `service::{mod,error}`, `Principal`,
   `AuthMethod::Desktop`, access helpers moved to core, `ServiceError` → response,
   `HTTPState.auth`, `embedded::Dispatcher`, `cors_origins`, conformance harness with
   baselines recorded, `desktop_principal` test.
2. **`refactor: run the chat socket through a transport-neutral topic connection`**:
   `topic.rs` (R5) plus the `topics` service.
3. **`refactor: move <domain> into the service layer`** × 7 (port order in
   [service-layer.md](contracts/service-layer.md)); each adds its conformance vectors first.
   These can merge in parallel with 4–7.
4. *(webui)* **`feat: add a transport seam for the desktop app`**: transport, call-site
   migration, ESLint/vitest guard, both transports' tests. No visible change over HTTP.
5. **`feat: add the desktop app shell`**: `app/` with engine, settings, IPC, bootstrap
   commands, smoke test, CI app job, `just app-dev`. *(webui)* bootstrap + runtime banner
   screens.
6. **`feat: add native desktop conveniences`**: tray, close-to-tray, single instance, deep
   links, navigation guard, notifications, save/open file. *(webui)* `notify` wiring.
7. **`feat: add opt-in web access to the desktop app`**: R10, password gate, Settings ›
   Desktop *(webui)*.
8. **`feat: publish the desktop app on GitHub releases`**: `build-app` job, `latest.json`,
   updater wiring, release notes, docs. **Merge gate**: release dry run green with all app
   artifacts and unchanged server assets.

When 3 finishes: narrow the `#[doc(hidden)]` adapter tier and refresh `public-api.txt`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Deviation from issue #102 phase 7 ("IPC commands over the service surface"): one generic `api` command dispatched in memory through the axum route table (R1) | Gives route-for-route parity with the server for free and needs no per-operation code in Rust or TypeScript | Typed commands per operation = 118 Rust commands + 118 TS mappings kept in step by hand (Principle II); an embedded localhost server opens a port (SC-004, issue decision #1) |
| The app always links `vizier-http` (axum), not only behind `web-access` as the issue sketched | R1 needs the route table in-process; web access needs it anyway | Reimplementing routing over services in the app duplicates the route table |
| The built WebUI is embedded twice in the app (Tauri `frontendDist` + `vizier-http`'s embed for web access) (R7) | Keeps the window on Tauri's default origin, where IPC and capabilities are best supported | Serving the window from a custom scheme over `vizier-http`'s embed saves a few MB but puts IPC on a less-proven origin |
