# Research: Project Coding Workspace

**Feature**: `041-project-coding-workspace` | **Date**: 2026-10-31

Each decision lists what was chosen, why, and what was rejected. Nothing in the Technical
Context is left as NEEDS CLARIFICATION.

---

## R1. Channel identity: a `Project` variant, keyed by person and project

**Decision**: `VizierChannelId::Project(String /* username */, String /* project id */)`, slug
`project__{username}__{project_id}`. A worktree is **not** part of the channel: it is bound to a
topic (R3). Clients keep addressing a project chat with `channel_id = "project__<id>"`;
`VizierChannelId::from_web(username, channel_id)` is the one function that turns a web
`channel_id` into a channel (project prefix → `Project`, otherwise `HTTP`), replacing the
hand-built `HTTP(username, channel_id)` in `topic.rs`, `service/topics.rs` and
`service/projects.rs`. `VizierChannelId::web_user() -> Option<&str>` returns the username for
`HTTP` and `Project`; every place that today destructures `HTTP(user, _)` to mean "the person in
this WebUI chat" (notifications `chat_reply`, client tools and deferral, side chat, `process.rs`,
`service/agents.rs`, `channel.rs`'s action username) uses it instead.

**Rationale**: FR-014 asks for recognition without names. The compiler lists every exhaustive
`match` on `VizierChannelId` (`usage_place`, `usage_activity`, `to_slug`, `platform`,
`is_scheduled_task` …), so each gets a deliberate answer; `web_user()` keeps the "a person's
WebUI chat" behaviours shared rather than duplicated (Principle II). Keeping the worktree out of
the channel avoids a second slug shape and a second migration if worktree identity ever changes.

**Alternatives considered**: (a) keep 040's `HTTP(user, "project__<id>")` — rejected, see
Complexity Tracking. (b) `Project { user, project_id, worktree: Option<String> }` — rejected:
one worktree is one chat, so the topic already identifies it; a worktree field would duplicate
that and change the slug of a chat when a worktree is deleted. (c) A new wire `channel_id`
(e.g. `project:<id>`) — rejected: the WebUI's URLs, socket paths and stored hrefs would all
change for no user benefit.

## R2. Migrating 040 data

**Decision**: `projects::migrate::migrate_project_channels(conn)` runs once from
`dependencies.rs` (beside `migrate_task_requester`) under the state key
`migration.project_channel_v1`, in one transaction:

1. Find every distinct old slug `http__{user}__project__{id}` in `session_history.channel`,
   `session_detail.channel`, `session_turn.channel` (the only places a chat can begin).
2. For each, compute the new slug `project__{user}__{id}` (the old-shape parser moves here from
   `projects/channel.rs` and is used nowhere else).
3. Rewrite exact-match columns: `session_history.channel`, `session_detail.channel`,
   `session_turn.channel`, `scratch_file.channel`, `client_tool_call.channel`,
   `artifact.origin_channel`, `artifact_event.source_channel`, `artifact_reference.channel`,
   `usage_event.channel`, `background_job.origin_channel`, `background_piece.session_channel`.
4. Rewrite composite columns with `REPLACE(col, old, new)` where `col LIKE '%' || old || '%'`:
   `session_detail.id`, `session_file.session_slug`, `usage_event.place` (also `http:` → `project:`
   prefix and `activity` → `project` for those rows), and dream conversations whose slug embeds
   the session (`DREAM__{agent}__{old}__…` in `session_history.channel`/`session_detail.channel`).
5. Rewrite serialised channels in JSON: `session_detail.data.channel` and
   `session_turn.request` (`session.1`), from `{"HTTP":[user,"project__<id>"]}` to
   `{"Project":[user,"<id>"]}`, by loading matching rows into `serde_json::Value` and saving
   them back.

The full list is generated during implementation by grepping every `channel`/`slug` column in
`init_*_schema` (the contract lists the expected set; a test builds a 040-shaped database with a
row in each and asserts nothing old remains). Notification `href`s already use the project route
and are untouched. An entry is added to #82 naming the migration and its marker.

