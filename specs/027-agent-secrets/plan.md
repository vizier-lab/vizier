# Implementation Plan: Agent Secrets

**Branch**: `027-agent-secrets` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/027-agent-secrets/spec.md`

## Summary

Add a per-agent secret store with two code-mode tools — `set_secret` creates or replaces a value, `get_secret` returns it only to the running `execute_python` script — and a WebUI Secrets page for the people allowed to manage that agent's secrets. Access is governed by exactly two new permissions, `all_agents:secret` and `owned_agents:secret`.

The technical approach:

- Two SQLite tables, `agent_secret` and `agent_secret_event`, behind a new `SecretStorage` trait composed into `VizierStorageProvider`. Values stay plaintext and rely on host/disk protection, as provider credentials do.
- A per-turn `SecretRedactor` on `ToolContext`. It starts empty and is armed only when `set_secret` or `get_secret` handles a value. Exact occurrences are replaced before nested tool arguments, errors, the execution report (`result`, `stdout`, traceback), telemetry, debug logs, saved history or the WebUI can see them.
- One `secrets::service` for validation and set/delete, shared by the tools and the HTTP handlers; one `Caller`/permission helper shared with integrations access.
- A small authenticated HTTP surface (`list`, `PUT` upsert, `DELETE`, `events`) and a Secrets page under the agent's Misc navigation. The page uses the existing design `SecretField`, `Dialog`, `ConfirmDialog` and `PageFrame`; the only design-library change is the sidebar entry plus an optional `hiddenPages` filter.
- Change events record actor, action, name and time, never the value.

## Technical Context

**Language/Version**: Rust, edition 2024 (`vizier` 0.13.1)

**Primary Dependencies**: axum 0.8, rusqlite (bundled SQLite), utoipa, schemars, serde, tokio, chrono, uuid; WebUI React 19, React Router 7, Tailwind v4, Zustand, `@vizier/design`. No new crate is needed.

**Storage**: Embedded SQLite. New tables `agent_secret` and `agent_secret_event`; values plaintext. No filesystem store, no migration of existing data.

**Testing**: `cargo test` and `cargo clippy`; `cd webui && npm run typecheck`; end-to-end via the `dummyplug` steps in [quickstart.md](quickstart.md), per the constitution's e2e gate.

**Target Platform**: The same single self-contained binary as vizier today (Linux, macOS, Windows; SQLite bundled). No platform-specific code.

**Project Type**: Single Rust binary serving a bundled React SPA; `design/` and `webui/` are submodules.

**Performance Goals**: No new network calls. Listing a secret's metadata is one indexed SQLite query; values are capped at 64 KiB. Redaction is a linear scan only after a secret tool is called; it is a no-op on ordinary turns.

**Constraints**: Must remain config-less and offline; no required external service. Secret values must not enter logs, telemetry, reports, saved history, management responses or the WebUI. Permission names are exactly `all_agents:secret` and `owned_agents:secret`.

**Scale/Scope**: Per agent, a small set of named secrets (tens, not thousands) and an append-only change log. One new agent category, two tools, four HTTP routes, one WebUI page, two permission entries.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default** — PASS. Two tables, two tools, one service, one page; no new dependency, trait parameter, config flag or per-agent switch. The redactor is the smallest mechanism that satisfies the non-disclosure requirement without changing every tool. The shared `Caller` extraction removes a second copy of the same ownership-permission rule rather than adding an abstraction ahead of use.
- **II. DRY via Trait-Based Extensibility** — PASS. Storage is one `SecretStorage` trait implemented once by `SqliteStorage` and composed into `VizierStorageProvider`; tools implement `VizierTool`; HTTP and tools both call `secrets::service`; roles and API use the same permission constants. There is no kind/type branching.
- **III. Self-Contained, Zero-Dependency Runtime** — PASS. SQLite only; no external service, no network requirement, no new runtime.
- **IV. Portability by Default** — PASS. No OS-specific path or process behavior; the default SQLite backend is already portable.
- **V. Unified Errors & Observability** — PASS. Storage follows the existing `anyhow::Result` trait convention; tools use `crate::Result`/`VizierError`; all logging goes through `tracing`. The redactor sits on every value-bearing log/telemetry path this feature touches.
- **Workflow gates** — PASS. `cargo clippy`, `cargo test`, WebUI typecheck, and a dummyplug end-to-end pass are in the task list. Conventional commits, with `[**breaking**]` only if a tool/category name must change later (none does here).

No violations; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/027-agent-secrets/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── agent-tools.md
│   ├── http-api.md
│   └── webui.md
├── checklists/requirements.md
└── tasks.md              # generated by /speckit.tasks
```

### Source Code (repository root)

