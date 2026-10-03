# Quickstart: WebUI Reasoning & Tool Activity Display

End-to-end verification, scripted as dummyplug steps per the constitution's e2e gate.

**Read this first — dummyplug cannot cover everything here.** Section 2 of the dummyplug protocol
contract states "Dummyplug never starts another tool call in the same turn", and dummyplug emits no
assistant text alongside a tool call. So a dummyplug turn is exactly
`Request → ToolCall → ToolResult → Response`. That is enough for `seq`, ordering, `intent`, nested-call
suppression and the single-tool trail. It cannot produce **narration**, and it enforces **no
message-shape rules**, so the highest-risk change in this feature — the narration write/replay pair —
needs a live provider. Those steps are marked **[LIVE]** and must not be skipped or substituted.

## Setup

```sh
just install          # first time only — build.rs needs webui/node_modules
just run              # or: cargo run -- run --config dev.vizier.yaml
```

Create an agent on the offline provider through the WebUI or HTTP API:

- provider: `dummyplug` (no credentials, no network)
- tools: enable `tools.python.enabled` **and** `tools.python.code_mode` — step 5 needs a script that
  calls tools
- note its `agent_id`

Open the WebUI at the configured port and start a topic with this agent.

---

## 1. `seq` is assigned, and the column survives an upgrade

**Why**: FR-001, FR-006, FR-007, research D2 — the project's first column addition to an existing table.

**Upgrade path** (do this *before* the other steps, on a database created by the previous build):

1. Check out the previous commit, `just run`, send one message, shut down.
2. Check out this branch, `just run`.
3. The startup must succeed, not panic on a missing column.

```sh
sqlite3 "$VIZIER_DATA_DIR/vizier.db" \
  "SELECT content_type, timestamp, seq FROM session_history ORDER BY rowid;"
```

**Expect**: rows written by the old build have `seq` empty (`NULL`); every row written after the upgrade
has an increasing integer. No row has a duplicate `seq`.

**Also expect**: restarting again adds no second column and logs no error — the `PRAGMA table_info`
guard makes the `ALTER` a no-op.

## 2. Ordering survives a same-millisecond tie

**Why**: FR-002, FR-003, SC-002. A turn's entries are flushed in one tight loop, so a `ToolCall` and its
`ToolResult` normally share a millisecond. That two-entry tie group is enough to catch the defect.

Send `tools`, then `read_core`, then the sample JSON it returns — one full tool turn.

```sh
sqlite3 "$VIZIER_DATA_DIR/vizier.db" \
  "SELECT timestamp, seq, content_type FROM session_history ORDER BY timestamp, seq;"
```

**Expect**: the `ToolCall` and `ToolResult` share a `timestamp` and differ in `seq`, with the call's
`seq` lower.

Now fetch the same conversation through the API and confirm the order matches:

```sh
curl -s -H "Authorization: Bearer $TOKEN" \
  "localhost:$PORT/api/v1/agents/$AGENT/channel/$CHANNEL/topics/$TOPIC/history" \
  | jq -r '.data[] | "\(.seq)\t\(.content | keys[0])"'
```

**Expect**: `Request`, `ToolCall`, `ToolResult`, `Response` in that order, `seq` ascending.

**Repeat the fetch 20 times** (SC-002 asks for order stability, not a single lucky read):

```sh
for i in $(seq 20); do
  curl -s -H "Authorization: Bearer $TOKEN" "…/history" \
    | jq -r '[.data[].seq] | @csv'
done | sort -u | wc -l
```

**Expect**: `1` — one distinct ordering across all 20 reads.

## 3. The trail persists, folded

**Why**: FR-014, FR-015, FR-016, FR-017, FR-018.

In the WebUI, send `read_core` and then its sample JSON.

**Expect while in progress**: the trail is expanded and live, as today.

**Expect once the reply lands**: the trail is still present, folded to a single summary line
(`▸ … 1 tool …`), with the reply below it. It does **not** disappear.

