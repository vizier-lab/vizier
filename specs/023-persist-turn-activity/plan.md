# Implementation Plan: Persist In-Progress Turn Activity

**Branch**: `023-persist-turn-activity` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/023-persist-turn-activity/spec.md`

## Summary

While a turn runs, its activity list (thinking, tool calls, python reports) exists only in the
open chat page. The server writes it to history at turn end, and the WebUI closes its socket when
you leave the chat page, so coming back, reloading or opening a second tab shows only later steps.

**Approach**: `VizierTransport` keeps the running turn's trail frames per session, in memory,
updated inside `publish_session_event`, which every WebUI turn frame already passes through. A
socket that opens on a session with a running turn gets one `turn_snapshot` frame first. Taking
that snapshot is atomic with subscribing to the session broadcast, so no frame is lost or
duplicated. The WebUI rebuilds its live list from the snapshot with the same `appendLiveEvent`
it uses for live frames. No storage, agent loop or hook changes.

## Technical Context

**Language/Version**: Rust 2024 (crate edition as in `Cargo.toml`); TypeScript 5 / React 19

**Primary Dependencies**: `tokio::sync::broadcast`, `parking_lot` (both already present); axum WebSocket; Zustand-style store in the WebUI

**Storage**: N/A. An in-memory buffer only; no tables or migrations.

**Testing**: `cargo test` (unit test of the buffer transitions and the exactly-once snapshot in `src/transport.rs`), `npm run typecheck`. End-to-end: dummyplug steps in [quickstart.md](quickstart.md), scripted as `scripts/turn-snapshot-e2e.mjs`, with the turn held open by a client tool call.

**Target Platform**: the vizier binary (all `Cross.toml` targets) and the bundled WebUI

**Project Type**: web service + bundled web app (single repo)

**Performance Goals**: the snapshot is the socket's first frame, so the list appears as soon as the socket opens (SC-002 ≤ 1 s)

**Constraints**: exactly-once delivery across snapshot and live frames (FR-003); one mutex acquisition per published frame, with no `.await` while it is held

**Scale/Scope**: one buffer entry per session with a running turn, cleared at the turn's end. About 5 files changed.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status |
|---|---|
| I. Lean by Default | ✅ No new dependency, trait or config. One map and one method on an existing type, one frame, one store field. |
| II. Trait-based extensibility | ✅ No dispatch over a type tag is added. The buffer classifies `VizierResponseContent` once, in one function, the way the WebUI already does. Nothing varies by channel, provider or tool. |
| III. Self-contained runtime | ✅ In-process memory only. |
| IV. Portability | ✅ No platform-specific code. |
| V. Unified errors & observability | ✅ Nothing fallible is added. A failed `is_thinking` read on connect is logged with `tracing::warn!` and treated as "no snapshot". |
| Migration/back-compat tracking (#82) | ✅ None added. The frame is additive. |
| `cargo clippy` / `cargo test` / `npm run typecheck` | Required at the end (tasks). |
| Dummyplug e2e gate | ✅ Planned in quickstart.md §1–§6. §7 is a WebUI walkthrough. |

**Post-design re-check**: unchanged. All gates pass, and Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/023-persist-turn-activity/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/turn-snapshot.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── transport.rs                                  # running-turn buffer; publish updates it; subscribe_session_with_turn
└── channels/http/api/v1/agents/channel.rs        # handle_socket: subscribe with snapshot, is_thinking check, send turn_snapshot first

webui/app/
├── interfaces/types.ts                           # TurnSnapshotFrame type
├── hooks/connectionStore.tsx                     # route turn_snapshot frames into `turnSnapshot` state; reset on connect/disconnect
├── lib/trail.ts                                  # foldLiveEvents(frames) over appendLiveEvent (+ test in trail.test.ts)
└── routes/chat.tsx                               # apply turnSnapshot before the lastMessage effect

docs/src/api-integration/rest-api.md              # turn_snapshot row in the WebSocket frame table
scripts/turn-snapshot-e2e.mjs                     # quickstart §1–§6
```

**Structure Decision**: a server-side change in the transport, where session frames are already
published, and a client-side change limited to the connection store and the chat route. The
Discord/Telegram `woken.rs` readers keep using `subscribe_session_events` unchanged.

## Design notes

- **`publish_session_event`** becomes: lock `running_turns`, update it for
  `SessionFrame::Response` (see data-model transitions), call `session_events.send`, unlock.
  All existing callers keep their signature.
- **`subscribe_session_with_turn(&VizierSession) -> (Receiver<SessionEvent>, Vec<VizierResponse>)`**:
  lock, `subscribe()`, clone the entry, unlock. **`discard_turn(&VizierSession)`** removes a stale
  entry.
- **`handle_socket`**: replace `subscribe_session_events()` with the call above. If the frames
  are non-empty, read `get_session_detail_by_topic(...)`. If `is_thinking` is set, send
  `{"turn_snapshot": {"frames": …}}` through `write_tx` before the loop. Otherwise call
  `discard_turn`. Since `write_tx` is the socket's only writer and the loop hasn't started, the
  snapshot goes out before any live frame.
- **WebUI**: `connectionStore.onmessage` recognises `turn_snapshot` before the `lastMessage`
  fallthrough and sets `turnSnapshot: { agentId, topicId, frames }`. In `chat.tsx`, an effect
  declared above the `lastMessage` handler applies it when it matches the page:
  `setThinking(true)`, then `setLiveTrail(() => foldLiveEvents(frames))`.

## Complexity Tracking

No violations.
