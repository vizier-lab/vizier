# Quickstart: Projects Mode Foundation

**Feature**: `040-projects-mode`

This is the validation guide for the foundation. It combines the offline
`dummyplug` end-to-end checks required by the constitution with the
desktop-only steps that cannot run over the plain HTTP test suite (project
creation) and one live-provider check where prompting is what is being tested.

Dummyplug's chat protocol is
`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`. Every agent
tool call goes through `execute_python` (code mode); dummyplug runs one tool per
turn. `workspace_shell` is script-callable, so the messages below are JSON tool
requests for `execute_python` whose script calls it.

Unless a step says otherwise, run on a machine with a POSIX shell or adapt the
command to PowerShell. Both are supported by `LocalShell`.

---

## 0. Prerequisites and automated checks

1. Build the workspace and the WebUI:

   ```sh
   cargo build
   cd webui && npm ci && npm run build && cd ..
   ```

2. Run the Rust and WebUI checks:

   ```sh
   cargo clippy --workspace
   cargo test --workspace
   cd webui && npm run typecheck && npm test && cd ..
   cd app && cargo test && cd ..
   ```

   Expected: all pass. The app test suite creates a project through the
   embedded desktop `Dispatcher` and asserts that the network router refuses
   the same call (`403 desktop_only`); this is the automated proof of FR-021.

3. Start Vizier and capture the log:

   ```sh
   just run 2>&1 | tee /tmp/vizier-projects.log
   ```

4. Create two dummyplug agents, `dp-a` and `dp-b`, one folder per project test:

   ```sh
   mkdir -p /tmp/vizier-project-alpha
   printf 'Always answer in French.\n' > /tmp/vizier-project-alpha/AGENTS.md
   mkdir -p /tmp/vizier-project-beta
   ```

---

## 1. Desktop creation and trust disclosure (US1, FR-001/FR-002/FR-028)

1. In the desktop app, sign in as the workspace owner and open `dp-a`.
2. In the empty conversation, choose **Open a Project**.
3. Pick `/tmp/vizier-project-alpha`.
4. Expected: a trust disclosure appears before anything is created, stating
   that commands run with the operating-system account, that there is no
   sandbox, and that only trusted folders should be opened. Cancel; no project
   appears.
5. Repeat, confirm. Expected: the project appears above Sessions with the
   session name **New session**, and the folder's basename as the project name.
6. Expected: a `session_detail` row titled `New session` exists for
   `channel = project__<id>`; the regular Sessions list does not contain it.
7. Open the same folder again. Expected: the same project is reused, not
   duplicated (FR-003/SC).
8. Try to open the app's own workspace (the `vizier` data folder shown in
   Settings → Desktop). Expected: refused with an explanation; no row is
   created (FR-028).
9. Repeat step 5 for `dp-b` with the same folder. Expected: a separate project
   for `dp-b`; `dp-a`'s sessions do not appear there.

---

## 2. Workspace shell (US1, FR-009–FR-012)

In `dp-a`'s project session, send:

```json
{"tool":"execute_python","arguments":{"code":"result = workspace_shell(command='echo hello')\nprint(result)"}}
```

Expected: the tool result contains `stdout` with `hello`, an empty `stderr`,
`exit_code: 0`, a non-negative `duration_ms`, `timed_out: false`,
`truncated: false`.

Create a file through the shell:

```json
{"tool":"execute_python","arguments":{"code":"print(workspace_shell(command='echo created > created.txt'))"}}
```

Expected: `exit_code: 0`, and `/tmp/vizier-project-alpha/created.txt` exists
with `created`.

A failing command is a normal result (FR-010):

```json
{"tool":"execute_python","arguments":{"code":"print(workspace_shell(command='exit 3'))"}}
```

Expected: `exit_code: 3`, no tool error, the turn completes.

Timeout and process-tree kill (FR-011):

```json
{"tool":"execute_python","arguments":{"code":"print(workspace_shell(command='sleep 30', timeout_secs=1))"}}
```

Expected: `timed_out: true`, `exit_code: null`, `duration_ms` about 1000, and
no `sleep`/`Start-Sleep` child left running.

Working directory escape is refused (FR-009 edge case):

```json
{"tool":"execute_python","arguments":{"code":"print(workspace_shell(command='pwd', workdir='..'))"}}
```

Expected: a tool error naming `workdir_outside_project`; no command runs.

Output cap (FR-012):

```json
{"tool":"execute_python","arguments":{"code":"print(workspace_shell(command=\"printf 'x%.0s' {1..70000}\"))"}}
```

Expected: `truncated: true` and `stdout` no longer than 64 KiB. On PowerShell
use `('x' * 70000)`.

Ordinary conversations must not have the tool:

1. Open a normal conversation with `dp-a`.
2. Send `{"tool":"list_tool_functions","arguments":{"category":"shell"}}`.
3. Expected: an unknown-category response (or no `workspace_shell`); the
   project session's same request lists `workspace_shell`.

---

## 3. Project sessions, sidebar and concurrency (US2, FR-005–FR-008/FR-027)

1. From the project row, create three sessions.
2. Expected: all three are listed under the project; switching preserves each
   session's history; the live marker appears only on a running session.
3. Expected: none of the three appears in the regular Sessions list, the
   conversation picker or session search (FR-006).
