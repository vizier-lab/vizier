# Contract: HTTP API

**Feature**: `041-project-coding-workspace`

All routes are under `/api/v1/agents/{agent_id}/projects/{project_id}`, behind `require_auth`,
use the existing `APIResponse` envelope, and put a stable `code` in `data.error` on refusal (040's
`refusal`). Authorization is in `service/projects.rs` through `projects::access` (R16).
`{topic}` selects the chat (and so its `cwd`) wherever the folder matters; without it, the project
root is used.

Common refusals: `404 project_not_found`, `403 forbidden`, `409 project_unavailable` (folder
missing), `409 git_unavailable` (git missing or not a repository), `409 turn_active`.

---

## 1. Sessions (changed)

`GET …/sessions` → each item gains:

```json
{ "topic": "t1", "title": "Add --json", "is_thinking": false, "last_activity": 1730000000000,
  "mode": "plan", "worktree": { "id": "w8Kd", "name": "agent-ps-json", "branch": "agent-ps-json", "available": true } }
```

## 2. Mode

`PUT …/sessions/{topic}/mode` — body `{ "mode": "plan" | "build" }` → `200 { "mode": "build" }`.
`can_use` and the chat must be the caller's. Effective from the next turn; no restart, no abort.

## 3. File suggestions

`GET …/files?q=age&topic=t1&limit=20` → `200 [{ "path": "crates/vizier/src/cli/agent.rs", "kind": "file" }, { "path": "crates/vizier-core/src/agents/", "kind": "dir" }]`

`limit` 1–50 (default 20). Empty `q` returns the shallowest entries. Ignored files excluded;
`.vizier/` excluded from the project root.

## 4. Git

| Method | Path | Body / query | Response | Access |
|---|---|---|---|---|
| GET | `…/git?topic=` | | `GitSummary` | use |
| GET | `…/git/changes?topic=` | | `[GitChange]` | use |
| GET | `…/git/diff?topic=&path=` | | `GitFileDiff` | use |
| GET | `…/git/branches?topic=` | | `[GitBranch]` | use |
| POST | `…/git/switch` | `{ topic?, branch, create?: bool }` | `GitSummary` | manage (root) / use (own worktree) |

```json
// GitSummary
{ "available": true, "branch": "041-coding", "detached": false, "ahead": 2, "behind": 0, "changed": 3 }
// GitChange
{ "path": "src/cli/agent.rs", "old_path": null, "kind": "modified", "added": 14, "removed": 2 }
// GitFileDiff
{ "path": "src/cli/agent.rs", "diff": "@@ -41,6 +41,9 @@\n…", "truncated": false, "binary": false }
// GitBranch
{ "name": "master", "current": false, "checked_out_in": { "worktree_id": "w8Kd", "name": "agent-ps-json" } }
```

`GET …/git` with git unavailable returns `200 { "available": false }` (the WebUI hides git
controls) rather than an error; the other git routes return `409 git_unavailable`.

Switch refusals: `409 turn_active`; `409 branch_checked_out` (in another worktree);
`409 local_changes` ("Your changes to N files would be overwritten. Commit or stash them first.");
`409 branch_exists` (create); `422 invalid_branch`.

## 5. Worktrees

| Method | Path | Body / query | Response |
|---|---|---|---|
| GET | `…/worktrees` | | `[WorktreeSummary]` |
| POST | `…/worktrees` | `{ name, base, branch? }` | `201 CreateWorktreeResponse` |
| GET | `…/worktrees/{id}` | | `WorktreeDetail` |
| DELETE | `…/worktrees/{id}` | `?force=false&delete_branch=false` | `200 { "deleted": true, "branch_deleted": false }` |

```json
// CreateWorktreeResponse
{ "worktree": { "id": "w8Kd", "name": "agent-ps-json", "branch": "agent-ps-json", "available": true },
  "topic": "Tq2…", "channel_id": "project__p3x" }
// WorktreeDetail
{ "id": "w8Kd", "name": "agent-ps-json", "branch": "agent-ps-json", "available": true,
  "base_ref": "master", "path": "/Users/a/vizier/.vizier/coder/agent-ps-json",
  "created_at": 1730000000000, "changed": 3, "untracked": 1, "branch_merged": false }
```

Create: `can_use`; refusals `422 invalid_name`, `422 invalid_branch`, `409 name_in_use`,
`409 branch_exists`, `409 path_exists`, `422 unknown_base`, `409 git_unavailable`.

Delete: owner of the chat with `can_use`, or `can_manage`; refusals `409 turn_active`,
`409 worktree_dirty { changed, untracked }` when not `force`. `delete_branch` uses `git branch -d`,
or `-D` with `force`; a branch that cannot be deleted leaves `branch_deleted: false` and the rest
succeeds.

The existing conversation delete route, given a worktree chat's topic, delegates to this delete
with the same checks (a `worktree_dirty` refusal surfaces there too).

## 6. Socket

The topic socket's `chat` frame accepts `mentions: [string]` (≤ 10, relative, ≤ 1,024 bytes each)
on project channels; elsewhere the field is ignored. Invalid entries are refused with
`client_error { code: "invalid_mentions" }` before a turn starts.