**Rationale**: queries match on slugs (`conversation_rows`, `project_sessions`, scratch keys),
so a deserialize-time alias cannot carry history over; doing it once at startup, in one
transaction with a marker, matches every other data migration in `dependencies.rs`.

**Alternatives considered**: a lenient `Deserialize` that maps the old JSON to `Project` —
rejected as insufficient on its own (slug columns) and redundant once the JSON is rewritten.

## R3. Worktree identity and the 1:1 chat

**Decision**: a `project_worktree` row binds `(project_id, topic)` uniquely and stores the owner,
name, branch, base, canonical path and creation time. The chat is an ordinary project chat
(`Project(owner, project_id)`, that topic). `context::for_session` looks up the worktree by
`(project_id, topic)`; when present, `ProjectTurnContext.cwd` is the worktree path, otherwise the
project folder.

**Rationale**: one worktree is one chat (spec, US3); the topic is already unique and immutable.
Deleting the worktree deletes the chat (FR-030) through the existing topic deletion, so history,
files, turns, jobs and reactions are cleaned by code that already exists.

**Alternatives considered**: a separate channel per worktree (R1 b); worktree rows keyed by
name with the chat looking it up by name — rejected, names are for people and the path is
derived from them only at creation.

## R4. Where the mode lives

**Decision**: `project_chat (agent_id, channel, topic, mode, updated_at)`, primary key
`(agent_id, channel, topic)`; no row means `plan`. `PUT …/sessions/{topic}/mode` upserts it.
`context::for_session` reads it with the project, so the mode is fixed for the turn.

**Rationale**: FR-009/FR-013 need it per chat, saved, and visible to resumed and woken turns,
which start from storage, not from a client. A table rather than a field in
`session_detail.data`, because `refresh_session_detail` rewrites that JSON wholesale (title
generation) and would race a mode change.

**Alternatives considered**: a per-request field from the client — rejected: a background
report or resumed turn has no client; per-project mode — rejected by the spec (two chats keep
their own modes).

## R5. Walking and searching files: the `ignore` crate

**Decision**: add `ignore` (workspace dependency, default features). `WalkBuilder` with
`hidden(false)`, `git_ignore(true)`, `git_exclude(true)`, `git_global(true)`, `parents(true)`,
`require_git(false)` (so `.gitignore` applies even outside a repository), plus a filter that skips
`.git/` and, from the main folder, `.vizier/`. `grep` reads each walked file (skipping binaries,
R7) and matches lines with `regex::Regex` (already a dependency); `glob` uses
`ignore::overrides::OverrideBuilder` (globset, pulled in by `ignore`). Walks run in
`spawn_blocking`.

**Rationale**: gitignore semantics are a correctness requirement (FR-005, FR-017) and the crate
is the de facto implementation; its transitive crates (`globset`, `regex-automata`, `memchr`,
`walkdir`, `same-file`, `crossbeam-deque`) are mostly already in the lockfile. Pure Rust, so
`cross`/musl targets are unaffected.

**Alternatives considered**: `walkdir` + hand-rolled gitignore (rejected, Complexity Tracking);
`git ls-files` (needs git, FR-024); `grep-searcher` (ripgrep's searcher, faster on huge trees,
but a second crate for a 10k-file scale where line-by-line `regex` is adequate).

## R6. Path confinement

**Decision**: one function, `projects::path::resolve(cwd: &Path, rel: &str, purpose) ->
Result<PathBuf>`:

- empty → `cwd`; absolute → `path_outside_project`;
- join, then canonicalise the deepest existing ancestor and re-append the missing tail
  (rejecting `..` in the tail), so new files can be created but a symlink anywhere on the way
  that leads out is caught;
- result must `starts_with(cwd)`; when `cwd` is the project root, a result under
  `{root}/.vizier/` is `path_in_worktree_area`;
- `purpose` = `Existing` (must exist) or `Create` (parent folders may be created).

The 040 shell `resolve_workdir` becomes `resolve(cwd, workdir, Existing)` plus a directory check.

**Rationale**: FR-006/FR-028 and the edge cases (symlinks out, `.vizier/` from the main folder)
in one tested place.

## R7. Read, write and edit semantics

**Decision**:

