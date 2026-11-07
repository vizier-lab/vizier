# Implementation Plan: Project Language Intelligence

**Branch**: `054-project-lsp` | **Date**: 2026-11-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/054-project-lsp/spec.md`

## Summary

Attach language servers to project chats, so an agent learns from an edit's own result whether it broke
something, and can ask about definitions, references, types and symbols instead of matching text.

1. **`LspHost`** in `vizier-core`: one process per `(project, root, language)`, started on first use,
   stopped when idle or when its project, worktree or agent goes away, and limited instance-wide.
2. **Diagnostics after an edit**: `edit_file` and `write_file` gain an optional `diagnostics` field,
   waiting at most 3 s and never claiming "clean" unless an answer reflects the edit and no work is running.
3. **Five `lsp_*` tools** in the existing `coding` registry, available in Plan and Build. They add nothing
   to the prompt, since they are reached from scripts.
4. **Automatic detection**: file type picks the language, and a per-language root rule picks the root, so
   repositories holding several projects need no setup.
5. **A popup from the chat's status strip**: the switch, server status (live through the inbox stream) and
   per-language overrides. New projects are on by default; existing ones stay off.

Vizier does not install servers. Research measured clangd and rust-analyzer, including on Vizier's own
workspace. A pull after an edit answered in 1.95 s there, so the 3 s target holds. rust-analyzer's
`cargo check` blocks an agent's own `cargo` commands, and R17 recommends turning it off by default.

## Technical Context

**Language/Version**: Rust (workspace edition 2024); TypeScript, React 19 and React Router 7 for the WebUI.

**Primary Dependencies**: Existing: `tokio` (`process`), `serde_json`, `sha2`, `ignore`, `libc`,
`parking_lot`. **New direct dependencies, both already in `Cargo.lock`**: `url` (file URIs) and, on
Windows only, `windows-sys` (job object). No `lsp-types` (R2).

**Storage**: SQLite and PostgreSQL through the shared `SqlStorage`: one new column
(`project.lsp_enabled`), one new table (`project_lsp_override`, env encrypted), PostgreSQL migration
`0005_project_lsp.sql`. Everything about running servers is in memory.

**Testing**: `cargo test --workspace`, `cargo clippy --workspace`,
`cargo test -p vizier-core --no-default-features`; `cd design && npm test`;
`cd webui && npm run typecheck && npm test`. Unit tests for framing, positions, roots, env filtering and
search paths. Host tests through the `fake_lsp` example. Real-server tests skip visibly if the binary
cannot run. End-to-end: dummyplug steps in [quickstart.md](./quickstart.md), using `fake_lsp` as an override.

**Target Platform**: Project hosts: the desktop app on macOS, Windows and Linux, and the server where
projects exist. Server binaries are the person's own installs.

**Project Type**: Single Rust workspace + embedded WebUI + Tauri shell (existing layout).

**Performance Goals**: Diagnostics within 3 s of an edit on a ready server (SC-001); `lsp_starting` in
under 1 s (SC-002); servers gone within 10 s of a stop (SC-006).

**Constraints**: A stuck server affects only its own calls (FR-036). No absolute host paths in results
(FR-022). No server writes through Vizier (FR-025). The prompt stays byte-stable (existing cache rule).

**Scale/Scope**: 6 running servers by default; 256 open documents per server; 50 items or 16 KiB per result.

## Constitution Check

*GATE: passes before research; re-checked after design.*

| Rule | Status | How |
|---|---|---|
| **I. Lean** | Pass | Hand-written protocol for about ten messages, no new crate outside the lockfile, no file watcher, no new trait: `LspHost` is one concrete type with one implementation |
| **II. Trait-based, no kind `match`** | Pass | Tools are registry entries. Languages are a data table, with `RootRule` as data. **No `match` on a language id**: server differences are read from negotiated capabilities |
| **III. Self-contained** | Pass | No network and no required install. A missing server reads as a clear state, and Vizier runs without any |
| **IV. Portability** | Pass | Spawn and kill reuse `process::no_window` and `process_tree`. `bind_to_parent` joins the platform code already isolated there. `url` handles Windows paths |
| **V. Errors and logging** | Pass | `crate::Result`, `tracing` only. Server stderr goes to a capped ring, not to logs at info |
| **Feature flags** | Pass, none needed | The rule names channels, addons, providers and gateways (R14). `--no-default-features` must still pass |
| **Design before UI** | Pass | Reviewed HTML mockup in `mockups/` |
| **Dummyplug e2e** | Planned | `quickstart.md` |
| **Migration tracking** | Planned | `add_column_if_missing` gets a checklist entry on #82 |
| **Matching branches** | Pass | `054-project-lsp` in all repos; commit order design → webui → vizier |

No violations, so Complexity Tracking is empty.

**One accepted exception to an existing promise**: Plan mode is described as read-only, and a server may run
project code and write caches. By decision this is on by default with no warning in the interface, and is
covered in `docs/src/configuration/projects.md` (FR-042).

## Project Structure

### Documentation (this feature)

```text
specs/054-project-lsp/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── lsp-spike.py            # Measures real servers
├── lsp-spike-2.py          # Process lifetime, build lock, Vizier's own workspace
├── mockups/lsp-status-popup.html
├── contracts/
│   ├── lsp-tools.md
│   └── http-api.md         # HTTP, inbox frame and UI
├── checklists/requirements.md
└── tasks.md                # /speckit-tasks
```

### Source Code

```text
crates/vizier-core/
├── examples/fake_lsp.rs                     # NEW scriptable test server
├── tests/lsp_host.rs, lsp_real.rs           # NEW
├── public-api.txt                           # regenerate: just public-api
└── src/
    ├── agents/
    │   ├── agent/system_prompt/project.rs   # one fixed sentence; state line
    │   ├── mod.rs                           # stop servers on agent delete
    │   └── shell/process_tree.rs            # bind_to_parent
    ├── config/mod.rs                        # lsp.max_servers
    ├── dependencies.rs                      # deps.lsp
    ├── runtime.rs                           # stop all on shutdown
    ├── projects/
    │   ├── context.rs                       # lsp_enabled, overrides, host
    │   ├── shell.rs                         # notify after a command
    │   ├── lsp/                             # NEW
    │   │   ├── mod.rs                       # LspHost: registry, capacity, idle, closed set, status
    │   │   ├── server.rs                    # one process: lifecycle, requests, progress, readiness
    │   │   ├── rpc.rs                       # Content-Length framing, request ids
    │   │   ├── languages.rs                 # the table, RootRule, root detection
    │   │   ├── search_path.rs               # lookup, login-shell PATH, env filter
    │   │   ├── position.rs                  # encodings, symbol to position, URIs to relative paths
    │   │   ├── sync.rs                      # open documents, hashes, file snapshots
    │   │   └── diagnostics.rs               # cache, the wait, the status rule
    │   └── tools/
    │       ├── mod.rs                       # needs_lsp; five entries
    │       ├── edit_file.rs, write_file.rs  # diagnostics field
    │       ├── read_file.rs                 # background start
    │       └── lsp_{diagnostics,definition,references,hover,symbols}.rs   # NEW
    ├── schema/project.rs                    # lsp_enabled, ServerOverride, LspStatusSnapshot
    ├── service/
    │   ├── projects.rs                      # open, update, remove, delete_worktree, git_switch
    │   ├── project_lsp.rs                   # NEW status, overrides, restart
    │   └── inbox.rs                         # judge_with, InboxFrame::Lsp
    ├── storage/
    │   ├── project.rs, sqlite/project.rs    # column, table, encryption
    │   └── sql/postgres/migrations/0005_project_lsp.sql   # NEW
    └── transport/mod.rs                     # InboxEvent::ProjectLsp

