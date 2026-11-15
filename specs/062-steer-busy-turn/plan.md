# Implementation Plan: Steer a Busy Turn

**Branch**: `062-steer-busy-turn` | **Date**: 2026-11-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/062-steer-busy-turn/spec.md`

## Summary

A person's message sent while their conversation's agent is running a turn is delivered **into that turn** at its next round boundary — after the round's tool results, before the next completion, in the same place background work is delivered today (spec 058/059). Steering is the default; Ctrl+Enter / Cmd+Enter in the WebUI or `/queue <message>` on any channel keeps today's behaviour (a turn of its own, after this one). A message the turn never reaches (it ends without another round) runs as its own turn in arrival order, as today. Silent reads (group-channel messages not addressed to the agent, unread Workspace entries) are taken in at every boundary too, recorded where they were read, with no chime-in of their own (R12).

Technically: the per-session queue in `agent_process` becomes a shared handle the running chat turn can take from (R1); each item carries a mode decided once by a pure predicate (R3; `/queue` is parsed once in core, R4); the boundary removes the item's `session_turn` row and saves its history entry **in one transaction** (R2), so a message is never lost or answered twice across aborts, restarts and crashes. A steered message is an ordinary `Chat` request flagged `mid_turn` (R5): the model sees it with `via: mid_turn` (R6), its own sender gets a terminal `Steered` frame (R7), and the WebUI splits the turn's trail at it — trail, message, trail, reply, both trail parts labelled "Worked" — from stored history alone (R8).

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 / React 19 for `webui/` and `design/`

**Primary Dependencies**: existing only — tokio, serde, the in-tree `llm` layer, React Router 7, Zustand, Tailwind v4. No new crates or npm packages.

**Storage**: SQLite (default) and PostgreSQL through the shared `SqlStorage`; one new fenced `TurnStorage` method (`steer_turns`), no schema change (new request fields ride the existing JSON columns of `session_turn` and `session_history`).

**Testing**: `cargo test --workspace` (unit tests for `steer::mode`, `strip_queue_command`, `opens_turn`, the replay merge, and `steer_turns` against SQLite in `storage/sqlite/fence_tests.rs`-style tests including the abort race); `cd webui && npm run typecheck` and vitest for `groupHistory`/`frames.ts`; design Storybook stories for the composer hint and the split trail. End-to-end: dummyplug steps in [quickstart.md](quickstart.md), using the `delay_ms` key and §8 echo added to dummyplug ([contracts/dummyplug-steer.md](contracts/dummyplug-steer.md)).

**Target Platform**: the vizier server (Linux/macOS/Docker) and the desktop app (macOS/Linux/Windows); WebUI in browsers and the app's webview.

**Project Type**: multi-crate web service + embedded SPA + desktop shell.

**Performance Goals**: a boundary with nothing waiting costs one mutex lock and an empty check (no storage call); a steered message reaches the next completion with one transaction of storage work.

**Constraints**: exactly-once per message (FR-003); the system prompt stays byte-identical across turns (the new BOOT.md sentence is fixed); live and replayed prompts agree (spec 060 rule); no change for non-person sessions (FR-008).

**Scale/Scope**: ~6 core files touched (`agents/process.rs`, `agents/agent/mod.rs`, `agents/steer.rs` new, `schema/request.rs`, `schema/history.rs`, `storage/turn.rs` + SQL impl), 3 channel files (commands list, Discord/Telegram renderers), `workspaces/commands.rs`, 1 design component (`Composer`) and 3 WebUI files (`connectionStore`, `lib/frames.ts`, `lib/trail.ts` + the turn renderer), dummyplug.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|---|---|---|
| I. Lean by default | ✅ | No new crate, table, config key or trait. One new module (`agents/steer.rs`) holding two pure functions and the inbox type; one storage method. |
| II. DRY via traits | ✅ | Steering reuses the delivery boundary; request preparation is extracted into one helper used by turn start and boundary (R6); turn-start detection collapses into `VizierRequest::opens_turn()` (R5); `/queue` parsed once in core, not per channel (R4). No `match` over channel kinds added. |
| III. Self-contained | ✅ | No external service. |
| IV. Portability | ✅ | Ctrl+Enter vs Cmd+Enter is a browser key check in `design/` (`metaKey` on macOS), no `cfg`. |
| V. Errors & observability | ✅ | `crate::Result`; `tracing` for the race-drop path (`debug!`). |
| Fenced writes (#164) | ✅ | `steer_turns` calls `check_fence` in its transaction; a test row is added to `fence_tests.rs`. |
| Migration code → #82 | ✅ | None added (new fields default to false; old rows read unchanged). |
| Design before UI code | ✅ | FigCraft unreachable; ASCII mockups in [contracts/webui.md](contracts/webui.md) approved 2026-11-15, including the copy "Waiting for the agent", "Queued · runs after this turn" and the busy hint. |
| dummyplug e2e | ✅ | [quickstart.md](quickstart.md) scripts every story; dummyplug protocol change documented and kept backward compatible. |
| Matching submodule branches | ⚠ to do | `just checkout 062-steer-busy-turn` before touching `design/`, `webui/`, `dummyplug/` (the spec hook created the workspace branch only; submodules currently have uncommitted changes, so run it once those are settled). Commit order design → webui → dummyplug → vizier. |
| Conventional commits | ✅ | e.g. `feat(agents): steer a busy turn with a person's message`. |

