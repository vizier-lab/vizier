# Contract: Coding Tools

**Feature**: `041-project-coding-workspace` (FR-001–FR-013, FR-028)

All tools are reached from `execute_python` like every other tool (code mode). They exist only in
a turn whose session is a `Project` chat; they are listed under category `coding`; they are never
in `BaseTools`, `DREAM_TOOL_NAMES` or any non-project `ToolContext`.

Refusals are tool errors whose message starts with a stable code and a colon (as 040's shell), so a
script can catch them as `ValueError`/`RuntimeError` and a model can read them.

---

## 1. Availability

| Tool | Access | Plan | Build | Needs git |
|---|---|---|---|---|
| `read_file` | Read | ✓ | ✓ | |
| `list_files` | Read | ✓ | ✓ | |
| `grep` | Read | ✓ | ✓ | |
| `glob` | Read | ✓ | ✓ | |
| `git_status` | Read | ✓ | ✓ | ✓ |
| `git_diff` | Read | ✓ | ✓ | ✓ |
| `git_log` | Read | ✓ | ✓ | ✓ |
| `propose_plan` | Plan | ✓ | | |
| `write_file` | Write | | ✓ | |
| `edit_file` | Write | | ✓ | |
| `workspace_shell` | Command | | ✓ (shell switch on) | |

- The table is fixed for a turn (mode read at turn start).
- Calling a name not offered this turn fails with
  `tool_not_offered: <name> is not available in <mode> mode` (or `… while the shell is off`), before
  any work. This holds for scripts that call by name.
- An unavailable folder (`cwd` missing) offers no `coding` tools; the category line is still
  listed, and the project section of the system prompt says the folder is unavailable.

## 2. Common rules

- `path` arguments are relative to the chat's folder (`cwd`): the worktree for a worktree chat,
  otherwise the project root. Absolute paths: `path_outside_project`. A path that resolves outside
  `cwd` (`..`, symlinks): `path_outside_project`. From the project root, a path under `.vizier/`:
  `path_in_worktree_area`.
- Missing file where one is required: `not_found: <path>`. A directory where a file is required:
  `not_a_file: <path>`.
- Paths in results are relative to `cwd`, with `/` separators on every platform.

## 3. Tools

### `read_file`

Input: `{ path: string, start_line?: int ≥ 1, end_line?: int ≥ start_line }`

Output (text):

```json
{ "path": "src/lib.rs", "start_line": 1, "end_line": 120, "total_lines": 342,
  "content": "     1\tuse std::…\n     2\t…", "truncated": true, "next_start_line": 121 }
```

Output (binary — NUL in first 8 KiB): `{ "path": "...", "binary": true, "size": 1048576 }`

Defaults: `start_line = 1`, `end_line = start_line + 1999`. Each line is cut at 2,000 characters
(with `…`), the whole `content` at 256 KiB. `truncated`/`next_start_line` say how to continue.
Records the file's hash in the stale-read ledger.

### `list_files`

Input: `{ path?: string, depth?: int 1–10 = 3, include_ignored?: bool = false }`

Output: `{ "entries": [{ "path": "src/", "kind": "dir" }, { "path": "src/lib.rs", "kind": "file", "size": 1234 }], "truncated": false }`

Sorted by path; directories end in `/`; at most 1,000 entries. Skips `.git/`; from the project
root skips `.vizier/`; skips ignored entries unless `include_ignored`.

### `glob`

Input: `{ pattern: string, path?: string }` (gitignore-style glob, e.g. `**/*.rs`)

Output: `{ "paths": ["src/lib.rs"], "truncated": false }` — files only, sorted, at most 1,000,
ignore rules applied.

### `grep`

Input: `{ pattern: string /* regex */, path?: string, glob?: string, case_insensitive?: bool,
context?: int 0–5 = 0, max_results?: int 1–1000 = 200 }`

Output:

```json
{ "matches": [{ "path": "src/lib.rs", "line": 42, "text": "fn main() {",
                "before": [], "after": [] }],
  "files_searched": 118, "files_skipped": 1, "truncated": false }
```

Invalid regex: `invalid_pattern: <regex error>`. Binary files and files over 4 MiB are skipped
(`files_skipped`). Lines cut at 500 characters.

### `write_file`

Input: `{ path: string, content: string }`

Output: `{ "path": "src/new.rs", "created": true, "bytes": 812 }`

Creates parent folders inside `cwd`. Over 4 MiB: `too_large`. Overwriting an existing file
requires a ledger entry equal to its current hash, else
`stale_read: read src/lib.rs again before writing it`. Atomic (temp + rename).

### `edit_file`

Input: `{ path: string, old_text: string, new_text: string, replace_all?: bool = false }`

Output: `{ "path": "src/lib.rs", "replacements": 1, "first_line": 42 }`

Refusals (file unchanged in every case):

| Code | When |
|---|---|
| `invalid_edit` | `old_text` empty, or equal to `new_text` |
| `stale_read` | no ledger entry for this file in this chat, or the file's hash differs |
| `edit_not_found` | `old_text` does not occur |
| `edit_ambiguous` | occurs more than once and `replace_all` is false; message gives the count and the first five line numbers |

When the file uses `\r\n`, `\n` in `old_text`/`new_text` is matched and written as `\r\n`.
Atomic; updates the ledger.

### `propose_plan` (ends the turn)

Input: `{ title: string ≤ 120, summary?: string ≤ 2000, steps: [string ≤ 500] (1–30) }`

Output: `{ "status": "proposed" }`. A successful call ends the turn (spec 026), so the agent's
reply is the plan. The WebUI draws it as a plan card with **Build this plan**.

### `git_status`

Input: `{}` — Output: `{ branch, detached, ahead, behind, changes: [GitChange] }` (see
[http-api.md](./http-api.md) `GitChange`).

### `git_diff`

Input: `{ path?: string, staged?: bool = false }` — unified diff against `HEAD` (or the index when
`staged`), capped at 256 KiB with `truncated`. Without `path`, the whole working tree.

### `git_log`

Input: `{ n?: int 1–50 = 10, path?: string }` — `{ commits: [{ hash, author, date, subject }] }`.

### `workspace_shell`

Unchanged from 040 except: category `coding`; `workdir` is resolved against `cwd` with the shared
path rules; offered only in Build with the shell switch on.

## 4. System prompt (project section)

Adds, after the folder line:

- `Mode: Plan — you can read the project but not change it. Finish with propose_plan.` or
  `Mode: Build — you can change files and run commands.` (or `… change files; commands are off.`)
- `Working folder: <cwd display path>` and, for a worktree, `Worktree <name> on branch <branch>`.
- `Git branch: <branch>` when known.

The section is per turn (it already is in 040); the category list in `SANDBOX.md` is unchanged
by mode.
