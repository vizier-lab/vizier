# Implementation Plan: Realtime Notifications

**Branch**: `051-realtime-notifications` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/051-realtime-notifications/spec.md`

## Summary

Replace the WebUI's 15-second notification poll and its separate setup poll with one push-only stream, `GET /api/v1/inbox/stream`, with a Tauri Channel carrying the same frames in the desktop app. Events travel the Fleet bus on a new `inbox.events` subject through the existing event pump, so they reach a person on whichever process holds their connection. Each connection checks permissions per delivery. Recovery is a `hello` snapshot plus the id high-water mark the store already keeps, so the notification table is the replay buffer.

Rows gain a conversation or task title and a plain-text excerpt. Discord and Telegram activity stops notifying: the hourly projection is removed and its rows are deleted. The docked column and phone slide-over become one minimal popup anchored to the bell.

## Technical Context

**Language/Version**: Rust 2024 workspace (`vizier-core`, `vizier-http`, `vizier-schema`, the `vizier-app` Tauri shell); TypeScript with React 19 in the `webui/` and `design/` submodules

**Primary Dependencies**: all already present: axum WebSockets, tokio `broadcast`, flume, `pulldown-cmark` (in `vizier-core`), Tauri `Channel`, Zustand, design's `Floating`/`useDismiss`/`Sigil`/`ProgressBar`. No new dependency

**Storage**: three nullable columns on `notification` (SQLite and PostgreSQL through the shared `SqlStorage`), plus a one-time delete of `channel_replies` rows; see [data-model.md](data-model.md). Live setup stays in memory

**Testing**: `cargo test` (excerpt, external-channel rule, storage, throttle, a stream integration test); vitest (frame handling, store, popup); end-to-end: dummyplug steps in [quickstart.md](quickstart.md), scripted in `scripts/inbox-e2e.mjs`, with Discord/Telegram steps marked live

**Target Platform**: Linux, macOS and Docker servers; the desktop app on macOS, Linux and Windows; browsers

**Project Type**: web service with an embedded SPA and a desktop shell

**Performance Goals**: 95% of notifications on screen within 2 s (SC-001); setup progress at most 2 s behind (SC-002), at most about 4 frames per second per agent; no periodic inbox requests while connected (SC-003)

**Constraints**: a delivery failure never fails or delays the work behind it (FR-008); revocation applies from the next event (FR-007); a frame fits the bus's 1 MiB limit; no open port in the desktop app; the default build still needs no external service

**Scale/Scope**: one connection per tab; a typical install has few people and few events a minute. About 20 Rust files, one migration, about 12 WebUI files, 2 new design components

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Evidence |
|---|---|---|
| I. Lean by default | Pass | No new dependency, trait or flag. Removes the projection and two polls. `InboxConnection` has two real callers (WebSocket, IPC) |
| II. DRY via trait-based extensibility | Pass | The external rule is the existing `platform()`, in one place. Visibility is shared with the REST path, not copied (R4). The pump gains one arm in the existing pattern |
| III. Self-contained default | Pass | Runs on the in-process bus and SQLite. PostgreSQL and S3 remain opt-in |
| IV. Portability | Pass | No OS-specific code. Desktop follows the existing Tauri seam on all three platforms |
| V. Unified errors and observability | Pass | `tracing` only. `notifications/` keeps its best-effort logging and its existing error style |
| Matching submodule branches | **To do first** | `051-realtime-notifications` exists only in the workspace. `just checkout` is not supported on Windows, so run it from WSL or Linux |
| FigCraft before UI code | **Open** | The agreed mockup was rendered outside FigCraft because the server failed to connect. Draw and review it in `Vizier.figcraft` before any `design/` or `webui/` work. Backend work is not blocked |
| Dummyplug end-to-end | Pass | [quickstart.md](quickstart.md); live steps are marked and also unit-tested |
| Manual verification | Pass | The schema change and new socket are exercised on the running binary (steps 1 and 9) |
| Migration tracking | To do during work | Add the SQLite `DELETE`, the stale state key and the retained `setup-state` route to #82 |
| Roles stay independent | Pass | Workers publish and gateways subscribe, only through the bus. `vizier-schema` stays a leaf |

Re-checked after Phase 1: no violations. Both open items are ordering requirements, not design conflicts.

## Project Structure

### Documentation (this feature)

```text
specs/051-realtime-notifications/
├── spec.md
├── plan.md                  # this file
├── research.md              # decisions R1–R12
├── data-model.md
├── quickstart.md
├── contracts/
│   └── inbox-stream.md      # frames, delivery rules, REST and bus changes
├── checklists/requirements.md
└── tasks.md                 # /speckit-tasks, not created here
```

### Source Code (repository root)

```text
crates/vizier-schema/src/subject.rs          # Subject::inbox_events()

