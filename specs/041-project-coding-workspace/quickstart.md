# Quickstart: Project Coding Workspace

**Feature**: `041-project-coding-workspace`

Validation guide. Dummyplug steps are the constitution's end-to-end gate; desktop steps cover
what needs the app (project creation is desktop-only, 040); one live-provider step checks
prompting. Dummyplug's protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`:
every tool runs through `execute_python`, one tool per turn, so messages below are JSON requests
whose script calls the coding tools.

---

## 0. Setup and automated checks

```sh
cargo clippy --workspace
cargo test --workspace
cargo test -p vizier-core --no-default-features
cd design && npm test && cd ..
cd webui && npm run typecheck && npm test && cd ..
```

Expected: all pass, including the new unit tests (path resolution, edit semantics, ledger,
ignore filtering, porcelain parsing, mode filtering, the 040→041 channel migration).

Prepare a repository to open as a project:

```sh
rm -rf /tmp/vz-041 && mkdir -p /tmp/vz-041 && cd /tmp/vz-041
git init -q -b master
printf 'target/\n' > .gitignore
mkdir -p src target && printf 'fn greet() {}\nfn main() { greet(); }\n' > src/main.rs
printf 'junk\n' > target/out.txt
printf 'line\r\nother\r\n' > crlf.txt
git add -A && git commit -qm init && cd -
just run 2>&1 | tee /tmp/vizier-041.log
```

In the desktop app, open `/tmp/vz-041` as a project on a dummyplug agent `dp-a` (040 flow). Start
a chat. It shows **Plan**.

## 1. Migration of a 040 install (US6, FR-015, SC-007)

Before upgrading, on a 040 build: open a project on `dp-a`, send two messages, leave one queued
behind a `workspace_shell(command='sleep 20')` turn, and stop Vizier mid-turn. Start the 041 build.

- Log: `migration.project_channel_v1` applied once; no errors.
- `sqlite3 <workspace>/.runtime/vizier.db "select count(*) from session_history where channel like 'http__%__project__%'"` → `0`.
- The project chat opens with full history; the interrupted turn resumes; Usage lists it under
  **Projects**; a notification from it links to the project chat.
- Restart again: the migration does not run a second time.

## 2. Plan mode is read-only (US2, FR-009–FR-011, SC-003)

In the new chat (Plan), send `tools`. Expected: `coding` is listed; `list_tool_functions` for
`coding` returns `read_file`, `list_files`, `grep`, `glob`, `git_status`, `git_diff`, `git_log`,
`propose_plan` — and not `write_file`, `edit_file`, `workspace_shell`.

```json
{"tool":"execute_python","arguments":{"code":"print(write_file(path='x.txt', content='hi'))"}}
```

Expected: the script reports `tool_not_offered: write_file is not available in plan mode`; no
`x.txt` exists.

```json
{"tool":"execute_python","arguments":{"code":"print(list_files())\nprint(grep(pattern='greet'))"}}
```

Expected: entries include `src/main.rs`, `.gitignore`, `crlf.txt`; nothing under `target/` or
`.git/`. `grep` returns two matches in `src/main.rs` (lines 1 and 2).

```json
{"tool":"execute_python","arguments":{"code":"propose_plan(title='Rename greet', steps=['Rename greet to hello in src/main.rs'])"}}
```

Expected: the turn ends on that call (`ended_by_tool: propose_plan`); the WebUI shows a plan card
with **Build this plan**.

## 3. Build mode and file tools (US1, FR-001–FR-007, SC-001, SC-002)

Press **Build this plan**. Expected: the toggle shows **Build**; a message "Build the plan." is
sent. Then:

```json
{"tool":"execute_python","arguments":{"code":"print(edit_file(path='src/main.rs', old_text='greet', new_text='hello'))"}}
```

Expected: `stale_read`, because this chat has not read the file yet (`grep` and `list_files` do
not count as reads; only `read_file` and mentions do). The file is unchanged.

```json
{"tool":"execute_python","arguments":{"code":"print(read_file(path='src/main.rs'))\nprint(edit_file(path='src/main.rs', old_text='greet', new_text='hello'))"}}
```

Expected: the read shows numbered lines `1\tfn greet() {}`; the edit fails `edit_ambiguous`
(2 occurrences, lines 1, 2); the file is byte-identical (`shasum` before/after).

```json
{"tool":"execute_python","arguments":{"code":"print(edit_file(path='src/main.rs', old_text='greet', new_text='hello', replace_all=True))\nprint(read_file(path='src/main.rs', start_line=2, end_line=2))"}}
```

Expected: `replacements: 2`; line 2 reads `fn main() { hello(); }`.

Stale read: change the file outside (`echo '// x' >> /tmp/vz-041/src/main.rs`), then
`edit_file(path='src/main.rs', old_text='hello()', new_text='hi()')` → `stale_read`.

CRLF: `read_file(path='crlf.txt')`, then `edit_file(path='crlf.txt', old_text='line\nother', new_text='a\nb')`
→ `replacements: 1`; `od -c crlf.txt` shows `a \r \n b \r \n`.

