# Implementation Plan: Waking an agent in any conversation

**Branch**: `067-wake-session` | **Date**: 2026-11-26 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/067-wake-session/spec.md`

## Summary

Two built-in tools let an agent work in a conversation other than the one it is in: `list_sessions`
lists the conversations it may reach, and `wake_session` starts a turn in one of them with an
instruction. The woken agent runs as itself, in that conversation, with that conversation's history
in view; its answer lands there. A scheduled run can therefore put its work in the person's own
thread instead of a throwaway one, with no change to how jobs are scheduled.

The technical approach is to **reuse the transport, not to add a subsystem** (research D11). A wake
is a call, not a record: there is no table, no handle, no state machine and no panel. What it does
is dispatch one request into a session — `transport.send_request` already accepts any session
(`transport/mod.rs:428`) — and, when the caller asks to wait, read the reply frames the same way
the scheduler reads a task run's (`scheduler/mod.rs:334-397`).

Two decisions carry the rest. First, **a new request content kind, `Woken`** (research D3):
`Unattended` runs with an empty history, and `Chat` means "a person spoke" and would offer the
question tools the spec forbids. The codebase has made this same call twice before, for the
background report and for a worker's task. Second, **one parser that is also the allowlist**
(research D2): the function that turns a channel slug back into a channel accepts only the four
wakeable kinds, so the machinery channels are refused by construction rather than by a second list.

Access is the consultation access, unchanged (research D9): the person a turn is working for
decides which other agents it may reach, and a WebUI conversation belongs to its own person. The
hop limit is the background-work limit (research D7) and the chain is the existing chain, widened
(research D8). The tools land in their own appended category, `sessions` (research D13).

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 + React 19 (WebUI)

**Primary Dependencies**: existing only (tokio, axum, serde, schemars, flume; React Router 7,
Zustand, Tailwind 4). No new crates and no npm packages.

**Storage**: no schema change. The new content kind rides the existing `session_history` rows;
there is no new table, no new column and no migration (research D11, D15).

**Testing**: `cargo test --workspace` (unit tests for the parser, the access rules, the depth and
chain rules, the frontmatter and the refusals; `tests/dummyplug_e2e.rs`). `vitest` for the
WebUI's request-content rendering. `node --test scripts/trail-tool-categories.test.mjs` for the
engine/WebUI category agreement. End to end: dummyplug steps in [quickstart.md](quickstart.md).

**Target Platform**: the server (Linux, macOS, Docker) and the desktop app (macOS, Linux, Windows),
all through the same core.

**Project Type**: a Rust workspace (engine library + HTTP crate + binary) with a WebUI folder and a
design folder in the same repository.

**Performance Goals**: no new hot path. A listing reads the sessions the engine already reads for
the conversation picker; a wake costs one dispatch, plus one access check (one user read and one
agent-config read) and, when the caller waits, the frames of one turn.

**Constraints**: the WebUI keeps its own copy of the tool categories and the trail test fails when
the two disagree, so the new category must land on both sides in the same change. The content kind
must be classified at every exhaustive match on the enum, or the build fails — that is the point
of those matches, and it is where FR-023 is enforced. No change to the dummyplug protocol.

**Scale/Scope**: about 700 lines added across core and WebUI: two tools, one content kind, one
category, one parser. No HTTP route, no new screen, no new panel.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design: still passes.*

| Gate | Verdict |
|---|---|
| **I. Lean by default** | Pass. No table, no handle, no state machine, no new setting, no new dependency. The wake is a call over an existing transport method, the hop limit is the existing background limit, the chain is the existing chain, and the listing is a call to an existing storage method (D7, D8, D11, D15). The one new abstraction — the slug parser — replaces a list, it does not add one (D2). |
| **II. DRY via traits** | Pass. No new trait and no new `match` over a kind in a call site: the new content kind is classified by the existing exhaustive matches, which is how every other kind is handled. The access rules are the consultation module's, not a copy (D9). The tools are one `VizierTool` impl each, like every other tool. |
| **III. Self-contained default** | Pass. Nothing needs a new service. A wake is an in-process request (or a bus request in a Fleet), exactly as a consultation is. |
| **IV. Portability** | Pass. There is no platform-specific code and no new dependency. |
| **V. Unified errors and observability** | Pass. Every refusal is a `VizierError`-free value returned as data (FR-026), as the consultation refusals already are. Failures are `crate::Result` with `throw_vizier_error`. Logging goes through `tracing`. |
| **Migration code tracked (#82)** | Not required. Nothing is removed and no data changes shape: the content kind is new, the chain field keeps its name and type. |
| **Design before UI code** | Not required, and the reason is stated: the feature adds no screen, no panel and no layout. The WebUI's work is a new category in the trail's own map and one more arm in the request-content union — a message that already exists, drawn the way a worker's task is drawn (contracts/session-tools.md). |
| **Matching submodule branches** | Required: run `just checkout 067-wake-session`, because the `speckit.git.feature` hook branched only the workspace. `landing` and `dummyplug` are submodules; `design/` and `webui/` are folders of this repository. The change touches the workspace only, but the branch rule still applies. |
| **dummyplug e2e** | Pass. [quickstart.md](quickstart.md) scripts every story as dummyplug steps, and none needs a live model: every check is about which tool ran, what was refused, and where the reply landed. |
| **Feature flags / roles** | Pass. No new optional capability. A wake rides the transport like any request, so a networked role needs nothing extra. The two tools are in the default toolset, as every built-in is. |
| **Verify what a change touches** | Pass. The CI paths are core and webui, which run their usual jobs. Nothing in `Cargo.toml` changes, so the full matrix is not triggered. |

No violations, so Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/067-wake-session/
├── spec.md                      # the feature
├── plan.md                      # this file
├── research.md                  # D1–D16
├── data-model.md                # the woken request, the address, the tool types
├── quickstart.md                # dummyplug checks
├── contracts/
│   └── session-tools.md         # the two tools, the addresses, the refusals, the WebUI's side
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/
│   ├── tools/
│   │   ├── sessions/mod.rs       # NEW: the two tools, the slug parser, the access check
│   │   ├── sessions/tests.rs     # NEW: parser, refusals, depth, chain, frontmatter
│   │   └── mod.rs                # ToolCategory::Sessions (appended); ALL 24→25; EXPECTED entries;
│   │                             #   every_tool() entries; the two tools in the default toolset
│   ├── process.rs                # Woken in the history arm; notifies_chat includes it
│   └── agent/system_prompt/      # (unchanged — the framing rides the content kind, D3)
├── schema/request.rs             # VizierRequestContent::Woken; WokenRequest; the frontmatter arm;
│                                 #   consultation_chain's doc comment widened (D8)
├── consultations/access.rs       # (read-only reuse: consulter, consultable, may_consult — D9, D10)
├── utils/mod.rs                  # tool_call_title: the two tools' lines
└── tests/dummyplug_e2e.rs        # the e2e cases

webui/app/
├── api/frames.ts                 # the woken request content in the content union
├── api/schema.d.ts               # regenerated
├── lib/toolCategories.ts         # 'sessions' in BuiltinCategory; the two tools in BUILTIN_TOOLS
└── lib/toolIcons.ts              # the category's label, icon and phrase

webui/openapi.json                # regenerated
crates/vizier-core/public-api.txt # regenerated
scripts/trail-tool-categories.test.mjs   # NOT edited: it is the test that fails when the two sides drift
CLAUDE.md                         # the SPECKIT pointer; the tools section; the category count
```