Then:
- Expand it → the tool event is listed. Collapse it → back to one line (FR-016).
- Send a plain message (`hello`, which takes dummyplug's lorem-ipsum path and calls no tool) →
  **no trail element at all**, not an empty disclosure (FR-017).
- **Reload the page** → the folded trail is still there, in the same place (FR-014). This is the step
  that proves the trail is rendered from stored history and not from page state.

## 4. `intent` is required

**Why**: FR-024, FR-025, contract C1–C3.

Send `execute_python`.

**Expect**: `intent` listed under `Required:` alongside `code`, with its description, and present in the
sample JSON block.

Send the sample JSON unchanged.

**Expect**: the script runs; the reply is dummyplug's `**Tool result**` echo.

Now send the same JSON with `intent` removed:

```json
{ "tool": "execute_python", "arguments": { "code": "1 + 1" } }
```

**Expect**: an error naming the missing `intent` field, and **no execution** — no `ExecutionReport` in
the reply, no new `ToolResult` row for it in `session_history`.

## 5. A python run reads as its intent, and the script's tool calls are hidden

**Why**: FR-027 to FR-032, FR-034, SC-011. Fully dummyplug-coverable: the nesting happens *inside* one
tool call, so the one-tool-per-turn limit does not bite.

Send a script that calls a tool in a loop:

```json
{ "tool": "execute_python", "arguments": {
    "intent": "count how many memories mention latency",
    "code": "hits = [memory_search(query=q) for q in ['latency','p99','slow']]\nsum(len(h) for h in hits)"
} }
```

**Expect in the WebUI**:
- the entry's visible text is `count how many memories mention latency`, **not** the code
- the script source is not shown until the entry is expanded
- expanding shows Code, Output, Result
- **no list of the three `memory_search` calls anywhere** — collapsed or expanded (FR-032)
- **no `ToolChoice` frames for them streamed live either** — watch the trail while it runs; the three
  nested calls must not appear as their own trail entries (FR-034, the half that is easy to miss)

Then run a failing script:

```json
{ "tool": "execute_python", "arguments": {
    "intent": "join deploy times to latency buckets",
    "code": "{}['missing']"
} }
```

**Expect**: the collapsed entry shows the failure without being expanded (FR-030); expanding shows the
error and traceback.

**Expect in the agent's own view** (FR-035): the tool result the agent receives still contains the
`tool_calls` records. Confirm from the tracing output — each nested call still logs
`python script called a tool`. Hiding is presentation-only.

## 6. Pre-existing conversations still work

**Why**: FR-007, FR-008, FR-023, SC-004.

Open a topic from before the upgrade (the one from step 1).

**Expect**: it loads with no error and no missing entries. Its already-recorded `think` and tool-call
entries render as a folded trail. Ordering within a single millisecond may be imperfect — that is the
accepted breaking change, not a bug to chase.

## 7. Paging does not skip or duplicate

**Why**: FR-005, SC-003, contract H9.

Not exercised by the WebUI, which requests history with no paging parameters — verify against the
endpoint directly. Build a conversation of ~200 entries, then page it in 20s, carrying
`(before, before_seq)` from the oldest entry of each page:

```sh
curl -s "…/history?limit=20" | jq -r '.data | last | "\(.timestamp) \(.seq)"'
# feed those back as before= and before_seq=, repeat to exhaustion
```

**Expect**: 200 distinct `uid`s collected, no duplicate, no omission.

---

## [LIVE] 8. Narration is recorded and replays correctly

**Why**: FR-010 to FR-013, SC-005, contract H14–H17. **This is the step that matters most and the one
dummyplug cannot do.** Dummyplug emits no assistant text alongside a tool call, so it never produces an
`AssistantMessage`; and it enforces no message-shape rules, so it would accept the broken replay shapes
this step exists to rule out.

Use an agent on a provider that enforces tool-call/result adjacency and forbids consecutive same-role
messages — any Anthropic-family model.

Ask something that makes the agent narrate before acting, e.g. *"check what you remember about latency,
then tell me the p99"*.

```sh
sqlite3 "$VIZIER_DATA_DIR/vizier.db" \
  "SELECT seq, content_type FROM session_history WHERE topic='$TOPIC' ORDER BY seq;"
```

**Expect**: an `AssistantMessage` row exists, and its `seq` is **lower** than the `ToolCall` rows from
the same assistant turn (FR-011). If it is higher, the write order was not fixed and step 9 will fail.

In the WebUI, expand the trail.

**Expect**: the narration appears as the first entry of the trail, above the thoughts and tool calls.

## [LIVE] 9. Replayed narrated history is accepted — SC-005

**Why**: SC-005. The failure this guards against is not cosmetic: a rejected replay means the agent
cannot take another turn in that conversation at all.

In the same conversation from step 8, **send at least 20 more multi-tool turns**, each of which replays
all prior history including the narration entries.

**Expect**: every turn completes. **Zero** provider errors of the form "tool_use ids … tool_result" or
"messages: roles must alternate" / consecutive-assistant-message complaints.

A single failure here means `history_entries_to_messages` is emitting one of the two forbidden shapes in
`data-model.md` — either narration flushed as its own assistant message, or narration recorded after its
tool calls.

**Also confirm the regression guard (H17)**: open a conversation recorded *before* this change, send a
turn, and confirm it completes. History with no `AssistantMessage` must replay exactly as it did before.

---

## Checks this quickstart does not cover

- **SC-010** (intents read as plain language to someone who has not seen the script) is a judgement over
  a sample of 20 real runs, not a scripted assertion. Dummyplug's intents come from an operator pasting
  JSON, so they prove nothing about what a model writes. Needs a live provider and a human read.
- **SC-012** (scroll smoothness at 100 folded trails) needs a conversation of that size; worth checking
  once by hand, not scripted here.
- **SC-006**'s two-line bound is verified by eye in step 3 rather than measured.
