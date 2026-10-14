# Feature Specification: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Feature Branch**: `014-tool-categories-code-mode`

**Created**: 2026-10-14

**Status**: Draft

**Input**: User description: "let's make programmatic tool calling the default behaviour from now on, to support this as the default behaviour i think we should also need to give tag/category for tools. agent then can list tool by categories. tools added by mcp should have the name of mcp server as the tag, while we are at it tools provided wont need the mcpname__ prefix anymore, wdyt?"

## Background

Today an agent can work in one of three ways. With **direct** tools (the default) every tool definition is sent to the model on every turn. **Sandbox additive** adds a Python runner on top. In **code mode** only a handful of tools are offered directly (`think`, `execute_python`, `list_tool_functions`, `describe_tool_function`), and the agent reaches every other tool by writing a script. Code mode is opt-in and needs two switches. When an agent asks what it can call, it gets one flat list of every function. MCP tools are named `mcp_<server>__<tool>`, and the chat views (WebUI, Discord, Telegram) parse that prefix to show "tool (server)".

This feature makes code mode the only way an agent works: the sandbox and code-mode switches are removed, so every agent, new or existing, runs in code mode with nothing to migrate. Because of that, finding the right tool has to scale: tools get categories, and the agent can browse by category instead of reading one long list. MCP tools are categorised by their server, and their names get simpler: `github_create_issue` instead of `mcp_github__create_issue`. The server name stays in the tool name so tools from different servers stay distinct, but the `mcp_` marker and the double underscore go.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every agent works in code mode (Priority: P1)

An operator creates an agent, or upgrades vizier with agents already set up. Every agent works in code mode: the model is offered only the small set of direct tools, and it reaches everything else (memory, scheduler, web, MCP, …) through scripts. There is no sandbox or code-mode setting to choose and nothing to migrate.

**Why this priority**: This is the headline change. Every other story exists to make code mode work well.

**Independent Test**: Start vizier on a workspace that holds an agent saved before this change with `code_mode: false`, on the offline `dummyplug` provider. Send `tools`: the listed direct tools must be the code-mode set. Then run a script through `execute_python` that calls a regular tool (e.g. a memory search) and returns its result. Repeat with a newly created agent.

**Acceptance Scenarios**:

1. **Given** any agent, new or created before this change, **When** it receives a turn, **Then** the model is offered only the code-mode direct tools, and every other enabled tool can be called from a script.
2. **Given** an agent saved before this change with Python sandbox or code-mode settings, **When** vizier starts, **Then** the agent loads and runs normally, its old settings are ignored, and no migration step runs.
3. **Given** the WebUI's agent tool settings, **When** the operator opens them, **Then** there are no Python sandbox or code-mode switches.
4. **Given** a client that still sends Python sandbox settings when creating or updating an agent, **When** the request is handled, **Then** those settings are ignored and the request is not rejected.

---

### User Story 2 - Agent discovers tools by category (Priority: P1)

An agent in code mode needs to do something, for example schedule a reminder or open a GitHub issue. It already knows the categories from its system prompt. Instead of reading one long list of every function, it lists only the functions in the category it needs, then asks for that function's details. If it is unsure which category fits, it can still list every function, each labelled with its category. Inside a script it can do the same lookup.

**Why this priority**: Without this, code mode gets more expensive as tools are added. One flat list of 60+ functions costs more context than the tool definitions code mode was meant to save.

**Independent Test**: On `dummyplug`, call the listing tool with one category and check that only that category's functions come back. Call it with no category and check that every function comes back, each labelled with its category. Do the same from inside a script.

**Acceptance Scenarios**:

1. **Given** a code-mode agent, **When** it lists tools without naming a category, **Then** it gets every function it can call, each with its name, summary and category.
2. **Given** a code-mode agent, **When** it lists tools for one category, **Then** it gets exactly the functions in that category, each with a name and a summary.
3. **Given** a category name that does not exist, **When** the agent lists it, **Then** it gets a clear "no such category" answer with the closest matching category names. This is not an error that ends the turn.
4. **Given** any agent's turn, **When** its system prompt is built, **Then** it contains a list of the agent's tool categories (name and one-line description each) and short instructions for finding and calling a tool, so the agent can go straight to listing one category without first asking what categories exist.
5. **Given** a script, **When** it calls the in-script listing with or without a category, **Then** it gets the same results as the direct listing tool.

