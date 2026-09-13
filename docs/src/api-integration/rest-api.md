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
| `GET` | `/agents/{id}/usage?start_date&end_date` | Token usage |
| `GET` / `PATCH` | `/agents/{id}/sharing` | `{ "add": [user_id], "remove": [user_id] }` |
| `GET` / `PUT` | `/agents/{id}/core` | `CORE.md` |
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
| `GET` | `/providers` | `[{ variant, has_api_key, base_url, enabled }]` |
| `GET` | `/providers/{variant}` | One provider |
| `PUT` | `/providers/{variant}` | Upsert: `{ "api_key"?, "base_url"?, "enabled"?, "access_token"?, "account_id"?, "endpoint"? }` |
| `DELETE` | `/providers/{variant}` | Remove |

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

## Misc

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/ping` | `"pong"`, no auth |

## WebSocket chat

```
ws://localhost:9999/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat?token=<jwt>
```

Text frames in both directions carry JSON. Ping/pong frames keep the connection alive; the server closes it after `ws_idle_timeout_secs` (default 300) of silence.

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

Requests to a session that is already processing one are **queued** and run in order after it finishes. `command: "abort"` (and `checkpoint`/`lobotomy`) cancels the in-flight request (you receive `abort`) **and discards the queue**.
