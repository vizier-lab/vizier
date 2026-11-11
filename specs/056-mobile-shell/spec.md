# Feature Specification: The Phone Shell

**Feature Branch**: `056-mobile-shell`

**Created**: 2026-11-11

**Status**: Implemented

**Input**: Issue #152 — the phone layout, which `055-pwa-webui` left out of scope. The design is the
`Shell · mobile` page of `Vizier shell.figcraft`, with its primitives on `Components · Mobile`.

## Context

The installable app (055) puts Vizier on a phone's home screen, but the phone layout under it is
the desktop shell folded up: a top bar whose ☰ opens a drawer holding the whole rail and the
whole agent column (essentials, projects, every conversation), a topic dropdown hung under the
bar, and a Home that is a greeting, an activity panel and a bordered list.

The design separates what belongs to the **app** from what belongs to the **agent**:

- The top bar carries the agent. Its body switches agents; a button on its side opens the
  agent's details. The bell stays at the right.
- Everything about one agent (Core, Memory, Tasks, Artifacts, Skills, Dreams, Channels, Addons,
  MCP, Usage, Secrets, Config) lives on one **Agent details** page, reached from that button.
- The menu drawer holds the app only: Chat, Home, Settings, Theme.
- Choosing an agent and choosing a topic are bottom sheets, not dropdowns.
- There is no bottom bar, and no control in the top bar has a border.

The desktop layout does not change.

## Scope

- The phone top bar: menu, the agent (sigil, name, status, ˅), the details button, the bell; a
  second row with the topic and its meta on chat screens; a title mode for other screens.
- A bottom sheet in `design/`, used for the agent switcher and the topic picker.
- The agent switcher sheet: agents, then workspaces, each row with its own details button.
- The Agent details route, `/:agentId/details`.
- A back bar on every agent page on a phone, returning to Agent details.
- The menu drawer: Chat, Home, Settings, Theme, and the person (profile, sign out).
- Home on a phone: an AGENTS list and a WORKSPACES list, each with a + beside its heading.
- Safe-area insets for the top bar and the composer.

## Out of scope

- A cross-agent Tasks page. The API lists tasks per agent only (`/agents/{agent_id}/tasks`), and
  the drawer does not carry Tasks: a person reaches them through Agent details. Decided
  2026-11-11.
- The desktop layout, the rail and the agent column.
- A new Memory view. The mockup's Memory frame (pinned / recalled) illustrates the back bar; the
  phone keeps the real memory explorer under that bar.
- Counts the server does not already return cheaply. A row shows a count only when the data is
  already loaded for the page (see FR-009).

## User Scenarios & Testing

### User Story 1 - A person reaches an agent's settings from the top bar (Priority: P1)

On a phone, a person in a conversation wants to look at the agent's memory. They press the
button beside the agent's name and land on its details, then on Memory, and back.

**Why this priority**: it is what the redesign is for. Today the same trip is ☰, then a scroll
past every conversation to the agent card, then its menu.

**Independent Test**: at a phone width, open `/{agent}/chat`, press "Agent details", select
Memory, press back.

**Acceptance Scenarios**:

1. **Given** a chat on a phone, **When** the person presses the details button in the top bar,
   **Then** `/{agent}/details` opens with the agent's name, status and every page the person may
   see.
2. **Given** Agent details, **When** the person selects a row, **Then** that page opens with a
   back bar, and back returns to Agent details.
3. **Given** a person without `canManageSecrets` for the agent, **When** Agent details renders,
   **Then** there is no Secrets row; without `canEditAgent`, Config is shown disabled; without
   `canViewArtifacts`, there is no Artifacts row. The same rules as the desktop `AgentMenu`.
4. **Given** a desktop width, **When** `/{agent}/details` is opened, **Then** it redirects to
   `/{agent}/config`, the desktop's nearest page.

---

### User Story 2 - A person switches agent from the top bar (Priority: P1)

**Independent Test**: at a phone width, press the agent's name in the top bar; a sheet lists
the agents and the workspaces; select one.

**Acceptance Scenarios**:

1. **Given** the top bar, **When** the person presses the agent's name, **Then** a bottom sheet
   lists AGENTS then WORKSPACES, the current agent marked, and nothing else (no "New agent").
2. **Given** the sheet, **When** the person presses a row's details button, **Then**
   `/{that agent}/details` opens without switching the chat first.