---

### User Story 3 - MCP tools are tagged by server and get simpler names (Priority: P2)

An operator attaches an MCP server called `github` to an agent. Its tools show up under a `github` category, named `<server>_<tool>` (`github_create_issue`, not `mcp_github__create_issue`). The agent calls them from scripts by those names. The old names stop working.

**Why this priority**: Shorter, more natural names are easier for the model to write correctly in scripts, while the server name in each tool name keeps tools from different servers distinct. Which server a tool belongs to comes from its category (Story 2), not from parsing the name: a server name can itself contain underscores, so `<server>_<tool>` cannot be split reliably.

**Independent Test**: Give a `dummyplug` agent an MCP server, list its category, and call one of its tools by its `<server>_<tool>` name. Calling the old `mcp_<server>__<tool>` name from a script fails with an unknown-function error.

**Acceptance Scenarios**:

1. **Given** an agent with MCP server `github` exposing `create_issue`, **When** the agent lists categories, **Then** a `github` category appears containing `github_create_issue`.
2. **Given** that agent, **When** it calls `github_create_issue`, **Then** the call goes to the `github` server.
3. **Given** an MCP tool whose `<server>_<tool>` name equals a built-in tool's name (e.g. server `memory` with tool `search` versus built-in `memory_search`), or two MCP tools that end up with the same name (e.g. server `a_b` tool `c` and server `a` tool `b_c`), **When** tools are listed, **Then** every tool is still listed and callable under a unique name: built-in tools keep their names, and colliding MCP tools get a predictable distinguishing suffix.
4. **Given** a model that calls a tool by its old `mcp_<server>__<tool>` name (e.g. copying from earlier history), **When** the call is dispatched, **Then** it fails as an unknown function, and the agent can find the current name through `list_tool_functions`.
5. **Given** a past conversation whose history contains calls under old names, **When** it is viewed, **Then** it still loads, and those calls show with the generic tool-call display under their full old name.

---

### User Story 4 - Built-in tools have meaningful categories (Priority: P2)

Every built-in tool belongs to a category that reflects what it is for, for example memory, scheduling, other agents, web, audio, media, each messaging channel (Discord, Telegram, WebUI), session files, skills, shell or identity/CORE. An agent looking for "something to do with reminders" finds the scheduling tools together.

**Why this priority**: Category browsing is only useful if the built-in catalogue is sorted sensibly. Built-in tools are most of what an agent uses.

**Independent Test**: List the categories of an agent that has every optional tool enabled. Check that every built-in tool appears in exactly one category, and that no category is empty.

**Acceptance Scenarios**:

1. **Given** any agent, **When** it lists all categories and then lists each one, **Then** every function it can call from a script appears in at least one category, and none is missing.
2. **Given** an optional tool family that is disabled for an agent (e.g. image generation), **When** the agent lists categories, **Then** that category is not shown in the system prompt or in listings.

---

### Edge Cases

- **MCP server named like a built-in category** (e.g. a server called `memory` or `discord`): the server's tools must not be merged into the built-in category, and both must still be clearly told apart in listings.
- **MCP server offline or slow**: its category stays in the system prompt (FR-010b). Listing that category reports it as currently unavailable rather than failing, and its tools cannot be called until it is back, as today.
- **MCP server's tool list changes mid-conversation** (a tool is added that collides with an existing name): names stay predictable from the current set of tools. A tool that was renamed because of a collision keeps that name while the collision lasts.
- **MCP tool names that are not valid script identifiers** (dashes, leading digits, reserved words): they are cleaned up the way they are today, and the cleaned-up name is unique.
- **Dream cycle**: the restricted dream tool set is unchanged and is not affected by code mode or categories.
- **Existing agents created before this change**: they run in code mode on upgrade (FR-003). An agent that had the sandbox off loses direct tool calls, which is the accepted trade-off of removing the switch.
- **Model that writes Python poorly** (e.g. a small local model): there is no fallback to direct tools. This is an accepted risk (see Assumptions).

## Requirements *(mandatory)*

### Functional Requirements

**Code mode as the default**

