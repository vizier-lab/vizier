# Feature Specification: WebUI Rewrite

**Feature Branch**: `024-webui-rewrite`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "we've prepared the webui write for this project, the design is in figcraft, as for the existing webui code base rename it into legacy, i'll delete it later. as for the new webui init a new submodule project i host it in git@github.com:vizier-lab/webui.git — for the rewrite itselves use the prepared design system in design submodule, you may add or modify the components if needed. there may be some new feature addition on the design, thus you may need also to adjust the rust codebase"

## Context

The "Celestial Court" redesign is finished as a design. Spec 022 turned its components into the design library (the `design` submodule), and that library's catalogue is the source of truth for every component. The screens themselves are drawn in the FigCraft file `Vizier.figcraft`, one page per screen:

- **App**: Login, Onboarding, Home, Chat, Core, Memory, Tasks, Skills, Dreams, Usage, Config, New agent
- **Integrations**: Channels, Addons, MCP
- **Settings**: Profile, API keys, Providers, Users, Roles
- **Reference**: States (shared progress, failure, empty, confirm and notify states), Mobile, Scaling (rules for every list, chart and graph at large sizes)

Each screen is drawn in a dark and a light version. Some pages add variants, such as Home on first run, Chat collapsed, and the skill editor.

This feature rebuilds the WebUI to that design. The new WebUI is its own repository, `vizier-lab/webui`, added to vizier as a submodule. The current WebUI code moves to a `legacy` folder and stays only as a reference until the maintainer deletes it. Vizier serves the new WebUI only.

Most of what the design shows is already in the server: agents, chat, CORE and memory history, tasks and runs, skills, dreams, usage, integrations, providers, users, roles and API keys. A few screens show things the server doesn't yet provide (listed under User Story 6), so the server changes as part of this feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Chat with an agent in the new WebUI (Priority: P1)

A person signs in, picks an agent and has a conversation with it on the redesigned Chat screen. They see the live working trail while it works, the reply with its stats and reactions, background jobs, recalled memory and session files, and every turn state the design draws: queued, stopped, checkpoint, woken by a background report, offline, and each kind of error.

**Why this priority**: chat is what people open the WebUI for. A rewrite that can't chat replaces nothing.

**Independent Test**: build vizier with the new WebUI, sign in and hold a multi-turn conversation that uses tools, a skill (`/name`), a queued message, Stop, a checkpoint and a background job. Check each state against the Chat page of the design, in both themes.

**Acceptance Scenarios**:

1. **Given** a signed-in person on an agent's Chat screen, **When** they send a message, **Then** Send turns into Stop, the live trail shows each tool call as it happens, and when the reply lands the trail folds into a one-line summary that still opens.
2. **Given** a turn is running, **When** the person sends another message, **Then** it shows as queued under the running turn and is sent when the turn ends. **When** they press Stop, **Then** the trail and every queued message are discarded and the topic is ready straight away.
3. **Given** the page is reopened, or a second tab is open, during a turn, **When** it loads, **Then** it shows the turn so far and carries on with the rest of it.
4. **Given** the server answers with an error (rate limited, provider unavailable, agent unavailable, tool timeout, turn timeout, unreadable reply), **When** it reaches the WebUI, **Then** a notice for that kind of error appears in place of the reply, with the action the design gives it (retry, switch model, open providers, open agent health).
5. **Given** a background job launched from the topic reports back, **When** the woken turn runs, **Then** its reply streams into the open topic, and the background-jobs panel shows the job's progress and result.
6. **Given** the person types `/` in the composer, **When** the skill picker opens, **Then** it lists built-in commands and the agent's skills, filtered as they type, with keyboard completion.
7. **Given** a reply that used memory, **When** the person opens the "Recalled memory" panel, **Then** it lists the passages the agent was given for that turn and the ones it searched for itself. Each shows document, passage, bundle and score, and opens the document at that passage. The trail summary counts them ("4 passages recalled").
8. **Given** a conversation where files were sent (by the person or the agent), **When** the person opens "Session files", **Then** it lists the newest 5 with name, type and size, then "Show all". **When** they open one, **Then** the preview shows who sent it and when, a CSV as a sortable table or raw text, an image, or rendered markdown, with download, open in a new tab, and previous/next across the conversation's files.
9. **Given** an ongoing conversation, **When** the person looks beside Send, **Then** the context ring shows how full the model's context window is (0 to 100%). Hovering it shows the exact tokens ("21k / 200k"), and a checkpoint divider states the share at which it handed over.
10. **Given** the agent has declared client tools or sends interactive UI, **When** it calls them, **Then** they render inside the reply just as they do in today's WebUI.

