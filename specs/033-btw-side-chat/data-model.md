# Data Model: /btw Ephemeral Side-Chat

**Feature**: `033-btw-side-chat` | **Date**: 2026-10-28

Nothing in this feature adds a table or a column. Every type below lives in memory or on the wire, with one exception: the Telegram deletion ledger (research R13), a content-free value in the existing key-value state storage.

## SideChat (request content)

`src/schema/request.rs` — carried by the new `VizierRequestContent::Btw(SideChat)`.

| Field | Type | Rules |
|-------|------|-------|
| `question` | `String` | Trimmed, 1–4,000 characters |
| `turns` | `Vec<SideChatTurn>` | Earlier turns of this side chat, oldest first. Carried in each request by whoever holds the side chat (the browser, or the Discord/Telegram reader's memory); never stored |

`SideChatTurn`:

| Field | Type | Rules |
|-------|------|-------|
| `question` | `String` | 1–4,000 characters |
| `answer` | `String` | Any length, within the side chat's budget |

**Budget** (research R8): the characters of every `turns[].question` and `turns[].answer` plus `question` must total at most **32,000** (`MAX_SIDE_CHAT_CHARS`), counted with `chars().count()`.

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

## Rate-limit key

A plain string, never stored, with no type of its own: `http:{username}`, `discord:{user id}`, `telegram:{user id}` (research R9). Each channel builds it where it calls `ask`. It also decides whether a reply may continue a live side chat. It is per platform identity because the profile links between a WebUI account and a Discord/Telegram id are self-declared; a key that spans platforms waits for verified identities (#116). The limiter (`RateLimiter`, 5 / 60 s) lives on `VizierDependencies.side_chat_limits`.

## SideChats (in-memory exchange store, Discord and Telegram)

`src/channels/side_chat.rs`. One instance per Discord or Telegram reader; never written to storage.

| Field | Type | Notes |
|-------|------|-------|
| `exchanges` | `Mutex<HashMap<ExchangeId, LiveExchange>>` | |
| `lifetime` | `Lifetime { idle: Duration }` | Discord 5 min, Telegram 2 min |

`LiveExchange`:

| Field | Type | Notes |
|-------|------|-------|
| `id` | `ExchangeId` (`nanoid`, 10 chars) | |
| `person` | `String` (the rate-limit key above) | Only this person may continue the exchange |
| `session` | `VizierSession` | The host conversation, fixed at start |
| `turns` | `Vec<SideChatTurn>` | Becomes the next ask's `SideChat.turns` |
| `last_active` | `Instant` | |
| `posted` | `Vec<P>` | Every message the bot posted in the aside (and, on Telegram, the person's), as `{ chat/channel id, message id }`. Used to recognise replies to the aside (`owner_of`), and on Telegram for deletion |

**State transitions**

```text
          ask ok                     ask ok (follow-up, same person, within budget)
 (none) ────────► Live ◄──────────────────────────────────────────────┐
                   │ └────────────────────────────────────────────────┘
                   │ idle ≥ lifetime.idle  (sweep, every 15 s)
                   ▼
                 Ended ──► Telegram: messages deleted (best-effort) ──► forgotten
                       Discord: messages stay in the channel ──► forgotten
```

- A failed ask (rate limit, timeout, agent error) adds no turn, but it still counts as activity (`last_active` moves) and its error reply joins `posted` (deleted with the rest on Telegram).
- Replies are sorted by the shared `classify_reply`: from someone other than the starter they are ignored; to a side-chat message no live aside owns they get "This side chat has ended" (research R11–R13).
- A follow-up that would take the side chat over its 32,000-character budget is refused (`Invalid`, "This side chat is full — start a new one"), and the exchange stays until it times out.

## Telegram deletion ledger

The only persisted value (research R13), in `StateStorage` under one key per agent, `side_chat_pending__{agent_id}`:

```json
{ "items": [ { "chat_id": -1001234, "message_ids": [512, 513], "delete_after": "2026-10-28T10:02:15Z" } ] }
```

- No text, no user id, no exchange id, no question or answer.
- Written (merged) when the reader posts a side-chat message or sees a person's side-chat message; `delete_after` is moved forward on activity.
- An item is removed as soon as its deletions have been attempted (success or Telegram refusal).
- At reader start, items older than 48 hours are dropped (Telegram refuses to delete them), every other item whose `delete_after` has passed is processed, and the rest are re-armed.
- A reply to a message on the ledger with no live exchange is answered "This side chat has ended…" and never forwarded (research R13).
- The key is removed (`StateStorage::delete_state`, new) when its list is empty, and when the agent is deleted.

## Unchanged entities (explicitly)

- `session_history`, `session_turn`, `session_detail` (title, `is_thinking`), `scratch_file`, memory documents and indexes, `notification`: **no writes** from a side chat (research R1, R2).
- `usage_event`: one row per answered side-chat completion, scoped to the host session, activity from the host channel (research R7).
