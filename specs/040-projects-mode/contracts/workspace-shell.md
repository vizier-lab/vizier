# Contract: Workspace Shell

**Feature**: `040-projects-mode` | **Tool name**: `workspace_shell` | **Category**: `shell`

The workspace shell is the only way an agent runs commands in a project. It is
a per-turn tool: it exists only while the current session is a project session,
the project folder is available, and the project's shell switch is on.

---

## 1. Availability

| Condition | Tool in turn catalogue | Prompt |
|-----------|------------------------|--------|
| Ordinary conversation / task / dream / subagent | no | no `## Project` section |
| Project session, shell on, folder available | yes | folder + instructions |
| Project session, shell off | no | `## Project` says the shell is off |
| Project session, folder missing/unreadable | no | `## Project` says the folder is unavailable |
| Background report waking in a project session | yes, per the same rules | inherited |
| Side chat inside a project session | no (side chats are tool-less) | `## Project` present in its prompt prefix |

Mechanism: `ToolContext.project` + `ToolTarget::Project`, appended by
`turn_catalogue`/`turn_categories`, dispatched by `RouterBridge`. It is never
registered in `BaseTools` or in the agent's shell addon, so it cannot leak into
another session kind.

---

## 2. Input schema

```json
{
  "type": "object",
  "required": ["command"],
  "properties": {
    "command": {
      "type": "string",
      "description": "The command line to run in the project folder."
    },
    "workdir": {
      "type": "string",
      "description": "Optional directory relative to the project root. Must stay inside it."
    },
    "timeout_secs": {
      "type": "integer",
      "minimum": 1,
      "maximum": 600,
      "description": "Optional time limit; defaults to 120 seconds."
    }
  }
}
```

---

## 3. Output schema

```json
{
  "stdout": "hello\n",
  "stderr": "",
  "exit_code": 0,
  "duration_ms": 18,
  "timed_out": false,
  "truncated": false
}
```

| Field | Contract |
|-------|----------|
| `stdout` | Captured standard output, UTF-8 lossy, capped. |
| `stderr` | Captured standard error, UTF-8 lossy, capped. |
| `exit_code` | The process exit code; `null` when killed by the timeout. |
| `duration_ms` | Wall-clock from spawn to exit or kill. |
| `timed_out` | `true` only when the time limit was reached and the process tree was killed. |
| `truncated` | `true` when either stream hit the 64 KiB cap. |

**A non-zero `exit_code` is a successful tool result** (FR-010). The tool
returns an error only for:

- the project being unavailable (`project_unavailable`);
- the shell switch being off (`shell_disabled`);
- `workdir` escaping the project root (`workdir_outside_project`);
- failure to spawn the shell (`spawn_failed`).

---

## 4. Execution semantics

1. Resolve the project root from `ToolContext.project` (canonical).
2. Resolve `workdir`:
   - absent or `""` → the project root;
   - absolute → refused;
   - otherwise `root.join(workdir)` canonicalized; must exist and
     `starts_with(root)`, otherwise `workdir_outside_project`.
3. Spawn the existing platform invocation (`sh -c <command>` on Unix,
   `powershell.exe -NoProfile -NonInteractive -Command` with UTF-8 output on
   Windows), with the host environment and the resolved working directory.
4. Read stdout/stderr concurrently; cap each at 64 KiB (UTF-8-safe cut) and set
   `truncated`.
5. Wait until exit or the timeout (default 120 s, per-call max 600 s).
6. On timeout: kill the process tree, then return the partial output with
   `timed_out: true`, `exit_code: null`.
7. Return the JSON object; the sandbox bridge records it as a normal tool
   result in history.

**Process-tree kill**

- Unix: `tokio::process::Command` with `kill_on_drop(true)` and
  `CommandExt::process_group(0)`; on timeout `libc::killpg(pgid, SIGKILL)`,
  falling back to `child.kill()`.
- Windows: `taskkill /PID <pid> /T /F`, then wait for the child.
- The platform code lives in `agents/shell/process_tree.rs`; no `cfg` in
  business logic (Principle IV).

**Not changed**

- The existing addon `shell_exec` keeps its `commands` input and text output;
  `LocalShell` renders the richer outcome back to the same text contract so the
  addon's behaviour is not silently changed.
- Docker shells are unchanged; a project folder is a host path and the project
  shell is always local.

---

## 5. Prompt contract

The system prompt gains one section, after CORE and before `config.documents`:

```text
## Project

You are working in the project "<name>", at folder <absolute path>.
The `workspace_shell` tool runs commands in that folder on the host machine,
with the user's account. It is not sandboxed.

[If the shell switch is off]
The project's workspace shell is turned off, so you cannot run commands here.

[If the folder is unavailable]
The project folder is unavailable; commands cannot run until it is restored.
The conversation history is still available.

## Project Instructions

<contents of AGENTS.md or CLAUDE.md>

[If truncated]
The project instructions were truncated at 32 KiB.
```

A missing instruction document omits the `## Project Instructions` section
entirely. The section is rebuilt each turn, so edits are picked up without a
restart. It is never added for non-project sessions (FR-014).

---

## 6. Verification hooks

- Unit: timeout kills a child that spawns a grandchild; output over the cap is
  cut and flagged; `exit_code` is preserved for a failing command; `..` and an
  absolute `workdir` are refused.
- Dummyplug: a JSON tool call returning stdout/stderr/exit/duration; the same
  tool is absent from `tools` in an ordinary conversation.
- Manual/desktop: a long-running command returns `timed_out: true` within its
  limit and leaves no child process.
