# Implementation Plan: Project Coding Workspace

**Branch**: `041-project-coding-workspace` | **Date**: 2026-10-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/041-project-coding-workspace/spec.md`

## Summary

Spec 040 gave an agent a folder and one per-turn tool, `workspace_shell`. This feature turns a
project chat into a coding workspace:

1. **Coding tools.** `read_file`, `list_files`, `grep`, `glob`, `write_file`, `edit_file`
   (exact text), `propose_plan`, read-only `git_status`/`git_diff`/`git_log`, and
   `workspace_shell`, all in a new `coding` tool category. They stay per-turn
   (`ToolTarget::Project`) and are served from one registry in `projects/tools/`, so dispatch,
   docs and the catalogue read the same table.
2. **Plan/Build mode** per chat, stored in a new `project_chat` table, read once when a turn
   starts into `ProjectTurnContext.mode`; the registry offers only `Read` tools in Plan, and
   `Write`/`Command` tools in Build (commands only when the project's shell switch is on).
   The mode is server state, so every client and every resumed or woken turn sees the same one.
   Subagent pieces launched from a project chat inherit its folder and the launching turn's mode
   through `VizierRequest.project_scope` (R11).
3. **A dedicated channel kind**, `VizierChannelId::Project(user, project_id)`, replacing the 040
   `HTTP(user, "project__<id>")` shape, with a one-time startup migration of every stored slug
   and serialised channel. The URL a client uses (`channel_id = project__<id>`) is unchanged.
4. **`@file` mentions**, resolved when the turn starts into a `## Mentioned files` section.
5. **Git interface** (status, per-file diff, branches, switch/create) over the `git` CLI with
   fixed argument lists, as HTTP routes under the project.
6. **Worktrees** at `{project}/.vizier/<agent-id>/<name>`, one row in `project_worktree` bound to
   exactly one chat topic; the chat's working folder (`cwd`) becomes the worktree path for every
   tool, mention and git call.

## Technical Context

**Language/Version**: Rust (workspace edition 2024); TypeScript, React 19 and React Router 7 for the WebUI; Tauri 2 for the desktop shell (unchanged).

**Primary Dependencies**: Existing: `tokio` (`process`, already enabled by 040), `regex`, `sha2`, `serde`/`schemars`/`utoipa`, `nanoid`, `rusqlite` (bundled), `axum`, `tracing`. **One new crate**: `ignore` (BurntSushi, the ripgrep walker) for gitignore-correct listing and searching; see research R5. `git` is an *optional runtime program*, detected per folder, never required.

**Storage**: Embedded SQLite: two additive tables (`project_chat`, `project_worktree`) and one data migration (`migrate_project_channels`, marker `migration.project_channel_v1`) rewriting project channel slugs in existing rows. Worktrees live on the host filesystem inside the project folder.

