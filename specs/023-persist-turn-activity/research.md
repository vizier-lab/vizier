# Research: Persist In-Progress Turn Activity

## What happens today (traced)

- **Producers.** A turn's live activity is a set of `VizierResponse` frames sent on the request's
  `response_tx`: `ThinkingStart` (`agents/process.rs::start_session_job`), `Thinking`
  (`hook/thinking.rs`, from the `think` tool), `ToolChoice` (`hook/tool_calls.rs`) and
  `ToolResponse` (only `execute_python`'s report, same hook). The turn ends with one of
  `Message`, `AudioReply`, `Error`, `Empty`, `Abort` or `Checkpoint`.
- **Delivery.** For a WebUI turn, `handle_socket` (`channels/http/api/v1/agents/channel.rs`)
  forwards every frame from `response_rx` onto `VizierTransport::publish_session_event`. A turn woken by a
  background report is published the same way (`agents/background/`). Every socket on the
  session receives it through `subscribe_session_events`. A socket that subscribes later receives
  only later frames.
- **Storage.** The trail reaches history only when the turn ends
  (`messages_to_history_entries`, `agents/agent/mod.rs` ~716/754). Mid-turn, history holds the
  request and any client tool calls (saved when made, spec 019), and nothing else.
- **WebUI.** `layout.tsx` calls `disconnect()` whenever the route has no topic and `connect()`
  when it has one. Leaving the chat page closes the socket, and coming back opens a new one, so
  `handle_socket` runs again. `routes/chat.tsx` keeps the running trail in `useState`
  (`liveTrail`), built with `appendLiveEvent` from each frame, and wiped on unmount. The
  `is_thinking` poll restores the indicator but not the list.
- **No elapsed timer exists.** `ThinkingIndicator` renders the agent name and trail only. The
  spec's original US3 assumed one, so it was removed from the spec rather than adding a new timer.

## Decision 1: Keep the running turn's frames in memory at the publication point

**Decision**: `VizierTransport` keeps, per session, the trail frames of the turn currently
running (`ThinkingStart`, `Thinking`, `ToolChoice`, `ToolResponse`), updated inside
`publish_session_event`. A `ThinkingStart` starts a fresh buffer. A trail frame appends,
creating the buffer if none exists. Any other `Response` frame (a turn's end) removes it.

**Rationale**: Every frame the WebUI can show already passes through this one function, so the
buffer holds exactly what a person who stayed would have received (FR-004/FR-005). That holds
for person-started and background-woken turns alike, with no change to the agent loop, the
hooks, or storage. A running turn already ends as `interrupted` on a server restart, so a
buffer that disappears with the process matches the spec's assumption.

**Alternatives considered**:
- *Write the trail to history as it happens.* This would make mid-turn history complete, but it
  changes the one-to-one `messages_to_history_entries` contract and its ordering guarantees
  (spec 010 FR-011). It also changes what `history_entries_to_messages` replays into the model's
  context for every later turn, and adds a database write per step. That is far more risk than
  the problem needs.
- *Keep the trail in a WebUI store that outlives the page.* This would fix in-app navigation
  only: the socket is closed while you are away, so steps taken then are never received (fails
  US1 scenario 2), and it does nothing for reloads or second tabs (fails US2).
- *Buffer in `agents/process.rs` (`SessionTurns`).* It knows the exact turn lifecycle, but
  frames reach the WebUI through `handle_socket`'s forwarder rather than from there, so a second
  copy of the routing would be needed. The transport already owns the session broadcast.

## Decision 2: Snapshot and subscribe atomically

**Decision**: A new `VizierTransport::subscribe_session_with_turn(&session)` takes the buffer's
lock, subscribes to the broadcast, and clones that session's buffer. `publish_session_event`
updates the buffer and calls `broadcast::Sender::send` while holding the same lock.

**Rationale**: Because both happen under one lock, every frame is either in the snapshot
(published before the subscribe) or delivered to the new receiver (published after), never both
and never neither (FR-003, the "return in the instant a step lands" edge case).
`broadcast::send` is synchronous and cheap, so holding a `parking_lot::Mutex` (already a
dependency) across it is fine. The existing `subscribe_session_events` stays for subscribers
that don't need a snapshot (Discord/Telegram `woken.rs`).

**Alternatives considered**: subscribe first, then snapshot, and de-duplicate on the client by
timestamp. Timestamps are not unique (several frames can share a millisecond), and this pushes
correctness into every client.

## Decision 3: One `turn_snapshot` frame, sent first, only while a turn runs

**Decision**: When the buffer is non-empty, `handle_socket` sends
`{"turn_snapshot": {"frames": [<VizierResponse>, …]}}` as the socket's first frame, before
entering its loop. If the buffer is non-empty but storage's `is_thinking` for the session is
false, the buffer is stale: it is dropped and nothing is sent.

**Rationale**:
- A single frame lets the client replace its list in one step. Replaying N ordinary frames
  through `lastMessage`, which keeps only the latest frame, would depend on React rendering
  between socket messages.
- The frames are the same `VizierResponse` values the client already decodes. It rebuilds the
  list by folding them through the existing `appendLiveEvent`, so the restored list matches a
  live one by construction (FR-004).
- The `is_thinking` check covers a turn that ended without any terminal frame (FR-007), so no
  stale list is ever presented as live. Its only false negative is the first instant of a turn
  (`ThinkingStart` published, `is_thinking` not yet written). In that case the buffer holds
  nothing but `ThinkingStart`, and the page's existing one-second `is_thinking` poll turns the
  indicator on anyway.
- Access is unchanged (FR-008): the snapshot goes only to a socket that the chat route has
  already authorised for that session, and sessions are per user (`HTTP(username, channel)`).

## Decision 4: WebUI consumes the snapshot through connection-store state

**Decision**: `connectionStore` sets `turnSnapshot: { agentId, topicId, frames }` when the
frame arrives. It clears the value on `connect()`/`disconnect()`, so a snapshot can't outlive
its socket or topic. `routes/chat.tsx` applies it in an effect declared before the
`lastMessage` effect: `setThinking(true)` and `setLiveTrail(() => fold(frames))`.

**Rationale**: Unlike a listener, state does not depend on the chat page registering before the
first frame arrives, so it is safe whichever of the layout and chat page effects runs first.
Declaring the effect before the `lastMessage` effect means a live frame delivered in the same
render is appended after the restored list, not overwritten by it.

**Alternatives considered**: a listener set like `subscribeReactionFrames`. It would work today
because child effects run before the layout's `connect()`, but correctness would then rest on
that ordering.

## Risks noted, not addressed

- A socket that falls behind the broadcast (`RecvError::Lagged`) drops frames, as it does today.
  Reconnecting now recovers the full list, which is an improvement, but a lag while connected
  still isn't repaired. This is out of scope.
- The buffer is unbounded within a turn and cleared at the turn's end. `ToolResponse` frames are
  `execute_python` reports, which are already bounded by the sandbox's output limits. The buffer
  holds one entry per session with a running turn.