---

### User Story 2 - Replace the current WebUI cleanly (Priority: P1)

A contributor clones vizier with submodules and builds it. The new WebUI is checked out from `vizier-lab/webui` and is what the binary serves. The old WebUI code sits in `legacy/`, untouched and not built. The new WebUI is built from the design library's components, and changes to a component are made in the library, not copied into the WebUI.

**Why this priority**: it is the structural ask, and every other story needs it.

**Independent Test**: clone vizier fresh with submodules, run the documented install and build steps, start vizier, and confirm the served pages are the new design. Confirm `legacy/` holds the old code, and that building vizier never builds it.

**Acceptance Scenarios**:

1. **Given** a fresh clone with submodules, **When** the contributor runs the documented install and build, **Then** the binary builds and serves the new WebUI, with no extra manual steps compared with today.
2. **Given** the repository, **When** a contributor looks at its layout, **Then** the new WebUI is a submodule pinned to a commit of `vizier-lab/webui`, the design library is the existing `design` submodule, and the old WebUI code is in `legacy/`.
3. **Given** a component the new WebUI needs is missing from the design library, or needs a change, **When** it is built, **Then** it is added or changed in the design library (with its catalogue entry) and the WebUI uses it from there.
4. **Given** vizier is packaged for release (the crate and the Docker image), **When** the package is built, **Then** it contains the new WebUI's built files and serves them without the submodules' sources.

---

### User Story 3 - Run an agent's day-to-day pages (Priority: P2)

An agent's owner works through the agent's pages: Home, Core, Memory, Tasks, Skills, Dreams and Usage. Each matches its design and does at least what today's page does.

**Why this priority**: these pages are how people look after an agent between conversations. They come after chat because chat is used most.

**Independent Test**: for each page, carry out what today's page allows (edit and restore CORE, browse, search, edit and import memory, create and edit a task and open a run, edit a skill, trigger a dream, read usage) and compare the result with its design in both themes.

**Acceptance Scenarios**:

1. **Given** the Core page, **When** the owner edits CORE.md and saves, **Then** a new revision appears in the history list. **When** they open an older revision, **Then** they see its diff and can restore it.
2. **Given** the Memory page, **When** the owner searches passages, opens a document, follows links in the neighbourhood graph or opens a bundle from "All bundles", **Then** each works as drawn. Import, export, new memory, edit and history are all available.
3. **Given** the Tasks page, **When** the owner filters, sorts and pages through tasks and opens one, **Then** they see its schedule, requester, next run and recent runs, and can open a run's conversation.
4. **Given** the Skills page, **When** the owner opens a skill, **Then** they can edit it in the rich, markdown or split editor with an outline. An unsaved draft is kept in the browser until they save or discard it.
5. **Given** the Dreams page, **When** the owner presses "Dream now", **Then** a dream starts, and its journal entry appears with the extraction and consolidation stages and the changes it made.
6. **Given** the Dreams page, **When** it loads, **Then** "Last 30 nights" shows one dot per night: size is the number of memories that night's dream wrote or revised, a ring means it changed CORE, and a night with no dream is an empty slot. **When** the owner picks a night's dot, **Then** the journal opens at that night's entry.
6. **Given** the Usage page, **When** the owner picks a period and a breakdown, **Then** the totals, daily chart, "where it went" table and costliest conversations match the server's usage report.

