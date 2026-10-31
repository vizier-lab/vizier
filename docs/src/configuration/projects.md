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

## The workspace shell

In a project session the agent has `workspace_shell`:

```json
{ "command": "cargo test", "workdir": "crates", "timeout_secs": 120 }
```

It returns `stdout`, `stderr`, `exit_code`, `duration_ms`, `timed_out` and `truncated`. A non-zero
exit is a normal result, not a tool failure. Each stream is capped at 64 KiB. The default time
limit is 120 seconds (maximum 600); a timed-out command and its child processes are stopped.

`workdir` is relative to the project root and must stay inside it. The command itself is not
sandboxed — the trust disclosure says so.

Turn the shell off from the project row's menu. The agent is then not offered the tool at all. The
switch takes effect on the next turn; no agent restart.

Two sessions of one project may run commands at the same time. They are not serialised, and file
changes can race; that is the person's responsibility.

## Project instructions

If the project folder contains `AGENTS.md`, the agent receives it (up to 32 KiB) in a
`## Project Instructions` section of its system prompt. `CLAUDE.md` is the fallback. A missing
document is harmless. The file is re-read every turn, so edits apply without a restart. The
instruction text is never returned by the API — only whether it exists and whether it was
truncated.

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

A project session's channel id is `project__<project_id>`; history, files and the chat socket use
the existing channel routes with that id.

## Out of scope

Git integration, diff review, a file editor, per-session worktrees, a terminal UI and sandboxing
are not part of projects.
