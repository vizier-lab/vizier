# 2.7 Agent Configuration

Agents are created and managed entirely at runtime — through the WebUI or `/api/v1/agents`. They are never defined in `.vizier.yaml`, and the old `*.agent.md` file format is not read.

Each agent runs as its own process inside the Vizier binary, with its own provider, tools, MCP clients, shell, embedding model, memory bundles, and `CORE.md`. Creating, updating, or deleting an agent restarts just that agent's process.

## Creating an agent

### WebUI

**Agents → New Agent**. Fill in name, provider, model; enable tools; save. The agent starts immediately.

### API

```http
POST /api/v1/agents
Authorization: Bearer <jwt>
Content-Type: application/json

{
  "agent_id": "assistant",
  "name": "Assistant",
  "description": "A helpful coding assistant",
  "provider": "anthropic",
  "model": "claude-sonnet-4-5",
  "system_prompt": "You are a helpful coding assistant specialized in Rust.",
  "tools": { "fetch": true, "shell": { "environment": "local", "path": "/home/me/project" } }
}
```

`PUT /api/v1/agents/{agent_id}` takes the **same body** and replaces the config (omitted optional fields fall back to their defaults, not to the previous value — send the full config). The creating user becomes the agent's owner.

## Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `agent_id` | string | required (create) | Stable identifier, used in URLs and the workspace path `agents/<agent_id>/` |
| `name` | string | required | Display name; injected into the boot prompt |
| `description` | string | `null` | Short purpose statement; injected into the boot prompt (defaults to "a Digital Steward") |
| `avatar_url` | string | `null` | Avatar shown in the WebUI |
| `provider` | string | required | A [provider id](./providers.md): catalogue, built-in or custom (old names like `gemini` are accepted) |
| `model` | string | required | Model id for that provider (for `ollama`, the model is pulled automatically on start) |
| `system_prompt` | string | `null` | Free-form system prompt. Sits between the built-in boot prompt and `CORE.md` |
| `thinking_depth` | number | `100` | **Max tool-call turns** per request. When exceeded the request fails with a `prompt_timeout` error |
| `prompt_timeout` | duration | `"60m"` | Wall-clock limit for one request (all turns included) |
| `max_tokens` | number | `null` | Cap on output tokens per completion; unset uses the model's own limit |
| `context_window` | number | `null` (catalogue) | Override the context window the catalogue gives for the model. Used for checkpoints |
| `checkpoint_threshold` | float 0–1 | `0.8` | Auto-checkpoint when `input_tokens / context_window` reaches this ratio |
| `silent_read_initiative_chance` | float 0–1 | `0.0` | Probability of replying to a `silent_read` (un-mentioned group message) |
| `tools` | object | all off | See [Tools & Embedding](./tools-embedding.md) |
| `embedding` | object | local `all_mini_lml6_v2` | Per-agent embedding model |
| `indexer` | object | `{ "kind": "sqlite" }` | Vector index |
| `discord_token` | string | `null` | Discord bot token — see [Channels](./channels.md) |
| `telegram_token` | string | `null` | Telegram bot token |
| `dream_enabled` | bool | `false` | Enable the dream cycle |
| `dream_schedule` | cron string | `null` | When to dream, e.g. `"0 3 * * *"` (required for the cycle to run) |
| `dream_provider` / `dream_model` | enum / string | `null` (same as main) | Use a different (cheaper) model for dreaming |

Duration strings use `duration_string` syntax: `30s`, `5m`, `2h`, `1d`.

Persisted but currently unused: `heartbeat_interval` (always `30m`), `include_documents`.

### Response shape

`GET /api/v1/agents` returns summaries (`agent_id`, `name`, `description`, `avatar_url`, `owner_id`, `owner_username`, `shared_to`). `GET /api/v1/agents/{id}` returns the full config; tokens and keys are included for users who can edit the agent.

## System prompt stack

Each request is prefixed with these system messages, in order:

1. **Boot** — built-in operating doctrine (`BOOT.md`), parameterized with `name` and `description`
   - **Sandbox** — `SANDBOX.md`, only when `tools.python.enabled`: how to use `execute_python` (sandbox-only or code-mode variant)
2. **`system_prompt`** — yours, or a one-line default
3. **Owner profile** — the owner user's profile (display name, Discord/Telegram ids, aliases) if set
4. **`CORE.md`** — the agent's self-maintained document

## `CORE.md`

Every agent has a persistent markdown document, seeded from the built-in template on creation (identity, operating rules, learnings). The agent reads and **overwrites** it with the `READ_CORE` / `WRITE_CORE` tools and is instructed to update it when it learns something durable about itself or its user. It is stored in the database, not on disk.

- **WebUI**: agent → Core (markdown editor)
- **API**: `GET|PUT /api/v1/agents/{id}/core`

## Sessions, topics, and checkpoints

A session is `(agent, channel, topic)`. History is stored per session and replayed in full into the model context on each request.

### Checkpoints

When the last completion's `input_tokens` reach `checkpoint_threshold × context_window`, the agent:

1. Generates a **handover summary** of the conversation so far
2. Saves a checkpoint to the session
3. Clears the model context, rebuilds the system prompts, and injects the handover as `# Conversation Context (Previous Checkpoint)`

Clients see a `checkpoint` response event with the handover text. Manual variants exist as channel commands:

- `/checkpoint` — same as above, on demand
- `/lobotomy` — checkpoint **without** a handover (clean break, history retained but not summarized)
- `/abort` — cancel the in-flight response

## Ownership and sharing