---

### User Story 4 - Create, configure and connect agents (Priority: P2)

A person creates an agent with the New agent wizard, changes it later on Config (identity, model, behaviour, memory index, sharing, danger zone), and connects it to Discord, Telegram, addons and MCP servers on the Integrations pages, including the agent's self-management grants and the activity log.

**Why this priority**: needed to get from no agents to a working one, and to keep agents configured. It comes after the daily pages because it is done less often.

**Independent Test**: create an agent through every wizard step, change its config, share it with another person, connect a channel, an addon and an MCP server, switch a grant on, and delete the agent behind the typed confirmation.

**Acceptance Scenarios**:

1. **Given** the New agent wizard, **When** the person goes through its steps and creates the agent, **Then** nothing is saved before the final step, and the preview shows the sigil, model and schedule as they will be.
2. **Given** Config, **When** the person saves a change, **Then** they are warned beforehand that saving rebuilds the agent and stops its running turns, and a config that can't start is refused with the server's reason.
3. **Given** the Channels page with a failed reader, **When** the person opens it, **Then** the failure and its cause are stated, along with the action that fixes it (replace token).
4. **Given** any destructive action (delete agent, integration, task, skill, user, role, key), **When** the person starts it, **Then** a confirmation states what will be removed. Deleting an agent also requires typing its ID.

---

### User Story 5 - Sign in, set up and administer (Priority: P2)

A first-time operator sees the Onboarding flow (account, provider, first agent). Returning people see Login. Settings cover Profile, Password, API keys, and for administrators Providers, Users and Roles.

**Why this priority**: required for a fresh install and for running the system. Most of it already works on the server, so this is mostly screens.

**Independent Test**: start vizier on an empty data directory and go through onboarding to a first reply. Then sign out and back in, create an API key, add a provider, add a user and create a custom role.

**Acceptance Scenarios**:

1. **Given** an empty install, **When** someone opens the WebUI, **Then** onboarding asks for an admin account, then a provider, then a first agent. Where no API key is available, it suggests Ollama.
2. **Given** the API keys page, **When** a person creates a key, **Then** the full key is shown once with a copy action, and only its prefix afterwards.
3. **Given** the Roles page, **When** an administrator opens a role, **Then** they can edit its permissions (built-in roles are read-only), duplicate it or delete it, and see how many people hold it.
4. **Given** someone without a permission, **When** they open the WebUI, **Then** pages and actions they can't use are hidden or disabled, never shown and then refused.

---

### User Story 6 - New capabilities the design adds (Priority: P3)

The design shows a few things today's server doesn't provide. They are added to the server so the screens can show real data:

- **Home overview across agents**: a greeting line with agent counts (total, live, failed) and today's estimated cost. "Since you were away" is a feed of notable events (failures, task answers, dream results, background reports, channel activity) since the person's last visit.
- **Home "Last 24 hours" activity dots**: a grid with one row per agent and one column per hour (00 to now). Each dot's size is the turns that agent took in that hour, and a green dot is an agent replying right now. The panel never grows past seven rows. With no agents, the hour axis stays with every hour empty, plus an empty message and "New agent". With one agent, a single full-width row with bigger dots and a line saying the day in words ("94 chat turns, 5 scheduled tasks and 2 dream stages today"). With 2 to 6 agents, one row each, most active first. With 7 or more, an "All N" total row plus the 5 most active, switchable.
- **Turn internals in Chat**: the Recalled memory panel needs the server to keep which passages each turn was given or found; today it uses them and discards them. The Session files panel needs a way to list a conversation's files; today only the agent can list them. The context ring needs the conversation's current context use. (User Story 1, scenarios 7 to 9.)
- **Dream timeline and dream changes**: "Last 30 nights" on the Dreams page (see User Story 3, scenario 6) needs, per dream cycle, how many memories it wrote, how many it revised, and whether it changed CORE. The journal's per-entry change list (wrote / revised / core, each with its document or CORE revision) needs the same attribution. Today, revisions are only tagged as coming from *a* dream, not from which cycle.
- **Conversation list per agent**: one searchable list of an agent's conversations across the WebUI, Discord and Telegram, filtered by channel, grouped by day, loading older conversations as you scroll.
- **Agent start progress**: while an agent starts, the WebUI shows what it is waiting on (for example a model download from Ollama, with percent, size and time left), and when a start has failed, the cause and the next retry time.
- **Lists that work at any size**: agents, tasks, skills, users, roles, MCP servers, bundles and conversations can be searched, filtered (with total counts per filter), sorted and paged on the server, so no screen ever loads all of a large set.
- **Server status on sign-in**: the Login and Onboarding screens show whether the server is online, its address and its version.