- **Read**: `start_line`/`end_line` are 1-based and inclusive; defaults 1 and
  `start + 1999`. Output `{path, start_line, end_line, total_lines, content, truncated,
  next_start_line}` where `content` is `"{n:>6}\t{line}"` per line, each line cut at 2,000
  characters, and the whole result capped at 256 KiB. A file whose first 8 KiB contains a NUL
  byte is `{binary: true, size}` with no content.
- **Write**: `{path, content}`; creates parent folders (inside `cwd`); refuses over 4 MiB;
  an existing file requires a ledger entry matching its current hash (R8).
- **Edit**: `{path, old_text, new_text, replace_all?}`; `old_text` non-empty and different from
  `new_text`. If the file contains `\r\n`, `\n` in both texts is translated to `\r\n` before
  matching. 0 matches → `edit_not_found`; >1 without `replace_all` → `edit_ambiguous` with the
  count and the line numbers of the first five. Returns `{path, replacements, first_line}`.
- **Atomicity**: write to `.{name}.vizier-tmp-{nanoid}` in the same folder, copy the original's
  permissions, `rename` over it. A refusal never touches the file (SC-002).
- No tool writes by line range (FR-002).

**Rationale**: matches the spec's acceptance scenarios; exact text cannot drift the way line
numbers do; temp-then-rename gives all-or-nothing writes on every platform.

## R8. The stale-read ledger

**Decision**: `ProjectReadLedger` on `VizierDependencies`: a `parking_lot::Mutex<HashMap<(String
/* session slug */, PathBuf), [u8; 32]>>`. `read_file` records sha256 of the whole file (also for
partial reads); `write_file`/`edit_file` check then record the new hash. Entries for a session
are dropped when the session is deleted; at most 1,024 entries per session (oldest dropped). It is
in memory: after a restart the agent must read again before editing, which a resumed turn handles
by reading (the refusal says so).

**Rationale**: FR-004's "changed on disk since the agent last read it in that chat" with no
schema; the cost of a restart is one extra read.

**Alternatives considered**: mtime+size (cheap but misses same-size edits within the mtime
granularity); storing hashes in SQLite (durability nobody needs).

## R9. Bounded results for listings and searches

**Decision**: `list_files {path?, depth? = 3 (max 10), include_ignored? = false}` → up to 1,000
entries `{path, kind: file|dir, size}`, sorted, `truncated`. `glob {pattern, path?}` → up to
1,000 paths. `grep {pattern, path?, glob?, case_insensitive?, context? (0–5), max_results? = 200
(max 1,000)}` → `{matches: [{path, line, text, before, after}], files_searched, truncated}`;
lines cut at 500 characters; files over 4 MiB skipped and counted. `include_ignored` lets the
agent reach ignored files deliberately ("unless the agent asks for them by path", US1-6) — and
reading by explicit path never consults ignore rules.

## R10. Plan and Build, including `propose_plan`

**Decision**:

| Access | Tools | Plan | Build |
|---|---|---|---|
| Read | `read_file`, `list_files`, `grep`, `glob`, `git_status`, `git_diff`, `git_log` | ✓ | ✓ |
| Plan | `propose_plan` | ✓ | — |
| Write | `write_file`, `edit_file` | — | ✓ |
| Command | `workspace_shell` | — | ✓ if shell switch on |

`propose_plan {title, summary?, steps: [string]}` ends the turn (`ends_turn() == true`, spec 026):
its reply text is the step's own text, and the WebUI draws the call from the `execute_python`
report (the invocation record carries `name` and `arguments`). Choosing **Build this plan** sets
the mode to `build` and then sends a chat message ("Build the plan.") in the same chat. The
system prompt's project section states the mode, the folder the agent is in (worktree or main)
and, in Plan, that it should finish with `propose_plan`.

**Rationale**: reuses turn-ending tools rather than inventing a plan message type; Plan has no
write path at all, which is what SC-003 checks.

**Alternatives considered**: a WebUI-declared client tool for the plan (rejected: plans must also
work from a resumed turn with no socket); a read-only shell allowlist in Plan (rejected: an
allowlisted command line is a bypass waiting to happen; the git read tools cover planning's
needs).