Post-design re-check: unchanged — all gates pass; the mockups were approved 2026-11-15.

## Project Structure

### Documentation (this feature)

```text
specs/062-steer-busy-turn/
├── plan.md              # This file
├── research.md          # R1–R11
├── data-model.md        # Queued item, request fields, SteeredTurn, frames
├── quickstart.md        # dummyplug end-to-end steps
├── contracts/
│   ├── steering.md      # engine contract: modes, boundary, exactly-once, lifecycle
│   ├── queue-command.md # /queue grammar per channel, Ctrl+Enter, request field
│   ├── webui.md         # waiting row, split trail, composer hint (ASCII mockups)
│   └── dummyplug-steer.md # delay_ms and the §8 "Read mid-turn" echo
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/
│   ├── steer.rs                 # NEW: Mode, mode(), strip_queue_command(), SteerInbox, take()
│   ├── process.rs               # shared SessionQueue with mode; /queue on receipt; abort clears it
│   ├── agent/mod.rs             # boundary: take → steer_turns → prepare → one user message
│   ├── agent/system_prompt/     # BOOT.md: one fixed sentence on `via: mid_turn`
│   ├── steps.rs                 # seal_open_calls uses opens_turn()
│   └── tools/mod.rs             # ToolContext.steer
├── schema/
│   ├── request.rs               # queue, mid_turn fields; opens_turn(); frontmatter via: mid_turn
│   ├── response.rs              # VizierResponseContent::Steered; ends_turn
│   └── history.rs               # replay merges adjacent user-side entries after a round
├── storage/turn.rs              # steer_turns (trait)
├── storage/sqlite/{turn,history}.rs # one-transaction impl; write_history_entry helper
├── storage/sqlite/fence_tests.rs    # steer_turns refused under a newer owner
├── skill/invocation.rs          # RESERVED += "queue"
├── channels/commands.rs         # builtin `queue`
├── channels/discord/mod.rs      # builtin entry; renderer ends on Steered
├── channels/telegram/mod.rs     # menu entry; renderer ends on Steered
└── workspaces/commands.rs       # /queue @agent <message>

crates/vizier-http/src/…/topic.rs # stamp_on_receipt clears mid_turn

design/src/components/Composer/  # onSubmit(…, { queue }), Ctrl/Cmd+Enter, `hint` prop, story
webui/app/
├── stores/connectionStore.ts    # queue flag on the request
├── lib/frames.ts                # queued match after /queue strip
├── lib/trail.ts                 # groupHistory: steer block in the open turn
└── screens/chat/…               # turn renderer cuts the trail at steer blocks

dummyplug/src/protocol.mjs       # delay_ms; §8 Read mid-turn
specs/008-dummyplug-provider/contracts/dummyplug-protocol.md  # §3 + §8
docs/src/…                       # chat behaviour page: steering and /queue
```

**Structure Decision**: Engine changes live in `vizier-core` (the queue, boundary, storage, channels); `vizier-http` only clears the engine-only field on receipt; UI changes follow design → webui; the test-harness change lives in the `dummyplug` submodule with its protocol doc in this repo.

## Complexity Tracking

No constitution violations to justify.