**Why this priority**: each one improves the WebUI, but each screen still works without it, falling back to what today's server provides.

**Independent Test**: with 50 agents, 1,000 bundles and 600 conversations seeded, open Home, the conversation list, Memory and each table. Check that each answers quickly, shows correct totals and never loads the whole set. Start an agent whose model must be downloaded, and watch the progress.

**Acceptance Scenarios**:

1. **Given** a person returns after some hours away, **When** they open Home, **Then** "Since you were away" lists what happened in that time, newest first, 20 at a time, with "Load older".
2. **Given** 1,000 agents, **When** Home loads, **Then** "Last 24 hours" shows an "All" total row plus the five most active agents, and the panel stays the same height as with two agents.
3. **Given** an agent took 12 turns between 14:00 and 15:00 and is replying now, **When** Home loads, **Then** that agent's 14:00 dot is sized for 12 turns, and its current-hour dot is green.
4. **Given** a dream cycle that wrote 2 memories, revised 3 and updated CORE, **When** the Dreams page loads, **Then** that night's dot is sized for 5 changes with a ring, and its journal entry lists the 2 written and 3 revised documents and the new CORE revision.
5. **Given** an agent with conversations on all three channels, **When** the person searches the conversation list, **Then** matching conversations from every channel appear, and the channel filter chips show total counts.
6. **Given** an agent pulling a model, **When** the person opens its chat, **Then** the starting card shows live progress, and messages sent meanwhile are queued and run once it starts.

---

### User Story 7 - Phone-sized screens and both themes (Priority: P3)

Every screen works in light and dark themes, follows the system theme by default, and can be switched by hand. On a phone-sized screen, the sidebar is replaced by the mobile top bar and the Home and Chat pages follow the Mobile designs.

**Why this priority**: the design asks for both themes equally, and for mobile. The desktop flows come first.

**Independent Test**: open every screen in both themes and at phone width, and compare with the design.

**Acceptance Scenarios**:

1. **Given** any screen, **When** the theme is switched, **Then** it re-renders in the other theme without a reload, and the choice is remembered on that device.
2. **Given** a phone-width screen, **When** the person opens Home or Chat, **Then** the layout matches the Mobile design, with no horizontal scrolling.

### Edge Cases

- The connection to the server drops mid-turn: the chat shows the offline state, reconnects on its own, and picks the turn up where it is without duplicating frames.
- An agent fails to start or is between restart attempts: its chat shows the failed banner with the cause, the retry time, "Retry now" and "Open config", and messages sent meanwhile wait in line.
- The person's role changes or an agent is unshared while a page is open: the next action is refused with a clear message, and the page drops what they can no longer see.
- Empty states: no agents, no tasks, no memory, no search results and no usage each get their designed empty state with the one action that starts it.
- Long names (bundle paths, conversation names, model ids) are truncated in the middle, with the full name in a tooltip.
- A large session file (CSV, image, markdown) opens in the file preview without freezing the page, and huge tables are paged or virtualised.
- An unsaved skill draft exists when the person comes back: they are offered to restore or discard it.
- Two people edit the same CORE or skill: the save that would overwrite a newer version is refused, or flagged, rather than silently losing work.
- A browser that has an old WebUI cached after vizier is upgraded loads the new one, never a mix.
- Someone opens a URL saved from the legacy WebUI: it redirects to the matching new page where one exists, or to Home.

