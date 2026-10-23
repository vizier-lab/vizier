# Quickstart: Persist In-Progress Turn Activity

All checks run offline against a `dummyplug` agent (constitution e2e gate). Dummyplug runs one tool
per turn, so the turn is held open with a **client tool** (spec 019): the turn waits until the
declaring socket answers, which gives a deterministic "still running" state.

The checks are scripted in `scripts/turn-snapshot-e2e.mjs`, written in the same style as
`scripts/client-tools-e2e.mjs` (Node ≥ 22, no dependencies):

```sh
just run   # with an agent `plug` on the dummyplug provider
VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/turn-snapshot-e2e.mjs
```

## §1 No running turn: no snapshot

1. Open socket **A** on a fresh topic. Send `hello`, then wait for the `message` reply.
2. Open socket **B** on the same topic.
3. **Expect**: B receives no `turn_snapshot` within 1 s.

## §2 Mid-turn reconnect restores the trail (US1, FR-002/003)

1. On socket **A**, declare client tool `hold` (category `e2e`, input `{}`) and send
   `{"tool":"execute_python","arguments":{"intent":"hold","code":"hold()"}}`.
2. Wait for A to receive `thinking_start`, `tool_choice` (`execute_python`) and the
   `client_tool_call` for `hold`. **Don't answer it.**
3. Open socket **B** on the same topic.
4. **Expect**: B's first frame is `turn_snapshot`. Its `frames`, compared by content, equal the
   `thinking_start` and `tool_choice` frames A received, in the same order.
5. On A, answer the `hold` call with `{"ok": true}`.
6. **Expect**: B then receives the `tool_response` (the script's report) and the final `message`
   once each, and receives no second copy of any frame from step 4.

## §3 Steps taken while away (US1 scenario 2)

§2 already covers this: B was not connected when `thinking_start` and `tool_choice` were
published, and receives them in the snapshot. Dummyplug can't produce a trail frame *after* a
held client call and still keep the turn open (one tool per turn, and the script's report comes
only when the script ends), so a separate "left and came back" round trip adds nothing here. The
buffer filling with no subscriber at all is covered by the unit test in `src/transport.rs`, and
the round trip in the WebUI by §7.1.

## §4 Turn ended while away (edge case, FR-007)

1. Run §2 to completion with B closed before step 5, then reopen B after the `message`.
2. **Expect**: no `turn_snapshot`. History (`GET …/history`) holds the full trail and the reply.

## §5 Aborted while away (FR-007)

1. Hold a turn as in §2 steps 1–2, close B, and send `{"command":"abort"}` on A.
2. Reopen B. **Expect**: no `turn_snapshot`.

## §6 Isolation (FR-006)

1. Hold a turn on topic `t1`, then open a socket on topic `t2`.
2. **Expect**: no `turn_snapshot` on `t2`.

## §7 WebUI by hand (US1/US2, FR-004/005)

Use an agent on a live provider (marked as such: the WebUI needs real tool activity to look at),
or dummyplug with the `hold` approach from the WebUI's own client tools.

1. Ask for something that makes several tool calls. After two activity entries appear, open
   Settings, wait for another step, then go back to the chat. **Expect**: every entry, in order,
   with none duplicated, and still updating live.
2. Reload mid-turn. **Expect**: the same.
3. Open the topic in a second tab mid-turn. **Expect**: both tabs show the same list.
4. Let the turn finish. **Expect**: the folded trail above the answer matches the one in a tab
   that never left. Reload, and it still matches.

## Gates

- `cargo clippy`, `cargo test` (the new unit test for the buffer transitions and the exactly-once
  snapshot, in `src/transport.rs`).
- `cd webui && npm run typecheck`, and `npx vitest run app/lib/trail.test.ts` if the fold helper
  is added there.
