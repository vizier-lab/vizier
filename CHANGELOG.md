# Changelog

All notable changes to this project will be documented in this file.

## [0.13.4] - 2026-10-28

### 🚀 Features

- *(cli)* Drop the provider and storage questions from onboard
- Replace sherpa-onnx with whisper-rs and any-tts (#107)

### ⚙️ Miscellaneous Tasks

- Bump design and webui
- Bump design and webui
- Bump webui for notification sound and toast position
- Bump webui for chat details panel under the header
- Bump webui for toast position
## [0.13.3] - 2026-10-27

### 🚀 Features

- Add the landing website submodule and release deploy
- Run dummyplug through standalone HTTP server
- *(storage)* Encrypt secrets at rest with a workspace key

### 🐛 Bug Fixes

- *(ci)* Push the release bump with a bypass-capable token

### 📚 Documentation

- Publish the design Storybook at www.vizier.rs/design
- Specify standalone dummyplug test server
- Keep dummyplug stateless and move test controls to harness
- Specify Node.js for standalone dummyplug server
- Align dummyplug tasks with Node.js plan
- Add local dummyplug testing guide
- Record the encrypted-storage migration in the #82 inventory
- Enumerate the credential surfaces covered by encrypted storage
- Restrict encryption to secret fields and keep non-secret settings readable

### 🧪 Testing

- Fix dummyplug retry scenario accounting

### ⚙️ Miscellaneous Tasks

- Add impeccable to development tools
- Redeploy the website when the design pin changes
- Sync Cargo.lock with 0.13.2
- Bump landing for the refreshed site and wordmark
- Pin landing to its master branch
- Pin dummyplug branch with master merge
- Pin dummyplug without workflow
- Bump dummyplug testing guide
- Pin full Node dummyplug server
- Bump version to 0.13.3
## [0.13.2] - 2026-10-25

### 🚀 Features

- Link submodule PRs and merge them bottom-up with /cascade-merge
- Per-agent secrets with script-only retrieval, redaction and a WebUI page (#96)
- OpenRouter as a media provider (STT, TTS, images) with model discovery (#99)
- *(secrets)* Page and search the secrets table
- Persist a notification inbox and retire the overview feed
- Notify a person when their WebUI chat turn replies
- Announce new notifications with a toast and simplify the row

### 🐛 Bug Fixes

- *(ci)* Name the repository when resolving the cascade's PR (#97)
- Agent tools form, collapsed sidebar and context ring (design + webui bump) (#98)
- Decode every concatenated MP3 stream in a spoken reply
- Keep a single notification bell on the phone layout

### 🚜 Refactor

- Move the notification bell into the page header

### 📚 Documentation

- Record the header bell in the 029 spec, plan and UI contract
- The bell is a header control; the phone menu opens from the left
- Eye read state, on-screen auto-read, collapsed details and outside-click close
- Unread notifications carry an emerald card outline
- Toasts stack top-right under the header

### ⚙️ Miscellaneous Tasks

- Sync design and webui to their master branches, bump lockfile to 0.13.1
- Bump design and webui for the usage chart
- Bump webui for recalled memory ordering
- Bump version to 0.13.2
## [0.13.1] - 2026-10-24

### 🚀 Features

- Add show_discord_chats/show_telegram_chats permissions, fix dream provider display

### ⚙️ Miscellaneous Tasks

- Bump version to 0.13.1
## [0.13.0] - 2026-10-24

### 🚀 Features

- Record turn steps as they happen and push them to watchers
- Journal durable requests and track each turn's row
- Resume turns that were running when vizier went down
- Resume turns across agent updates and mark resumed replies
- Abort drops journaled turns; test handover re-send order
- Recover background jobs backed by pending turns
- Recover task runs backed by pending turns
- Durable-turns e2e script and docs
- Add opt-in paging model for list endpoints
- Foundation for the WebUI rewrite (shell, routes, paging)
- Recall history entries, session file frames and listing, Range downloads, agent restart
- Chat for the WebUI rewrite (US1) and pins
- Paging on agents, tasks, skills and bundles; CORE and skill stale-save checks; dream nights and cycles with change attribution
- US3 day-to-day pages, task is_active, and pins
- US4 MCP listing paging; pin design and webui
- US5 public status, paged users/roles/api keys, key prefix; pin webui
- US6 overview, feed and conversations APIs, visit tracking, seed script; pin design and webui
- US7 pins
- Agent health reports live while a turn runs; pin design and webui
- Motion across the WebUI; pin design and webui
- Split paralel_subtasks into parallel_subagents and background_subagents [**breaking**]
- Title a session from its first request and push the title live
- Ends_turn flag makes a successful tool call the turn's final answer
- [**breaking**] Generative UI replaces render_interactive_ui

### 🐛 Bug Fixes

- Pin webui for the calmer screen changes
- Dialogs, modals and slide-overs had no size; pin design and webui
- Deploy the book from master, the default branch
- Keep a person's attachments on the saved request so they show after reload
- Bump design, sigil module name collided with Sigil.tsx on macOS

### 📚 Documentation

- Adopt new tagline "A digital steward for Your Majesty"
- Make storybook the design source of truth, drop figcraft integration
- Mark the Picker task done
- Record the quickstart checks for the design library
- Spec 024 covers home activity dots and dream timeline
- Spec 024 covers chat turn internals (recalled memory, session files, context ring)

### ⚙️ Miscellaneous Tasks

- Add design library submodule
- Bump design library
- Bump design library
- Move the old WebUI to legacy/ and add the webui submodule
- Build, CI and docs use the webui and design submodules; legacy/ is reference only
- Pin design
- Pin design and webui for the rewrite; role lookup, docs for the new endpoints
- Tasks: T059 walked
- Tasks: T137 done (#82 updated)
- Pin design and webui
- Pin design and webui
- Remove legacy webui
- Pin design and webui
- Bump design and webui for the live trail head
- Bump design and webui for generative UI
- Bump webui for logic runner and surfaces
- Bump webui for links and media
- Bump webui
- Bump webui
- Bump webui
- Bump webui
- Bump version to 0.13.0
- Bump version to 0.13.0
## [0.13.0-rc.5] - 2026-10-18

### 🚀 Features

- Deferrable client tools and page controls for interactive pages
- Themed style kit and built-in interactive-ui skill for interactive pages
- Nudge agents to check skills before multi-step work
- Editorial register and typography recipe for the interactive-ui skill
- Scratchpad tools for temporary per-conversation notes
- Show agent build and model download progress in the WebUI
- Tools to get a session file's path and load a path as a session file
- Highlight an invoked skill in the WebUI chat

### 🐛 Bug Fixes

- Point agents at the scratchpad for pages built in chunks
- Let update_skill and delete_skill reach the agent's own skills

### 📚 Documentation

- Place interactive pages in the reply bubble as soon as they render
- Let interactive pages save their own state
- Plan client tools (019)
- Break client tools (019) into tasks
- Spec for scratchpad tools (020)
- Scratchpad spec — durable storage, dream access
- Plan for scratchpad tools (020)
- Tasks for scratchpad tools (020)

### ⚙️ Miscellaneous Tasks

- Bump version to 0.13.0-rc.4
- Bump version to 0.13.0-rc.5
## [0.13.0-rc.3] - 2026-10-16

### 🚀 Features

- Run Jaeger alongside vizier in the sample compose file and pass VIZIER_JWT_SECRET through [**breaking**]

### 🐛 Bug Fixes

- Tell agents their reply is delivered automatically, not through the send_message tools

### ⚙️ Miscellaneous Tasks

- Bump version to 0.13.0-rc.3
## [0.13.0-rc.2] - 2026-10-16

### 🚀 Features

- Install.sh accepts --version and --install-dir flags
- Export OpenTelemetry traces and metrics following the GenAI conventions
- [**breaking**] Count every model call and rebuild the usage page on cost, activity and model [**breaking**]
- Show Discord and Telegram conversations by server, channel and topic name on the usage page
- Teach agents to split large tasks and keep memories to one topic

### 🐛 Bug Fixes

- One scrollbar, truncated long names and topic titles on the usage page
- Tell the aborted turn's own channel when /abort comes from Discord or Telegram

### 📚 Documentation

- Document OpenTelemetry export, spans, attributes and the Jaeger setup

### ⚙️ Miscellaneous Tasks

- Bump version to 0.13.0-rc.2
## [0.13.0-rc.1] - 2026-10-16

### 🚀 Features

- [**breaking**] Code mode by default, tool categories, <server>_<tool> MCP names [**breaking**]
- Tell agents the Vizier version they run on
- Add the src/llm provider layer and catalogue data model
- Add media and embedding clients on the llm transport, pinned by rig fixtures
- Add the catalogue handle, built-in providers, seed loading and model resolution
- [**breaking**] Replace rig with the in-tree provider layer [**breaking**]
- Agents use catalogue limits and handle images by capability
- Catalogue read API, logos, SQLite cache and lazy conditional refresh
- Custom providers API, full providers API, and estimated cost in usage
- Catalogue-driven provider and model pickers, providers settings, cost in usage
- 016 bots belong to their agent; Channels page as one row per platform [**breaking**]
- 017 agents manage their own integrations; integrations are per-agent [**breaking**]

### 🐛 Bug Fixes

- [**breaking**] Name sandbox catalogue entries by "name", not "function"
- Dream journal listing without a limit, stale dream tool names
- Pin fastembed 4.9.1 and sherpa-onnx 1.13.3 so release builds pass
- Keep the WebUI chat consistent across page changes
- Discord treats a typed /abort as the abort command

### 💼 Other

- Begin the 015 cutover from rig to src/llm (does not compile yet)

### 🚜 Refactor

- One agent form for create and edit
- Drop subtitles from integrations page headers
- Drop the redundant bot name from Channels rows

### 📚 Documentation

- Spec 014 code mode by default, tool categories, simpler MCP tool names
- Plan 014 code mode by default, tool categories, simpler MCP tool names
- Tasks 014 code mode by default, tool categories, simpler MCP tool names
- Spec 015 catalogue-driven providers, custom providers, simpler agent form
- Plan 015 removes rig in one cutover after building the provider layer
- Plan 015 simplifies the provider layer instead of porting rig's API
- Spec/plan 015 lazy catalogue refresh cached in SQLite, seeded at build time
- Plan 015 fetches the catalogue seed once per release
- Record 015 implementation deviations and golden differences
- Catalogue-driven providers, custom providers and catalogue settings
- 015 end-to-end results and PR description
- Record the live provider check
- 016 retire agents:mcp_config in favour of integration permissions
- 016 scope Integrations to the selected agent; add Misc › Config
- 016 rename Services to Addons, app-store catalogue, Usage under Misc
- 016 keep MCP tool names, namespace addon tools (brave_search_*)
- 016 drop the upgrade migration

### 🧪 Testing

- Record golden fixtures from rig 0.39 and check every dialect against them
- Run the 015 upgrade and retry quickstart checks end to end
- Live-provider smoke tests per dialect; clippy fixes in the providers API

### ⚙️ Miscellaneous Tasks

- Cargo update to latest semver-compatible versions
- Upgrade dirs, strum, nanoid, base64, sha2, argon2, jsonwebtoken, tower-http, rusqlite, croner, indicatif, termimad, bzip2, zip, similar, calamine, pdf-extract, symphonia, bollard, tungstenite, text-splitter
- Upgrade utoipa 6, teloxide 0.17, fastembed 7
- Upgrade rig-core 0.39, rmcp 3
- Upgrade monty to 1.0.0
- Enable speckit auto-commit hooks
- Bump version to 0.12.1
- Add model catalogue seed
- Embed the catalogue seed at build time and add the stub LLM fixture
- Bump version to 0.13.0-rc.1
## [0.12.0] - 2026-10-14

### 🐛 Bug Fixes

- Seal unanswered tool calls on failed turns
- Queue handovers instead of dropping pending messages
- Release the session when a turn panics
- Derive session detail from storage instead of an in-memory count
- Keep an agent working when one of its MCP servers fails
- Retry transient provider failures and enforce prompt_timeout inside a completion
- Supervise agents so they restart, and build before swapping on update
- Return agent errors from the chat endpoint instead of waiting them out
- Hand a timed-out tool call back to the model instead of ending the turn
- Let a turn degrade when an optional step fails
- Checkpoint only once a reply's tool calls have their results
- Supervise channel readers and subsystems, stop turns with their agent
- Answer with what the turn has when thinking_depth is reached
- Re-download sherpa-onnx libs on cached release builds, allow retrying a release
- Clear cached sherpa-onnx prebuilt dir before release builds

### ⚙️ Miscellaneous Tasks

- Remove twilight client construction test
- Bump version to 0.12.0
- Bump version to 0.12.0
- Bump version to 0.12.0
## [0.12.0-rc.4] - 2026-10-09

### 🚀 Features

- [**breaking**] background subagent jobs with report-back and cancel
- Reaction awareness across webui, discord and telegram

### 🐛 Bug Fixes

- Post background-woken replies to discord and telegram

### 📚 Documentation

- Add spec and plan for background subagent results
- Add task list for background subagent results
- Add spec for reaction awareness
- Add plan for reaction awareness
- Add task list for reaction awareness

### ⚙️ Miscellaneous Tasks

- Bump version to 0.12.0-rc.4
## [0.12.0-rc.3] - 2026-10-05

### 🐛 Bug Fixes

- Keep discord typing alive for the whole turn and render mentions

### ⚙️ Miscellaneous Tasks

- Bump version to 0.12.0-rc.3
## [0.12.0-rc.2] - 2026-10-05

### 🚀 Features

- Version history for CORE.md and memories
- Add dummyplug offline test provider
- Dummyplug tool listing, sample requests, and tool calls
- Python sandbox and code mode (programmatic tool calling)
- Passage-level memory retrieval [**breaking**]
- [**breaking**] Keep reasoning and tool activity attached to the turn that produced it
- [**breaking**] Give each task firing its own conversation
- [**breaking**] Rename the Task request content variant to Unattended
- [**breaking**] Replace a task's free-text user with a requester
- Record and surface task run results

### 🐛 Bug Fixes

- Use rustls for twilight to unblock x86_64 macOS cross-build

### 🚜 Refactor

- Share one frontmatter parser across utils and memory storage

### 📚 Documentation

- *(spec)* Add memory version history spec, plan, and tasks
- *(spec)* Add dummyplug test provider spec, plan, and tasks
- Document the dummyplug test provider
- Amend constitution to v1.2.0 (dummyplug end-to-end verification gate)
- *(spec)* Add python sandbox & code mode (programmatic tool calling) spec
- *(spec)* Add plan, research, contracts and tasks for python sandbox & code mode
- Align 007 spec artifacts with constitution v1.2.0 and analysis fixes
- *(spec)* Add spec for semantic chunking of memories
- *(spec)* Add plan, research, contracts and tasks for memory chunking
- *(spec)* Add spec for webui reasoning and tool activity display
- *(spec)* Add plan, research, contracts and tasks for webui reasoning display
- *(spec)* Add spec for task results, requester and run reports
- *(spec)* Fold unattended-run framing into the task results spec
- *(spec)* Add plan, research, contracts and quickstart for task run results
- *(spec)* Add tasks for task run results, requester and framing
- *(spec)* Mark the task-run-results tasks complete

### ⚡ Performance

- Make agent prompts cacheable across requests
- Shrink what an execute_python report costs the model

### ⚙️ Miscellaneous Tasks

- Drop per-write and per-document passage logging
- Bump version to 0.12.0-rc.1
- Bump version to 0.12.0-rc.2
## [0.11.0] - 2026-09-13

### 🚀 Features

- Simplify agent core documents
- Show all memory by default
- Add opencode provider
- Open knowledge format base memory (#17)
- Add discord guild, channel, and member info tools

### 🐛 Bug Fixes

- *(ci)* Bump vizier-derive dependency requirement on release

### 🚜 Refactor

- Replace skill activation gating with uniform tool access + recommendation

### 📚 Documentation

- Add CLAUDE.md and symlink AGENTS.md to it, refresh README
- Track Spec Kit tooling and ratify project constitution v1.0.0
- Rewrite documentation to match current code

### ⚙️ Miscellaneous Tasks

- Bump version to 0.11.0-rc.1
- Bump version to 0.11.0
## [0.10.11] - 2026-07-02

### 🚀 Features

- Optimize memory explorer

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.11
## [0.10.10] - 2026-07-02

### 🐛 Bug Fixes

- Token eating dreaming checkpointing

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.10
## [0.10.9] - 2026-06-27

### 🚀 Features

- Add openai compatible custom provider
- Implement abort on checkpoint
- Auto-checkpoint on dreaming
- Add checkpoint feedback

### 💼 Other

- Tweak read count weight

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.9
## [0.10.8] - 2026-06-20

### 🚀 Features

- Strict memory deletion
- Implement poplarity-based memory score factor

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.8
## [0.10.7] - 2026-06-20

### 🚀 Features

- Granular thinking toggle
- Reimplement kokoro tts
- Implement whisper transcriber

### 🐛 Bug Fixes

- Kokoro model intermittent download issue

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.7
## [0.10.6] - 2026-06-19

### 🚀 Features

- Attach as default run mode
- Append to history when using send message tool
- Add memory reranking

### 🐛 Bug Fixes

- Remove unstable providers
- Duplicate discord handler
- Remove duplicate title

### 💼 Other

- Single image docker registy
- Fix docker build
- Add dockerhub image
- Integrate docker publish to main workflow
- Bundle essential runtime for docker image

### 🚜 Refactor

- Change agent channel ownership

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.5
- Bump version to 0.10.6
## [0.10.4] - 2026-06-16

### 🐛 Bug Fixes

- Running in docker failed

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.4
## [0.10.3] - 2026-06-16

### 🚀 Features

- Implement config-less run
- Add sample docker-compose

### 💼 Other

- Set default to sqlite

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.3
## [0.10.2] - 2026-06-16

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.2
## [0.10.1] - 2026-06-16

### 🚀 Features

- Adjust prompt and core document
- Implement rest api chat
- Implement vision model tool
- Add image gen tools
- Implement various providers
- Memory with attachments
- Add context window stats
- Persistant error context
- Add pdf export
- Implement checkpoint system
- Add lobotomy and checkpoint command
- Implement kokoro tts
- Add additional tool call formating
- Replace surreal in favor of sqlite

### 🐛 Bug Fixes

- Image in context halucination
- Agent not aware of memory attachemnts
- Add additional headers to webfetch
- Queue indicator
- Align checkbox
- Checkpoint not shown
- Agent spawning blocking
- Thought rendering in discord/telegram
- Silent read and sqlite missing history

### 💼 Other

- Adjust defaults
- Add vizier-derive publish step

### 🚜 Refactor

- Separate image read
- Uniformised api key resolver
- Move embedding and indexing to per agent basis
- Rename embedding config
- Add MarkdownDoc helper
- Save tool calls to history
- Save history as plain json
- Exclude assistant message
- Uniformize logging
- Remove ptc

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.0
- Bump version to 0.10.1
## [0.10.0-rc.3] - 2026-06-10

### 🚀 Features

- Implement stt models
- Implement audio message
- Implement auto-tts mode

### 🚜 Refactor

- Rename context files to session files
- Adjust prompt
- Change audio models target directory

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.0-rc.3
## [0.10.0-rc.2] - 2026-06-08

### 🐛 Bug Fixes

- Cant create a new agent without tts

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.0-rc.2
## [0.10.0-rc.1] - 2026-06-07

### 🚀 Features

- Implement mistralrs provider
- Implement context file system
- Integrate context file
- Implement send_attachment tools
- Handle send_attachment in discord/telegram
- Implement tts tools
- Additional file preview
- Implement piper tts provider
- Implement kitten model

### 🐛 Bug Fixes

- Pdf and image not read properly

### 🚜 Refactor

- Optimize system prompt

### ⚙️ Miscellaneous Tasks

- Bump version to 0.10.0-rc.1
## [0.9.2] - 2026-06-07

### 🚀 Features

- Uploaded file load too long in webui

### 🐛 Bug Fixes

- Various channel fixes

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.2
## [0.9.1] - 2026-06-06

### 🚀 Features

- Add agent checkhealth features

### 🐛 Bug Fixes

- Missing thinking state on first message
- Intermittent quick chat not sent

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.1
## [0.9.0] - 2026-06-06

### 🚀 Features

- Adjust memory graph visual
- Implement slide over layout for content details
- Minor webui bugs
- Add title
- Move mcp and shell config to per-agent basis
- Implement reaction feedback in webui
- Emoji filter
- Add general homepage
- Add quickchat home
- Change session_detail creation and titling
- Overhaul dream behaviour (#15)

### 🐛 Bug Fixes

- Missing chat from quick chat
- Missing thinking state on webui
- Thinking too persisted when should not

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.0
## [0.9.0-rc.1] - 2026-06-04

### 🚀 Features

- Add missing migrations
- Implement graph memory view
- Add message when running server in background
- Rewrite and improve agent transport layer

### 🐛 Bug Fixes

- Broken links

### 💼 Other

- Remove shared_document features in favor of global memory

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.0-rc.1
## [0.8.3] - 2026-06-03

### 🐛 Bug Fixes

- Optimize blocking in websocket

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.3
## [0.8.2] - 2026-06-02

### 🚀 Features

- Add configurable thread worker

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.2
## [0.8.1] - 2026-06-02

### 🐛 Bug Fixes

- Run error when using surreal
- Https api not working

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.1
## [0.8.0] - 2026-06-02

### 🚀 Features

- Implement skill system
- Implement llama cpp support (#14)
- Add skill-maker skill
- Fix wrong skill category
- Implement user and RBAC
- Allow agent to create skills with resouces
- Add agent sharing functionality
- Add user profile settings
- Add agent memory visibility

### 📚 Documentation

- Update docs with skills

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.0
## [0.7.0] - 2026-05-31

### 🚀 Features

- Add daemonize run mode (#11)
- Implement new unified sidebar layout (#12)
- Implement runtime agent configuration
- Move some config to runtime
- Move mcp and shell config to runtime
- Bump rig and add xiaomi provider
- Add missing fields and standardized icon
- Enhance input style and UX
- Add glass effect to input
- Implement wyiwyg input in chat
- Standardize markdown editor
- Rearrange tools
- Refine sidebar behaviour
- Enhance upload process and add custom avatar upload
- Add placholder and thinking word variations
- Update onboarding command
- Add abort and message queue logic
- Enhanced scroll to bottom behaviour
- Presistant selected agent and topic
- Auto reconcile channel update
- Adjust tooltip description

### 🐛 Bug Fixes

- Input now properly apply modifier
- Glassmorphism on input
- Cropped scroll fix

### 💼 Other

- Minor design enhancement

### 📚 Documentation

- Add AGENTS.md
- Update docs

### ⚙️ Miscellaneous Tasks

- Bump version to 0.7.0
## [0.5.5] - 2026-05-02

### 🐛 Bug Fixes

- Crash when model halucinating mcp function_name

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.5
## [0.5.4] - 2026-04-28

### 🚀 Features

- Add additional context around time
- Allow tools to have attachment

### 💼 Other

- Optimize tools description

### 🚜 Refactor

- Remove unused import

### ⚙️ Miscellaneous Tasks

- Missing amd64 docker version
- Bump version to 0.5.4
## [0.5.3] - 2026-04-25

### 🐛 Bug Fixes

- Enable overflow on recv

### 💼 Other

- Fix docker build
- Fix docker build
- Fix docker build
- Fix docker build
- Change docker image base to ubuntu

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.3
## [0.5.2] - 2026-04-24

### 🚀 Features

- Adjust default system prompt

### 🐛 Bug Fixes

- Enable transport overflow

### 💼 Other

- Fix docker publish

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.2
## [0.5.1] - 2026-04-23

### 🚀 Features

- Implement memory auto-retrieval
- Emphasize slug on auto-retrieval
- Reenable ptc

### 🐛 Bug Fixes

- Enhance error messages and improve PATH setup in install script (#10)

### ⚙️ Miscellaneous Tasks

- Update workflow
- Bump version to 0.5.1
## [0.5.0] - 2026-04-21

### 🚀 Features

- Implement basic png attachment
- Saved attachments to storage
- Implement attachment for telegram channel
- Implement upload/download api
- Implement attachment on webui

### 🐛 Bug Fixes

- Llm unable to parse local url
- Docker deployment

### 🚜 Refactor

- Schema structure
- Standardise tool call response

### ⚙️ Miscellaneous Tasks

- Bump version to 0.5.0
## [0.5.0-rc.3] - 2026-04-17

### 🚀 Features

- Add docker image publishing workflow
- Implement additional tool for tasks and scheduling
- Add http client tool
- Add openapi documentation

### 🐛 Bug Fixes

- Duplicate running schedule

### 📚 Documentation

- Update documentations

### ⚙️ Miscellaneous Tasks

- Remove unused data from fs storage
- Drop windows support (for now) :(
- Bump version to 0.5.0-rc.3
## [0.5.0-rc.2] - 2026-04-16

### 🚀 Features

- Included document is injected directly to context
- Implement additional memory tools
- Detailing in think indicator

### 🐛 Bug Fixes

- Onboarding for discord

### ⚙️ Miscellaneous Tasks

- Install script to directo stable only
- Add update install script
- Bump version to 0.5.0-rc.2
## [0.5.0-rc.1] - 2026-04-15

### 🚀 Features

- Optimize prompt
- Better think indicator
- Adjust tool indicator
- Make python programmatic sandbox tool, need no runtime depedencies (#5)
- Add think tool
- Add agent shared_document
- Adjust runtime path
- Implement simple dreaming
- Adjust dreaming

### 🐛 Bug Fixes

- Multiline textarea not reset
- Usage layout

### 🚜 Refactor

- Optimize chat input

### 📚 Documentation

- Update book

### ⚙️ Miscellaneous Tasks

- Adjust install script
- Bump version to 0.5.0-rc.1
## [0.4.0] - 2026-04-12

### 🚀 Features

- Add usage stats detail in message bubble
- Add usage analytics page
- Adjust token user by channel type
- Adjust chat layout
- Implement subagent tool

### 🐛 Bug Fixes

- Agent generation cli command
- Dropdown on analytics

### 🚜 Refactor

- Adjust history schema and fix dedup on webui
- Adjust analytics layout
- Adjust prompt function

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.0
## [0.3.3] - 2026-04-11

### 🐛 Bug Fixes

- Title too long
- Reindex error on windows
- Docker dist

### 💼 Other

- Remove subagent capabilities

### ⚙️ Miscellaneous Tasks

- Fix docker image build
- Bump version to 0.3.2
- Temporarily remove docker publish
- Bump version to 0.3.2
- Bump version to 0.3.3
## [0.3.1] - 2026-04-10

### 🐛 Bug Fixes

- Remove unused docker files
- Various release hotfixes

### ⚙️ Miscellaneous Tasks

- Add docker release
- Update lock
- Bump version to 0.3.1
## [0.3.0] - 2026-04-10

### 🚀 Features

- Add copy button to chat balloon
- Adjust thinking and tool calls debug hook

### ⚙️ Miscellaneous Tasks

- Remove unused route
- Bump version to 0.3.0
## [0.3.0-rc.7] - 2026-04-10

### 🐛 Bug Fixes

- Surreal storage
- Ensure Windows compatibility by using PathBuf for all filesystem paths
- Include .exe files in GitHub release artifacts

### ⚙️ Miscellaneous Tasks

- Add rust caching
- Add rust caching
- Update lock
- Bump version to 0.3.0-rc.7
## [0.3.0-rc.6] - 2026-04-09

### ⚙️ Miscellaneous Tasks

- Windows installer script
- Bump version to 0.3.0-rc.6
## [0.3.0-rc.5] - 2026-04-09

### ⚙️ Miscellaneous Tasks

- Window installer build
- Bump version to 0.3.0-rc.5
## [0.3.0-rc.4] - 2026-04-09

### ⚙️ Miscellaneous Tasks

- Fix windows-installer build error
- Bump version to 0.3.0-rc.4
## [0.3.0-rc.3] - 2026-04-09

### 🚀 Features

- Implement various dm tools
- Tool adjustment

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.0-rc.3
## [0.3.0-rc.2] - 2026-04-09

### 🚀 Features

- Adjust how thinking progress is implemented
- Adjust webui to the new thinking flow
- Adjust default and system prompt
- Adjust onboarding and add command to generate new agent
- Tweak styling
- Add windows  support
- Adjust light theme coloscheme
- Adjust codeblock
- Syntax hightlighting
- Adjust chat text input
- Adjust layout
- Tweak paddings and layouts
- Add utility to modify primary documetns of agents
- Replace chat header with topic title
- Add delete session
- Add logo
- Implement date picker for task
- Add telegram channel

### 🐛 Bug Fixes

- Windows build

### ⚙️ Miscellaneous Tasks

- Delete default vizier agent
- Rename template file
- Bump version to 0.3.0-rc.2
- Fix windows build
- Bump version to 0.3.0-rc.2
- Remove arm 64 windows release
- Bump version to 0.3.0-rc.2
## [0.3.0-rc.1] - 2026-04-04

### 🚀 Features

- Implement inter-agent communication
- Add basic structure to skill system
- Change scheduler interval to 1m
- Implement topic creation
- Add local mcp server integration
- Implement http mcp server
- Implement topic switching on discord
- Optimize default AGENT.md
- Implement heartbeat loop
- Make env on mcp not mandatory
- Add dockerfile mode to docker shell mode
- Implement memory and task CRUD API
- Implement new channel api
- Implement auth layer to http channel
- Implement new webui
- Tweak some design
- Implement subagent tools
- Adjust subagent description
- Unrendered table

### 🐛 Bug Fixes

- Missing skill table
- Missing openrouter implementation

### 🚜 Refactor

- Restructure agent code
- Restructure request struct
- Change fs file directory
- Improve agent process handling

### 📚 Documentation

- Update install instruction
- Fix subagent configuration details and update intro (#3)

### ⚙️ Miscellaneous Tasks

- Add remote install script
- Update repository ownership
- Remove unused file
- Remove old api
- Bump version to 0.3.0-rc.1
## [0.2.3] - 2026-03-26

### 🐛 Bug Fixes

- Separate session for each task running

### 📚 Documentation

- Fix docs numbering and paging

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.3
## [0.2.2] - 2026-03-25

### 🐛 Bug Fixes

- Remove summary in favor of session window
- When init session using fs storage

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.2
## [0.2.1] - 2026-03-24

### 💼 Other

- Missing document_index table

### 📚 Documentation

- Fix ascii diagram

### ⚙️ Miscellaneous Tasks

- Remove useless ci
- Bump version to 0.2.1
## [0.2.0] - 2026-03-24

### 🚀 Features

- Implement modular storage and filesystem based storage
- Change id of history based on provider
- Implement multiple embedding providers
- Add env interpolation support
- Add anthropic, openai, and gemini provider
- Add timeout logics
- Implement chat intteruption
- Implement cursor pagination
- Implement sliding window for session history
- Implement image based docker sandboxing
- Reimplement turn depth as thinking depth
- Recall old history on session start
- Implement embedded document

### 🐛 Bug Fixes

- Missing context on interruption
- Timeout now canceling tool call

### 💼 Other

- Update loading agent config behaviour
- Rename IDENT.md to IDENTITY.md
- Add package keyword

### 🚜 Refactor

- Make sessions processes shared its agent
- Make db/storage modular

### 📚 Documentation

- Update documentation

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0
## [0.2.0-rc.5] - 2026-03-20

### 🚀 Features

- Make Python an optional feature
- [**breaking**] Make Python an opt-in feature (not default)
- Transparent thinking feature
- Add simpler tui

### 🚜 Refactor

- Refine config and init process
- Refine config and init behaviour

### 📚 Documentation

- Book scaffold
- Add chapter 1 of documentation
- Add book pages workflow

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0-rc.5
## [0.2.0-rc.4] - 2026-03-17

### 🐛 Bug Fixes

- Python bundling

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0-rc.4
## [0.2.0-rc.3] - 2026-03-17

### 🐛 Bug Fixes

- Bin dir
- Macos build run error

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0-rc.3
## [0.2.0-rc.2] - 2026-03-17

### 🐛 Bug Fixes

- Webui missing asset error on SPA route
- Formatting
- Binstall release

### 🚜 Refactor

- Spawn agent by session
- Simplify transport with broadcast

### 📚 Documentation

- Update readme

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0-rc.2
## [0.2.0-rc.1] - 2026-03-16

### 🚀 Features

- Setup webui subproject
- Embed webui to vizier
- Implement stale session collector
- Implement rudementary chat interaction in webui
- Migrate db to surrealdb
- Allow user to add workspace from params when onboarding
- Use fastembed embedding model
- Add primary documents as system prompts
- Implement custom vector memory
- Implement multi agent orchestration
- Refine system prompts
- Add scheduler and task system
- Implement initiative on silent read
- Auto download model on ollama provider
- Fix locking issue
- Loading bar on downloading ollama model
- Implement python sandboxing
- Implement programmatic tool calling
- Configurable ptc
- Implement .env settings
- Add python_tools_docs for programmatic_tools discovery
- Implement new web ui base layout
- Reimplement webui chat
- Implement persistance history
- Map chat response
- Change package name

### 🐛 Bug Fixes

- Duplicate tool name
- Publishing error
- Adjust include list
- Include list

### 💼 Other

- Repeat answer
- Version
- Version

### 🚜 Refactor

- Code cleanup
- Development justfile
- Remove debug log
- Optimize multi-agent memory
- Change session to process-based
- Adjust discord_send_message description
- Optimize default AGENT.md
- Unify schemas into one module
- Simplify ptc
- Refactor system prompts
- Move embedder as deps

### 📚 Documentation

- Update readme

### ⚙️ Miscellaneous Tasks

- Modify run command in Readme.md
- Update GitHub Sponsors username in FUNDING.yml
- Remove unused markdown
- Removed unused md file
- Change package name
- Adjust package to publishing
- Add workflows
- Fix workflow
- Bump version to 0.2.0-rc.1
- Fix build error
- Fix build error
- Bump version to 0.2.0-rc.1
## [0.1.4] - 2026-02-22

### ⚙️ Miscellaneous Tasks

- Release v0.1.4
## [0.1.3] - 2026-02-22

### 🚀 Features

- Adjust memory summarising

### 🐛 Bug Fixes

- Compilation error
- Typo from 'etch' to 'etc' in Readme.md
- Can't connect to sqlite
- Default config path

### 💼 Other

- Version

### 📚 Documentation

- Add update instruction

### ⚙️ Miscellaneous Tasks

- Add release workflow
- Fix undetected github workflow
- Add workflow cache
- Fail to push release
- Release v0.1.3
## [0.1.1] - 2026-02-21

### 🚀 Features

- Add help message

### 💼 Other

- Add additional metadata
- Version

### 📚 Documentation

- Update readme
## [0.1.0] - 2026-02-21

### 🚀 Features

- Add basic discord functionality
- Add short term memory
- Refine discord ux (continious thinking, multipart long message)
- Implement brave search tools
- Implement basic document based memory
- Add rest api channel (mainly for debugging)
- Improve memory and add memory utilities
- Hardcode default worker threads
- Implement vector memory
- Adjust default templates
- Implement websocket channels
- Add cli exec tool
- Add onboarding command
- Adjust default log behaviour
- Add foundation for tui
- Rewrite vector memory with sqlite

### 💼 Other

- Readjust config structure
- Initial release
- Adjust crates name

### 🚜 Refactor

- Add middleware to transport

### 📚 Documentation

- Update readme