- **FR-001**: Every agent MUST run in code mode. Only the code-mode direct tools are offered to the model, and every other tool is reached from scripts.
- **FR-002**: The per-agent Python sandbox and code-mode settings MUST be removed, along with the direct and sandbox-additive modes they selected.
- **FR-003**: Agents that existed before this change MUST run in code mode on upgrade without a migration step. Stored sandbox and code-mode values MUST be ignored when an agent is loaded, never cause an error.
- **FR-004**: The WebUI MUST no longer show sandbox or code-mode switches, and the agent create/update API MUST accept but ignore them from older clients.

**Tool categories**

- **FR-005**: Every tool an agent can call MUST belong to at least one category. A category has a name and a short description.
- **FR-006**: Every built-in tool MUST be given one of the built-in categories below. The planning phase may rename or split them, but every current tool MUST end up in exactly one.

  | Category | Description (as shown to the agent) | Tools today |
  |----------|-------------------------------------|-------------|
  | `memory` | Search, read, write and organise long-term memory | `memory_search`, `memory_read`, `memory_write`, `memory_list`, `memory_follow`, `memory_graph`, `memory_delete`, `memory_delete_bundle` |
  | `identity` | Read and update your CORE identity document | `read_core`, `write_core` |
  | `scheduling` | Schedule one-time and recurring tasks and check their results | `schedule_one_time_task`, `schedule_cron_task`, `list_task`, `get_task_detail`, `delete_task`, `list_task_runs`, `get_task_run_detail` |
  | `agents` | Consult or delegate to other agents, run parallel subtasks, manage background jobs | `consult_agent`, `delegate_agent`, `paralel_subtasks`, `list_background_jobs`, `cancel_background_job` |
  | `skills` | Create, find and use reusable skills | `create_skill`, `update_skill`, `delete_skill`, `list_skills`, `get_skill_details`, `use_skill`, `read_skill_resource`, `execute_skill_resource` |
  | `files` | Read files attached to this conversation and send attachments | `list_session_files`, `read_document_file`, `read_image_file`, `send_attachment` |
  | `web` | Search the web and news, fetch pages, make HTTP requests | `web_search`, `news_search`, `fetch`, `http_client` |
  | `audio` | Generate speech from text and transcribe audio | `tts_generate`, `stt_transcribe` |
  | `media` | Generate images | `image_generate` |
  | `discord` | Send, read and react to Discord messages; look up guild, channel and member info | `discord_send_message`, `discord_react_message`, `discord_get_message_by_id`, `discord_get_guild_info`, `discord_get_channel_info`, `discord_get_member_info` |
  | `telegram` | Send, read and react to Telegram messages | `telegram_send_message`, `telegram_react_message`, `telegram_get_message_by_id` |
  | `webui` | Message WebUI users and list their topics | `webui_send_message`, `webui_list_topics` |
  | `shell` | Run shell commands | `shell_exec` |

  `think`, `execute_python`, and the two discovery tools are offered to the model directly, so they are not in any category.
- **FR-007**: Every tool provided by an MCP server MUST have that server's configured name as its category.
- **FR-008**: One listing tool MUST cover both cases, through a direct tool call and from inside a script. With a category, it returns that category's functions (name and summary). Without one, it returns every function the agent can call (name, summary and category). There is no separate wildcard value; leaving the category out is how the agent asks for everything.
- **FR-009**: Listing with an unknown category MUST return a non-fatal "not found" result that suggests the closest matching category names.
- **FR-010**: The agent's system prompt MUST list its tool categories, built-in and MCP alike, each with its name and one-line description. An MCP category's description is "Tools from the `<server>` MCP server". The server's own self-description is not used: it is only known once the server connects, which would change the prompt between turns (FR-010b).
- **FR-010a**: The system prompt MUST include short instructions for finding and calling tools:
  1. Pick a category from the list.
  2. List its functions, directly or from inside a script.
  3. Describe the function you need to see its parameters.
  4. Call it from an `execute_python` script.

  It MUST also tell the agent to batch several calls in one script where it can, and say how tool errors show up inside a script. With this, finding a tool normally takes no more than two lookups.
- **FR-010b**: The category list in the system prompt MUST stay the same from turn to turn while the agent's configuration is unchanged. It shows every configured MCP server even when that server is temporarily unreachable, and it leaves out function counts, which change when a server drops out. Live availability is reported by the listing tool instead.
- **FR-011**: A built-in category whose tools are all disabled for the agent (e.g. `audio` when speech generation and transcription are off, or `discord` when the agent has no Discord channel) MUST NOT be listed. An MCP category is listed for every configured server (FR-010b).
- **FR-012**: The order of categories and of functions within a category MUST be stable across restarts, so the system prompt stays the same from turn to turn.