**Testing**: `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features`; `cd webui && npm run typecheck && npm test`; `cd design && npm test`; end-to-end through dummyplug agent steps in [quickstart.md](./quickstart.md) (the constitution's e2e gate). New unit tests: path resolution, edit semantics, stale-read ledger, gitignore filtering, git porcelain parsing, mode filtering, the channel migration on a 040-shaped database, mention resolution.

**Target Platform**: Project hosts are the desktop app on macOS, Windows and Linux (040). Browser users reach existing projects through web access. Git features need `git` on the host's `PATH`.

**Project Type**: Single Rust workspace + embedded WebUI + Tauri shell (existing layout).

**Performance Goals**: `@` suggestions under 0.5 s on 10,000 tracked files (SC-008); worktree creation to typing under 30 s on a Vizier-sized repo (SC-005, dominated by `git worktree add`); file tools return without blocking the runtime (`spawn_blocking` for walks and reads).

**Constraints**: Every file path is confined to the chat's `cwd` (FR-006/FR-028). Plan mode exposes no tool that changes the folder or runs a command (SC-003). Refused edits leave files byte-identical (SC-002). Git runs only with fixed argv, `GIT_TERMINAL_PROMPT=0`, a 15 s limit, and never `push`/`fetch`. Mode changes take effect next turn (FR-013). No new runtime service. UI is drawn in FigCraft before code (CLAUDE.md "Design first").

**Scale/Scope**: Dozens of chats and worktrees per project; repositories up to ~10k tracked files and files up to a few MiB; several chats of one project running at once.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Lean by default | One new crate (`ignore`) with a stated reason (R5): gitignore semantics (nested files, negation, `info/exclude`, global excludes) are not a few lines. Fuzzy matching and diff parsing are hand-rolled (small, pure, tested); diffs come from git, not a diff crate. Mode and worktree are two small tables, no config surface. A new `VizierChannelId` variant was rejected by 040 as unnecessary; it is now justified by FR-014 (see Complexity Tracking). | PASS (justified) |
| II. DRY via trait-based extensibility | All project tools implement `VizierTool` and are listed once in `projects/tools::REGISTRY` with their `Access`; catalogue, dispatch (`RouterBridge`), docs (`describe_tool_function`) and mode filtering read that table, so adding a tool is one entry, no `match` on names. Path confinement is one function (`projects::path::resolve`) shared by every tool, the shell's `workdir`, mentions and git routes. Git calls go through one runner (`projects::git::Git`). Channel construction from a web `channel_id` is one function (`VizierChannelId::from_web`) replacing six hand-built `HTTP(..)`s; "the person in this chat" is one helper (`web_user`). | PASS |
| III. Self-contained, zero-dependency runtime | `git` is optional: absent git or a non-repository hides git features (FR-024) and everything else works. No network: `fetch`/`push`/`gh` are out. Storage stays in bundled SQLite. WebUI stays embedded. | PASS |
| IV. Portability by default | Paths via `Path`/`canonicalize`, Windows verbatim prefixes handled by the existing `display_path`; line endings preserved by edits (CRLF-aware matching); `git` invoked by argv (no shell), which behaves the same on Windows; the shell stays `LocalShell`. `ignore` is pure Rust and builds for every `Cross.toml` target. | PASS |
| V. Unified errors & observability | Tools return `crate::Result`, with refusals as stable `code: message` strings like 040's shell (`path_outside_project`, `edit_not_found`, `edit_ambiguous`, `stale_read`, …); routes map `ServiceError` onto the envelope; `tracing` spans for git runs and the migration; content (file text) is never logged. | PASS |

**Post-design re-check (after Phase 1)**: unchanged. The channel variant touches every exhaustive
`match` on `VizierChannelId` once (compiler-enforced) and removes the slug parsing 040 needed; the
migration is recorded on #82. No new runtime dependency or service.

## Project Structure

### Documentation (this feature)

```text
specs/041-project-coding-workspace/
├── plan.md                       # This file
├── research.md                   # Phase 0: decisions R1–R16
├── data-model.md                 # Phase 1: tables, entities, state
├── contracts/
│   ├── coding-tools.md           # tool names, inputs, outputs, refusals, mode table
│   ├── channel-and-migration.md  # Project channel, slugs, wire format, migration
│   ├── http-api.md               # mode, files, git, worktree routes
│   └── ui.md                     # WebUI surfaces, FigCraft pass, interactions
├── quickstart.md                 # dummyplug + desktop validation
└── tasks.md                      # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/vizier-core/
├── Cargo.toml                        # + ignore
└── src/
    ├── schema/
    │   ├── session.rs                # VizierChannelId::Project, from_web, web_user, slugs
    │   ├── usage.rs                  # UsageActivity::Project
    │   └── project.rs                # ProjectMode, Worktree, ProjectChat value types
    ├── projects/
    │   ├── channel.rs                # wire prefix only; slug parsing moves to the migration
    │   ├── context.rs                # for_turn(): Project chats and scoped subagent pieces; cwd, mode, worktree, git
    │   ├── path.rs                   # NEW: resolve() confinement for all project paths
    │   ├── ledger.rs                 # NEW: stale-read ledger (session, path) → sha256
    │   ├── mentions.rs               # NEW: resolve @mentions into a context section
    │   ├── suggest.rs                # NEW: file walk cache + fuzzy scorer for `@`
    │   ├── git/                      # NEW: Git runner, porcelain parsers, worktree ops
    │   │   ├── mod.rs
    │   │   ├── status.rs
    │   │   └── worktree.rs
    │   ├── shell.rs                  # category → Coding; workdir via path::resolve
    │   └── tools/                    # NEW: REGISTRY + one file per tool
    │       ├── mod.rs                # Access, REGISTRY, offered(ctx), call(name, …), docs
    │       ├── read_file.rs  list_files.rs  grep.rs  glob.rs
    │       ├── write_file.rs  edit_file.rs  propose_plan.rs
    │       └── git_status.rs  git_diff.rs  git_log.rs
    ├── service/
    │   ├── projects.rs               # mode, worktree create/delete/detail, files, git DTOs
    │   ├── topics.rs                 # readable_channel → from_web; delete delegates to worktree
    │   └── conversations.rs          # kind_of on the variant (no string parsing)
    ├── storage/
    │   ├── project.rs                # ProjectStorage: + chat mode, + worktree rows
    │   └── sqlite/project.rs  session.rs  mod.rs   # tables; drop project_of_slug filters
    ├── projects/migrate.rs           # NEW: migrate_project_channels (old slug parsing lives here only)
    ├── dependencies.rs               # call the migration beside migrate_task_requester; hold the ledger
    ├── schema/request.rs             # VizierRequest.project_scope, .mentions
    ├── agents/
    │   ├── background/mod.rs         # dispatch_piece copies ctx.project into project_scope
    │   ├── tools/mod.rs              # ToolCategory::Coding; turn_catalogue/categories via registry
    │   ├── tools/python/bridge.rs    # ToolTarget::Project → projects::tools::call
    │   ├── tools/python/docs_tools.rs# output schema via registry
    │   ├── agent/mod.rs              # mentions into context_md; project context per turn
    │   ├── agent/system_prompt/project.rs # stable per chat: project, cwd, worktree, two-mode paragraph
    │   ├── agent/system_prompt/context.rs # ## Project state (mode, branch, shell, availability) per message
    │   └── process.rs                # web_user() for chat_reply; mentions metadata
    ├── usage/mod.rs                  # project activity/place
    └── notifications/mod.rs          # href from the variant

crates/vizier-http/src/
├── api/v1/agents/projects.rs         # + /mode, /files, /git/*, /worktrees routes
├── topic.rs                          # from_web; re-check on the variant; mentions on inbound
└── lib.rs                            # OpenAPI

design/src/                           # after FigCraft: ModeToggle, PlanCard, DiffView,
                                      # BranchMenu, MentionMenu (+ stories)
webui/app/
├── api/endpoints/projects.ts         # mode, files, git, worktrees
├── stores/projects.ts                # worktrees, mode, git summary per chat
├── screens/projects/                 # WorktreeDialog, DeleteWorktreeDialog, sidebar grouping
├── screens/chat/Composer.tsx         # mode toggle, @ mentions
├── screens/chat/ChangesPanel.tsx     # NEW: changes + diff
├── screens/chat/ConversationHeader.tsx # branch chip + switcher
└── screens/chat/client/PlanCard.tsx  # draws propose_plan from the execute_python report
```

**Structure Decision**: all project behaviour stays under `crates/vizier-core/src/projects/`
(tools, git, paths, mentions), authorization and DTOs in `service/projects.rs`, routes in the
existing project router. The only cross-cutting change is the channel variant, which is
deliberately compiler-driven.

## Design Overview

### 1. Channel kind (US6, FR-014–FR-016a)

`VizierChannelId::Project(String /* username */, String /* project id */)`, slug
`project__{user}__{id}`. Clients keep sending `channel_id = "project__<id>"`;
`VizierChannelId::from_web(username, channel_id)` (one constructor, used by the socket, topic
routes and `readable_channel`) maps it to the variant. `web_user()` answers "which person owns
this chat" for `HTTP` and `Project` alike and replaces the `HTTP(user, _)` destructures in
notifications, client tools, side chat, process and agents. Usage reports `UsageActivity::Project`
with place `project:{slug}:{topic}`. The dream cycle includes the variant because
`is_non_user_channel` does not exclude it (pinned by a test). Details and the full migration list:
[contracts/channel-and-migration.md](./contracts/channel-and-migration.md).

### 2. The registry and modes (US1, US2, FR-001–FR-013)

```rust
pub enum Access { Read, Write, Command }
pub struct ProjectToolSpec { tool: &'static dyn VizierToolDyn, access: Access, needs_git: bool }
pub static REGISTRY: &[ProjectToolSpec] = &[ /* read_file, list_files, …, workspace_shell */ ];
```

`offered(ctx)` = entries whose access the turn's mode allows (`Plan` → `Read` plus
`propose_plan`; `Build` → all, minus `Command` when the shell switch is off) and, for `needs_git`,
whose folder is a repository. `turn_catalogue` appends `offered(ctx)`; `RouterBridge` refuses a
`ToolTarget::Project` name not in `offered(ctx)` before calling it, so a script cannot reach a
Build tool from Plan by name. `turn_categories` adds `coding` whenever `ctx.project` is set
(both modes, so `SANDBOX.md` stays stable within a chat). The `## Project` system-prompt section
is byte-identical for the life of a chat; the mode, branch, shell state and folder availability
are told with each message in a `## Project state` section of `context_md`, so switching mode or
branch never invalidates the prompt cache for the history (R17). The mode is read once per turn
(`context::for_session`), so a switch applies next turn and a resumed or woken turn reads the
saved mode. Contracts: [contracts/coding-tools.md](./contracts/coding-tools.md).

