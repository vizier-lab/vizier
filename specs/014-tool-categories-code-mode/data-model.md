# Data Model: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Feature**: `014-tool-categories-code-mode` | **Date**: 2026-10-14

Nothing new is persisted. Every entity below lives in memory, is derived from the agent's configuration and its registered tools, and is rebuilt whenever `VizierTools::new` runs. The only change to persisted data is a field being removed (`AgentToolsConfig.python`).

## ToolCategory (built-in)

A closed enum. Variant order is display order (FR-012).

| Variant | `name()` | `description()` |
|---------|----------|-----------------|
| `Memory` | `memory` | Search, read, write and organise long-term memory |
| `Identity` | `identity` | Read and update your CORE identity document |
| `Scheduling` | `scheduling` | Schedule one-time and recurring tasks and check their results |
| `Agents` | `agents` | Consult or delegate to other agents, run parallel subtasks, manage background jobs |
| `Skills` | `skills` | Create, find and use reusable skills |
| `Files` | `files` | Read files attached to this conversation and send attachments |
| `Web` | `web` | Search the web and news, fetch pages, make HTTP requests |
| `Audio` | `audio` | Generate speech from text and transcribe audio |
| `Media` | `media` | Generate images |
| `Discord` | `discord` | Send, read and react to Discord messages; look up guild, channel and member info |
| `Telegram` | `telegram` | Send, read and react to Telegram messages |
| `Webui` | `webui` | Message WebUI users and list their topics |
| `Shell` | `shell` | Run shell commands |

**Tool → category** (via `VizierTool::category()`):

| Category | Tools |
|----------|-------|
| `Memory` | `memory_search`, `memory_read`, `memory_write`, `memory_list`, `memory_follow`, `memory_graph`, `memory_delete`, `memory_delete_bundle` |
| `Identity` | `read_core`, `write_core` |
| `Scheduling` | `schedule_one_time_task`, `schedule_cron_task`, `list_task`, `get_task_detail`, `delete_task`, `list_task_runs`, `get_task_run_detail` |
| `Agents` | `consult_agent`, `delegate_agent`, `paralel_subtasks`, `list_background_jobs`, `cancel_background_job` |
| `Skills` | `create_skill`, `update_skill`, `delete_skill`, `list_skills`, `get_skill_details`, `use_skill`, `read_skill_resource`, `execute_skill_resource` |
| `Files` | `list_session_files`, `read_document_file`, `read_image_file`, `send_attachment` |
| `Web` | `web_search`, `news_search`, `fetch`, `http_client` |
| `Audio` | `tts_generate`, `stt_transcribe` |
| `Media` | `image_generate` |
| `Discord` | `discord_send_message`, `discord_react_message`, `discord_get_message_by_id`, `discord_get_guild_info`, `discord_get_channel_info`, `discord_get_member_info` |
| `Telegram` | `telegram_send_message`, `telegram_react_message`, `telegram_get_message_by_id` |
| `Webui` | `webui_send_message`, `webui_list_topics` |
| `Shell` | `shell_exec` |
| `None` | `think`, `execute_python`, `list_tool_functions`, `describe_tool_function` (direct), `read_dream_journal`, `write_dream_journal` (dream only) |

**Validation**: every tool in `ToolRouter.default_toolset` or `user_toolset` has `Some` category. A unit test builds a toolset with every optional tool on and checks this.

## CategorySummary

What the system prompt lists, computed once per `VizierTools`.

| Field | Type | Notes |
|-------|------|-------|
| `name` | `String` | The built-in `ToolCategory::name()`; for MCP, the server key, or `<key>_mcp` if the key equals a built-in name (R5) |
| `description` | `String` | The built-in `description()`, or "Tools from the `<key>` MCP server" |

**Order**: built-in categories in enum order, only those with at least one registered tool (FR-011), then MCP categories sorted by `name`. Every configured server is included, whether or not it is connected (FR-010b).

## CatalogueEntry

One script-callable function, built by `ToolRouter::catalogue()` for each listing, description or script run.

| Field | Type | Notes |
|-------|------|-------|
| `name` | `String` | Script and model-facing name (R4): `python_identifier(tool)` for native tools; `python_identifier("<server>_<tool>")` for MCP tools, with `_2`, `_3`, … on a collision |
| `category` | `String` | A `CategorySummary.name` |
| `target` | `Native(String)` \| `Mcp { server: String, tool: String }` | What dispatch calls; `tool` is the server's own name for it |
| `definition` | `ToolDefinition` | Description and parameters, as today |

**Invariants**:
- `name` values are unique within one catalogue.
- A native entry's `name` never carries a suffix.
- For the same set of tools, the names come out the same, whatever order the servers connected in.

**Lifecycle**: an MCP server going up or down adds or removes its entries the next time the catalogue is built. Suffixes are recomputed from the current set (spec edge case).

## Removed

| Item | Where | Replaced by |
|------|-------|-------------|
| `PythonSandboxConfig { enabled, code_mode }` | `src/schema/agent.rs` | nothing: code mode always on |
| `AgentToolsConfig.python` | `src/schema/agent.rs` | — (stored values ignored by serde) |
| `python` on HTTP agent DTOs | `src/channels/http/api/v1/agents/mod.rs` | — (still accepted, then ignored) |
| `ToolExposure`, `CODE_MODE_DIRECT_TOOLS`, `exposed_directly` | `src/agents/tools/mod.rs` | `sandbox_toolset` is the direct set |
| `PythonSandboxConfig` / `DEFAULT_PYTHON_SANDBOX` / `PythonSandboxSection` | `webui/app/...` | — |
| `mcp_<server>__<tool>`, `READ_CORE`, `WRITE_CORE` tool names | dispatch and renderers | `<server>_<tool>`, `read_core`, `write_core`; old names are not resolved |