## R11. Subagents inherit the project; other contexts do not

**Decision**: a subagent piece launched from a project chat carries the project with its request:

```rust
/// The project a subagent piece works in, copied from the launching turn (FR-007a).
pub struct ProjectScope {
    pub project_id: String,
    pub owner: String,          // the launching chat's username
    pub topic: String,          // the launching chat's topic: resolves its worktree
    pub mode: ProjectMode,      // the launching turn's mode, fixed for the piece
}
// VizierRequest
#[serde(default, skip_serializing_if = "Option::is_none")]
pub project_scope: Option<ProjectScope>,
```

- `BackgroundJobs::dispatch_piece` takes `ctx.project` and sets `project_scope` on the piece's
  request; this covers `background_subagents` and `parallel_subagents` (both dispatch through it).
  It is serialised, so a durable background piece resumed after a restart keeps it.
- `projects::context::for_turn(deps, session, request)` replaces `for_session`: a `Project`
  session resolves as before (mode from `project_chat`); a `Subagent` session with
  `project_scope` resolves the same project, the scope's topic's worktree as `cwd`, the scope's
  mode, and re-checks that `owner` may still use the project (a revoked or removed project gives
  the piece no project). Any other session ignores the field.
- Pieces never get `propose_plan` (`Access::Plan` is filtered out when the session is not a
  `Project` chat): a piece reports to its launcher, it does not end a person's planning turn.
- Mode is the launching turn's, not the chat's current one: a Plan turn's pieces are read-only
  even if the person switches to Build while they run, matching "a mode change applies from the
  next turn".
- The stale-read ledger is per session, so each piece reads before it edits, and two pieces
  editing one file cannot overwrite each other unseen.
- Not inherited: `delegate_agent`/`consult_agent` (another agent; a project belongs to one agent),
  scheduled tasks, dreams. A `BackgroundReport` woken into the project chat is a turn of that
  chat and uses its saved mode.
- `dispatch_piece` is the only producer of `project_scope`; clients cannot send it (the socket
  never deserialises a `VizierRequest` from a client).

**Rationale**: splitting work across subagents is most useful exactly in coding (search one area
each, edit separate files); without the tools a piece can only reason about text it was handed.
Carrying the scope on the request (like `background_depth`) instead of on the `Subagent` channel
keeps subagent sessions as they are.

**Alternatives considered**: a `Subagent(project)` channel variant (rejected: changes every
subagent slug for one optional field); re-reading the chat's saved mode for the piece (rejected:
a switch mid-turn would change a running piece's powers).

## R12. Git runner and parsing

**Decision**: `projects::git::Git { dir }` with `async fn run(&self, args: &[&str]) ->
Result<Output>`: `tokio::process::Command::new("git")`, `-C dir`, `-c core.quotepath=off`,
`-c color.ui=false`, env `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` (reads),
`LC_ALL=C`, 15 s timeout with kill. Probes: `git --version` once per process (cached);
`rev-parse --show-toplevel --git-common-dir --abbrev-ref HEAD` per call. Reads:

- status: `status --porcelain=v2 -z --branch --untracked-files=all` (parsed into
  branch/ahead/behind/entries);
