# 2.13 Projects

A **project** mounts one host folder to exactly one agent. Inside a project session the agent
works in that folder: it knows the path, reads the folder's `AGENTS.md` (or `CLAUDE.md`), and can
run commands there through the `workspace_shell` tool. Projects are created only from the desktop
app; the browser can use the projects that already exist.

## Opening a project

Desktop app → an agent → **Open a Project** in the Projects section (or the empty conversation).
Choose a folder, then accept the trust disclosure.

The disclosure is the safeguard, because commands are **not sandboxed**:

- they run with your operating-system account;
- anything you can reach, the agent can reach;
- only open folders you trust with that access.

The folder's final component becomes the project name; rename it afterwards if you like. Opening
the same folder again for the same agent reuses the existing project (no duplicate). Opening it
for another agent creates a separate project: a project belongs to one agent, and its sessions
belong to one person.

The application's own workspace, and anything inside it, is refused: `.runtime` holds the
database and encryption key, `agents/` the memory documents, and `uploads/` session files.

## Sessions

A project holds several independent sessions. Start one from the project row's **+**, or from the
project header's **New conversation**. Sessions are grouped under the project in the sidebar above
the regular Sessions list, and **never** appear in that list, in session search or in the
conversation picker.

Each session is private to the person who opened it. Two people with the project permission share
the project and its files, but not their session lists or histories.

## Plan and Build

Every project chat has a mode, saved with the chat. A new chat starts in **Plan**.

- **Plan** offers only tools that read: `read_file`, `list_files`, `grep`, `glob` and the
  read-only git tools. The agent is told to finish with `propose_plan`, which the WebUI draws as a
  plan card with **Build this plan**.
- **Build** adds `write_file` and `edit_file`, and `workspace_shell` when the project's shell
  switch is on.

The mode is server state, so every tab, a resumed turn and a background report see the same one.
A switch applies from the next turn; a running turn keeps the tools it started with. Subagents
launched from a project chat inherit its folder (the worktree, when it has one) and the launching
turn's mode, minus `propose_plan`.

## Coding tools

All project tools are listed in the `coding` category. They take paths relative to the chat's
folder and refuse anything that leaves it (`path_outside_project`), including through `..`,
symbolic links or an absolute path. In the main folder, `.vizier/` is refused as
`path_in_worktree_area`.

| Tool | What it does |
|---|---|
| `read_file` | Reads a file, optionally a 1-based line range, with line numbers and a continuation hint |
| `list_files` | Lists a folder, sorted, with sizes, honouring ignore rules |
| `glob` | Finds files by a gitignore-style pattern |
| `grep` | Regex search with optional glob and context lines |
| `write_file` | Writes a whole file atomically, creating parents inside the folder |
| `edit_file` | Replaces exact text; refuses a miss, an ambiguity or a stale read, leaving the file byte-identical |

`edit_file` and `write_file` only overwrite a file the chat has read and whose bytes still match
what it read; a changed file is `stale_read` until it is read again. Edits preserve CRLF line
endings and the final-newline state. Listings and searches follow `.gitignore`, nested ignore
files and `info/exclude`; `include_ignored` reaches ignored files deliberately.

## Mentions

In a project or worktree chat, typing `@` suggests files and folders from that chat's folder
(ignored files excluded). A picked file's current contents — or a folder's shallow listing — are
given to the agent with the message, bounded per file and per message. A mention counts as a read
for the stale-read guard. The saved conversation keeps the text and the list of mentions, never the
contents.

## Git

When git is installed and the folder is inside a repository, the chat header shows the branch and
the number of changed files; the details panel lists every change with added/removed counts and a
diff. The branch menu switches or creates a branch. A switch is refused while a turn is running in
that folder (`turn_active`), when another worktree has the branch checked out
(`branch_checked_out`), or when git would overwrite local changes (`local_changes`).

The agent has read-only `git_status`, `git_diff` and `git_log`. There is no commit or push in the
UI: commit through the workspace shell in Build mode, or outside Vizier.

## Worktrees

In a git project a person who may use it can create a worktree: a separate checkout under
`.vizier/<agent-id>/<name>` on its own branch, bound to exactly one chat. `.vizier/` is added to the
repository's `info/exclude`, so it does not appear in `git status` and no tracked file changes.

Every tool, the shell, mentions and git act in the worktree for that chat, and paths outside it are
refused. Deleting the worktree chat removes the chat and the folder; with uncommitted or untracked
changes the confirmation says how many files will be lost, and **Also delete branch** is
pre-selected only when the branch is merged into the branch it started from.

A fresh worktree contains only tracked files: install dependencies or run setup through the agent in
Build mode, guided by the project's own instructions. Note that build tools or file watchers in the
main folder may react to changes inside `.vizier/`. Removing a project leaves its worktree folders
on disk.

## The workspace shell

In a Build-mode project session, with the shell switch on, the agent has `workspace_shell`:

```json
{ "command": "cargo test", "workdir": "crates", "timeout_secs": 120 }
```