4. With two sessions open, send the 3-second command to both at the same time:

   ```json
   {"tool":"execute_python","arguments":{"code":"print(workspace_shell(command='sleep 3', timeout_secs=10))"}}
   ```

   Expected: both finish in about 3 seconds, not 6 (FR-027 / SC-008).
5. Turn the project's shell switch off in the project menu. In a project
   session, send the `list_tool_functions` request. Expected: no
   `workspace_shell`; `## Project` says the shell is off. Turn it back on.
6. Create a project with no sessions and confirm it offers **New session**.

---

## 4. Project context and instructions (US4, FR-015/FR-016)

**Automated (offline)**: unit tests assert that a project session's system
prompt contains the project name, the absolute folder and the instruction file,
that a 40 KiB `AGENTS.md` is cut at 32 KiB with a truncation note, and that an
ordinary conversation's prompt contains none of these. Dummyplug cannot reveal
a system prompt, so this is the offline proof.

**Live provider (marked: depends on real model output)**: point `dp-a` at any
real provider in the same project session and ask:

```text
Where are you working, and what project instructions are you following?
```

Expected: the reply names `/tmp/vizier-project-alpha` and mentions
"Always answer in French." Ask the same question in an ordinary conversation;
expected: no project folder or project instructions.

Missing instruction document: remove `AGENTS.md`, send any message; expected:
no error, no `## Project Instructions` section, a normal reply.

---

## 5. Remote access and the host indicator (US3, FR-018/FR-020/FR-021/FR-023)

1. Enable web access in Settings → Desktop and note the URL.
2. Sign in from a browser as a person with `all_agents:projects` or
   `owned_agents:projects`.
3. Expected: the Projects section lists the existing projects for the agents
   they can view; there is **no** "Open a Project" action.
4. Open a project session and send the `echo hello` request from §2. Expected:
   the command runs on the host and the same JSON result appears in the
   browser; a persistent "Commands run on the host machine" notice is visible.
5. Attempt a direct creation from the browser:

   ```sh
   curl -i -X POST localhost:9999/api/v1/agents/dp-a/projects \
     -H "Authorization: Bearer $TOKEN" \
     -H 'content-type: application/json' \
     -d '{"folder":"/tmp/anything"}'
   ```

   Expected: `403` with `desktop_only`.

6. A user without a project permission: the Projects section is absent, and

   ```sh
   curl -i localhost:9999/api/v1/agents/dp-a/projects -H "Authorization: Bearer $TOKEN"
   curl -i localhost:9999/api/v1/agents/dp-a/channel/project__<id>/topic/<topic>/history \
     -H "Authorization: Bearer $TOKEN"
   ```

   Expected: `403` for both (FR-018).

7. Two permitted people open the same project. Expected: each sees only their
   own sessions (FR-024); each can start a new session.

8. Rename/remove as a non-owner without `all_agents:projects`. Expected: `403`.
   As the owner, expected: success and the sidebar updates.

9. Revoke the permission from a signed-in user, then send a turn from the open
   browser session. Expected: refused; a turn already running may finish
   (FR-018 edge case).

---

## 6. Removal, restoration and unavailability (FR-025/FR-026)

1. Note a project's session ids and create a marker file in the folder.
2. Remove the project. Expected: it and its sessions leave the sidebar; the
   marker file and every file in the folder remain; history rows remain.
3. Re-open the same folder in the same agent. Expected: the same project id and
   the previous sessions are listed (SC-006).
4. Start a long turn (`sleep 30`, `timeout_secs=60`) and try to remove the
   project. Expected: `409` with `turn_active`; nothing changes. After the turn
   ends, removal succeeds.
5. Rename the folder on disk. Reload. Expected: the project is marked
   unavailable; its history opens; a new command is refused (no
   `workspace_shell`). Restore the folder name. Expected: available again with
   no restart.
6. Move the folder to a new path and open it. Expected: a new project (new id)
   with new sessions; the old one stays unavailable (no relinking).

---

## 7. Attribution and notifications (FR-029)

1. Finish a turn in a project session while the session is not on screen.
   Expected: the owner receives the normal reply notification; the notification
   opens `/<agent>/projects/<id>/chat/<topic>`.
2. Expected: usage rows exist with `channel` containing
   `http__<owner>__project__<id>` and a place that includes the topic.
3. Expected: the other permitted person's session list and search do not
   contain that session (SC-010).

---

## 8. Scheduled tasks, dreams and subagents stay out (FR-014)

1. Schedule a task on `dp-a` that asks for a tool listing. Expected: no
   `workspace_shell`, no project folder in its prompt.
2. Run a background subagent or delegation from a project session, then inspect
   the piece's tool list. Expected: the piece has no `workspace_shell` and no
   project context. A background report waking back into the project session
   does inherit it (spec Assumptions).
3. Run a dream cycle. Expected: no project context and no workspace shell.

---

## 9. Restart and persistence (FR-004)

1. `just shutdown`, then `just run`.
2. Restart the desktop app. Expected: projects, availability, shell switches
   and sessions are unchanged; a project turn can run without re-opening the
   folder.
3. Expected: the regular Sessions list still excludes every project session.

---

## 10. Out of scope (FR-030)

Confirm absent: git integration, diff review, a file editor, per-session
worktrees, a terminal UI and sandboxing. The trust disclosure remains the
safeguard for host command execution.