Confinement: `read_file(path='../etc/passwd')`, `read_file(path='/etc/hosts')`, and a symlink
`ln -s /etc /tmp/vz-041/etc_link` then `read_file(path='etc_link/hosts')` → all
`path_outside_project`.

Shell: `workspace_shell(command='git status --short')` runs; turn the project's shell switch off
and, next turn, `workspace_shell` is `tool_not_offered … while the shell is off` while `edit_file`
still works (US2-6).

Mode switch mid-turn (FR-013): start `workspace_shell(command='sleep 10')`, switch to Plan while it
runs. Expected: the call completes; next turn `workspace_shell` is not offered.

Subagents inherit (FR-007a). In the Build chat:

```json
{"tool":"execute_python","arguments":{"code":"print(parallel_subagents(tasks=[{'prompt':'{\"tool\":\"execute_python\",\"arguments\":{\"code\":\"print(read_file(path=\\'src/main.rs\\'))\"}}'}]))"}}
```

Expected: the piece's answer contains the numbered contents of `src/main.rs`. Repeat with a piece
that calls `write_file(path='from-piece.txt', content='x')`: the file appears in the chat's
folder. Switch the chat to Plan, repeat the write piece: it fails `tool_not_offered`. A piece
calling `propose_plan` fails `tool_not_offered`. Launch `background_subagents` with a long
`workspace_shell(command='sleep 20')` piece, restart Vizier mid-piece: the resumed piece still has
the coding tools. A `delegate_agent` to `dp-b` gets no `coding` category.

## 4. Git interface (US4)

With the edits above uncommitted:

- The chat header shows `master · 2 changed` (src/main.rs, plus crlf.txt).
- `GET …/git/changes` lists both with counts; selecting `src/main.rs` shows its diff.
- Agent tools: `git_status()` lists the same; `git_diff(path='src/main.rs')` returns the diff;
  `git_log(n=1)` shows `init`.
- Branch menu → **New branch from master** `feature-x` → header shows `feature-x`.
- Start `workspace_shell(command='sleep 15')` in a main-folder chat and try switching back to
  `master` → refused "turn running"; after it ends, switch with the uncommitted change conflicting
  (commit on `master` a different `src/main.rs` first) → refused with the plain "changes would be
  overwritten" message, nothing changed.
- Open a project on a folder with no `.git` → no branch chip, no changes section, file tools work,
  `git_*` tools are not offered (FR-024, SC-009).

## 5. Worktrees (US3)

From the project row: **New worktree** `fix-ps`, start from `master`.

- Folder `/tmp/vz-041/.vizier/dp-a/fix-ps` exists; `git -C /tmp/vz-041 branch` lists `fix-ps`;
  `git -C /tmp/vz-041 status --short` does not mention `.vizier/`; `git diff -- .gitignore` is empty
  (excluded via `info/exclude`).
- A chat named `fix-ps` opened, in Plan, listed under the project with the branch.
- Switch it to Build: `write_file(path='only-here.txt', content='x')` → the file is in the
  worktree, not in `/tmp/vz-041`.
- `read_file(path='../../../src/main.rs')` → `path_outside_project`.
- From a main-folder chat: `list_files()` shows no `.vizier/`; `read_file(path='.vizier/dp-a/fix-ps/only-here.txt')`
  → `path_in_worktree_area`.
- The main folder's branch menu shows `fix-ps` as "in worktree fix-ps", not selectable.
- Create a second worktree named `fix-ps` → `name_in_use`; named `bad..name` → `invalid_name`.
- Delete the `fix-ps` chat: the confirmation says 1 untracked file will be lost; **Also delete
  branch** is unchecked (not merged). Confirm. Folder and chat are gone; branch `fix-ps` remains.
- Create `clean-wt`, delete it immediately: plain confirmation, branch checkbox checked (merged
  with base), branch removed.
- Create `gone-wt`, `rm -rf` its folder outside Vizier: the chat shows unavailable with history;
  delete needs no change warning; `git worktree list` no longer shows it.
- Remove the project: the confirmation says worktree folders stay; they do.

## 6. Mentions (US5)

In a Build chat type `@mai` → `src/main.rs` is suggested (and nothing under `target/`). Pick it,
send `context`.

Expected: the echoed context contains `## Mentioned files` with `src/main.rs` numbered contents;
the stored history entry has `metadata.mentions: ["src/main.rs"]` and no file contents
(`sqlite3 … "select data from session_history where …"`). Immediately
`edit_file(path='src/main.rs', …)` succeeds without a `read_file` (the mention counts as a read).

Mention a missing path (type it and delete the file before sending) → the section says it is
missing. Mention a 1 MiB file → "too large, use read_file". Mention `src/` → a listing.

## 7. Dream cycle (FR-016a)

Trigger `dp-a`'s dream (`POST /api/v1/agents/dp-a/dream/trigger` or the WebUI action). Expected: the extraction
stage reads the project chat's history; the dream's tool list has no `coding` category.

## 8. Live provider (prompting; mark as live)

With a real model, in Plan: "rename greet to hello". Expected: the agent reads and greps, then
calls `propose_plan` rather than editing; after **Build this plan**, it uses `read_file` +
`edit_file`, not shell `sed`/heredocs.