**Structure Decision**: the existing layout. The tools go in `vizier-core` under `agents/tools/`,
in their own module beside `webui/`, `telegram/` and `discord/`; the content kind goes in
`schema/request.rs` where the other kinds live; the WebUI's two copies are updated in place. No
new crate, no new HTTP route, nothing in the binary crate.

## Order of work

1. **The content kind and its classification** (no behaviour an agent can see yet): `Woken` and
   `WokenRequest` in `schema/request.rs`, the frontmatter arm, the `consultation_chain` doc
   comment, the history arm and `notifies_chat` in `agents/process.rs`, the `is_interactive_turn`
   arm in `agents/tools/mod.rs`. The exhaustive matches will not compile until each is classified,
   which is the checklist.
2. **The tools**: the new module, the slug parser, the access check, the depth and chain rules,
   the two tools, the category, the wiring, the `tool_call_title` lines. Unit tests. This delivers
   Stories 1, 2, 3 and 5 to agents.
3. **The WebUI**: the request-content union, the category map, the icon map, the regenerated
   schema, the `tool_call_title` lines' counterpart in the trail. `just web-check`.
4. **The scheduled run's use of it** (Story 4): no scheduler change — the agent in the run uses
   the same tools. What lands is the quickstart step and, if the agent's doctrine needs it, one
   sentence in the operating doctrine saying a job may name the conversation to work in (FR-027).
5. **Docs and verification**: `CLAUDE.md`, the regenerated public API, the dummyplug cases, and
   the full [quickstart.md](quickstart.md) run against a live binary.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations: the Constitution Check above passes on every gate, so this table is empty.