crates/vizier-core/src/
├── transport/events.rs                      # InboxEvent, third pump arm, publish/subscribe
├── notifications/
│   ├── mod.rs                               # announce on insert; excerpts; job origin; projection removed
│   ├── excerpt.rs                           # NEW: Markdown to bounded plain text
│   └── text.rs                              # remove channel_replies
├── schema/notification.rs                   # item and draft fields; variant and ReplyBatch removed
├── setup.rs                                 # throttled Setup and SetupCleared events
├── service/notifications.rs                 # fill titles; publish read changes; projection removed
├── service/setup_state.rs                   # visibility rule shared with the stream
├── service/inbox.rs                         # NEW: snapshot and per-event delivery decision
├── agents/process.rs, agents/background/mod.rs, dependencies.rs   # new call shapes
└── storage/
    ├── notification.rs, sqlite/notification.rs   # columns, RETURNING, batch removed
    ├── history.rs, sqlite/...               # read one saved entry by uid
    ├── sqlite/mod.rs, sqlite/session.rs     # schema; delete rows with the conversation
    └── sql/postgres/migrations/0003_notification_source.sql   # NEW

crates/vizier-http/src/
├── inbox.rs                                 # NEW: InboxConnection, driven by socket and IPC
├── api/v1/notifications.rs                  # the stream route
├── auth/middleware.rs                       # pass token expiry to the handler
├── embedded.rs                              # Dispatcher::open_inbox
└── lib.rs                                   # OpenAPI
crates/vizier-http/tests/inbox_stream.rs     # NEW
crates/vizier-http/tests/conformance/pending.txt

app/src-tauri/src/{ipc.rs,shell.rs}          # inbox_open, inbox_close

design/src/components/
├── NotificationItem/                        # NEW, with stories
├── SetupItem/                               # NEW, with stories
└── ProgressBar/                             # indeterminate state, if missing

webui/app/
├── api/transport/{common,index,http,tauri,websocket}.ts   # openInbox; socket generalised
├── api/endpoints/notifications.ts, api/schema.d.ts        # title, excerpt, generated types
├── lib/inboxFrames.ts                       # NEW: pure frame handling, with tests
├── stores/notifications.ts, stores/agents.ts   # stream instead of timers
├── screens/notifications/NotificationsPopup.tsx   # NEW, replaces NotificationsPanel.tsx
├── screens/notifications/{NotificationRow,SetupActivity}.tsx   # map onto design components
├── screens/shared/{NotificationsBell,NotificationToasts}.tsx   # anchor, toggle, burst
└── layouts/AppShell.tsx                     # one popup; aside removed

scripts/inbox-e2e.mjs                        # NEW
docs/src/{api-integration/rest-api.md, fleet/bus-contract.md,
          configuration/encryption.md, getting-started/desktop-app.md}
CLAUDE.md                                    # Notifications section
```

**Structure Decision**: Follows the existing split: engine logic and authorization in `vizier-core`, the route and connection in `vizier-http`, transport in `app/`, components in `design/`, and screens in `webui/`. Commit order across repos is design → webui → vizier.

### Build order

Each step works on its own, so the backend can merge before the popup if FigCraft stays unavailable.

1. **Content and exclusion** (US3, US4): columns, excerpt, titles, projection removed. Shows up through REST.
2. **Stream** (US1, US5): event, pump, `InboxConnection`, route and IPC, store switched over, polls removed.
3. **Popup** (US2): FigCraft drawing, design components, WebUI.

## Risks

- **`RETURNING` through the `SqlConn` boundary**: no existing storage code uses it. Confirm it works on both backends; if the abstraction can't express it, select by `event_key` after inserting.
- **`Services::resolve` per event**: fine at expected volume. If a busy install shows otherwise, cache for about a second, which still meets FR-007 in practice. Measure before adding it.
- **Bell number during setup**: the bell keeps its existing separate busy indicator while an agent sets up. Dropping it is a one-line change if the drawing says so.

## Complexity Tracking

No constitution violations to justify.
