# 3.1 REST API

Base URL: `http://localhost:<port>/api/v1`. Interactive docs: `http://localhost:<port>/swagger` (OpenAPI JSON at `/openapi.json`). The Swagger spec covers the core endpoints; this page lists everything the router actually mounts.

## Response envelope

Every JSON endpoint returns:

```json
{ "status": 200, "message": null, "data": { ... } }
```

`status` mirrors the HTTP status. On error `data` is `null` and `message` holds the reason.

## Authentication

### Login

```http
POST /api/v1/auth/login
{ "username": "alice", "password": "…" }
→ { "data": { "token": "eyJ…" } }
```

### Sending credentials

| Method | Header / param | Notes |
|--------|----------------|-------|
| JWT | `Authorization: Bearer <jwt>` | Expires after `jwt_expiry_hours` (default 30 days) |
| API key | `Authorization: ApiKey <key>` | Keys start with `vk_`; created under `/auth/api-keys`. Same permissions as the owning user |
| WebSocket | `?token=<jwt>` query parameter | Only when no `Authorization` header is present |

### First-run setup

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/auth/setup-status` | none | `{ "needs_setup": bool }` — true until a user exists |
| `POST` | `/auth/setup` | none | `{ "username", "password" }` — creates the first user with the `superadmin` system role. Fails once any user exists |

### Account

| Method | Path | Permission | Description |
|--------|------|------------|-------------|
| `GET` | `/auth/users/me` | any | Current user, role, permissions |
| `GET` / `PUT` | `/auth/users/me/profile` | any | Profile used to build the agent's owner prompt: `discord_id`, `discord_username`, `telegram_id`, `telegram_username`, `alias[]` |
| `POST` | `/auth/change-password` | `settings:password` | `{ "current_password", "new_password" }` |
| `GET` / `POST` | `/auth/api-keys` | `settings:api_keys` | List / create `{ "name", "expires_in_days"? }` → `{ id, name, key, expires_at }` (the `key` is shown once) |
| `DELETE` | `/auth/api-keys/{key_id}` | `settings:api_keys` | Revoke |

### Users and roles

| Method | Path | Permission | Body |
|--------|------|------------|------|
| `GET` / `POST` | `/auth/users` | `users:manage` | `{ "username", "password", "role_id"? }` |
| `PUT` / `DELETE` | `/auth/users/{user_id}` | `users:manage` | `{ "username"?, "role_id"?, "password"? }` |
| `GET` / `POST` | `/auth/roles` | `roles:manage` | `{ "name", "permissions": [..] }` |
| `PUT` / `DELETE` | `/auth/roles/{role_id}` | `roles:manage` | same |
| `GET` | `/auth/roles/available-permissions` | `roles:manage` | All permission strings |

Permissions: `all_agents:view|create|edit|delete`, `owned_agents:view|edit|delete`, `agents:view`, `agents:mcp_config`, `agents:shell_config`, `settings:providers|password|api_keys`, `users:manage`, `roles:manage`. The built-in `superadmin` role is a *system* role that bypasses all checks; a default `user` role gets `agents:view`, `settings:password`, `settings:api_keys`.

## Agents

All agent routes require auth. Visibility: owner, users in `shared_to`, or `all_agents:view`. Editing: owner or `all_agents:edit`.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/agents` | Summaries of agents you can see |
| `POST` | `/agents` | Create — body in [Agent Configuration](../configuration/agents.md) |
| `GET` | `/agents/health` | Process status of every agent |
| `GET` | `/agents/{id}` | Full config |
| `PUT` | `/agents/{id}` | Replace config (same body as create). Restarts the agent |
| `DELETE` | `/agents/{id}?delete_workspace=true\|false` | Delete; optionally wipe `agents/{id}/` on disk |
| `GET` | `/agents/{id}/ping` | Liveness of one agent |
| `GET` | `/agents/{id}/usage?start_date&end_date&activity` | Usage report: cost, tokens, activities, models, costliest conversations (see [Agents › Usage](../configuration/agents.md#usage)) |
| `GET` | `/agents/{id}/usage/conversation?channel&topic&…` | One conversation's model calls |
| `GET` / `PATCH` | `/agents/{id}/sharing` | `{ "add": [user_id], "remove": [user_id] }` |
| `GET` / `PUT` | `/agents/{id}/core` | `CORE.md` |
| `GET` / `POST` | `/agents/{id}/core/history` | **Version history** of `CORE.md` (see below) |
| `POST` | `/agents/{id}/chat` | **Synchronous chat** (see below) |

### Synchronous chat

```http
POST /api/v1/agents/{id}/chat
{
  "channel_id": "my-integration",
  "topic_id": "ticket-42",
  "content": "Summarize the attached file",
  "attachments": [ { "filename": "a.pdf", "content": { "url": "https://…/a.pdf" } } ]
}
→ { "data": { "content": "…", "stats": { "total_tokens": 1234, … }, "attachments": [] } }
```

Blocks until the agent's final message. The session is `(agent, HTTP(username, channel_id), topic_id)` — the same sessions the WebSocket uses, so history is shared. Attachment `content` is one of `{ "url": "…" }`, `{ "base64": "…" }`, `{ "bytes": [..] }`, or `{ "local": "/api/v1/files/<id>" }` (from an upload).

### Version history (CORE and memories)

Every save of `CORE.md` or a memory concept — by the agent (`WRITE_CORE`, `memory_write`, …, in conversation or a dream cycle), the WebUI, the API, an import, or a rollback — appends a full-content revision tagged with who saved it and why. History is append-only; a rollback is just a new save.

Same routes for both kinds; CORE lives at `/agents/{id}/core/history`, a memory at `/agents/{id}/memory/history/{bundle}/{path}` (nested paths allowed, `.md` optional). Access rules are the ones for reading the document.

| Method | Path | Query / body | Returns |
|--------|------|--------------|---------|
| `GET` | `…/history` | `?offset=0&limit=50` (max 200) | Paginated list, newest first: `{ revisions: [ { seq, is_current, size_bytes, actor, trigger, created_at, deleted? } ], total, offset, limit }` — no content, so listing 1,000 versions is cheap |
| `GET` | `…/history` | `?seq=N` | One version with `content` (memory versions also carry parsed `title` / `tags`; a deletion entry has `content: null`, `deleted: true`) |
| `GET` | `…/history` | `?to=B` or `?from=A&to=B` | Line diff — the changes `to` introduced relative to `from` (`from` defaults to `to - 1`): `{ from_seq, to_seq, additions, deletions, hunks: [ { old_start, old_lines, new_start, new_lines, lines: [ { op: "equal"\|"insert"\|"delete", old_line, new_line, text } ] } ] }` |
| `POST` | `…/history` | `{ "seq": N }` | Restore version `N` as a **new** revision → `{ no_change, new_seq, restored_from }`. `no_change: true` when `N` is already identical to the current content. Restoring a content version of a deleted memory recreates it |

`actor` is `{ "type": "agent" }`, `{ "type": "user", "user_id", "username" }` or `{ "type": "system" }`; `trigger` is `{ "type": "conversation" | "dream" | "webui" | "api" | "import" | "baseline" }` or `{ "type": "rollback", "restored_from": N }`. A document that pre-dates the feature gets a `system` / `baseline` `seq: 1` the first time it is saved or listed. Unchanged saves add no revision. Restoring a deletion entry is `400`; an unknown `seq` is `404`. History is removed with the agent and is **not** part of bundle export/import; edits made directly to the markdown files on disk are not versioned.

```sh
curl -H "Authorization: Bearer $TOKEN" "$BASE/agents/$AGENT/core/history?to=3"          # what v3 changed
curl -H "Authorization: Bearer $TOKEN" "$BASE/agents/$AGENT/memory/history/default/tea?seq=2"
curl -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
     -d '{"seq": 2}' "$BASE/agents/$AGENT/core/history"                                  # restore v2 as a new version
```

### Channel / topics (sessions)

Under `/agents/{id}/channel/{channel_id}`. The WebUI uses `channel_id = "vizier-webui"`; integrations can use any string.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/topics` | Topics in this channel (`topic_id`, `title`, `is_thinking`) |
| `GET` | `/topic/{topic_id}/history?before=<ts>&limit=<n>` | History entries (requests, responses, tool calls, commands, checkpoints) |
| `GET` | `/topic/{topic_id}/detail` | Session detail |
| `DELETE` | `/topic/{topic_id}` | Delete the session and its history |
| `ANY` | `/topic/{topic_id}/chat` | **WebSocket** upgrade |

### Memory, tasks, skills, dream

| Prefix | Docs |
|--------|------|
| `/agents/{id}/memory/*` | [Memory → HTTP API](../configuration/memory.md#http-api) |
| `/agents/{id}/tasks[/ {slug}]` | [Agents → Tasks](../configuration/agents.md#tasks) |
| `/agents/{id}/skills[/ {slug}]` | [Skills → HTTP API](../configuration/skills.md#http-api) |
| `/agents/{id}/dream/{trigger,status,journal,journal/{entry_id}}` | [Agents → Dream cycle](../configuration/agents.md#dream-cycle) |

## Providers (`settings:providers`)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/providers` | Connected providers (saved or from env): `[{ id, name, source, has_api_key, endpoint, base_url, migrated_from, used_by, reachable, … }]` — never a secret |
| `GET` | `/providers/{id}` | One connected provider |
| `PUT` | `/providers/{id}` | Save credentials: only the fields the provider uses (`api_key`, `endpoint`, `base_url`, `access_token`, `account_id`); an omitted field keeps its value, `""` clears it |
| `DELETE` | `/providers/{id}[?confirm=true]` | Remove saved credentials; `409` listing the agents that use it unless confirmed |
| `POST` | `/providers/{id}/test` | One cheap call with the saved (or given) credentials: `{ ok, model_count? , status?, error? }` |
| `GET`/`POST` | `/providers/custom` | List / create custom providers |
| `PUT`/`DELETE` | `/providers/custom/{id}` | Edit (id is fixed) / remove |
| `POST` | `/providers/custom/{id}/fetch-models` | Merge the server's model list |
| `POST` | `/providers/custom/test` | Test an unsaved custom provider |

`{id}` is a catalogue or built-in provider id; pre-catalogue names (`gemini`, `opencode_zen`, …) are accepted.

## Catalogue (any signed-in user)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/catalogue` | Snapshot source, date and refresh state |
| `GET` | `/catalogue/providers?q=&kind=chat\|media\|local\|custom&connected=` | Selectable providers with connection state |
| `GET` | `/catalogue/providers/{id}/models?all=false` | Models, newest first; tool-capable and current only unless `all=true` |
| `GET` | `/catalogue/providers/{id}/models/{model}` | One model, or `404 "not in catalogue"` |
| `GET` | `/catalogue/logos/{id}.svg` | Logo (no auth) |
| `POST` | `/catalogue/refresh` | Check models.dev now (`settings:providers`) |

## Skills (global)

See [Skills → HTTP API](../configuration/skills.md#http-api).

## Files

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/files/upload` | required | `{ "file": "<base64>", "filename": "a.png" }` → `{ file_id, filename, url }` where `url` is `/api/v1/files/{file_id}` |
| `GET` | `/files/{file_id}` | none | Download |

Use the returned `url` as `{ "local": url }` in an attachment, or reference files the agent produced (TTS, image generation, `send_attachment`).

## Embedding models

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/embedding-models/local` | The fastembed model list with size profiles |

## Home: overview, activity and feed

Limited to the agents the signed-in person can see. `tz` is the person's UTC offset (`+07:00`); encode the `+` as `%2B`.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/overview?tz=` | `{ agents: { total, live, failed }, cost_today_usd, accounting_since }`. Cost is priced from the catalogue; models without a price add nothing |
| `GET` | `/overview/activity?rank=most_active\|mine\|live\|failed` | The last 24 hours: `hours` (24 hour starts, oldest first), `rows` (at most 6) of `{ agent_id, label, turns[24], live }`, an `all` row when there are 7 or more agents, `agent_total`, and `today_by_kind` when there is exactly one agent |
| `GET` | `/overview/feed?before=&limit=` | "Since you were away": events since the person's previous visit (else 24 hours), newest first, `limit` 1–50 (default 20), `next` is the cursor for `before`. Start failures are pinned to the top of the first page |

A visit is a run of requests with a JWT, with no gap over 30 minutes; API-key requests are never visits.

## Conversations

`GET /agents/{id}/conversations?q=&channel=webui|discord|telegram&cursor=&limit=` lists the conversations a person can open with an agent, newest first: their own WebUI topics (one row per topic), and the agent's Discord and Telegram conversations (one row per channel). The response is `{ items, counts: { all, webui, discord, telegram }, total, next }`. `q` matches titles and the text of the agent's newest 5,000 messages only, so an older match may not be found.

Read-only routes under `/agents/{id}/channel/{channel_id}` (`history`, `files`, `detail`) also accept a Discord or Telegram channel slug (`discord__123`, `telegram__-9`) as `channel_id`, with any `topic_id`, to read those conversations.

## Paged lists

A list endpoint answers with the plain array it always did. Add `limit` (1–100) and it answers `{ items, total, counts }` instead, with `q`, `filter`, `sort`, `order` and `offset`. `counts` has a count for `all` and for each filter, ignoring the current `filter`.

| Route | Filters | Sorts |
|-------|---------|-------|
| `GET /agents` | `mine`, `live`, `failed` | `last_active`, `name` |
| `GET /agents/{id}/tasks` | `active`, `last_run_failed` | `next_run`, `name`, `last_run` |
| `GET /skills`, `GET /agents/{id}/skills` | `agent`, `global` | `name`, `updated` |
| `GET /auth/users` | `role:<name>` | `created`, `username` |
| `GET /auth/roles` (adds `people` to each role) | `builtin`, `custom` | `people`, `name` |
| `GET /auth/api-keys` (adds `prefix` to each key) | none | `created`, `last_used` |
| `GET /agents/{id}/integrations?family=mcp` | `down`, `disabled` | `name`, `tools` |
| `GET /agents/{id}/memory/bundles` | none | `documents`, `updated`, `links` |

An unknown `filter` is a `400`.

## Misc

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/ping` | `"pong"`, no auth |
| `GET` | `/status` | No auth. `{ online, version, setup_required }`: what the sign-in page shows before anyone has signed in |

## WebSocket chat

```
ws://localhost:9999/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat?token=<jwt>
```

Text frames in both directions carry JSON. Ping/pong frames keep the connection alive; the server closes it after `ws_idle_timeout_secs` (default 300) of silence.

A request may also declare tools of the client's own (`client_tools`), which the agent can call during that turn; see [Client tools](./client-tools.md).

### Client → server: request

```json
{
  "timestamp": "2026-04-18T12:00:00Z",
  "user": "alice",
  "content": { "chat": "Hello!" },
  "metadata": {},
  "attachments": [],
  "expect_audio_reply": false
}
```

`content` is a tagged union with exactly one key:

| Key | Payload | Meaning |
|-----|---------|---------|
| `chat` | string | Normal message; persisted to history |
| `prompt` | string | Stateless prompt: answered with **no prior history, memory, or skill recommendations** (the exchange is still recorded) |
| `silent_read` | string | Passive observation; the agent replies only with `silent_read_initiative_chance` |
| `command` | `"abort"` \| `"checkpoint"` \| `"lobotomy"` | Session control (same as the Discord/Telegram slash commands) |
| `audio_chat` | `[attachment, transcript?]` | Voice message; transcribed with the agent's STT if `transcript` is null |
| `audio_prompt` | `[attachment, transcript?]` | Voice, stateless like `prompt` |
| `client_event` | `{ text, data?, source_call_id? }` | A message the client sends on the person's behalf, e.g. a click in a page the agent drew; starts a turn like `chat`. See [Client tools](./client-tools.md#5-client--server-a-client-event) |
| `task` | string | Used internally by the scheduler |
| `reaction` | `{ user_id, emoji, action }` | Used internally |

`attachments[]` items are `{ "filename", "content": { url | base64 | bytes | local } }`; they're uploaded into session files and the agent is told to use `read_document_file` / `read_image_file`. Set `expect_audio_reply: true` to get an `audio_reply` (requires the TTS tool).

### Client → server: reaction

```json
{ "reaction": { "message_uid": "…", "emoji": "👍", "action": "added" } }
```

Records a reaction on a stored message and forwards a `reaction` request to the agent.

### Server → client: response stream

Each frame:

```json
{ "timestamp": "…", "content": { "<type>": … }, "attachments": [] }
```

| `content` key | Payload | When |
|---------------|---------|------|
| `thinking_start` | `null` | The model began a turn |
| `thinking` | string | Reasoning text (if the model exposes it) |
| `tool_choice` | `{ "name", "args" }` | About to call a tool |
| `tool_response` | `{ "response" }` | Tool result |
| `message` | `{ "content", "stats" }` | **Final answer**. `stats`: `input_tokens`, `cached_input_tokens`, `cache_creation_input_tokens`, `total_*`, `duration`, `current_context_size`, `context_window` |
| `audio_reply` | `[attachment, text?, stats?]` | Final answer as audio (when `expect_audio_reply`) |
| `checkpoint` | `{ "handover" }` | A checkpoint happened (auto or manual); `handover` is the summary, `null` for lobotomy |
| `error` | `{ "kind": "completion" \| "tool_timeout" \| "prompt_timeout", "message" }` | Request failed |
| `abort` | `null` | Response was aborted |
| `empty` | `null` | No response (e.g. ignored `silent_read`) |

### Server → client: a file was added (`session_file`)

`{ "type": "session_file", "file": SessionFileRecord }` is sent to every connection on a topic when a file is added to it (an upload, or one the agent attached). The WebUI prepends it to the Session files panel. Clients that don't know it ignore it.

### Server → client: saved steps (`history_entry`)

As a turn runs, each step it takes is saved to history and pushed as its own frame:

```json
{ "history_entry": { "uid": "…", "vizier_session": [ … ], "timestamp": "…", "content": { "ToolCall": { "call_id": "…", "name": "execute_python", "arguments": { … } } } } }
```

The payload has the shape of one element of `GET …/topic/{topic_id}/history`. It is sent for `AssistantMessage`, `ToolCall` and `ToolResult` entries, including the results that close a call a stop cut off (their text starts `Stopped:` or `Interrupted:`). The `thinking`, `tool_choice` and `tool_response` frames above are unchanged.

To build a turn's activity without gaps or repeats, **open the socket first, then read history, and merge the two by `uid`**: every step is then in the read, in a frame, or in both. Do the same after a reconnect.

### Requests survive restarts

A request is recorded when it is accepted and removed when its turn ends. If vizier restarts (or the agent is updated) while a turn runs, the turn resumes from its last saved step: a call that was cut off is closed with a `Stopped:`/`Interrupted:` result and not run again. Its final reply carries `"resumed": true`. Requests that were queued come back in order. `abort` discards all of it. A turn that brings vizier down repeatedly without making progress is given up on with an `error` reply.

Requests to a session that is already processing one are **queued** and run in order after it finishes. `command: "abort"` (and `checkpoint`/`lobotomy`) cancels the in-flight request (you receive `abort`) **and discards the queue**.
