# Contract: WebUI

**Feature**: `041-project-coding-workspace`

Every new surface below is **drawn in FigCraft first** (page `- App`, the project chat screen, in
both themes), then built in `design/` with stories, then used in `webui/` (CLAUDE.md "Design
first"; commit order design → webui → vizier). Popups follow the dismissal convention (visible
close, Escape, outside press). The sketches made during brainstorming are reference only.

---

## 1. Surfaces

| Surface | Where | Design component (to confirm in FigCraft) |
|---|---|---|
| Mode toggle (Plan / Build) | Composer, left of send, project chats only | segmented control |
| Mode hint | beside the toggle: "Read-only: can look, can't change anything" / "Can edit files and run commands" (or "…; commands are off") | text |
| Plan card | in the reply bubble, from a `propose_plan` call | `PlanCard`: title, summary, numbered steps, **Build this plan**, **Keep planning** |
| Branch chip + menu | chat header, git available only | chip with branch and changed count; menu of branches, "in worktree X" marks, **New branch from …** |
| Changes panel | chat details side panel, new section | file rows with `+a −r`, select → `DiffView` |
| Diff view | in the changes panel (and a wide dialog on small screens) | unified diff, line numbers, added/removed tints, "truncated" and "binary" states |
| New worktree dialog | project row `+` menu: **New worktree** | name, start from (branch select), branch (defaults to name), path preview `.vizier/<agent>/<name>` |
| Delete worktree confirmation | chat menu **Delete**, for worktree chats | clean: plain confirm; dirty: destructive, "N files changed, M untracked will be lost"; checkbox **Also delete branch** (checked when merged) |
| Sidebar grouping | Projects section | project → main chats; then each worktree chat with branch icon, branch name, change dot, unavailable state |
| Mention menu | Composer, after `@` | list of paths (file/folder icon, matched characters emphasised), keyboard navigation; picked mention shown as a chip in the input |

## 2. Behaviour

- **Mode**: read from `ProjectSession.mode`; toggling calls `PUT …/mode` and updates the store
  optimistically (reverted on error). While a turn runs, the toggle changes and shows "applies to
  the next message".
- **Build this plan**: `PUT …/mode {build}` then send "Build the plan." in the same chat; the
  card's buttons hide once a later message exists. **Keep planning** focuses the composer.
- **Plan card source**: the `execute_python` report's invocation record named `propose_plan`
  (`arguments`), drawn by a renderer registered beside the existing report rendering; no client
  tool is declared for it.
- **Git**: `GET …/git` on chat open and after each turn ends (`history_entry` of the final
  response) and every 15 s while visible; `available: false` hides the chip and the changes
  section. A switch shows git's refusal message in a toast.
- **Worktree create**: on success, navigate to `/<agent>/projects/<project>/chat/<topic>`.
  Creation shows progress (it can take seconds).
- **Worktree delete**: fetch `GET …/worktrees/{id}` for the confirmation's counts; on
  `worktree_dirty` (counts changed meanwhile) re-show the destructive form.
- **Mentions**: typing `@` + text queries `GET …/files` (debounced 120 ms, stale responses
  dropped). Enter/Tab picks; Escape closes; a pick inserts `@path ` and records it; on send the
  frame carries `mentions` for chips still present in the text. History shows chips from
  `metadata.mentions`.
- **Browser users**: everything works through web access; the 040 host-execution notice stays.

## 3. Transport seam

All calls go through `webui/app/api/transport/` (no direct `fetch`); new endpoint functions live
in `api/endpoints/projects.ts`; `schema.d.ts` is regenerated from OpenAPI.