### 3. Files (US1)

`projects::path::resolve(cwd, rel, Purpose)` canonicalises the deepest existing ancestor,
requires it to start with `cwd`, refuses `.vizier/` from the main folder, and refuses absolute
paths. Reads are bounded and numbered; walks use `ignore::WalkBuilder` (hidden files shown,
`.git/` skipped, `.vizier/` skipped in the main folder). Edits match exact text (CRLF-aware),
write through a temp file + rename in the same folder, and consult the **stale-read ledger**: a
`(session slug, canonical path) → sha256` map on `VizierDependencies`, written by every read and
write, required equal to the file's current hash by `edit_file` and by `write_file` on an
existing file.

### 4. Mentions (US5)

The socket's `chat` frame gains `mentions: [path]` (validated, ≤ 10). `VizierRequest.mentions`
is `#[serde(default)]` and copied into the history entry's metadata. `chat_turn` resolves them
against the turn's `cwd` into a `## Mentioned files` section of `context_md` (never a system
prompt), 64 KiB per file, 256 KiB per message, folders as a 200-entry shallow listing.
Suggestions: `GET …/projects/{pid}/files?q=&topic=` over a 10 s cached walk with a hand-rolled
subsequence scorer.

### 5. Git and worktrees (US3, US4)

`projects::git::Git::new(cwd)` runs `git -C <cwd>` with argv, `GIT_TERMINAL_PROMPT=0`,
`GIT_OPTIONAL_LOCKS=0` for reads, `-c core.quotepath=off`, 15 s timeout, and returns
`ServiceError`s with plain messages. Status uses `--porcelain=v2 -z --branch`, numstat counts
`git diff HEAD --numstat -z` plus untracked files' line counts. Branch switch/create:
`git switch [-c]`, refused while any main-folder turn of the project is pending. Worktrees:
`git worktree add -b <branch> <path> <base>`; `.vizier/` appended once to
`<git-common-dir>/info/exclude`; delete runs `git worktree remove [--force]`, optionally
`git branch -d|-D`, then the existing topic deletion. Routes:
[contracts/http-api.md](./contracts/http-api.md).