## Requirements *(mandatory)*

### Functional Requirements

**Repository and delivery**

- **FR-001**: The current WebUI code MUST move to `legacy/` unchanged. It MUST NOT be built, served or packaged, and nothing in vizier may depend on it.
- **FR-002**: The new WebUI MUST live in the repository `vizier-lab/webui`, added to vizier as a git submodule at the path the server builds and serves the WebUI from, pinned to a specific commit.
- **FR-003**: The new WebUI MUST build its screens from the components in the `design` library. A missing or changed component MUST be added or changed in the library, with its catalogue entry, not duplicated in the WebUI.
- **FR-004**: Building vizier MUST still produce a binary that serves the WebUI, with install and build steps no harder than today's. Crate and Docker packaging MUST keep working with pre-built WebUI files.
- **FR-005**: The project's documentation (README, CLAUDE.md, contributor docs) MUST describe the new layout: the `webui` and `design` submodules, the `legacy` folder, and how to install and build.

**Screens**

- **FR-006**: The new WebUI MUST provide every screen in the design (Login, Onboarding, Home, Chat, Core, Memory, Tasks, Skills, Dreams, Usage, Config, New agent, Channels, Addons, MCP, Profile, Password, API keys, Providers, Users, Roles), in dark and light, matching the design and the design library.
- **FR-007**: Every capability of the current WebUI MUST be available in the new one (feature parity), including client tools and interactive UI, reasoning display, reactions, session files, background jobs, durable and resumed turns, checkpoints, voice and file attachments, and the usage report.
- **FR-008**: Chat MUST show every state on the design's "Chat — states and flow" sheet: empty topic, choosing a skill, thinking, working, answered, queued, aborted, checkpoint (with and without a handover), woken, offline, and an actionable notice for each server error kind.
- **FR-008a**: Chat MUST have the side panels the design draws for the open conversation: **Recalled memory** (per turn), **Background jobs** (running and reported, with progress, jump to start and cancel), and **Session files**. Each shows the newest 5, then "Show all", and they collapse on narrow screens.
- **FR-008b**: Each reply's trail summary MUST say what the turn did in numbers (time worked, script runs including failed ones, passages recalled), and the reply footer MUST show model, tokens in and out, estimated cost and reactions.
- **FR-008c**: The composer MUST show the context ring: how full the model's context window is after the latest turn, in the foreground colour only, with exact token counts on hover.
- **FR-009**: The shared states on the design's States page (agent starting, agent failed, empty, nothing matched, destructive confirm, notifications, loading, live) MUST be used consistently on every screen. Failures MUST always be stated, never hidden.
- **FR-010**: Emerald MUST appear only on what is happening right now (a replying agent, a running job, a focused field, a connected reader), and lapis only on rank (owner, superadmin, grants), as the design's colour rules set out.
- **FR-011**: Every unbounded list, chart and graph MUST follow the design's Scaling rules: pickers start with search, then recent items, then A–Z. Tables have search, filter chips with total counts, sort and 25 rows per page. Charts show the top N and fold the rest into "Other". The memory graph shows a 1–2 hop neighbourhood of at most 40 documents. Feeds and histories show the newest 20, then "Load older". Side panels show the newest 5, then "Show all".
- **FR-012**: Pages and actions MUST follow the person's permissions and agent sharing: what they can't use is hidden or disabled, not offered and then refused.
- **FR-013**: The WebUI MUST work at phone width, following the Mobile designs for Home and Chat, with a sidebar replacement for navigation and no horizontal page scroll.
- **FR-014**: Every interactive element MUST be usable by keyboard, with the visible focus ring from the design library. The skill picker, pickers, dialogs and the file preview MUST support the keyboard shortcuts the design lists.
- **FR-015**: Theme MUST follow the system preference by default, be switchable from the sidebar, and be remembered per device.
- **FR-016**: Links saved from the legacy WebUI MUST land on the matching new page where one exists, or on Home otherwise.
- **FR-017**: User-facing copy MUST NOT mention the offline development provider. Where a no-key option is suggested, it MUST be Ollama.

