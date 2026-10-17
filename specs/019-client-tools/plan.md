# Implementation Plan: Client Tools

**Branch**: `019-client-tools` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/019-client-tools/spec.md`

## Summary

WebSocket clients declare their own tools on each request (`VizierRequest.client_tools` / `client_categories`). vizier validates their shape, resolves their names against the agent's catalogue with the MCP naming rules, and offers them to that one turn's code-mode scripts. Each call is a round trip to the connection that declared it. It is saved to history at call time, its outcome and saved state live in a side table, and it is redrawn by any client on reload. A new `client_event` request kind lets a client (or a page it draws) start a turn as the person. The WebUI gains a `registerClientTool` extension API, and its first tool, `render_interactive_ui`, draws agent-written HTML in a sandboxed iframe inside the reply bubble. The page can send client events and save its own state.

The technical approach is in [research.md](research.md) (R1–R17). The design is in [data-model.md](data-model.md) and [contracts/](contracts/).

## Technical Context

**Language/Version**: Rust 2024 (stable), TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: existing only — axum (WebSocket), tokio (`mpsc`, `oneshot`, `broadcast`), serde/schemars, rusqlite, uuid; React Router v7 + Zustand in the WebUI. **No new crates or npm packages** (R3, R14).

**Storage**: embedded SQLite — one new table `client_tool_call` (data-model); one new `SessionHistoryContent` variant in the existing `session_history` blob.

**Testing**: `cargo test` — unit tests for `declare::validate`, name/category resolution (`reserved_category_name`, `assign_names` reuse), `turn_catalogue` merge, `sandbox_md` client lines, `ClientEvent` frontmatter/Display, `fill_client_calls`, the `ClientConnection` pending/close/drop-guard behaviour, `RateLimiter`; `webui`: `trail.test.ts` cases for `clientCalls` grouping (history and live, aborted turn, unknown tool). End-to-end: dummyplug steps in [quickstart.md](quickstart.md) run by `scripts/client-tools-e2e.mjs` (Node ≥ 22 global `WebSocket`) plus the browser steps of §9, per the constitution's e2e gate.

**Target Platform**: the vizier binary on Linux/macOS/Windows (all `Cross.toml` targets, unchanged); evergreen browsers for the WebUI (`iframe sandbox`, `srcdoc`, meta CSP, `ResizeObserver`).

**Project Type**: single Rust binary with an embedded web frontend.

**Performance Goals**: a page drawn within 1 s of the call, excluding model time (SC-002); state reaches other tabs within 1 s (SC-008); a disconnect fails the waiting call within 5 s (SC-005). Per turn the extra cost is one `router.catalogue()` and one small frame; per history read, one indexed `IN (…)` query, as for reactions.

**Constraints**: client tools never outlive their turn or connection; no network from pages; no new external service (Principle III); the system prompt stays byte-identical for a stable declaration (SC-007).

**Scale/Scope**: ≤ 32 tools and 16 categories per request; pages ≤ 256 KB; state ≤ 64 KB/call; results ≤ 1 MB. About 14 Rust files touched or added, about 8 WebUI files, 1 Node script, 2 docs pages.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design: still passes.*

| Principle / Gate | Assessment |
|---|---|
| **I. Lean by Default** | No new dependencies: schema checking is structural rather than `jsonschema` (R3), and the e2e client is a dependency-free Node script (R14). The new abstractions each have more than one user. `ClientConnection` serves every client tool. `turn_catalogue` is used by three readers (`RouterBridge`, `ListToolFunctions`, `DescribeToolFunction`). `RateLimiter` has two instances (events, state saves). `reserved_category_name` replaces logic MCP already has. No config surface is added: the limits are constants. ✅ |
| **II. DRY via traits (NON-NEGOTIABLE)** | Category clash handling is *shared* with MCP (refactoring `mcp_category_name` onto `reserved_category_name`), and naming reuses `assign_names`. Storage follows the pattern: a new `ClientCallStorage` trait composed into `VizierStorageProvider` and forwarded by `VizierStorage`. Client tools are not a new "kind" behind a `match`. The new `ToolTarget::Client` arm extends the existing `Native`/`Mcp` match over *dispatch targets* in `RouterBridge`, which is where dispatch targets are resolved today; it adds no per-tool branching. The live and history views of a turn stay one module (`lib/trail.ts`). ✅ |
| **III. Self-contained runtime** | No external service. The page sandbox uses a same-binary `srcdoc` iframe, not a separate origin that would need deployment configuration (R12). ✅ |
| **IV. Portability** | No OS-specific code; `Cross.toml` targets unaffected. ✅ |
| **V. Unified errors & observability** | All new fallible code returns `crate::Result`; `tracing` only; client calls run inside the existing `execute_tool` span (`ToolRouter::call`-equivalent span in `RouterBridge`), with arguments and results recorded only under `capture_content()`. ✅ |
| Gate: clippy/test/typecheck | Planned per task; `npm run typecheck` and `trail.test.ts` for the WebUI. ✅ |
| Gate: e2e through dummyplug | quickstart.md §1–§8 scripted, §9 browser; no dummyplug protocol change (R14). ✅ |
| Gate: conventional commits | `feat:` for the feature; additive protocol, so not breaking. ✅ |
| Migrations tracked in #82 | None added: a new table and a startup sweep are not compatibility code (R16). ✅ |

No violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/019-client-tools/
├── plan.md                         # this file
├── research.md                     # R1–R17
├── data-model.md
├── quickstart.md                   # dummyplug e2e script (+ browser §9)
├── contracts/
│   ├── client-tools-protocol.md    # public WebSocket contract v1 → docs/
│   ├── webui-extension-api.md      # registerClientTool + page bridge + isolation list
│   └── agent-surface.md            # SANDBOX lines, describe prefix, frontmatter, report line
├── checklists/requirements.md
└── tasks.md                        # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── client/                      # NEW
│   │   ├── mod.rs                   # ClientBinding, ClientTools (resolve at turn start, call), drop guard
│   │   ├── connection.rs            # ClientConnection: outbound, pending, close()
│   │   └── declare.rs               # ClientToolDeclaration / ClientCategoryDeclaration + validate()
│   ├── agent/mod.rs                 # chat(): build ClientTools, send client_tools_resolved, turn categories → prompt, ctx.client
│   ├── agent/system_prompt/sandbox.rs   # unchanged signature; tests for client lines
│   ├── background/report.rs         # closing "client tools unavailable" line (FR-012)
│   ├── process.rs                   # ClientEvent joins the Chat arm + ThinkingStart; ctx.client: None
│   └── tools/
│       ├── mod.rs                   # ToolContext.client, ToolTarget::Client, turn_catalogue/turn_categories,
│       │                            # reserved_category_name (mcp_category_name refactored onto it)
│       └── python/{bridge.rs,docs_tools.rs}   # read through turn_catalogue; Client dispatch; arg reference in records
├── schema/
│   ├── request.rs                   # client_tools, client_categories, client; ClientEvent content + frontmatter
│   └── history.rs                   # SessionHistoryContent::ClientToolCall, ClientCallOutcome; skipped in replay
├── storage/
│   ├── client_call.rs               # NEW ClientCallStorage trait + VizierStorage forwarding
│   ├── mod.rs                       # compose into VizierStorageProvider
│   └── sqlite/{mod.rs,client_call.rs,history.rs,session.rs,agent.rs}
│                                    # schema, impl, fill_client_calls, delete_session/delete_agent cleanup
├── transport.rs                     # SessionFrame::{ClientToolCall, ClientCallOutcome, ClientCallState}
├── dependencies.rs                  # interrupt_orphaned_client_calls at startup
└── channels/http/
    ├── rate_limit.rs                # NEW RateLimiter
    ├── state.rs                     # two limiters on HTTPState
    └── api/v1/agents/channel.rs     # ClientConnection per socket; validate + bind; client_tool_result,
                                     # client_call_state, client_event checks; new frame serialisation

webui/app/
├── lib/clientTools.ts               # NEW registry + types
├── lib/clientTools/builtin.tsx      # NEW render_interactive_ui registration
├── lib/trail.ts (+ trail.test.ts)   # Turn.clientCalls from history + live frames
├── hooks/connectionStore.tsx        # declare on send; route client frames; answer calls
├── components/client/InteractivePage.tsx   # NEW sandboxed iframe host + bridge
├── components/client/ClientCallChip.tsx    # NEW unknown-tool view
├── interfaces/types.ts              # frame and history types, client_event content
└── routes/chat.tsx                  # render clientCalls in the reply bubble; client_event user bubble

scripts/client-tools-e2e.mjs         # NEW Node e2e client (quickstart §1–§8)
docs/src/api-integration/client-tools.md   # NEW (from contracts/client-tools-protocol.md)
docs/src/api-integration/rest-api.md       # client_event row + link
docs/src/SUMMARY.md
CLAUDE.md                            # architecture note for client tools
```