### 6. WebUI

Designed in FigCraft first (mode toggle, plan card, changes panel with diff, branch menu,
worktree dialog and delete confirmation, sidebar grouping, mention menu), built in `design/`
with stories, then used in `webui/`. Surfaces and behaviour: [contracts/ui.md](./contracts/ui.md).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New `VizierChannelId::Project` variant (040 chose not to add one) | FR-014: project chats must be recognised by listings, usage, notifications, dreams and access without parsing a name. Today nine call sites parse `http__<user>__project__<id>`, and every new `HTTP` consumer must remember the carve-out. | Keeping `HTTP(user, "project__<id>")` leaves the string convention as the only marker; a missed carve-out silently shows a project chat as a WebUI chat (the class of bug FR-014 exists to remove). |
| One-time data migration of channel slugs and serialised channels | FR-015: 040 chats must keep history, files, turns, jobs and usage. | A lenient deserializer alone cannot change slug columns that queries match on; keeping the old slug for the new variant would make `project__…` and `http__…` both mean "project" forever. |
| New crate `ignore` | FR-005/FR-017: listings, searches and suggestions must follow the project's ignore rules. | Hand-rolling `.gitignore` (nested files, negation, anchoring, `info/exclude`, global `core.excludesFile`) is hundreds of lines and a source of subtle bugs; `walkdir` + `git ls-files` needs git (FR-024 says file tools work without it). |
