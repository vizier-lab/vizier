# Changelog

## What's Changed in v0.2.0-rc.1

* chore: add workflows (43d6262)
* chore: adjust package to publishing (d683232)
* fix: include list (f43ae45)
* fix: adjust include list (7cfe7d4)
* fix: publishing error (fce35ad)
* bump: version (7685c28)
* bump: version (3d04bd7)
* chore: change package name (046588b)
* feat: change package name (ca586f0)
* feat: map chat response (020c3d0)
* refactor: move embedder as deps (1ad880f)
* feat: implement persistance history (edaf08c)
* feat: reimplement webui chat (e74e37d)
* feat: implement new web ui base layout (c99e2dc)
* refactor: refactor system prompts (b5e9700)
* feat: add python_tools_docs for programmatic_tools discovery (58cfff5)
* feat: implement .env settings (0a61737)
* feat: configurable ptc (9b06db7)
* refactor: simplify ptc (7d39ae0)
* docs: update readme (a8f56bb)
* feat: implement programmatic tool calling (36014a1)
* feat: implement python sandboxing (e9fd5f7)
* refactor: unify schemas into one module (5618faa)
* refactor: optimize default AGENT.md (d1838de)
* feat: loading bar on downloading ollama model (e522964)
* feat: fix locking issue (8239aec)
* feat: auto download model on ollama provider (27193c1)
* refactor: adjust discord_send_message description (05985bf)
* fix: duplicate tool name (d44da3c)
* refactor: change session to process-based (d4febeb)
* feat: implement initiative on silent read (d08b3bd)
* feat: add scheduler and task system (ed9b5fd)
* feat: refine system prompts (1299d43)
* refactor: optimize multi-agent memory (be99546)
* feat: implement multi agent orchestration (08d3538)
* refactor: remove debug log (f692902)
* feat: implement custom vector memory (e5f91da)
* feat: add primary documents as system prompts (81b6b5d)
* chore: removed unused md file (3adc6aa)
* chore: remove unused markdown (3a8e399)
* feat: use fastembed embedding model (8af9c11)
* refactor: development justfile (2adccab)
* feat: allow user to add workspace from params when onboarding (1e8535a)
* feat: migrate db to surrealdb (9c6eb2c)
* feat: implement rudementary chat interaction in webui (1ce723a)
* hotfix: repeat answer (e377cd1)
* feat: implement stale session collector (52080ed)
* refactor: code cleanup (5b03118)
* feat: embed webui to vizier (5e12658)
* Merge branch 'master' of github.com:blinfoldking/vizier (3bec0a9)
* feat: setup webui subproject (aefb912)
* chore: Update GitHub Sponsors username in FUNDING.yml (0bcc894)
* chore: Modify run command in Readme.md (fd71c35)

**Full Changelog**: https://github.com/blinfoldking/vizier/compare/v0.1.4...v0.2.0-rc.1

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
