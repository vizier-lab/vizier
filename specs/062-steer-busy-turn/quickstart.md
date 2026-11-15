# Quickstart: verifying steering end to end (dummyplug)

All steps are offline: one agent on the `dummyplug` provider, driven over the topic WebSocket. They are scripted as `scripts/steer-e2e.mjs` (same shape as `scripts/client-tools-e2e.mjs`: log in, open `…/topic/{topic}/stream`, send frames, read history over REST). Needs the dummyplug additions in [contracts/dummyplug-steer.md](contracts/dummyplug-steer.md).

## Setup

```bash
just run
```

Create an agent `steer-test` on provider `dummyplug` (WebUI or `POST /api/v1/agents`). Each step uses a fresh topic.

## E1 — A message steers the running turn (Story 1, FR-001/002/005, SC-001)

1. Send `{"tool": "current_time", "arguments": {}, "delay_ms": 2000}`.
2. Within 2 s, send `keep the old token format`.
3. **Expect** the turn's single reply: `**Read mid-turn**:\nkeep the old token format`.
4. **Expect** history, in order: `Request(chat: {"tool"…})`, `ToolCall current_time`, `ToolResult`, `Request(chat: "keep the old token format", mid_turn: true)`, `Response(Read mid-turn…)`. No second `Response`.
5. **Expect** no `session_turn` rows for the topic after the reply.

## E2 — Several messages at one boundary (Story 3)

Same as E1, sending `one`, `two`, `three` within the delay. **Expect** `**Read mid-turn**:\none\ntwo\nthree` and three `mid_turn` requests in that order.

## E3 — `/queue` and the `queue` field keep today's behaviour (Story 5, FR-011)

1. Send the delayed tool request; within the delay send `/queue lorem please` (or `lorem please` with `"queue": true` on the frame).
2. **Expect** reply 1 = the §4 tool-result echo (no `Read mid-turn`), then a second turn whose request text is `lorem please` (no `/queue`) and whose reply is lorem ipsum.
3. Send `/queue` alone while idle. **Expect** the usage line, nothing saved in history.

## E4 — No boundary left: fall back to the queue (Story 2, FR-004)

1. Send `{"say": "done", "delay_ms": 2000}` (a final answer after 2 s, no tool call, so no round boundary).
2. Within 2 s, send `second`.
3. **Expect** reply 1 = `done`; then `second` runs as its own turn with its own reply (lorem ipsum), and its request has no `mid_turn`.

## E5 — Abort while a message waits (FR-009)

1. Delayed tool request; within the delay send `wait`, then `/abort`.
2. **Expect** no `Request(mid_turn)` for `wait`, no turn for it afterwards, no `session_turn` rows.

## E6 — Restart mid-turn (FR-003, SC-002)

1. Delayed tool request with `delay_ms: 5000`; within it send `after restart`; then restart the agent (`PUT /agents/{id}` with an unchanged config, or `vizier shutdown` + `just run` for the crash path).
2. **Expect** the resumed turn's reply is `**Read mid-turn**:\nafter restart` (or, if the restart landed after the boundary, the message already in history and not re-run) — in every case `after restart` appears exactly once in history and is answered once.

## E7 — Excluded kinds (FR-007/008)

While a delayed turn runs: `/btw what time is it` → answered beside the turn, nothing steered; `/skills` → answered after the turn as today; a scheduled task firing on the agent → unaffected.

## E8 — Chat platforms (manual, needs a Discord or Telegram test bot)

Same as E1 through the platform: the reply is posted **once**, no "agent unavailable" message for the steered message; `/queue <message>` behaves as E3; on Discord `/queue` appears in the slash-command list.

## E9 — WebUI (manual, after the design review)

1. While a delayed turn runs, type a message: the composer shows the busy hint; press Enter → "Waiting for the agent" row → it moves into the turn: trail · bubble · trail · reply, both parts labelled "Worked".
2. Reload and open a second tab: identical order.
3. Ctrl+Enter (Cmd+Enter on macOS) instead → "Queued · runs after this turn", then its own turn.
4. Phone width: no hint; `/queue …` works.