It returns `stdout`, `stderr`, `exit_code`, `duration_ms`, `timed_out` and `truncated`. A non-zero
exit is a normal result, not a tool failure. Each stream is capped at 64 KiB. The default time
limit is 120 seconds (maximum 600); a timed-out command and its child processes are stopped.

`workdir` is relative to the chat's folder (the worktree, for a worktree chat) and must stay
inside it. The command itself is not sandboxed — the trust disclosure says so.

Turn the shell off from the project row's menu. The agent is then not offered the tool at all,
even in Build mode. The switch takes effect on the next turn; no agent restart.

Two sessions of one project may run commands at the same time. They are not serialised, and file
changes can race; that is the person's responsibility.

## Project instructions

If the project folder contains `AGENTS.md`, the agent receives it (up to 32 KiB) in a
`## Project Instructions` section of its system prompt. `CLAUDE.md` is the fallback. A missing
document is harmless. The file is re-read every turn, so edits apply without a restart. The
instruction text is never returned by the API — only whether it exists and whether it was
truncated.

## Project skills

The folder's own skills, in `.agents/skills/<slug>/` (then `.claude/skills`), are available to the
chat and listed in a `## Project Skills` section of its system prompt. They are read where they lie,
never copied into the workspace and never indexed, so a branch switch shows up on the next turn.
`/skills` lists them with the workspace ones, each slug once, and typing `/` in the composer offers
them too, read from the chat's worktree when it is bound to one. [Skills](skills.md#project-skills)
has the frontmatter, the precedence and what happens to a skill's scripts and hooks.

## Permissions

| Permission | Meaning |
|---|---|
| `all_agents:projects` | Use projects on every agent the person can view; manage every project |
| `owned_agents:projects` | Use projects on agents the person owns |

The desktop workspace owner holds both through the system role. A person without a project
permission sees no Projects section, no creation action and no project data; a direct URL or API
call is refused.

Renaming, removing and the shell switch are limited to the project's owner or a person with
`all_agents:projects`. Creating a project requires a **desktop principal**: the browser has no
folder to choose, and a direct `POST` from it is refused with `403` and `code: "desktop_only"`.

The browser shows a persistent "Commands run on the host machine" notice on project sessions.

## Removing, restoring and unavailable folders

**Remove** is a soft removal. The project and its sessions leave the sidebar; no file in the folder
is deleted and no history row is removed. Opening the same folder again in the same agent restores
the same project and its sessions. Removal is refused with `409 turn_active` while any turn in the
project is queued or running.

If the folder is moved, renamed or deleted, the project is marked **unavailable**. Its history
stays readable, but the agent is not offered the shell and commands are refused. Restoring the
folder makes it available again on the next turn, with no restart. Moving the folder to a new path
and opening that path creates a new project; relinking is out of scope.

## API

Agent-scoped routes under `/api/v1/agents/{agent_id}/projects`:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/` | List the agent's open projects |
| `POST` | `/` | Open a folder (desktop only) |
| `GET` | `/{project_id}` | Project detail; instructions presence only |
| `PATCH` | `/{project_id}` | Rename and/or toggle the shell |
| `DELETE` | `/{project_id}` | Soft-remove |
| `GET` | `/{project_id}/sessions` | The caller's sessions |
| `POST` | `/{project_id}/sessions` | Create an empty session |
| `PUT` | `/{project_id}/sessions/{topic}/mode` | Switch one chat between `plan` and `build` |
| `GET` | `/{project_id}/files?q=&topic=` | `@` file suggestions |
| `GET` | `/{project_id}/git?topic=` | Branch and change count (`available: false` without git) |
| `GET` | `/{project_id}/git/changes?topic=` | Uncommitted changes with line counts |
| `GET` | `/{project_id}/git/diff?topic=&path=` | One file's diff against the last commit |
| `GET` | `/{project_id}/git/branches?topic=` | Branches, and the worktree holding each |
| `POST` | `/{project_id}/git/switch` | Switch or create a branch |
| `GET` | `/{project_id}/worktrees` | The project's worktrees |
| `POST` | `/{project_id}/worktrees` | Create one worktree and its chat |
| `GET` | `/{project_id}/worktrees/{id}` | Availability, change counts, merged state |
| `DELETE` | `/{project_id}/worktrees/{id}?force=&delete_branch=` | Delete the chat and folder |

A project session's channel id is `project__<project_id>`; history, files and the chat socket use
the existing channel routes with that id. Internally the channel is its own kind, so a project
chat is reported as **Projects** in usage, links to its project chat in notifications, and never
appears in the regular Sessions list. Chats created before this change are migrated at startup.

## Out of scope

Line-range writes, GitHub (`#123`) references, merging or rebasing a worktree from the UI,
agent-created worktrees and a per-project worktree setup command are out of scope. Per-action
approval (ask before running a command) belongs to the guardrails work, not Plan mode. There is no
terminal UI and commands are not sandboxed.
