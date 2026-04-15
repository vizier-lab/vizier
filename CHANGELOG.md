# Changelog

All notable changes to this project will be documented in this file.

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
