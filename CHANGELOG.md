# Changelog

All notable changes to this project will be documented in this file.

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
