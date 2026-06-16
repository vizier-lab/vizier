# Changelog

All notable changes to this project will be documented in this file.

## [0.10.1] - 2026-06-16

### 🚀 Features

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
## [0.10.0-rc.3] - 2026-06-10

### 🚀 Features

- Implement stt models
- Implement audio message
- Implement auto-tts mode
- Adjust prompt and core document

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
- Implement mistralrs provider

### 🐛 Bug Fixes

- Various channel fixes

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.2
## [0.9.1] - 2026-06-06

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.1
## [0.9.0] - 2026-06-06

### 🚀 Features

- Add title
- Move mcp and shell config to per-agent basis
- Implement reaction feedback in webui
- Emoji filter
- Add general homepage
- Add quickchat home
- Change session_detail creation and titling
- Overhaul dream behaviour (#15)
- Add agent checkhealth features

### 🐛 Bug Fixes

- Missing chat from quick chat
- Missing thinking state on webui
- Thinking too persisted when should not
- Missing thinking state on first message
- Intermittent quick chat not sent

### ⚙️ Miscellaneous Tasks

- Bump version to 0.9.0
## [0.9.0-rc.1] - 2026-06-04

### 🚀 Features

- Implement graph memory view
- Add message when running server in background
- Rewrite and improve agent transport layer
- Adjust memory graph visual
- Implement slide over layout for content details
- Minor webui bugs

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

- Add missing migrations

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.2
## [0.8.1] - 2026-06-02

### 🚀 Features

- Add configurable thread worker

### ⚙️ Miscellaneous Tasks

- Bump version to 0.8.1
## [0.8.0] - 2026-06-02

### 🚀 Features

- Implement llama cpp support (#14)
- Add skill-maker skill
- Fix wrong skill category
- Implement user and RBAC
- Allow agent to create skills with resouces
- Add agent sharing functionality
- Add user profile settings
- Add agent memory visibility

### 🐛 Bug Fixes

- Run error when using surreal
- Https api not working

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
- Implement skill system

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

- Bump version to 0.5.4
## [0.5.3] - 2026-04-25

### 🐛 Bug Fixes

- Enable overflow on recv

### 💼 Other

- Fix docker build
- Fix docker build
- Fix docker build
- Change docker image base to ubuntu

### ⚙️ Miscellaneous Tasks

- Missing amd64 docker version
- Bump version to 0.5.3
## [0.5.2] - 2026-04-24

### 🚀 Features

- Adjust default system prompt

### 🐛 Bug Fixes

- Enable transport overflow

### 💼 Other

- Fix docker publish
- Fix docker build

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
- Implement basic png attachment

### 🐛 Bug Fixes

- Duplicate running schedule

### 📚 Documentation

- Update documentations

### ⚙️ Miscellaneous Tasks

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
- Remove unused data from fs storage
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

- Dropdown on analytics

### 🚜 Refactor

- Adjust history schema and fix dedup on webui
- Adjust analytics layout
- Adjust prompt function

### 📚 Documentation

- Update book

### ⚙️ Miscellaneous Tasks

- Bump version to 0.4.0
## [0.3.3] - 2026-04-11

### 🐛 Bug Fixes

- Reindex error on windows
- Docker dist
- Agent generation cli command

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

- Title too long

### ⚙️ Miscellaneous Tasks

- Update lock
- Bump version to 0.3.1
## [0.3.0] - 2026-04-10

### 🐛 Bug Fixes

- Remove unused docker files
- Various release hotfixes

### ⚙️ Miscellaneous Tasks

- Add docker release
- Bump version to 0.3.0
## [0.3.0-rc.7] - 2026-04-10

### 🚀 Features

- Add copy button to chat balloon
- Adjust thinking and tool calls debug hook

### ⚙️ Miscellaneous Tasks

- Update lock
- Remove unused route
- Bump version to 0.3.0-rc.7
## [0.3.0-rc.6] - 2026-04-09

### 🐛 Bug Fixes

- Surreal storage
- Ensure Windows compatibility by using PathBuf for all filesystem paths
- Include .exe files in GitHub release artifacts

### ⚙️ Miscellaneous Tasks

- Add rust caching
- Add rust caching
- Bump version to 0.3.0-rc.6
## [0.3.0-rc.5] - 2026-04-09

### ⚙️ Miscellaneous Tasks

- Windows installer script
- Bump version to 0.3.0-rc.5
## [0.3.0-rc.4] - 2026-04-09

### ⚙️ Miscellaneous Tasks

- Window installer build
- Bump version to 0.3.0-rc.4
## [0.3.0-rc.3] - 2026-04-09

### 🚀 Features

- Tool adjustment

### ⚙️ Miscellaneous Tasks

- Fix windows-installer build error
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
- Implement various dm tools

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
