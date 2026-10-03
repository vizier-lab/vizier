# Implementation Plan: WebUI Reasoning & Tool Activity Display

**Branch**: `010-webui-reasoning-display` | **Date**: 2026-10-03 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/010-webui-reasoning-display/spec.md`

## Summary

An agent's thinking and tool calls are shown live and then thrown away when the answer arrives. This
feature keeps them, folded, attached to the turn that produced them; reworks a python run so the
agent's stated **intent** is the headline and the script is one disclosure deeper; and stops showing
the tool calls a script makes.

Most of this is not a storage problem. Reasoning is **already recorded** — `think` is a tool, so each
thought is already a `ToolCall` entry in `session_history` — and the replay path already handles
intermediate narration. Nothing renders any of it: the WebUI handles only `Request`, `Response`,
`Checkpoint` and `Command`, and holds the live trail in throwaway state wiped by `clearInlineEvents()`
on every answer.

What *is* broken is ordering. `list_session_history` runs `ORDER BY timestamp DESC` and then a
**stable** `sort_by_key`, so entries sharing a timestamp keep whatever order the query plan returned.
Ties are the normal case, not an edge case: a turn's entries are all flushed in one tight loop at turn
end, each stamped at millisecond resolution. Rendering history is precisely what would expose this, so
the ordering fix is a prerequisite, not a nice-to-have.

Two findings from Phase 0 shape the work more than anything in the spec did.

First, **narration cannot simply be un-filtered.** Recording it in its current position yields
`assistant(tool_use) → assistant(text) → user(tool_result)` on replay, breaking the tool-call/result
adjacency providers enforce — almost certainly why it was filtered out originally. Fixing the write
order alone then yields *two consecutive assistant messages*, which Anthropic also rejects. Replay has
to merge narration and its tool calls back into the single assistant message the model actually sent.
This is the one change here that can break turns outright rather than merely look wrong.

Second, **hiding a script's tool calls is two fixes, not one.** `RouterBridge::dispatch` runs every
nested call through `on_tool_call`, and `ToolCallsHook` emits a `ToolChoice` frame for each, so a loop
of twelve nested calls streams twelve frames live — independently of the `tool_calls` list in the
report view. Nested calls are never persisted, though, so the history-sourced trail is clean for free.

The work lands in five places: the `seq` column and ordering in `src/storage/sqlite/history.rs`, the
narration write/replay pair in `src/schema/history.rs`, a nested-call hook method across
`src/agents/hook/`, the required `intent` on `ExecutePython`, and the WebUI trail — a normalized model
with two producers (live stream, stored history) feeding one renderer.

## Technical Context

**Language/Version**: Rust (edition 2024, per `Cargo.toml`) for the backend; TypeScript / React 19 /
React Router v7 for the WebUI. Both are touched.

**Primary Dependencies**: existing only — `rusqlite` (bundled), `rig-core` for message shapes,
`chrono`, `serde`/`schemars`, `flume` for the transport. WebUI: `react-markdown`, `rehype-highlight`,
already present. **No new crate and no new npm package.** The collapsed trail is a native
`<details>`/`<summary>`, which `ExecutionReportView` already uses.

**Storage**: embedded SQLite. One nullable column (`seq`) plus one index on an existing table; no new
table. This is the project's first column addition to an existing table — see research Decision 2.

**Testing**: `cargo test` for the two pure, cheaply-testable pieces — the history entry round trip
(`messages_to_history_entries` → `history_entries_to_messages` producing valid message shapes) and
`seq` assignment/ordering against an in-memory connection, following the existing
`src/storage/memory.rs` test pattern. `cd webui && npm run typecheck` plus unit tests for the pure
history→turns grouping function. `cargo clippy`. End-to-end per [quickstart.md](./quickstart.md) as
dummyplug steps — **with two steps dummyplug cannot cover**. Dummyplug "never starts another tool call
in the same turn" and emits no assistant text alongside a tool call, so it cannot produce an
`AssistantMessage` at all; and it enforces no message-shape rules, so it would accept the very replay
shapes SC-005 exists to rule out. Quickstart steps 8 and 9 are marked **[LIVE]** against an
Anthropic-family provider. Everything else — `seq`, ordering, `intent`, nested-call suppression, the
single-tool trail, paging — is fully dummyplug-covered.

**Target Platform**: Linux/macOS/Windows single binary including static `musl`. No new system library,
so `Cross.toml` targets are unaffected.

**Project Type**: single Rust binary (agent framework) with a bundled WebUI. This feature spans both.

**Performance Goals**: one extra B-tree seek per history insert (the `MAX(seq)` lookup, indexed).
Scrolling 100 turns each with a folded trail stays as smooth as today (SC-012) — the folded trail
renders a summary line, not its contents.

**Constraints**: the collapsed trail is bounded at two lines regardless of content (FR-018, SC-006).
Ordering guarantees apply only to entries recorded after this change (FR-008); pre-existing rows keep
`NULL` `seq` and today's ordering.

**Scale/Scope**: ~38 functional requirements across 5 source areas. `session_history` grows without
bound, which is why no backfill pass over it is acceptable.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Re-checked post-design —
still passing, with one deviation recorded in Complexity Tracking.

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | PASS. No new crate, no new npm package, no new table, no config flag. One nullable column, one index, one trait method, one tool input. The `add_column_if_missing` helper is ~10 lines rather than a migration framework, and the backfill — the largest piece of work in the original plan — was removed outright. Deliberately *not* added: a schema-version counter, a correlation id on `ToolChoice`, `intent` on `ExecutionReport`. |
| **II. DRY via Trait-Based Extensibility** | PASS. Nested-call behaviour is a new `VizierSessionHook` method each hook answers for itself, defaulting to today's behaviour — no caller branches on "am I nested". On the WebUI side, the live stream and stored history feed **one** normalized trail model and **one** renderer, specifically so the two cannot drift. The `seq` counter is global partly to avoid restating the session-matching predicate that already appears three times in `history.rs`. |
| **III. Self-Contained, Zero-Dependency Runtime** | PASS. Nothing added needs a network or an external service. Storage stays embedded SQLite. The WebUI change is static assets built into the binary as before. |
| **IV. Portability by Default** | PASS. No `cfg`, no platform-specific path or process handling, no new system library. `PRAGMA table_info` and `ALTER TABLE ADD COLUMN` are core SQLite, available in the bundled build on every target. |
| **V. Unified Errors & Observability** | PASS. New fallible paths return `crate::Result<T>`; the column-add helper surfaces a real `VizierError` rather than swallowing SQLite errors (explicitly chosen over ignoring "duplicate column name"). No new `unwrap()`/`expect()`. No `println!`. |
| **Workflow: clippy + tests + typecheck** | PASS, planned. `cargo clippy`, `cargo test`, `npm run typecheck`. |
| **Workflow: conventional commits** | PASS. Ships `[**breaking**]` — two breaking changes, recorded in the spec's Breaking Changes section. |
| **Workflow: manual verification** | PASS. Touches storage schema and the HTTP/WebUI surface, so it must be run, not assumed. |
| **Workflow: dummyplug e2e gate** | PASS, with a stated limit. Agent-observable behaviour changes (hooks, tools, history), so quickstart.md scripts dummyplug steps. **Two checks are not coverable by dummyplug**: it never starts a second tool call in a turn and emits no assistant text beside a tool call, so it cannot produce an `AssistantMessage` to test narration with; and it enforces no message-shape rules, so it cannot verify SC-005's replay adjacency. Steps 8 and 9 are marked live-provider, as the constitution permits for behaviour depending on provider-specific parsing. |

**Gate result**: no unjustified violations. One justified deviation below.

## Project Structure

### Documentation (this feature)

```text
specs/010-webui-reasoning-display/
├── plan.md                    # This file
├── research.md                # Phase 0 — 8 decisions
├── data-model.md              # Phase 1
├── quickstart.md              # Phase 1 — dummyplug steps + 1 live-provider step
├── contracts/
│   ├── execute-python-input.md    # The `intent` input contract
│   ├── history-api.md             # `seq`, ordering, paging cursor
│   └── trail-model.md             # WebUI normalized trail + grouping contract
├── checklists/
│   └── requirements.md        # From /speckit-specify
├── spec.md
└── tasks.md                   # Phase 2 — NOT created by /speckit-plan
```

### Source Code (repository root)

```text
src/
├── storage/sqlite/
│   ├── mod.rs                 # + seq column in CREATE TABLE, + idx_sh_seq,
│   │                          #   + add_column_if_missing helper call
│   └── history.rs             # seq assignment on insert; ordering tie-break at
│                              #   every ordered read (5 sites); (timestamp, seq)
│                              #   paging cursor
├── schema/
│   └── history.rs             # narration recorded BEFORE sibling tool calls;
│                              #   replay merges narration + tool calls into one
│                              #   assistant message (pending_text)
├── agents/
│   ├── agent/mod.rs           # stop dropping AssistantMessage on the write path
│   │                          #   (2 sites: error path + success path)
│   ├── hook/
│   │   ├── mod.rs             # + on_nested_tool_call, default → on_tool_call
│   │   ├── tool_calls.rs      # override: pass through, emit nothing
│   │   └── thinking.rs        # override: pass through, emit nothing
│   └── tools/python/
│       ├── mod.rs             # + required `intent` on ExecutePythonInput;
│       │                      #   description text for it
│       └── bridge.rs          # dispatch calls on_nested_tool_call
└── channels/http/api/v1/agents/
    └── channel.rs             # stop filtering AssistantMessage on the read path;
                               #   + before_seq on HistoryQuery