- counts: `diff HEAD --numstat -z` (+ untracked files' line counts, files over 4 MiB as `-`);
- file diff: `diff HEAD -- <path>`; untracked: a synthetic all-added unified diff from the file;
  output capped at 1 MiB with `truncated`;
- log (agent tool): `log -n <n ≤ 50> --format=%H%x1f%an%x1f%aI%x1f%s -z [-- <path>]`;
- branches: `for-each-ref refs/heads --format=%(refname:short)%1f%(objectname:short)%1f%(upstream:short)`,
  plus `worktree list --porcelain` to mark branches checked out elsewhere.

Writes: `switch <branch>`, `switch -c <name>`, `worktree add -b <branch> <path> <base>`,
`worktree remove [--force] <path>`, `worktree prune`, `branch -d|-D <name>`,
`merge-base --is-ancestor <branch> <base>`. Git's own stderr is mapped to plain messages for the
known cases (local changes would be overwritten, branch exists, branch checked out elsewhere,
not a repository) and passed through otherwise.

**Rationale**: no git library is in the tree (`git2` needs libgit2 and OpenSSL on some targets,
`gix` is heavy); the CLI is what the person already uses, so behaviour matches their terminal.
Argv only, never a shell, so names cannot inject.

## R13. Worktree lifecycle

**Decision**:

- **Create** (`POST …/worktrees {name, base, branch?}`): name matches
  `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, not ending in `.lock`, no `..`; branch defaults to name and
  must pass `git check-ref-format --branch`; unique per project; path
  `{root}/.vizier/{agent_id}/{name}` must not exist. Ensure `/.vizier/` is a line in
  `<git-common-dir>/info/exclude`. Run `worktree add`. Insert the row and a `session_detail`
  (title = name) in one storage call; on a storage failure, `worktree remove --force` the folder.
  Respond with the topic. Mode starts `plan`.
- **Delete** (`DELETE …/worktrees/{id}?force=&delete_branch=`): refused with `turn_active` when
  the topic has a pending `session_turn`; when the folder exists and status is not clean and
  `force` is false → `409 worktree_dirty {changed, untracked}`; else `worktree remove [--force]`,
  `branch -d` (or `-D` when `force`) if requested, `worktree prune`, then the existing topic
  deletion and the row. A missing folder skips the status check and runs `prune`.
- **Deleting the chat** through the ordinary conversation delete route delegates here (same
  checks), so there is one path.
- **Detail** (`GET …/worktrees/{id}`) returns `available`, `changed`, `untracked` and
  `branch_merged` for the confirmation dialog (FR-030/FR-031).
- **Project removal** leaves worktrees (rows and folders) untouched; reopening the folder brings
  them back with the project's chats (040 soft removal).

## R14. Branch switching guard

**Decision**: a switch in the main folder is refused with `turn_active` when any `session_turn`
row exists for a `project__*__{project_id}` channel whose topic is not bound to a worktree.
A switch inside a worktree chat checks only that chat's topic. Branches checked out in another
worktree are returned with `checked_out_in` and refused if requested. Switching the main folder
requires `can_manage` (FR-034); switching inside one's own worktree requires `can_use`.

## R15. Mentions

**Decision**: the socket `chat` frame accepts `mentions: [string]` (≤ 10, each ≤ 1,024 bytes,
relative). They ride `VizierRequest.mentions` (`#[serde(default, skip_serializing_if =
"Vec::is_empty")]`) and are stored as `metadata.mentions` on the request's history entry (like
`invoked_skill`). At turn start, `projects::mentions::section(ctx, &mentions)` resolves each
through `path::resolve(cwd, Existing)`: a file within 64 KiB as a numbered block, larger as a
"too large, use read_file" line, a folder as up to 200 shallow entries, a miss as "missing"; the
whole section capped at 256 KiB. It goes into `context_md` as `## Mentioned files`. A successful
mention also records the file in the ledger (the agent has, in effect, read it).

Suggestions: `GET …/projects/{pid}/files?q=&topic=&limit=20` walks the chat's `cwd` (R5 rules),
caches the path list for 10 s per folder (bounded to 50,000 entries), and ranks with a
hand-rolled subsequence scorer (consecutive and word-boundary bonuses, basename bonus, shorter
path wins ties). Folders are included with a trailing `/`.

**Rationale**: contents are fresh at turn time (spec US5-2) and never stored (FR-019); the
walk-and-rank fits SC-008 at 10k files without a fuzzy crate (the one in the lockfile is the
CLI's `inquire` dependency, not core's).

## R16. Access

**Decision**: all new routes call `projects::access` as 040 does. Use (`can_use`): mode, file
suggestions, git reads, switching branches inside one's own worktree, creating worktrees, and
deleting one's own worktree chat. Manage (`can_manage`, owner or all-agents permission):
switching the main folder's branch and deleting another person's worktree chat. Every route is
also re-checked on the project's availability. Creation of a *project* stays desktop-only (040);
creating a worktree inside an existing project works from the browser too, since it touches only
an already-trusted folder.
