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
| Mode toggle (Plan / Build) | Composer, left of send, project chats only | `FilterTabs` used as a mode switch (no counts), as the library documents |
| Mode hint | beside the toggle: "Read-only: can look, can't change anything" / "Can edit files and run commands" (or "…; commands are off") | text |
| Plan card | in the reply bubble, from a `propose_plan` call | `PlanCard` (design library, X89): label, title, summary, numbered steps, and one action, **Build this plan** (a plan is rejected or revised by typing a new message) |
| Branch chip + menu | chat header, git available only; on a phone, in the Changes section of the details sheet | `git-branch` icon, branch and changed count; `Floating` menu of branches (a filter field past 8), "in <worktree>" marks and disabled rows, **New branch from <current>** opening an inline name field (no browser prompt) |
| Changes panel | first section of the chat details panel | `PanelHeader` ("Changes", file count), `EmptyState` when clean, file rows with a status letter and `+a −r`, the selected row highlighted and toggling its `DiffView` |
| Diff view | in the changes panel (and a wide dialog on small screens) | unified diff, line numbers, added/removed tints, "truncated" and "binary" states |
| New worktree dialog | project row `+` menu: **New worktree** | name, start from (branch select), branch (defaults to name), path preview `.vizier/<agent>/<name>` |
| Delete worktree confirmation | chat menu **Delete**, for worktree chats | clean: plain confirm; dirty: destructive, "N files changed, M untracked will be lost"; checkbox **Also delete branch** (checked when merged) |
| Sidebar grouping | Projects section | worktree chats carry a leading `git-fork` icon (red when unavailable) and show their branch beside the title only when it differs from it; **New worktree** (`git-fork`) is offered only for a git repository |
| Mention menu | Composer, after `@` | `MentionMenu` (design library, X88): folder part dimmed, name strong, match emphasised, file/folder icons; the composer's `suggest` prop owns ↑ ↓ Tab Enter Esc like the slash menu (Enter never sends while it is open). A sent message shows its recorded mentions as inline-code chips |

## 2. Behaviour

- **Mode**: read from `ProjectSession.mode`; toggling calls `PUT …/mode` and updates the store
  optimistically (reverted on error). While a turn runs, the toggle changes and shows "applies to
  the next message".
- **Build this plan**: `PUT …/mode {build}` then send "Build the plan." in the same chat; the
  card's action hides once a later message exists, and shows a loading state while the mode switches.
- **Plan card source**: the `execute_python` report's invocation record named `propose_plan`
  (`arguments`), drawn by a renderer registered beside the existing report rendering; no client
  tool is declared for it.
- **Git**: `GET …/git` on chat open and after each turn ends (`history_entry` of the final
  response) and every 15 s while visible; `available: false` hides the chip and the changes
  section. A switch shows git's refusal message in a toast.
- **Worktree create**: on success, navigate to `/<agent>/projects/<project>/chat/<topic>`.
  Creation shows progress (it can take seconds).
- **Worktree delete**: from the sidebar or the chat header, the same dialog; fetch `GET …/worktrees/{id}` for the confirmation's counts; on
  `worktree_dirty` (counts changed meanwhile) re-show the destructive form.
- **Mentions**: typing `@` + text queries `GET …/files` (debounced 120 ms, stale responses
  dropped). Enter/Tab picks; Escape closes; a pick inserts `@path ` and records it; on send the
  frame carries `mentions` for chips still present in the text. History shows chips from
  `metadata.mentions`.
- **Browser users**: everything works through web access; the 040 host-execution notice stays.

## 3. Transport seam

All calls go through `webui/app/api/transport/` (no direct `fetch`); new endpoint functions live
in `api/endpoints/projects.ts`; `schema.d.ts` is regenerated from OpenAPI.

## 4. Design-library changes made for this feature

`git-branch` and `git-fork` icons; `MentionMenu` (X88) and `PlanCard` (X89), both `needs-review`;
`Composer` gains a `suggest` prop (`ComposerSuggest`); `Dialog` gains `confirmDisabled` and
`confirmLoading`. FigCraft drawings for these surfaces were not made (the tool was unavailable);
they remain open as T043/T056/T068/T080 and should be drawn from the built components.
