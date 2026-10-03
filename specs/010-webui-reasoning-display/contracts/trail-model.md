# Contract: the WebUI activity trail

**Feature**: `010-webui-reasoning-display`

A UI contract, not a wire contract. It exists because the trail has **two producers** — the live
WebSocket stream and stored history — and they must not drift. One normalized model, one renderer.

---

## 1. The model

```ts
type TrailEvent =
  | { kind: 'narration'; id: string; text: string }
  | { kind: 'thought';   id: string; text: string }
  | { kind: 'tool';      id: string; name: string; args: Record<string, unknown> }
  | { kind: 'python';    id: string; intent: string | null; report: ExecutionReport | null }

interface Turn {
  key: string
  request?: ChatMessage
  trail: TrailEvent[]
  outcome?: ChatMessage
  live: boolean
}
```

---

## 2. Producers

### From stored history — `groupHistory(entries: SessionHistory[]): Turn[]`

Pure. Input is already ordered by the storage layer (see `history-api.md`); this function does not
sort.

| Entry | Produces |
|---|---|
| `Request` | opens a turn, sets `request` |
| `AssistantMessage(text)` | `narration` |
| `ToolCall { name: "think" }` | `thought` from `arguments.thought` |
| `ToolCall { name: "execute_python" }` | `python`, `intent` from `arguments.intent` |
| `ToolCall { … }` | `tool` |
| `ToolResult` | attaches to the `python` event with the matching `call_id`; otherwise ignored |
| `Response` | closes the open turn as `outcome` |
| `Checkpoint`, `Command` | standalone, belong to no turn |
| anything unrecognised | skipped |

### From the live stream — `liveEvent(content): TrailEvent | null`

| Frame | Produces |
|---|---|
| `Thinking(text)` | `thought` |
| `ToolChoice { name: "execute_python", args }` | `python`, `intent` from `args.intent`, `report: null` |
| `ToolChoice { name, args }` | `tool` |
| `ToolResponse { response }` where `looks_like` a report | attaches to the most recent `python` event with `report: null` |
| `ThinkingStart`, `Empty`, `Abort`, `Message`, `Checkpoint` | not trail events |

No `narration` from the live path — intermediate assistant text is not streamed. A turn therefore gains
its narration only once reloaded from history. This asymmetry is accepted: the alternative is a new
WebSocket frame for a cosmetic gain.

---

## 3. Guarantees

| # | Guarantee | Requirement |
|---|---|---|
| T1 | `groupHistory` is pure and does not reorder its input. Entry order in, event order out. | FR-003 |
| T2 | A turn with an empty `trail` renders no trail element — not an empty disclosure. | FR-017 |
| T3 | Adjacent `thought` events merge into one block; adjacent `narration` events likewise. | FR-020 |
| T4 | `live: true` renders expanded; `live: false` renders folded. | FR-015 |
| T5 | Expanding and collapsing one turn's trail does not affect any other turn's. | FR-016 |
| T6 | The folded trail is bounded to two lines regardless of how many events it holds. | FR-018, SC-006 |
| T7 | A turn whose `outcome` is an error still renders its trail. | FR-021 |
| T8 | An unrecognised entry kind is skipped; the rest of the conversation still renders. | FR-023 |
| T9 | A `python` event with `intent: null` renders a neutral fallback label. | FR-032 |
| T10 | **No nested tool call is representable.** The model has no variant for one. | FR-034 |
| T11 | Entries with no turn open — an agent-initiated turn — produce a turn with `request: undefined`, not a dropped trail. | FR-019 |
| T12 | A turn closed live and the same turn reloaded from history render through the same component. | — |

**T10 is structural, not a filter.** There is no `nested` flag to forget to check: nested calls never
reach history, and `on_nested_tool_call` stops them reaching the live stream, so neither producer can
emit one.

---

## 4. The python entry

Collapsed:

```
▸ 🐍 compute p99 from the last 24h of samples        ✅ 1.8s
▸ 🐍 join deploy times to latency buckets       ❌ KeyError 0.3s
▸ 🐍 Python run                                      ✅ 0.2s     ← intent: null
```

Expanded: `Code`, then `Output`, then `Result` or `Error` with traceback.

| # | Guarantee | Requirement |
|---|---|---|
| P1 | The intent is the always-visible text; the script source is not shown until expanded. | FR-027, FR-028 |
| P2 | Success or failure is evident collapsed. | FR-030 |
| P3 | An over-long intent is truncated collapsed, full text available expanded. | FR-031 |
| P4 | The expanded section holds source, stdout, result, and error + traceback. | FR-029 |
| P5 | **No `Tool calls` list, collapsed or expanded.** | FR-032 |

P5 removes an existing section of `ExecutionReportView`, which today renders `report.tool_calls` as an
ordered list above `Output`.

---

## 5. Collapsed trail label

```
▸ Reasoning · 2 thoughts, 3 tools, 1 python run · 4.2s
```

Counts by kind, plus the turn duration from `VizierResponseStats.duration` on the closing `Response`.
Duration is omitted when absent (an error outcome, or a turn with no stats).

---

## 6. Known defect this contract does not fix

`formatToolChoice` labels `tool` events, and it was never updated for the spec 009 memory tool rename:
`memory_read` is labelled with the old search semantics and reads `args.query`, which no longer exists,
so it renders `🔍 Searching memory for 'undefined'`. There is no `memory_search` case at all, and
`memory_detail` is still handled though retired.

Out of scope here (research D8), but this feature raises its cost: today the wrong label flashes past in
an ephemeral indicator; afterwards it is permanent text in every stored transcript, on the most
frequently called tool family. Worth fixing as its own change, soon.