crates/vizier-http/
├── src/api/v1/agents/projects.rs            # four routes
└── tests/conformance/groups/projects.rs     # vectors

design/src/components/StatusStrip/           # lsp id, dot, display, headerAction, stories
webui/app/
├── screens/chat/LspPanel.tsx, ChatStatusStrip.tsx
├── lib/lsp.ts, lib/inboxFrames.ts
└── api/endpoints/projects.ts, api/schema.d.ts

docs/src/configuration/projects.md           # section, including the Plan mode note
docs/src/configuration/main-config.md        # lsp.max_servers
CLAUDE.md                                    # Projects section
```

**Structure Decision**: All engine code goes in `projects/lsp/`, beside the tools that use it. New routes
go in `vizier-http`, with authorization in `service`, as CLAUDE.md requires.

## Build Order

Each slice can be verified alone, and Story 1 (the main payoff) lands before the question tools.

1. **Prerequisites**: `url` and Windows `windows-sys` dependencies; rust-analyzer's `cargo check` is off
   (R17).
2. **Host core**: `rpc`, `position`, `languages`, `search_path`, `server`, `LspHost`, `fake_lsp`. No
   agent-visible change yet.
3. **Switch, overrides and status**: column, table, service, routes, inbox frame, turn context, state line.
4. **Diagnostics after an edit**: `sync`, `diagnostics`, the `edit_file`/`write_file` field, and the
   background start from `read_file`. (Story 1.)
5. **Question tools**: the five registry entries and the prompt sentence. (Story 3.)
6. **Staying correct**: notify after `workspace_shell` and branch switch; all stop paths; `bind_to_parent`.
7. **UI**: design, then webui. (Story 2's popup and Story 5's form.)
8. **Finish**: docs, `CLAUDE.md`, #82 entry, `public-api.txt`, conformance, real-server tests, quickstart.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| With `cargo check` off, some Rust errors go unreported | Borrow-check errors, and call sites in unopened files, are not in an edit's result | Prompt says to run the build before finishing; the shell does that (R17) |
| Startup runs build scripts | On Vizier itself this would likely run `npm run build`, and a concurrent `cargo` can wait | Accepted with the on-by-default decision; docs cover it. Inferred, not run |
| A server is memory-heavy | Peak 2.4 GB on Vizier, and worktrees multiply it | Limit of 6, idle stop, per-project switch. Consider lowering the default if 14 GB is too much |
| On by default, with no warning | Opening an untrusted repo can run its build scripts as soon as the agent reads a file, even in Plan mode | Accepted by decision. Docs cover it. A server only runs if installed, and the switch stops it |
| Many servers in worktrees | Memory | Capacity limit, idle stop, per-project switch |
| macOS has no parent-death signal | An orphan after `kill -9` if a server ignores stdin close | Measured for rust-analyzer: gone in 0.2 s. Other servers are covered by `lsp_real.rs` where installed |
| Login-shell `PATH` read runs profile scripts | A slow or noisy shell | Only on a lookup miss, 3 s limit, once per process |

## Complexity Tracking

No constitution violations to justify.