3. **Given** the sheet, **When** the person presses Escape, the scrim or the grab handle's
   close, **Then** it closes (the dismissal convention).

---

### User Story 3 - Home is the court and the rooms (Priority: P2)

**Independent Test**: at a phone width, open `/`.

**Acceptance Scenarios**:

1. **Given** Home on a phone, **Then** it shows a top bar titled Vizier, an AGENTS list and a
   WORKSPACES list, with no greeting, no activity panel and no borders around rows.
2. **Given** a person with `all_agents:create`, **Then** AGENTS has a + that opens New agent;
   with `workspaces:create`, WORKSPACES has a + that opens New workspace; without the permission
   there is no +.
3. **Given** no workspaces, **Then** the WORKSPACES heading still shows with its + when allowed,
   and an empty line says there are none; without the permission the section is hidden.

---

### User Story 4 - The menu is the app (Priority: P2)

**Acceptance Scenarios**:

1. **Given** any phone screen, **When** the person presses ☰, **Then** a left drawer lists
   Chat, Home, Settings and Theme, and the person's own row with profile and sign out (hidden in
   the desktop app, as today).
2. **Given** the drawer, **Then** the current place is marked, and choosing a row closes it.

---

### User Story 5 - Topics are a sheet (Priority: P3)

**Acceptance Scenarios**:

1. **Given** a chat, **When** the person presses the topic in the top bar's second row,
   **Then** a bottom sheet lists the conversations grouped by day, with the projects section,
   search, "new topic", rename and delete, as the agent column does today.

## Requirements

- **FR-001** The phone top bar has no border on any control and no rule under the bar. Every
  control keeps a 44px target.
- **FR-002** The notification badge sits on the bell's corner with a ring in the ground colour.
- **FR-003** The top bar pads by `env(safe-area-inset-top)`; the composer and every sheet pad by
  `env(safe-area-inset-bottom)`. No constants.
- **FR-004** The bottom sheet traps focus, closes on Escape, a scrim press and a visible close,
  and returns focus to its trigger.
- **FR-005** The agent shown in the bar is the route's agent, else the last one the person was
  on (the drawer's existing `drawerAgent` rule), else the bar is in title mode.
- **FR-006** Agent details rows are derived from the same page sets and permission rules as
  `AgentMenu`/`ContextSidebar`, in one pure function with tests, so the two cannot drift.
- **FR-007** Agent pages on a phone show a back bar (back, title, the agent's name) instead of
  ☰. Back goes to `/{agent}/details`, not `history.back()`, so a page opened from a link still
  has a way up.
- **FR-008** The topic picker keeps everything the agent column offers today (projects, search,
  paging, rename, delete, unread marks, live dots).
- **FR-009** A count is shown on an Agent details row only when the data is already in a store
  (open tasks, integrations); a row never waits on a request to render.
- **FR-010** No horizontal scroll at 360px on any route, including `/{agent}/details`.

## Decisions

1. **Tasks live in Agent details, not the drawer.** The API is per agent; a cross-agent list
   would need a new endpoint or one request per agent. Decided by the maintainer, 2026-11-11.
2. **Home follows the mockup**: the greeting and the activity panel leave the phone Home. The
   desktop Home keeps both. Decided by the maintainer, 2026-11-11.
3. **Agent details is a route, not a sheet**, so the phone's back gesture and a shared link both
   work.
4. **The bottom sheet is a `SlideOver` side**, not a new component: one dismissal and focus
   implementation.

## The branches

| Repo | What changes |
|---|---|
| `vizier-lab/design` | `MobileTopBar` rework, `SlideOver side="bottom"`, the `sliders` icon, the safe-area utilities, the switcher row, the Agent details list, the section heading with an action. |
| `vizier-lab/webui` | The phone branch of `AppShell`, the drawer, the switcher and topic sheets, `/:agentId/details`, the back bar, `MobileHome`, tests. |
| `vizier-lab/vizier` | This spec, the pin bumps, the phone-layout lines of `AGENTS.md`. |

Commit order: design → webui → vizier.

## Verification

- `design`: a story per component, both themes, at 360 and 402 wide.
- `webui`: `npm run typecheck`, `npm test`, `npm run build`, `npm run e2e` (the mobile projects).
- `vizier`: `cargo build -p vizier-http` with the new WebUI embedded.