**Structure Decision**: single project, using the existing layout. The client-tool runtime gets its own module, `src/agents/client/`, beside `background/` and `mcp/`, because it is per-turn agent machinery used by both the HTTP channel and the tool layer. Storage follows the one-trait-per-concern layout of `src/storage/`. The WebUI keeps the registry in `lib/` (pure, testable) and the views in `components/client/`.

## Implementation order (for /speckit-tasks)

1. **Foundations**:
   - schema types (declarations, `ClientEvent`, `ClientToolCall`, `ClientCallOutcome`);
   - `declare::validate`;
   - `ClientCallStorage` with its table, fill, cleanup and sweep;
   - `SessionFrame` variants;
   - `RateLimiter`.
2. **Server round trip** (US3, US5):
   - `ClientConnection` in `handle_socket`;
   - binding and validation, then `client_tool_result` routing;
   - `ToolContext.client`, `turn_catalogue`, `ToolTarget::Client`, `ClientTools::call` with its drop guard;
   - resolution and `client_tools_resolved` in `chat()`;
   - SANDBOX and describe labelling.
3. **Client events** (US2 server half): the `ClientEvent` arm and frontmatter, plus the WebSocket checks (rate limit, `source_call_id` scoping).
4. **No-client turns** (US4): the report line, and tests that the other `ToolContext` sites stay `None`.
5. **Saved state** (US6 server half): `client_call_state` handling and the broadcast.
6. **e2e script** (quickstart §1–§8), run against dummyplug.
7. **WebUI** (US1, US2, US6 client half, US3 extension API):
   - registry, then `connectionStore` routing;
   - `Turn.clientCalls` with tests;
   - `InteractivePage` with the bridge, interaction gate and throttles;
   - chip, bubble placement, client-event bubble;
   - quickstart §9.
8. **Docs and CLAUDE.md**.

## Complexity Tracking

No constitution violations; nothing to justify.
