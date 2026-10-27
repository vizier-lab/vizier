# Data Model: /btw Ephemeral Side-Chat

**Feature**: `033-btw-side-chat` | **Date**: 2026-10-28

Nothing in this feature adds a table or a column. Every type below lives in memory or on the wire, with one exception: the Telegram deletion ledger (research R13), a content-free value in the existing key-value state storage.

## SideChat (request content)

`src/schema/request.rs` — carried by the new `VizierRequestContent::Btw(SideChat)`.

| Field | Type | Rules |
|-------|------|-------|
| `question` | `String` | Trimmed, 1–4,000 characters |
| `exchange` | `Vec<SideChatTurn>` | Earlier turns of this exchange, oldest first; at most 8 |

`SideChatTurn`:

| Field | Type | Rules |
|-------|------|-------|
| `question` | `String` | 1–4,000 characters |
| `answer` | `String` | 0–16,000 characters |

- `VizierRequestContent::kind()` → `"btw"`; `Display` → the question; `is_side_chat()` → `true` only for this variant.
- The request carrying it always has `transient: true`, no attachments, no `client_tools`, `background_depth: 0`, `scheduled_task: None`.
- Serialisable (it derives what every content variant derives) but **never serialised to storage**: `is_durable` is false, and `run_btw` saves nothing. The schema appears in the OpenAPI document only because `VizierRequestContent` does.

## SideChatAnswer (response)

What `run_btw` sends on the response channel: one `VizierResponse` with `VizierResponseContent::Message { content, stats }`, `history_uid: None`, no attachments.

- `content`: the model's text with any tool calls removed; the fixed no-tools notice when nothing textual is left (research R5).
- `stats`: `VizierResponseStats` (provider, model, token counts) as for a chat reply, so the WebUI can show the model and the person can see the cost.
- On failure: `VizierResponseContent::Error { kind, message }`; `ErrorKind` as classified for any turn.

## SideChatError (channel-side outcome)

`src/channels/side_chat.rs` — what `side_chat::ask` returns when there is no answer. Each channel maps it to its own surface (contracts).

| Variant | When | HTTP |
|---------|------|------|
| `RateLimited { retry_after_secs }` | Over 5 asks / 60 s for this person | 429 |
| `Invalid(reason)` | Empty or over-long question, too many earlier turns, over-long earlier answer | 422 |
| `Timeout` | No reply within the agent's `prompt_timeout` | 408 |
| `Agent { kind, message }` | The agent answered with an error, an abort, or its response channel closed (restart) | 500 (`503` for `AgentUnavailable`) |

## PersonKey (rate-limit key)

A string, never stored: `http:{username}`, `discord:{user id}`, `telegram:{user id}` (research R9). The limiter (`RateLimiter`, 5 / 60 s) lives on `VizierDependencies.side_chat_limits`.

## SideChats (in-memory exchange store, Discord and Telegram)

`src/channels/side_chat.rs`. One instance per Discord or Telegram reader; never written to storage.

| Field | Type | Notes |
|-------|------|-------|
| `exchanges` | `Mutex<HashMap<ExchangeId, LiveExchange>>` | |
| `lifetime` | `Lifetime { idle: Duration, max: Option<Duration> }` | Discord `{5 min, Some(14 min)}`, Telegram `{2 min, None}` |

`LiveExchange`:

| Field | Type | Notes |
|-------|------|-------|
| `id` | `ExchangeId` (`nanoid`, 10 chars) | In Discord button `custom_id`s as `btw:{id}` |
| `person` | `PersonKey` | Only this person may continue the exchange |
| `session` | `VizierSession` | The host conversation, fixed at start |
| `turns` | `Vec<SideChatTurn>` | Becomes the next ask's `exchange` |
| `started_at`, `last_active` | `Instant` | |
| `posted` | `Vec<PostedMessage>` | What to delete on end: Discord `{ token, message_id: Option<Id> }` (None = the original response); Telegram `{ chat_id, message_id }` |

**State transitions**

```text
          ask ok                     ask ok (follow-up, same person, turns < 8)
 (none) ────────► Live ◄──────────────────────────────────────────────┐
                   │ └────────────────────────────────────────────────┘
                   │ idle ≥ lifetime.idle  or  age ≥ lifetime.max  (sweep, every 15 s)
                   ▼
                 Ended ──► platform messages deleted (best-effort) ──► forgotten
```

- A failed ask (rate limit, timeout, agent error) adds no turn, but it still counts as activity (`last_active` moves) and its error reply joins `posted`, so it is deleted with the rest.
- A follow-up from a different person, or on an unknown/ended id, is refused without touching any exchange.
- At 8 earlier turns, the next follow-up is refused (`Invalid`) and the exchange stays until it times out.

## Telegram deletion ledger

The only persisted value (research R13), in `StateStorage` under `side_chat_pending__{agent_id}__{chat_id}`:

```json
{ "items": [ { "chat_id": -1001234, "message_ids": [512, 513], "delete_after": "2026-10-28T10:02:15Z" } ] }
```

- No text, no user id, no exchange id, no question or answer.
- Written (merged) when the reader posts a side-chat message or sees a person's side-chat message; `delete_after` is moved forward on activity.
- An item is removed as soon as its deletions have been attempted (success or Telegram refusal).
- At reader start, every item whose `delete_after` has passed is processed; the rest are re-armed.
- The key is removed when its list is empty.

## Unchanged entities (explicitly)

- `session_history`, `session_turn`, `session_detail` (title, `is_thinking`), `scratch_file`, memory documents and indexes, `notification`: **no writes** from a side chat (research R1, R2).
- `usage_event`: one row per answered side-chat completion, scoped to the host session, activity from the host channel (research R7).