- The creating user is the **owner**. Owners can always view/edit/delete their agents.
- `GET|PATCH /api/v1/agents/{id}/sharing` with `{ "add": [user_id…], "remove": [user_id…] }` shares an agent read/chat-only with other users.
- Roles with `all_agents:view` / `all_agents:edit` / `all_agents:delete` bypass ownership; `owned_agents:delete` allows deleting own agents. The `superadmin` system role has everything.
- `DELETE /api/v1/agents/{id}?delete_workspace=true` also removes `agents/<id>/` (memory, skills) from disk.

## Dream cycle

An optional, scheduled reflection pass. When `dream_enabled` is true and `dream_schedule` is a valid cron expression, the scheduler runs (per agent, never concurrently):

1. **Extraction** — for every user session with activity since the last dream, the agent (using `dream_provider`/`dream_model` if set) writes a structured *extraction report*: facts & preferences, feedback, task progress, relationship context, learnings, action items
2. **Consolidation** — with the [restricted dream toolset](./tools-embedding.md#dream-cycle-tool-subset), the agent writes durable facts into memory (`memory_write`), updates `CORE.md`, schedules follow-up tasks, and may create/update skills
3. Each stage's output is saved as a **dream journal** entry

Endpoints (all under `/api/v1/agents/{id}/dream`):

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/trigger` | Run a dream cycle now |
| `GET` | `/status` | `idle` / `extracting` / `consolidating` with progress |
| `GET` | `/journal` | List journal entries |
| `GET` | `/journal/{entry_id}` | One entry |

The WebUI exposes this on the agent's **Dream** page.

## Tasks

The scheduler runs cron and one-time tasks that the agent creates for itself (`schedule_cron_task`, `schedule_one_time_task`) or that you create via the API:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/agents/{id}/tasks` | List |
| `POST` | `/api/v1/agents/{id}/tasks` | Create |
| `GET|PUT|DELETE` | `/api/v1/agents/{id}/tasks/{slug}` | Read / update / delete |

A task's prompt runs as the agent in a dedicated `Task` session; the agent can report results into a WebUI topic with `webui_send_message`, or into Discord/Telegram with the platform tools.

## Usage

Every model call an agent makes is recorded as it is answered, whether or not a reply is delivered: chat turns (each tool round on its own), scheduled runs, background jobs and consults, dream cycles, checkpoint summaries, and image reads on a vision model. Rounds of a turn that failed, timed out or was aborted still count, and are marked as spent on a failed turn. Each call records its provider, the model requested and the model version the provider says it served.

Each call has one **activity**:

| Activity | What it covers |
|---|---|
| Chat | Turns answering a person on Discord, Telegram, the WebUI or the HTTP API, including the turn a finished background job wakes |
| Background | Subagents (`parallel_subagents` and `background_subagents`), delegations (`delegate_agent`) and consults (`consult_agent`), counted for the agent that ran them |
| Scheduled | Scheduled task runs |
| Dreams | Dream extraction and consolidation |
| Compaction | Checkpoint summaries, automatic and `/checkpoint` |
| Vision | `read_image_file` on the configured vision model |
| Other | Anything else (none today) |

Not counted: speech-to-text, text-to-speech, image generation, embeddings, and the provider **Test connection** check.

Cost is estimated from the provider catalogue's prices. Models without a known price (custom providers, for now) are counted in tokens and reported as unpriced, never as `$0`.

The WebUI **Usage** page shows the period's cost with its change against the previous period, an activity filter, a daily cost chart, a breakdown by activity, model or channel, the cache hit rate, what was spent on failed turns, and the ten costliest conversations with their calls one by one.

When upgrading, the usage each saved reply already carried is copied over once, so totals from before the upgrade don't change. Dreams, checkpoints, failed rounds and vision reads were never recorded before then, so the page marks days before the upgrade as incomplete.

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/agents/{id}/usage?start_date&end_date&activity` | The usage report (RFC 3339 dates, default the last 30 days, at most 366 days) |
| `GET` | `/api/v1/agents/{id}/usage/conversation?channel&topic&start_date&end_date&activity&limit` | One conversation's calls, most expensive first, and a link to it in the WebUI |

## Health

- `GET /api/v1/agents/health` — all agent processes with alive/dead status (same data as `vizier agent ps`)
- `GET /api/v1/agents/{id}/ping` — one agent

## Complete example

```json
{
  "agent_id": "steward",
  "name": "Steward",
  "description": "Personal assistant and project manager",
  "avatar_url": null,
  "provider": "openrouter",
  "model": "anthropic/claude-sonnet-4.5",
  "system_prompt": "Be concise. Prefer bullet points.",
  "thinking_depth": 60,
  "prompt_timeout": "30m",
  "max_tokens": 8192,
  "checkpoint_threshold": 0.75,
  "silent_read_initiative_chance": 0.05,
  "tools": {
    "timeout": "10m",
    "shell": { "environment": "docker", "image": { "source": "pull", "name": "ubuntu:latest" }, "container_name": "steward-sandbox" },
    "brave_search": true,
    "brave_search_settings": { "api_key": "BSA…", "safesearch": true },
    "fetch": true,
    "http_client": true,
    "discord": true,
    "telegram": false,
    "mcp_servers": {
      "github": { "host": "local", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": { "GITHUB_TOKEN": "ghp_…" } }
    },
    "tts": true,
    "tts_settings": { "provider": "openai", "voice": "nova" },
    "stt": true,
    "stt_settings": { "provider": "whisper" },
    "read_image": true,
    "read_image_settings": { "provider": "openrouter", "model": "google/gemini-2.5-flash" },
    "image_gen": false
  },
  "embedding": { "provider": "local", "model": "bge_small_env15" },
  "indexer": { "kind": "sqlite" },
  "discord_token": "MTA…",
  "dream_enabled": true,
  "dream_schedule": "0 4 * * *",
  "dream_provider": "openrouter",
  "dream_model": "google/gemini-2.5-flash"
}
```