```text
src/
├── access.rs                         # shared Caller + ownership/manage predicate
├── main.rs                           # mod access; mod secrets;
├── schema/
│   ├── mod.rs                        # pub mod secret; re-exports
│   └── secret.rs                     # SecretAction, AgentSecretEvent
├── secrets/
│   ├── mod.rs                        # limits, name/value validation, re-exports
│   ├── access.rs                     # secret permission constants + can_manage
│   ├── redact.rs                     # SecretRedactor, execution-report scrubbing
│   └── service.rs                    # validate + set/get/list/delete/events facade
├── storage/
│   ├── mod.rs                        # SecretStorage in VizierStorageProvider + forwarding
│   ├── secret.rs                     # SecretRecord, SecretWrite, SecretStorage trait
│   └── sqlite/
│       ├── mod.rs                    # init_secret_schema, call from init_schema
│       ├── agent.rs                  # delete agent's secret rows/events
│       └── secret.rs                 # SqliteStorage impl + unit tests
├── agents/
│   ├── agent/mod.rs                  # turn SecretRedactor in ToolContext
│   ├── process.rs                    # remaining ToolContext constructions
│   ├── background/mod.rs             # remaining ToolContext construction
│   ├── hook/debug.rs                 # stop logging raw nested args/responses
│   └── tools/
│       ├── mod.rs                    # Secrets category, BaseTools registration, telemetry redaction, tests
│       ├── secrets.rs                # set_secret / get_secret
│       └── python/
│           ├── mod.rs                # scrub ExecutionReport before return
│           └── bridge.rs             # redact arguments/errors before log/record
├── channels/http/
│   ├── mod.rs                        # OpenAPI paths/schemas
│   └── api/v1/agents/
│       ├── mod.rs                    # nest /secrets
│       └── secrets.rs                # list, PUT, DELETE, events handlers
├── integrations/
│   ├── mod.rs                        # use shared mask_secret
│   ├── access.rs                     # re-export shared Caller
│   └── kind.rs                       # use shared mask_secret
├── utils/mod.rs                      # mask_secret moved here
└── storage/user.rs                   # AVAILABLE_PERMISSIONS

webui/app/
├── api/endpoints/secrets.ts          # typed calls
├── api/schema.d.ts                   # regenerated
├── layouts/AppShell.tsx              # secrets route path + hiddenPages
├── lib/access.ts                     # canManageSecrets
├── lib/permissions.ts                # Secret permission group
├── routes.ts                         # :agentId/secrets route
└── routes/secrets.tsx                # Secrets page

design/src/components/Sidebar/
├── Sidebar.tsx                       # "secrets" page + hiddenPages
└── Sidebar.stories.tsx               # story option

docs/src/
├── SUMMARY.md                        # Secrets page entry
├── configuration/secrets.md          # operator/agent guide
├── configuration/agents.md           # permission mention
└── api-integration/rest-api.md       # endpoints + permissions
```

**Structure Decision**: single Rust project with its existing embedded WebUI and design-library submodules. There is no new service or package. WebUI work follows the submodule order `design/` → `webui/` → vizier pin bump.

## Design Decisions

### Storage boundary

`SecretStorage` is the only interface the rest of the code sees. `get_agent_secret` is the only method that returns a value; list returns `SecretRecord` values that already carry `masked`, never the raw value. Set and delete run in one SQLite transaction with their event row. `delete_agent` removes both tables on its existing connection.

### Retrieval boundary

`get_secret` returns a plain value to the Python bridge, so the script can pass it to another tool. The model's copy of the execution report is scrubbed before it is serialized, persisted or streamed. The redactor is armed by the two secret tools only; before that it contains no strings and every scrub call is a no-op. Values are never persisted in the redactor or carried to another turn.

### Access boundary

The agent tools act only on the `agent_id` captured when `BaseTools` was built; there is no name parameter for the agent. HTTP callers are checked with `Caller::manages(agent, MANAGE_ALL, MANAGE_OWNED)`. A non-manager gets `403`; a missing agent gets `404`. No read-only secret permission exists.

### Change record boundary

`agent_secret_event` stores one row per successful create, replace or delete. It stores `Requester` JSON (person id or agent id), the action, name and timestamp. It never stores a value. The events endpoint resolves actor names from the same helpers the integrations pages use.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| A script transforms the value (base64, URL-encoding) and prints it, defeating exact-match redaction. | The tool description tells the model to pass the value to another tool rather than print it; the limitation is stated in the spec and docs. Do not claim encoding-proof secrecy. |
| A value supplied to `set_secret` is already in the script/message. | Explicitly out of the non-disclosure guarantee; the set input is scrubbed from nested tool records and every later retrieval is protected. |
| A tool outside the bridge logs its own secret-bearing arguments. | Audit `http_client`, fetch and shell paths during implementation; the bridge covers the recorded/logged call, and any direct `tracing` of headers/commands is removed or redacted. |
| Debug logging leaks a raw nested response. | `DebugHook` stops logging nested arguments and response payloads entirely; it never needs the redactor. |
| A value under 8 bytes makes redaction corrupt ordinary text. | Reject values shorter than 8 bytes with a clear validation error. |
| WebUI and server disagree on permission semantics. | One `canManageSecrets` helper in `access.ts` mirrors the documented `Caller` rule; the server is still the source of truth and returns 403. |

## Complexity Tracking

No Constitution Check violations.
