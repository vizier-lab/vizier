# Implementation Plan: Agent Apps

**Branch**: `071-agent-apps` | **Date**: 2026-11-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/071-agent-apps/spec.md`

## Summary

An **app** is a person-owned, versioned thing on the rail. It is driven by an agent the person chooses and has:
- an HTML page, reusing spec 063's host, kit and edits;
- its own SQLite database file;
- functions, which are spec 061 scripts owned by the app;
- schedules;
- files;
- any number of conversations with its driver.

Each conversation is an ordinary one on a new channel kind, `App(user, app_id)`, so the whole chat experience applies unchanged, and the driver can wake them (spec 067).

The feature compiles under a new `subapp` flag (ungrouped, like `telemetry`: apps are not an agent addon) and runs only when the desktop host sets `RuntimeOptions.apps`. The server never does (research D1).

The engine gains:
- an `apps` module: service, storage, requirement check, per-turn context, `AppSource` tools, schedules;
- a new `AgentsClient::app_function` seam operation, so the page can run approved functions in the driver's host without a turn;
- a generalised frame lookup (`FrameSource`) so app pages use the 063 frame route.

The WebUI gains:
- mixed agent/app rail tiles with a server-side order;
- a + menu;
- `/apps/:appId` with a resizable two-inset layout;
- the chat screen extracted into a reusable `ChatView`;
- new kit abilities: `db`, `fn`, `chat`, `files`, `tools`.

## Technical Context

**Language/Version**: Rust (workspace toolchain, root `Cargo.toml`); TypeScript, React 19, React Router v7, Tailwind v4 in `webui/` and `design/`

**Primary Dependencies**: Existing only.
- **Rust**: `rusqlite` (bundled; also opens the per-app database), `storage::sql`, the blob store, `ruff_python_parser` (function calls), the `sandbox` (monty), the bus (`vizier-schema`), and axum in `vizier-http`.
- **Web**: `motion` (rail reorder), `lucide-react` through `design/src/icons`.

**No new crate or npm package.**

**Storage**: Main SQLite database, new tables `app`, `app_version`, `app_approval`, `app_schedule`, `app_schedule_run` and `rail_order` ([data-model.md](data-model.md)). Page HTML and app files go in the local blob store under `apps/{app_id}/`. Each app's own database is at `{workspace}/apps/{app_id}/app.sqlite`. There is no PostgreSQL migration, because apps never run on the server (research D3).

**Testing**:
- `cargo test -p vizier-core --features subapp`.
- `cargo test -p vizier-core --no-default-features` (apps compiled out).
- `cargo test -p vizier-http` with a new `tests/apps.rs`, run through both the network router and the dispatcher.
- `cargo clippy --workspace`.
- `just web-check`: design stories and vitest for `lib/apps/*`, `lib/uiPage/host.ts`, the rail order, `SplitInsets`.
- **End to end**: dummyplug steps in [quickstart.md](quickstart.md), driven by a dummyplug agent in an apps-enabled runtime (`tests/dummyplug_e2e.rs` and a scripted desktop run).

**Target Platform**: The desktop app (Windows, macOS, Linux; Tauri 2; SQLite + local), including its opt-in web access server. The server binary carries the code with the switch off.

**Project Type**: Engine library (`vizier-core`) + HTTP crate (`vizier-http`) + desktop host (`app/src-tauri`) + WebUI (`webui/`, `design/`)

**Performance Goals**:
- A page database write is visible as saved in ≤ 200 ms, with no model call (SC-002).
- A schedule runs ≤ 1 minute after its due time (SC-008).
- Opening an app needs one ticket and one manifest fetch.

**Constraints**:
- The page script keeps spec 063's CSP: no network, no `connect-src`.
- App SQL never touches `vizier.db`.
- Page function calls run only when approved (SC-006).
- Page chat sends happen only within the gesture window.
- Apps are invisible in the server edition (SC-009).
- Migrations are append-only and rollback never touches data.

**Scale/Scope**:
- One person has tens of apps.
- An app database stays within a few hundred MB (limit set in `apps::limits`; default 256 MiB).
- Files: 256 MiB per app.
- A manifest has at most 64 functions, 16 schedules and 32 manifest tools (the client-tool cap).
- Page queries: 2 s and 5,000 rows.
- Functions: the custom-tool timeouts.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status | Notes |
|---|---|---|
| I. Lean by Default | ✅ | No new crate or package. The engines are reused: 063 edits, ticket and frame; 061 validate, calls and run; 019 client tools; the projects pattern for the channel and turn context; the scheduler tick. New types exist only where a second real use appears: `FrameSource` has two implementations, `AppSource` is the fourth turn source. One snapshot table (`app_version`) instead of per-function version tables (D4). No PostgreSQL code with no caller (D3). |
| II. DRY via traits | ✅ | Tools come in through `TurnToolSource`. Frames resolve through a `FrameSource` trait, not an id-prefix `match` in the route. The page-initiated function call goes through the `AgentsClient` seam, so the gateway never reads agent state. Requirement facts come from the existing published views and storage traits. The rail item is one union type in `design/`, drawn by one component. |
| III. Self-contained default | ✅ | Embedded SQLite files and the local workspace only. Nothing in apps needs a network. Outside data comes through the driver's existing opt-in addons and tools. |
| IV. Portability | ✅ | The per-app database path goes through the workspace path helpers. Windows names are validated by the existing blob rules (`windows_segment_problem`). No OS-specific code. |
| V. Unified errors & observability | ✅ | `crate::Result` / `VizierError`, `ServiceError` kinds mapped to the HTTP envelope, refusals returned as `status` results in tools. `tracing` only. Spans: `execute_tool` for functions, plus `vizier.app.id` and `vizier.app.version` attributes. |
| Optional capability behind a flag | ✅ | `subapp` in `vizier-core` (in `full`), with the CI `changes` entry and flag list, and a row in `docs/src/configuration/build-flavours.md`. The naming rule covers channels, addons, providers and gateways; apps are none of these, so `subapp` is ungrouped like `utoipa` and `telemetry` and needs no amendment. The flag adds and removes routes and tools but never changes a shared type's shape. `RuntimeOptions.apps` exists in every build and is ignored without the flag. |
| Roles and components stay independent | ✅ | The gateway reaches the driver only through `AgentsClient` (D6). `FakeAgentsClient` answers `app_function` for gateway-side tests. |
| Design before UI code | ✅ | Drawn and reviewed in `Vizier shell.figcraft` › `Shell · chosen`: frames B, D, E, F, H, I, J, K and the rail drag strip (J's header supersedes the driver-only header of D, E, F and I). Still to draw before coding: the "from <app>" bubble mark (FR-021a), the version history list, and the table and row-count view (FR-018b). They are listed as the first UI tasks. |
| E2E through dummyplug | ✅ | [quickstart.md](quickstart.md) scripts each story as dummyplug steps. |
| Manual verification (startup / storage) | ✅ | New schema init plus the startup draft sweep, verified with `just app-dev` on a fresh and an existing workspace (quickstart §0). |
| Migration and compat code tracked on #82 | ✅ | One shim: the one-time upload of the localStorage `vizier.rail.order` into `rail_order`. It gets a #82 checklist entry. |

No violations; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/071-agent-apps/
├── spec.md
├── plan.md              # this file
├── research.md          # decisions D1–D17
├── data-model.md        # tables, files, manifest, state machines
├── quickstart.md        # dummyplug + desktop checks per story
├── contracts/
│   ├── http.md          # /api/v1/apps*, /me/rail-order, /utils/capabilities
│   ├── manifest.md      # the app manifest and migration rules
│   ├── app-tools.md     # tools offered in an app's chat (AppSource)
│   ├── frame-protocol.md# new kit abilities and host↔frame messages
│   └── agents-client.md # the app_function seam operation (addendum to spec 048)
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/
├── Cargo.toml                      # feature subapp (in full)
└── src/
    ├── runtime.rs                  # RuntimeOptions.apps; builds Apps; refuses non-SQLite
    ├── apps/                       # cfg(feature = "subapp")
    │   ├── mod.rs                  # Apps service: create/draft/save version/rollback/remove/drivers
    │   ├── manifest.rs             # Manifest types + validation (functions via custom_tools::validate)
    │   ├── database.rs             # per-app SQLite: open, migrate (append-only), query/exec limits
    │   ├── requirements.rs         # pure check(manifest, AgentFacts) -> Report
    │   ├── context.rs              # for_turn -> AppTurnContext
    │   ├── source.rs               # AppSource: TurnToolSource
    │   ├── tools.rs                # app_read, app_edit, app_query, app_functions, app_files, app_schedules, app_set_badge
    │   ├── functions.rs            # run a function (turn or page-initiated) via custom_tools::run
    │   ├── schedules.rs            # tick(now), runs, episodes
    │   ├── files.rs                # app files over BlobStore
    │   ├── frame.rs                # impl FrameSource for Apps
    │   ├── events.rs               # AppEvent (DataChanged, Saved, DriverChanged)
    │   ├── access.rs               # owner-only rules, can_drive
    │   └── limits.rs
    ├── ui_pages/frame_source.rs    # FrameSource trait; impl for UiPages
    ├── agents/tools/ui_kit.rs      # ui_describe_elements / ui_examples offered in App sessions too
    ├── agents/tools/ui_pages.rs    # ui_render not offered in App sessions
    ├── schema/session.rs           # VizierChannelId::App + slug/usage/web helpers
    ├── schema/request.rs           # AppOrigin, VizierRequestContent::AppScheduled
    ├── schema/usage.rs             # UsageActivity::App
    ├── agents_client.rs            # + app_function (Local + Fake)
    ├── agents/                     # host handles AgentCommand::AppFunction; ToolContext.app
    ├── agents/tools/turn_source.rs # sources() += AppSource
    ├── agents/agent/system_prompt/app.rs  # stable app section + ## App state in context_md
    ├── scheduler/mod.rs            # minute tick → apps.tick(now)
    ├── service/apps.rs             # Services for routes (owner checks, ServiceError)
    ├── service/conversations.rs    # ConversationKind::App
    ├── storage/app.rs              # AppStorage trait
    ├── storage/sqlite/app.rs       # init_app_schema + impl on SqlStorage
    ├── storage/rail.rs             # RailOrderStorage
    ├── notifications/              # app_schedule_failed, app_driver_lost
    └── transport/events.rs         # app.{id}.events pump arm

crates/vizier-core/assets/app-kit/GUIDE.md   # app_edit's guide (research D7)

crates/vizier-http/src/
├── api/v1/apps.rs                  # routes (contracts/http.md)
├── api/v1/me.rs                    # rail-order
├── api/v1/utils.rs                 # capabilities.apps
├── ui_frame.rs                     # resolve tickets through FrameSource
└── api/v1/agents/channel.rs        # bind_client_tools: App manifest filter; app_data_changed relay
crates/vizier-http/tests/apps.rs

app/src-tauri/src/engine.rs         # RuntimeOptions { apps: true } (window + web access)

design/src/
├── components/Shell/Rail.tsx       # RailItem union, app tiles, draft tile, onCreate menu
├── components/SplitInsets/         # new: two insets + separator (+ story)
├── components/AppHeader/           # new: icon, name, version, history, menu (+ story)
└── components/ChatBubble…          # "from <app>" mark (after its FigCraft pass)

webui/app/
├── routes/app.tsx                  # /apps/:appId
├── routes/chat.tsx                 # thin route over ChatView
├── screens/chat/ChatView.tsx       # extracted reusable chat
├── screens/apps/                   # AppView, DriverPicker, NoDriver, DraftEmpty, VersionHistory, Tables
├── screens/shell/ShellRail.tsx     # mixed items, server order, + menu
├── screens/chat/client/SandboxedPage.tsx  # ticket fn + abilities props
├── kit/runtime.ts                  # db / fn / chat / files / tools abilities
├── lib/apps/                       # pure: rail merge, draft rules, abilities dispatch
├── lib/clientTools.ts              # per-topic registrations
├── stores/apps.ts
└── api/endpoints/apps.ts

docs/src/configuration/apps.md      # new page (+ SUMMARY.md)
```

**Structure Decision**: The existing four-crate workspace plus the WebUI and design folders. All engine code sits in one new `apps` module behind `subapp`. Small, listed touch points go in the channel, request, usage, turn-source, scheduler, seam and frame code. There is no new crate: apps are an engine capability like projects, not a Fleet role.

## Complexity Tracking

None.