**Server additions the design needs**

- **FR-018**: The server MUST provide a cross-agent overview for the signed-in person, covering only agents they can see: agent counts by state (total, live, failed) and estimated cost today.
- **FR-018a**: The server MUST provide hourly activity for the last 24 hours for the agents the person can see: turns per agent per hour, the agents ranked by activity (the top 5, or all of them when there are 6 or fewer), an all-agents total per hour, and which agents are replying right now. For the single-agent case, it MUST also give today's turns split by kind (chat, scheduled task, dream stage, background). The response size MUST NOT grow with the number of agents beyond the rows shown.
- **FR-018b**: Home MUST draw "Last 24 hours" as the design's dot grid, with each variant: no agents, one agent, 2 to 6 agents, and 7 or more (an "All N" row plus the 5 most active, switchable). Dot size scales with turns in that hour, emerald marks only an agent replying now, and hovering a dot shows the exact count.
- **FR-018c**: Every memory and CORE revision made by a dream MUST record which dream cycle made it. The server MUST provide, per dream cycle, the number of memories written (new documents), the number revised, whether CORE changed, and the list of those documents and CORE revisions, for the last 30 nights at least. Revisions recorded before this change are counted by time window, or shown as unattributed, never miscounted.
- **FR-018d**: The Dreams page MUST draw "Last 30 nights" as the design's dot timeline: one slot per night, dot size = memories written plus revised, a ring when CORE changed, an empty slot for a night with no dream, and the running cycle shown as live. Each night opens its journal entry, and each journal entry lists its changes as wrote / revised / core, linking to the document or revision.
- **FR-018e**: The server MUST record, for every turn, the memory passages automatic context gave the agent (document, passage, bundle, score), and the passages its own memory searches returned. This is kept with the turn's history and sent to an open WebUI as part of the turn, so the Recalled memory panel and the trail count survive a reload. Turns saved before this change show no recalled memory, not an error.
- **FR-018f**: The server MUST list a conversation's session files (name, type, size, sender, time, the turn it came with), newest first and paged, to people who can see that conversation, and MUST tell an open WebUI when a file is added. Previews MUST use the existing file download, and large files MUST be previewed without loading them whole.
- **FR-018g**: The server MUST give the WebUI each conversation's context use: tokens in the model's context after the latest turn, and the window size, so the context ring and checkpoint dividers show real numbers.
- **FR-019**: The server MUST provide a "since you were away" feed for the signed-in person: notable events across the agents they can see (start failures, task results, dream results, background reports, channel activity) since their last visit, newest first, paged. It MUST also record each person's last visit.
- **FR-020**: The server MUST provide one list of an agent's conversations across all its channels, searchable by name and content, filterable by channel with total counts per channel, newest first, and paged.
- **FR-021**: The server MUST report an agent's start progress while it builds, including what it is waiting on and, for model downloads, percent done, size and estimated time left. This MUST be in addition to the failure state, error and next retry time already reported.
- **FR-022**: The list endpoints behind the Scaling rules (agents, tasks, skills, users, roles, MCP servers, bundles, conversations) MUST support server-side search, filtering, sorting and paging, and MUST return total counts per filter. Existing callers that send no paging parameters MUST keep getting today's responses.
- **FR-023**: The server MUST expose, without signing in, whether it is online and its version, for the Login and Onboarding screens.
- **FR-024**: Every server change MUST follow the project's conventions: access decided where it is decided today, nothing that breaks existing API clients, and each new endpoint documented in the API docs.
- **FR-025**: Saving CORE or a skill over a version newer than the one the editor loaded MUST be detected and refused, not silently overwritten, and the editor MUST offer to reload or keep the person's text.

### Key Entities