webui/app/
├── interfaces/
│   ├── types.ts               # + seq on history entries; + Trail types
│   └── chat.ts                # (unchanged)
├── lib/
│   └── trail.ts               # NEW — pure: SessionHistory[] → Turn[]; live
│                              #   events → TrailEvent[]; unit-tested
├── components/
│   ├── ActivityTrail.tsx      # NEW — the folded/expanded trail, one renderer
│   │                          #   for both producers
│   ├── ThinkingIndicator.tsx  # live path emits TrailEvent[], renders via
│   │                          #   ActivityTrail
│   └── ExecutionReportView.tsx# intent as headline; code into the disclosure;
│                              #   DROP the tool_calls list
└── routes/chat.tsx            # attach the collected trail to the closing turn
                               #   instead of clearInlineEvents(); render stored
                               #   trails on load
```

**Structure Decision**: The repository is a single Rust binary with a bundled React WebUI, and this
feature spans both halves, so the existing split is used as is — no new top-level directory. One new
WebUI module (`lib/trail.ts`) is introduced to hold the grouping logic as a pure function, separate
from the component that renders it: that separation is what makes the ordering guarantees unit-testable
without a browser, and it is the seam where the two producers converge.

## Phase ordering and risk

Phase 0 and Phase 1 artifacts are complete. The implementation order that matters:

1. **Ordering first** (`seq` column, assignment, tie-break at all read sites). Everything downstream
   displays ordered history; building the UI against unordered data would make every rendering bug
   indistinguishable from an ordering bug.
2. **Narration write + replay together, in one commit.** These must not be separable — the write
   change alone produces history that a provider will reject on the next turn. This is the highest-risk
   step and the one SC-005 exists to gate.
3. **Nested-call suppression** (hook method + bridge + report view). Independent of 1 and 2.
4. **`intent`** on the tool. Independent.
5. **WebUI trail.** Depends on 1; reads better after 2 and 3.

Steps 3, 4 and 5's rendering work could proceed in parallel with 1–2 by someone else, but 2 should not
land split.

## Complexity Tracking

> Filled only for Constitution Check deviations needing justification.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| Two steps of the e2e gate run on a live provider rather than dummyplug | Narration cannot be produced on dummyplug at all — it never starts a second tool call in a turn and emits no assistant text beside one, so no `AssistantMessage` is ever written. And SC-005 asserts a provider that enforces tool-call/result adjacency and forbids consecutive same-role messages *accepts* the replayed history; dummyplug enforces neither. | Dummyplug-only verification would give a green e2e run while leaving the feature's highest-risk change — the one that breaks turns outright rather than merely looking wrong — entirely unchecked. The constitution already permits live-provider steps "where behavior depends on real model output ... provider-specific parsing", which is exactly this. The steps are marked `[LIVE]` in quickstart.md rather than quietly substituted, and the dummyplug limitation is named so a future reader does not assume it was laziness. |