**MCP tool naming**

- **FR-013**: MCP tools MUST be offered and called as `<server>_<tool>`, where `<server>` is the configured server name and `<tool>` is the name the server gives the tool. Characters that are not valid in a function name are cleaned up as today.
- **FR-014**: When an MCP tool's `<server>_<tool>` name collides with a built-in tool or with another MCP tool, the built-in tool MUST keep its name, and each colliding MCP tool MUST get a unique, predictable name (e.g. a numeric suffix). The result MUST be the same for the same set of tools, whatever order servers connect in.
- **FR-015**: Old `mcp_<server>__<tool>` names MUST NOT be supported: they are neither listed nor dispatched. A call under one fails like any unknown function.
- **FR-016**: No view parses tool names to find an MCP server. New MCP calls are made only from scripts, and script calls are deliberately not shown individually (spec 007). The special `mcp_` prefix rendering in the WebUI chat and on Discord/Telegram is removed, so calls already in history under old names fall back to the generic tool-call display, which shows the full name. Where a tool's server matters to the agent, it comes from the tool's category.
- **FR-017**: Every part of this feature MUST be checkable end to end on the offline `dummyplug` provider without provider keys: listing by category, calling `<server>_<tool>` names from scripts, old names failing, and the system prompt's category list.

**Tool name normalisation**

- **FR-018**: `READ_CORE` and `WRITE_CORE` MUST be renamed to `read_core` and `write_core`, so every built-in tool name is lowercase snake_case. This applies everywhere the names appear: the tool list, the dream cycle's tool set and its prompt, the WebUI's CORE page, and the tool-call labels in the WebUI, Discord and Telegram (which replace the upper-case labels).
- **FR-019**: `READ_CORE` / `WRITE_CORE` MUST NOT be supported after the rename: neither listed nor dispatched, the same as old MCP names (FR-015).

### Key Entities

- **Tool category**: a named group of tools with a short description. It is either built-in (curated) or comes from an MCP server (named after the server). Each agent sees only the categories that have tools available to it.
- **Tool**: anything the agent can call. It now also carries its categories, and for MCP tools the server it comes from, which no longer has to be read from the tool's name.
- **Agent tool settings**: no longer include Python sandbox or code-mode switches. Code mode is how every agent works.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For an agent that used direct tools before the upgrade, at least 80% less tool-definition text is sent to the model per turn after it.
- **SC-002**: A code-mode agent can go from "I need tool X" to having X's full parameter description in at most 2 lookups: list one category, or every function when unsure which category fits, then describe one function.
- **SC-003**: A category-filtered listing for an agent with 50+ tools returns under a quarter of the text of the full flat listing.
- **SC-004**: 100% of MCP-provided tools appear under their server's category and can be called by their `<server>_<tool>` name, or by their suffixed name when they collide.
- **SC-005**: Every conversation recorded before this change still loads and displays. Calls under old names show with the generic display.

## Assumptions

- "Programmatic tool calling" means the existing code mode (spec 007). No new script runtime is introduced.
- A tool may be in more than one category. In practice, built-in tools have one curated category each and MCP tools have exactly one (their server).
- Operators cannot edit categories in this iteration. Built-in categories are fixed by the product, and MCP categories come from the server name.
- Agents whose model cannot write working Python have no direct-tool fallback. This is accepted; reintroducing an opt-out is a separate decision if it proves necessary.
- Listing without a category returns the full flat list, as today, now with each function's category. Category browsing narrows it; it does not replace it. The category overview itself lives in the system prompt, so no listing call is needed to see what categories exist.
- Renaming MCP tools from `mcp_<server>__<tool>` to `<server>_<tool>`, and `READ_CORE`/`WRITE_CORE` to lowercase, is a breaking change to the tool names the model sees, which the project's changelog convention will flag. It is accepted as part of this feature. Old names are dropped outright, with no alias period. Agents whose CORE documents or history mention old names (e.g. `WRITE_CORE`) may call them once, get an unknown-function error, and must look up the current name. This is accepted.
- Calling one tool function at a time from scripts stays the same. Only discovery and naming change.
