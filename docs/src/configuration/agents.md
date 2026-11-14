# 2.7 Agent Configuration

Agents are created and managed entirely at runtime — through the WebUI or `/api/v1/agents`. They are never defined in `.vizier.yaml`, and the old `*.agent.md` file format is not read.

Each agent runs as its own process inside the Vizier binary, with its own provider, tools, MCP clients, shell, embedding model, memory bundles, and Core memory. Creating, updating, or deleting an agent restarts just that agent's process.

## Creating an agent

### WebUI

**Agents → New Agent** (a modal on Home). Give it a name, choose its model, add any addons it should start with, and save. The agent starts immediately.

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
  "tools": { "tts": false, "stt": false, "read_image": false, "image_gen": false }
}
```

Shell, HTTP and search are **addons** now: install one for the agent with `POST /api/v1/agents/{id}/integrations` (the WebUI does this from the agent’s Addons page and the creation wizard).

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
| `system_prompt` | string | `null` | Free-form system prompt. Sits between the built-in boot prompt and the Core memory |
| `thinking_depth` | number | `100` | **Max tool-call turns** per request. When exceeded the request fails with a `prompt_timeout` error |
| `prompt_timeout` | duration | `"60m"` | Wall-clock limit for one request (all turns included) |
| `max_tokens` | number | `null` | Cap on output tokens per completion; unset uses the model's own limit |
| `context_window` | number | `null` (catalogue) | Override the context window the catalogue gives for the model. Used for checkpoints |
| `checkpoint_threshold` | float 0–1 | `0.8` | Auto-checkpoint when `input_tokens / context_window` reaches this ratio |
| `silent_read_initiative_chance` | float 0–1 | `0.0` | Probability of replying to a `silent_read` (un-mentioned group message) |
| `tools` | object | all off | Media tools and timeouts. Shell, HTTP, search, Office and MCP are addons. See [Tools & Embedding](./tools-embedding.md) |
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
4. **Core** — the agent's identity, as the addressed memory block `<memory bundle="core" path="soul">`

## Core

Every agent has a **reserved memory bundle** called `core` holding one document, `soul`: its identity, operating rules and what it has learned about itself and its people. It is ordinary memory in every mechanical respect — read it with `memory_read`, write it with `memory_write`, at `bundle="core"`, `path="soul"` — and it is the only memory injected into every turn, because an agent must always know who it is. A new agent's soul is seeded from the built-in template; an existing agent's is copied from its previous identity document on upgrade.

Because it is paid for on every turn, it has tighter [size limits](./memory.md#size-limits) than other memories. The `core` bundle accepts nothing but `soul` and cannot be deleted, but the soul itself can be deleted and rewritten freely; a deleted soul falls back to the template so the agent never sees an empty identity.

- **WebUI**: agent → Memory → `core` → `soul`
- **API**: `GET|PUT /api/v1/agents/{id}/memory/doc/core/soul`. The older `GET|PUT /api/v1/agents/{id}/core` still works — it is a **deprecated** projection over the same document — but new clients should use the memory routes.

## Secrets

An agent can keep named credentials in its own store, read them from Python scripts, and never show them to the model. The `secrets` tool category (`set_secret`, `get_secret`) is in every ordinary, scheduled and background turn but never in the dream cycle. People manage the store in **WebUI → Secrets** or over `GET|PUT|DELETE /api/v1/agents/{id}/secrets[/{name}]` (plus `GET …/secrets/events`), under `all_agents:secret` or `owned_agents:secret` for the agent's owner. No route returns a value; see [Agent Secrets](./secrets.md) for the limits and the redaction boundary.

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

## Background work

An agent runs slow work in the background by calling `execute_python` with `detached: true`. The call returns a handle such as `{"detached": true, "id": "d_k3x9a1b2", "state": "running", "timeout_secs": 600}` at once, the script runs in its own task, and the agent carries on: it can keep talking to the person while a test suite runs or several pages are fetched. Any tool the script calls runs in the background with it.

- **Time limit.** `timeout_secs` (1 to 3600, default 600) bounds the whole script, every tool and wait included. Without `detached` the field is ignored.
- **What is not available inside.** Asking the person (`ask_question`, `ask_sensitive_input`), the person's client tools (the WebUI's `ui_render` and the like) and every tool that ends the turn: the turn that launched the script may be over. They are absent from `list_tools()` there.
- **Delivery.** When the script ends, its result reaches the agent exactly once as a message starting `[Background work finished]`: in the agent's next step if it is working on a turn of that conversation, otherwise as a new turn that wakes it. Files the script produced become session files of the conversation and are named in the message.
- **The agent's tools.** `list_detached_scripts`, `get_detached_script` and `cancel_detached_script` (category `agents`) list, inspect and stop the conversation's background work. A cancel also stops what the script was waiting on and any background work started beneath it; nothing is delivered for a cancelled script.
- **People.** The chat's status strip shows **Background work** with a count while something runs; its panel lists each script by its intent and opens it the way the activity trail shows a script (code, the calls inside, the result). Each finished script also raises one notification. Cancelling from the WebUI is not available yet; ask the agent to cancel.
- **Lifecycle.** Updating or restarting the agent, or restarting the server, interrupts its running scripts; the agent is told in a delivery. Deleting the agent or the conversation stops and removes them. Scheduled-task and dream turns cannot detach (nobody is waiting to be told), and a script at the third level of background nesting cannot start another.

`background_subagents`, `list_background_jobs` and `cancel_background_job` were removed in this release; an agent that calls one is told what replaced it.

## Subagents

An agent splits work with independent parts between **workers**: temporary helpers that are the agent itself, working on one task each. A worker acts with the agent's tools, memory and permissions, and what it does is credited to the agent; it has no settings of its own. Its prompt tells it so: it works *for* the agent, the agent's identity (`core/soul`) and owner are described as its employer's, and its answer goes back to the agent, not to a person.

- **The tools** (category `subagents`). `spawn_subagents` hires one worker per task and returns their handles (`s_…`) at once. `wait_subagents` waits until one (`until: "any"`) or every (`until: "all"`) worker has settled; a worker that is asking a question counts as settled, so the agent is never stuck waiting on a worker that waits for it. `reply_subagent` answers a worker's question, `stop_subagent` stops one worker, and `list_subagents` and `get_subagent` read them.
- **Questions.** A worker that cannot go on without the agent's decision calls `ask_main_agent`. Its step ends, its state reads `asking`, and the agent's reply continues it in the same conversation. Workers never ask the person.
- **Lifetime.** A worker lives as long as the conversation it was hired in. It keeps running when the agent's turn ends or a person sends `/abort`, it resumes after an update or a restart, and it stops only when it finishes, is stopped, reaches its time limit (`timeout_secs`, 1 to 3600, default 600; time spent waiting for a reply does not count), or its conversation or agent is deleted.
- **Delivery.** An answer or a question the agent has not already read through `wait_subagents` or `get_subagent` reaches it exactly once, as a message starting `[Subagent update]`: in its next step if it is working, otherwise as a new turn.
- **Depth.** Only the main agent hires workers. The [`subagents.max_depth`](./main-config.md#subagents) setting raises the limit.

`parallel_subagents` was removed in this release; an agent that calls it is told to use `spawn_subagents` and `wait_subagents`.

## Dream cycle

An optional, scheduled reflection pass. When `dream_enabled` is true and `dream_schedule` is a valid cron expression, the scheduler runs (per agent, never concurrently):

1. **Extraction** — for every user session with activity since the last dream, the agent (using `dream_provider`/`dream_model` if set) writes a structured *extraction report*: facts & preferences, feedback, task progress, relationship context, learnings, action items
2. **Consolidation** — with the [restricted dream toolset](./tools-embedding.md#dream-cycle-tool-subset), the agent writes durable facts into memory (`memory_write`), updates its Core at `core/soul`, schedules follow-up tasks, and may create/update skills
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

The WebUI shows them on the agent's **Schedule** page, as a calendar (a month, or one day) or as a table. The calendar places each task on the days its schedule names, in the [instance's timezone](./time.md), whatever your browser's is; cron expressions are evaluated in that same zone, as the scheduler does. The past shows only each task's latest run, since that is what the task list carries. The filter tabs (Active, Last run failed, All) open on Active and apply to both views.

A task's prompt runs as the agent in a dedicated `Task` session; the agent can report results into a WebUI topic with `webui_send_message`, or into Discord/Telegram with the platform tools.

## Usage

Every model call an agent makes is recorded as it is answered, whether or not a reply is delivered: chat turns (each tool round on its own), scheduled runs, sub-agent work (inline or in a detached script), dream cycles, checkpoint summaries, and image reads on a vision model. Rounds of a turn that failed, timed out or was aborted still count, and are marked as spent on a failed turn. Each call records its provider, the model requested and the model version the provider says it served.

Each call has one **activity**:

| Activity | What it covers |
|---|---|
| Chat | Turns answering a person on Discord, Telegram, the WebUI or the HTTP API, including the turn a finished detached script wakes |
| Subagents | Worker turns (`spawn_subagents`), counted for the agent that hired them |
| Scheduled | Scheduled task runs |
| Dreams | Dream extraction and consolidation |
| Compaction | Checkpoint summaries, automatic and `/checkpoint` |
| Vision | `read_image_file` on the configured vision model |
| Other | Anything else (none today) |

Not counted: speech-to-text, text-to-speech, image generation, embeddings, and the provider **Test connection** check.

Cost is estimated from the provider catalogue's prices. Models without a known price (custom providers, for now) are counted in tokens and reported as unpriced, never as `$0`.

The WebUI **Usage** page shows the period's cost with its change against the previous period, an activity filter, a daily chart you can switch between estimated cost and tokens spent, a breakdown by activity, model or channel, the cache hit rate, what was spent on failed turns, and the ten costliest conversations with their calls one by one.

When upgrading, the usage each saved reply already carried is copied over once, so totals from before the upgrade don't change. Dreams, checkpoints, failed rounds and vision reads were never recorded before then, so the page marks days before the upgrade as incomplete.

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/agents/{id}/usage?start_date&end_date&activity` | The usage report (RFC 3339 dates, default the last 30 days, at most 366 days) |
| `GET` | `/api/v1/agents/{id}/usage/conversation?channel&topic&start_date&end_date&activity&limit` | One conversation's calls, most expensive first, and a link to it in the WebUI |

## Health

- `GET /api/v1/agents/health` — all agent processes with alive/dead status (same data as `vizier agent ps`)
- `GET /api/v1/agents/{id}/ping` — one agent

## Addons

Tools beyond the always-on set are **addons**, attached to one agent and installed like small apps:

- **Shell** — `shell_exec` in a working directory on the Vizier host.
- **HTTP** — `fetch` and `http_client` together.
- **Brave Search** — web and news search.
- **Office** — read and write Word and Excel files.

Install or remove one from the agent's **Addons** page, or with `POST /api/v1/agents/{id}/integrations`:

```json
{ "kind": "http", "name": "HTTP", "settings": {} }
```

Each addon runs under its own category in code mode (`shell`, `http`, `brave_search`, `office`). Shell and HTTP were per-agent switches before; installing them on an existing agent carries the old setting across on the first start after the upgrade.

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