- **New WebUI**: the browser application rebuilt to the design. Lives in its own repository, pinned in vizier as a submodule, and built into the files the server serves.
- **Legacy WebUI**: the current WebUI code, kept in `legacy/` as a reference only, until the maintainer deletes it.
- **Design library**: the existing `design` submodule. The source of truth for components and tokens, and extended where the screens need it.
- **Overview**: a per-person summary across the agents they can see: counts by state, today's cost, and hourly turns over 24 hours.
- **Recalled passage**: a memory passage the agent had during one turn (given automatically, or found by its own search), with its document, passage number, bundle and score. Belongs to that turn.
- **Session file**: a file sent in a conversation, by the person or the agent, with name, type, size, sender, time and the turn it came with.
- **Context use**: for one conversation, the tokens in the model's context after its latest turn and the model's window size.
- **Hourly activity**: for one agent and one hour of the last 24, the number of turns it took (by kind), and whether it is replying now. Used for the Home dot grid.
- **Dream cycle summary**: for one dream cycle, its night, time span, state, memories written, memories revised, whether CORE changed, and the documents and CORE revision behind those counts. Used for the Dreams timeline and journal.
- **Away feed event**: a notable thing that happened to an agent (start failure, task answer, dream result, background report, channel post), with a time, the agent and a short description. Shown to people who can see that agent.
- **Last visit**: the time a person last used the WebUI, kept per person and used to bound the away feed.
- **Conversation listing**: one entry per conversation of an agent, across channels: name, channel, place (server/DM/topic), last activity, and number of topics.
- **Start progress**: what an agent is waiting on while it builds (step, percent, size, time left), shown with the existing failure state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every screen and variant in the design has a matching page in the new WebUI, in both themes. A side-by-side review by the maintainer finds no screen missing.
- **SC-002**: Every task a person can do in today's WebUI can be done in the new one. A parity checklist made from today's pages passes 100%.
- **SC-003**: A new operator goes from an empty install to a first agent reply through onboarding in under 5 minutes, without reading documentation.
- **SC-004**: With 50 agents, 1,000 bundles, 600 conversations and 300 tasks, every page shows its first content in under 2 seconds on a local install, and no page loads more than one page of any list.
- **SC-005**: In a chat, the first visible sign of the agent working appears within 1 second of sending, and reopening the page mid-turn shows the turn so far within 2 seconds.
- **SC-006**: A clean clone with submodules builds and serves the new WebUI using only the documented steps, on the first try.
- **SC-007**: Nothing in the built product references or loads the legacy WebUI. Removing `legacy/` entirely leaves the build and every page working.
- **SC-008**: Every page can be operated by keyboard alone, and text and controls meet WCAG AA contrast in both themes. The design's faint text is used only where it is decorative.
- **SC-009**: Existing API clients (scripts, the client-tools end-to-end check, integrations calling the REST and WebSocket API) keep working unchanged after the server additions.

## Assumptions

- "Rename into legacy" means moving the current `webui/` folder to `legacy/` at the repository root. The new submodule takes the `webui/` path, so the server's build and serving wiring stays pointed at the same place.
- The new WebUI keeps today's stack (React Router, React, TypeScript, Tailwind), which is also the design library's stack. The plan will confirm this.
- The design library is consumed from the `design` submodule. The plan will decide how the WebUI references it (for example a workspace or git dependency) and how the pinned versions stay in step.
- The repository `git@github.com:vizier-lab/webui.git` exists and the maintainer can push to it. It is initialised as part of this feature.
- The FigCraft screens are the design for layout and content. Where a screen and the design library disagree on a component's look, the library wins, as decided in spec 022.
- The sample data in the design (agent names, costs, counts) is illustrative only. Real data replaces it.
- The legacy WebUI is not maintained during or after this feature. Fixes go into the new WebUI only.
- New capabilities beyond the server additions above (for example new agent abilities) are out of scope. Where the design labels something "proposed" on the Chat states sheet (error notices with actions, topic search, the new-message count, file previews in the reply), it is in scope.
- The rewrite may land in stages on its branch, but vizier switches to serving the new WebUI only once every P1 and P2 story passes.
