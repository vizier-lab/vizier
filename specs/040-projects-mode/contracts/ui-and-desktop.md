# Contract: WebUI and Desktop Behavior

**Feature**: `040-projects-mode`

The WebUI is one application for both editions. Only the folder picker and the
creation affordance are desktop-specific; the server enforces desktop-only
creation.

---

## 1. Routes

| Route | Screen |
|-------|--------|
| `/:agentId/projects/:projectId/chat/:topicId` | Project session (same chat screen, project channel) |
| `/:agentId/projects/:projectId` | Redirects to the caller's most recent session, or creates one if none |
| `/:agentId/chat/:topicId` | Ordinary session (unchanged; channel `vizier-webui`) |

`routes.ts` registers the project route under the agent-scoped block.
`AppShell`'s agent resolution, `pages.ts` page parsing and the sidebar's
current-session match learn the `projects` segment. A project URL always
includes the agent id so every existing agent-scoped permission check still
applies.

"New conversation" in a project creates a session through
`POST …/projects/{id}/sessions` and navigates to the returned topic. The
ordinary "New conversation" is unchanged.

---

## 2. Sidebar: Projects above Sessions

`AgentSidebar` renders, in order:

```text
Agent card
[New]
Essentials
Projects section      ← new, above Sessions
Sessions section
```

**Projects section**

- Hidden entirely when the person lacks a project permission or the agent has
  no projects (FR-005).
- Header row: "Projects" and, on desktop when permitted, "Open a Project".
- One row per project: name, folder basename, availability dot, session count.
  Clicking expands the caller's sessions; the row itself does not navigate.
- Session row: title, live marker while `is_thinking`, and a new-session "+".
- Expanded state is local UI state (not persisted).
- Project menu (owner or `all_agents:projects` only): Rename, Toggle workspace
  shell, Remove. Remove asks for confirmation and explains that files are kept.
- An unavailable project shows a warning marker and its sessions remain
  clickable (history readable); starting a turn is refused by the server.

**Sessions section**

- Unchanged, and never contains project sessions (FR-006).
- When the last project is removed, the Projects section disappears on the
  next refresh.

Data comes from the `projects` store (`GET …/projects` and
`GET …/projects/{id}/sessions`), refreshed on navigation and every 15 s while
visible, matching the sessions hook. It keeps previous data on a failed poll.

The design-library equivalents are a `ProjectsSection`/`ProjectRow`
component under `design/src/components/Shell/`, with stories in
`Shell.stories.tsx` (Storybook runs axe in both themes).

---

## 3. Desktop creation and trust disclosure

```text
"Open a Project"
  → transport.pickFolder()                (desktop only)
  → trust dialog (danger tone)            FR-002
  → POST /agents/{id}/projects {folder}   (desktop principal)
  → POST /agents/{id}/projects/{id}/sessions
  → navigate /:agentId/projects/:id/chat/:topic
```

**Trust dialog** must state, before creation:

- commands run with the person's operating-system account;
- there is no sandbox;
- only trusted folders should be opened;
- the chosen absolute path, with the folder name.

Confirm is the only path that posts; cancel discards. A failed creation leaves
the dialog open with the server's message (invalid folder, workspace folder,
forbidden).

**Browser**

- No "Open a Project" action and no creation dialog.
- The Projects section lists existing projects for agents the person can view
  and permits opening sessions and running turns.
- A direct `POST` from the browser is refused with `403 desktop_only`; the UI
  also never sends one.

**Folder picker IPC**

- New Tauri command `desktop_pick_folder` using `tauri-plugin-dialog`'s
  blocking folder picker, registered in both debug and release handler lists.
- Exposed as `DesktopShell.pickFolder(): Promise<string | null>` through the
  transport seam (`common.ts`, `tauri.ts`; HTTP transport returns `null`).
- No new plugin, capability or JS dependency: `dialog:allow-open` already
  exists.

---

## 4. Project session screen

- The chat screen takes its channel from the route: `project__<projectId>` for
  project routes, `vizier-webui` otherwise. Topic list, socket open and history
  requests all pass that channel explicitly.
- Header meta shows the project name and folder; the project menu mirrors the
  sidebar.
- **Host execution notice**: when `transport.authMode !== 'desktop'`, a
  persistent `Notice`/badge in the header and above the composer reads
  "Commands run on the host machine." (FR-023). Desktop mode shows no such
  notice.
- An unavailable-folder banner says commands are refused and history remains
  readable.
- An agent with no projects and an empty desktop conversation offers
  "Open a Project" in the greeting area; a browser empty conversation offers
  nothing project-related.

The notice is a single shared component so every entry point (sidebar,
notification href, direct URL) shows it.

---

## 5. Permission-aware UI

- `lib/access.ts` gains `canUseProjects(permission list, agent)` and
  `canManageProject(project, permissions)`.
- The Projects section and creation affordance use them; the server is still
  the authority (direct URL/API is refused).
- `lib/permissions.ts` adds a "Projects" group with the two permission labels:
  - `all_agents:projects` — "Use and manage projects for every agent"
  - `owned_agents:projects` — "Use projects for agents they own"
- The role editor shows the group automatically from the catalogue.
- A person without the permission sees no section, no CTA and no project data;
  a direct project address renders the standard refused/empty state.

---

## 6. Notifications and navigation

- A project reply notifies the session owner and links to
  `/<agentId>/projects/<projectId>/chat/<topic>`.
- Selecting the notification opens the project session and marks it read as
  today.
- Sidebar "current session" highlighting matches the project route shape.
- Session search and the conversation picker never show project sessions.

---

## 7. Responsive and empty states

- Mobile: the Projects section lives in the same drawer/slide-over as Sessions;
  creation is desktop-only, so mobile web never offers it.
- No projects: section hidden; desktop empty chat offers "Open a Project";
  browser empty chat offers nothing.
- No sessions in a project: the project row shows "New session" and a
  create action.
- Removed project: disappears from the sidebar after refresh; its conversation
  history remains if it is reopened later.

---

## 8. Tests

- Vitest: projects store merge/refresh, permission helpers, route-to-channel
  derivation, host-notice visibility (`authMode` desktop vs http), and the
  creation flow's "desktop only" branch with a mocked transport.
- Design: `ProjectsSection` story in `Shell.stories.tsx`, both themes, axe
  clean.
- Playwright: no new desktop suite; existing HTTP e2e can assert the section
  is absent without permission. Desktop creation is covered by the Tauri smoke
  test instead.
