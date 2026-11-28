# Feature Specification: Agent Apps

**Feature Branch**: `071-agent-apps`

**Created**: 2026-11-28

**Status**: Draft

**Input**: User description: "Take generative UI a step further: sub-applications, or just apps. Let an agent create any app inside Vizier, and have it appear in the sidebar rail." Refined in discussion: an app belongs to a person, not an agent; any agent the person can use may *drive* it and the driver can be switched; an app carries a manifest that says what a driver needs (addons, tools, secrets, model features); an app has its own stored assets, including its functions, and carries no memory of its own; apps sit on the rail in one list with the agents, ordered by the person; the app's chat is a separate, resizable inset on the right that is always open and is the ordinary chat, so every future chat behaviour reaches it; the driver is switched from the agent's name at the top of that chat, not from the app window; a person can also start an app from scratch from the rail's +, with a minimal empty state ("tell your agent what to build"); desktop edition only (including the server the desktop app can expose); export and import are out of scope.

## Context

Spec 063 (HTML generative UI) is phase 1 of a longer path: an agent writes an HTML page that is drawn inside its reply, kept with every version and changed by small edits. Its own spec names this feature as phase 2: pages saved as named **apps** opened outside a conversation.

A page in a reply has three limits that stop it from being an app:

- **It lives in a conversation.** It scrolls away, and nobody opens a chat to use a habit tracker.
- **It has no lasting data of its own.** Its state is one value, and anything more needs a turn.
- **It belongs to the agent that drew it.** It cannot outlive that agent, move to another one, or someday be shared as a file.

This feature turns a page into an **app**: a named thing that belongs to a person, has a place on the rail, keeps its own data and files, has its own functions, and is *driven* by an agent the person chooses. The agent builds and changes the app and runs its background work; the app keeps working when nobody is talking to it.

The design was drawn first (constitution: design before code), in `Vizier shell.figcraft`, page `Shell · chosen`:

- **Frame F (chosen)**: the app with its chat as a separate inset on the right, the resize handle, and the rail as one mixed list.
- **Rail strip beside frame G**: drag to reorder.
- **Frame B**: adding a page from a chat to the rail.
- **Frame D**: the driver picker's contents with the requirement check. Its placement is superseded: the picker opens from the agent's name at the top of the chat inset (frames F and I).
- **Frame E**: an app whose driver was deleted.
- **Frame H**: the rail's + opening New agent / New app.
- **Frame I**: a new app as a draft, before anything is built.

The alternatives that were not chosen (apps in their own rail section, apps listed under their agent, the chat inset on the left) were removed from the file.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create an app (Priority: P1)

There are two ways in.

**From scratch.** The person presses + on the rail and picks **New app** (the other choice is New agent). A draft app opens at once: a dashed tile at the end of the rail, the name "Untitled app", one quiet line in the app panel ("Tell Archivist what to build"), and the chat inset beside it with its composer focused. The driver defaults to the last agent the person used and can be changed from the agent's name at the top of the chat. The person describes what they want; the driver builds the first version, names the app and picks its icon, and the tile becomes an ordinary one. A draft the person leaves without sending anything is discarded.

**From a conversation.** A person asks their agent for something they will use again: "make me a habit tracker". The agent draws it as a page in the chat (spec 063) and says it can become an app. The page's header offers **Add to rail**. The person presses it, confirms in a small popover that says the app will keep its own data and its own conversation with that agent, and the app's tile appears on the rail. The agent can also offer to make an app itself; the person's confirmation is still what puts it on the rail.

**Why this priority**: Without a way to create an app there is nothing else to test, and the explicit action (New app, or the confirmation) is what keeps agents from filling the rail.

**Independent Test**: Press + → New app, type "a habit tracker" and send. The draft's tile becomes a named app with the driver's first version. Separately, in a desktop conversation, ask for a habit tracker, press Add to rail and confirm: a new tile appears, opening it shows the same page outside the conversation, and deleting the original conversation leaves the app intact.

**Acceptance Scenarios**:

1. **Given** the rail, **When** the person presses +, **Then** a small menu offers New agent and New app, and closes on Escape, an outside press or a choice.
2. **Given** the person chooses New app, **When** the draft opens, **Then** it shows a dashed tile at the end of the rail, the name "Untitled app", one line inviting the person to tell the driver what to build, and the chat inset with its composer focused and the last-used agent as driver.
3. **Given** a draft app, **When** the person sends a first message, **Then** the driver builds the first version, gives the app a name and icon, and the tile becomes an ordinary app tile.
4. **Given** a draft app, **When** the person leaves it without sending any message, **Then** the draft is discarded and its tile disappears.
5. **Given** a draft app, **When** the person changes the driver before the first message, **Then** the chosen agent builds the first version.

6. **Given** a page drawn in a desktop conversation, **When** the person presses Add to rail and confirms, **Then** an app exists, owned by that person, driven by the agent that drew the page, with that page as its first version, and its tile is on the rail.
7. **Given** the confirmation popover, **When** the person chooses Not now, presses Escape or presses outside it, **Then** no app is created and the page stays in the chat as before.
8. **Given** an agent that proposes an app in a conversation, **When** the person has not confirmed, **Then** no tile appears on the rail.
9. **Given** an app made from a conversation, **When** that conversation is deleted, **Then** the app, its versions, data and files are unaffected.
10. **Given** the server edition (not the desktop app), **When** a person asks for an app or presses + on the rail, **Then** no New app choice and no Add to rail action are offered, and the agent is offered no app tools.

---

### User Story 2 - Use the app on its own (Priority: P1)

The person clicks the app's tile. The app fills the main panel; its chat with the driving agent is a second inset on the right, always open. The person ticks today's habits. Each tick is saved at once, without the agent being asked and without any model call. The next day, after a restart, the ticks are still there.

**Why this priority**: This is the difference between an app and a page in a reply. Data that is kept without a turn, in a place of its own, is what makes it worth opening.

**Independent Test**: Open an app, change something in it, close Vizier, reopen it and open the app again. The change is there, and the usage report shows no model call for it.

**Acceptance Scenarios**:

1. **Given** an app on the rail, **When** the person selects its tile, **Then** the app fills the main panel, its header shows the app's name, icon and version, and its chat inset is open on the right with the driving agent's name at its top.
2. **Given** an open app, **When** the page saves or reads its data, **Then** the change lasts across reloads and restarts and no model call is made.
3. **Given** an open app, **When** the person drags the handle between the two insets, **Then** the chat inset changes width within its limits (chat at least 320 px, app at least 560 px). The width is remembered for that app, and a double-click on the handle restores the default.
4. **Given** an open app, **When** its page script fails to start or keeps crashing, **Then** the app shows the same paused state and "Run anyway" as a page in a reply, and the chat inset keeps working.
5. **Given** an open app, **When** its page tries to reach the network, the WebUI, storage or the sign-in token, **Then** the attempt fails exactly as it does for a page in a reply (spec 063).

---

### User Story 3 - Change the app by talking to its driver (Priority: P1)

Beside the app is its own conversation with the driving agent. The person types "add a weekly view and make Sunday optional". The agent changes the app with small edits, checks the new version in the person's open app, and the app reloads in place. The change shows in the chat as a step, for example "Edited Habits, v3 → v4, verified, data kept". The person's data is untouched. If the new version is worse, the person rolls back to the previous one from the version history.

This conversation **is the ordinary chat**: every chat feature (the activity trail, steering a busy turn, attachments, skills, background work, notifications, and anything added to chat later) works here with no extra effort.

**Why this priority**: An app that cannot be changed by asking is just a saved page. Using the ordinary chat keeps app chat from falling behind.

**Independent Test**: In an app's chat, ask for a visible change. The app reloads with it, the chat shows the edit step, the stored data is unchanged, and rolling back restores the previous look with the same data.

**Acceptance Scenarios**:

1. **Given** an app's chat, **When** the driver changes the app, **Then** a new version is saved, the open app reloads to it, and the chat shows which versions changed and whether the result was verified.
2. **Given** a new version whose check reports problems, **When** the turn continues, **Then** the driver is told the problems, as for a page in a reply.
3. **Given** an app with several versions, **When** the person picks an earlier version in the version history and confirms, **Then** the app runs that version and its data is unchanged.
4. **Given** a feature added to the ordinary chat after this one ships, **When** the person uses an app's chat, **Then** the feature is there with no app-specific work.
5. **Given** the driver's other conversations, a scheduled task or a dream, **When** they run, **Then** they cannot read or change this app's data or versions; only the app's own chat and the app's own background work can.

---

### User Story 4 - Switch the agent that drives the app (Priority: P2)

The top of the app's chat shows the driving agent: **Archivist ▾**, "drives this app". The person opens it and sees what the app needs (for example "time tools", "Google Calendar connection", "secret CAL_TOKEN") and every agent they can use, each marked **ready** or **missing N**. Expanding an agent shows what it lacks and offers a link to set it up on that agent. The person picks Scout. Scout becomes the driver, a new chat thread starts with Scout, and the old thread stays readable from Archivist's conversations.

**Why this priority**: Apps that are tied to one agent cannot outlive it or move to a better-suited one, and that is the core reason they belong to the person. It comes after creating, using and changing an app, which are useful on their own.

**Independent Test**: With two agents, one of them missing a required connection, open the driver picker. The ready agent is marked ready and the other lists the missing connection. Switch to the ready agent and ask for a change: the new driver makes it.

**Acceptance Scenarios**:

1. **Given** an app, **When** the person opens the driver picker from the agent's name at the top of the chat inset, **Then** it lists the app's requirements and every agent the person may chat with, each marked ready or with the exact missing items.
2. **Given** an agent missing some requirements, **When** the person makes it the driver, **Then** the switch succeeds; the functions and background work that need the missing items are turned off and say why, and everything else works.
3. **Given** a switch to a new driver, **When** it completes, **Then** a new chat thread with the new driver starts beside the app, the previous thread is kept and readable by people allowed to read the previous agent's conversations, and the app's data and versions are unchanged.
4. **Given** a switch to a new driver, **When** the app has approved functions that use secrets or have outside effects, **Then** those approvals are asked again before they run under the new driver.
5. **Given** a new driver's first turn, **When** it starts, **Then** the driver is given the app's description, requirements, current version and the shape of its data, so it can work on the app without having built it.

---

### User Story 5 - An app outlives its driver (Priority: P2)

Archivist is deleted. The Habits tile gets an amber mark. Opening the app shows a banner: Habits has no driver, the data and the page still work, and its chat, background work and driver-dependent functions are paused. **Choose a driver** opens the picker.

**Why this priority**: Deleting an agent must never delete a person's app or data. Without this, apps would not really belong to the person.

**Independent Test**: Make an app, delete its driving agent, and open the app. It shows the banner, its page and data work, and choosing a new driver brings back its chat and background work.

**Acceptance Scenarios**:

1. **Given** an app whose driver is deleted, **When** the person opens it, **Then** the page works, its stored data is intact, the chat inset is disabled with a reason, and the banner offers Choose a driver.
2. **Given** an app with no driver, **When** its scheduled background work comes due, **Then** it does not run and is shown as paused, not failed.
3. **Given** an app with no driver, **When** the person chooses a new one, **Then** paused background work resumes under the new driver (subject to approvals, Story 4).

---

### User Story 6 - The app does work without a conversation (Priority: P2)

A "PR board" app shows open pull requests. Its functions, saved with the app, fetch them through the driver's tools. A button in the app calls such a function directly, with no model call. A schedule runs the same function every morning, updates the app's data, and puts a count on the app's tile.

Because a button press can now cause effects with no model judging it, the person approves which functions the app's page may call, and approves again when that list grows.

**Why this priority**: This is what makes an app feel like an app rather than a chat with storage. It is P2 because Stories 1–3 are useful without it and it carries the most security weight.

**Independent Test**: Make an app with a function that reads the current time and saves it. Approve it, press the button: the data updates with no model call. Add a schedule: it runs on time without the app open, and the tile shows the count it sets.

**Acceptance Scenarios**:

1. **Given** an app function the person has approved for the page, **When** the page calls it, **Then** it runs with the driver's tools and secrets, within limits on time and frequency, and returns a result without a model call.
2. **Given** a function the person has not approved for the page, **When** the page calls it, **Then** the call is refused with a reason and nothing runs.
3. **Given** an app whose list of page-callable functions grows in a new version, **When** the new version first runs, **Then** the new functions stay refused until the person approves them; previously approved ones keep working.
4. **Given** an app schedule, **When** it comes due, **Then** it runs without any open app or conversation, records whether it succeeded, and may set the tile's count.
5. **Given** a function result or app data read back by the driver, **When** it reaches the model, **Then** it is marked as data, never as instructions.

---

### User Story 7 - Arrange the rail (Priority: P3)

Agents and apps share one list on the rail. The person drags a tile to a new place; the others move apart to show where it will land. The order is kept for that person.

**Why this priority**: Ordering matters once a person has several apps, but a fixed order is usable at first.

**Independent Test**: Drag an app between two agents, reload, and the order is kept. Another person signed in to the same desktop app sees their own order.

**Acceptance Scenarios**:

1. **Given** the rail, **When** the person drags a tile and drops it, **Then** the list keeps the new order across reloads and restarts.
2. **Given** a new app, **When** it is added, **Then** its tile goes to the end of the list (before the + tile) and can be moved.
3. **Given** a keyboard user, **When** they move a tile with the keyboard, **Then** they can reorder the list without a pointer.

---

### User Story 8 - Files go in and out of an app (Priority: P3)

The person drops a CSV into the app's chat: "import my old habits from this". The driver reads it and saves the result into the app. The page can also hand one of its own files to the driver ("look at this export"), and the person can pick a file from their computer directly in the page.

**Why this priority**: Files make apps practical for real data, but apps are useful without them.

**Independent Test**: Drop a file into an app's chat and ask the driver to keep it. It appears among the app's files and survives deleting that chat thread or switching drivers.

**Acceptance Scenarios**:

1. **Given** a file uploaded in an app's chat, **When** the driver copies it into the app, **Then** it is kept with the app and survives a driver switch.
2. **Given** an app page, **When** it lists files, **Then** it sees its own files and the uploads of its own chat thread, never uploads from any other conversation.
3. **Given** an app file, **When** the page shares it with the driver, **Then** it appears in the app's chat as an attachment the driver can read.

---

### User Story 9 - The driver can act on what the person sees (Priority: P3)

While the app is open, its chat's driver can use tools the app provides on screen: "highlight the habits I missed this week", "show me a preview before you change it". These tools are declared in the app's manifest and answered by the open page.

**Why this priority**: It makes the chat feel connected to the app, but data and functions already cover what must work with the app closed.

**Independent Test**: Give an app a manifest tool that highlights a row and a page handler for it. Ask the driver to highlight a row: the open app highlights it, and the driver gets a result.

**Acceptance Scenarios**:

1. **Given** an open app with manifest tools, **When** the person sends a message in its chat, **Then** the driver is offered those tools for that turn, and a call reaches the open page and returns its result.
2. **Given** a manifest tool the page has not registered a handler for, **When** the driver calls it, **Then** it reports unavailable instead of waiting.
3. **Given** any conversation other than the app's own chat, including the driver's ordinary chats while the app is open elsewhere, **When** a turn runs, **Then** the app's tools are not offered.
4. **Given** a page that tries to offer a tool not in its manifest, **When** a turn starts, **Then** that tool is not offered.

---

### Edge Cases

- **Driver lacks a requirement after a change to it** (an integration removed, a secret deleted): the app is checked again when the app opens and before background work runs; the affected functions are turned off with a reason rather than failing silently.
- **The driver adds a requirement in a new version**: the picker and the app header show the new state; a version cannot be saved while it uses tools that the manifest's derived requirements do not list.
- **The person loses the right to chat with the driving agent** (permission removed): the app treats it as having no driver until the person picks one they may use.
- **Two windows or tabs open the same app**: both stay in step. Data written in one appears in the other, and only one chat thread exists.
- **An app version breaks the page but the data is fine**: the person can roll back from version history, which stays reachable when the page cannot load.
- **App data grows past its limit**: the write is refused with a clear reason that the page and the driver both receive. Nothing is silently truncated.
- **A schedule's function fails or exceeds its time limit**: the run is recorded as failed with the reason and the next run still happens. No notification storm: a repeating failure notifies once per episode.
- **The person removes an app**: it is deleted with its versions, data, files, approvals and schedules after a confirmation that names what is lost; its chat threads remain readable as ordinary conversations of their agents.
- **A draft is left after a message was sent but before any version was saved** (the driver failed or was stopped): the draft is kept, so the conversation is not lost; it stays a dashed tile until a version exists or the person removes it.
- **The person deletes the app's chat thread**: the app is unaffected and a new thread starts with the current driver on the next message.
- **Phone-width window** (the desktop's web access opened on a phone): the two-inset layout does not fit; the app takes the screen and the chat opens as a sheet over it.
- **Web access is turned on**: the person can open their apps from a browser exactly as in the desktop window, under the same sign-in.

## Requirements *(mandatory)*

### Functional Requirements

**Availability and ownership**

- **FR-001**: Apps MUST be available only in the desktop edition, including when that edition's web access is on. The server edition MUST offer no app surfaces, routes or agent tools.
- **FR-002**: Every app MUST belong to exactly one person. Only that person MUST be able to see, open, use, change, drive-switch or remove it.
- **FR-003**: An app MUST be created only by an explicit action of its owner: choosing New app from the rail's +, or confirming Add to rail for a page in a conversation or an agent's proposal. An agent MUST NOT be able to place an app on the rail by itself.
- **FR-003a**: The rail's + MUST offer New agent and New app (in the desktop edition; the server edition keeps opening New agent directly). New app MUST open a draft at once, with the person's most recently used agent as driver, a minimal empty state inviting the person to tell the driver what to build, and the chat composer focused.
- **FR-003b**: A draft MUST become an ordinary app when its driver saves the first version, at which point the driver gives it a name and icon (both changeable by the owner later). A draft with no message sent MUST be discarded when the person leaves it.
- **FR-004**: An app MUST be self-contained. Its pages, functions, data, files, manifest and version history MUST be stored with the app, and nothing in it MUST depend on any one agent's memory, custom tools, artifacts or conversations.
- **FR-005**: An app MUST carry no memory of its own. What a driver needs to work on it MUST come from the app's description, manifest, current version and the shape of its data.

**Rail and layout**

- **FR-006**: An app's tile MUST appear in the rail's single list together with the agents, in an order the person sets by dragging (or keyboard), kept per person.
- **FR-007**: An app's icon MUST be chosen from the WebUI's own line-icon set by name. When there is no icon or the name is unknown, the tile MUST show the app name's first letter. Icons MUST be monochrome like the rest of the WebUI.
- **FR-008**: Opening an app MUST show it in the main panel with a header holding its icon, name, version and a menu (version history, rename, remove), and its chat as a separate inset on the right that is always open. The top of the chat inset MUST show the driving agent and be the way to open the driver picker.
- **FR-009**: The chat inset MUST be resizable from the gap between the insets within set limits (chat at least 320 px, app at least 560 px). The width MUST be remembered per app for that viewer, and a double-click MUST restore the default. The handle MUST be usable from the keyboard.
- **FR-010**: At phone widths the app MUST take the screen and its chat MUST open as a sheet over it.

**The app's chat**

- **FR-011**: The app's chat MUST be the ordinary conversation experience (the same components and behaviour as a conversation with an agent), so that any chat behaviour present now or added later applies to it without app-specific work.
- **FR-012**: Each app MUST have one chat thread per driver. Switching driver MUST start a new thread with the new driver and keep the previous one readable by those allowed to read that agent's conversations.
- **FR-013**: When a driver's turn starts in an app's chat, the driver MUST be given the app's description, requirements, current version, available functions and data shape.
- **FR-014**: App tools (to edit the app, read and write its data and files, and manage its functions and schedules) MUST be offered to the driver only in that app's chat and in that app's own background work. They MUST NOT be offered in other conversations, scheduled tasks, dreams, Discord or Telegram.

**Versions and changes**

- **FR-015**: Every change to an app's page, functions or manifest MUST be saved as a new version. Changes MUST be made as small edits against the latest version, as for generative UI pages (spec 063).
- **FR-016**: Each new version MUST be checked in the person's open app when one is open. Problems MUST be reported to the driver, and the result MUST be shown as a step in the chat.
- **FR-017**: The person MUST be able to see the version history and run an earlier version, after a confirmation. Rolling back MUST NOT change the app's data.

**Data and files**

- **FR-018**: An app MUST be able to keep structured data (named collections of records) that its page and its driver can read and write, saved immediately, with no model call for page reads and writes.
- **FR-019**: App data and files MUST have size limits. A write over a limit MUST be refused with a reason given to whoever made it.
- **FR-020**: Changes to app data MUST appear in every open view of that app without a reload.
- **FR-021**: An app MUST be able to keep files. Its page MUST be able to read its own files and the uploads of its own current chat thread, and MUST NOT be able to read any other conversation's files. The driver MUST be able to copy a chat upload into the app, and the page MUST be able to share an app file into the chat.

**Manifest, requirements and drivers**

- **FR-022**: Every app MUST have a manifest stating its name, description, icon, entry page, data collections, functions, page-callable functions, schedules, manifest tools and requirements.
- **FR-023**: Requirements MUST cover tool categories, addons, connected servers (and the tools needed from them), secrets by name only (never values), and model features. Tool needs MUST be derived from what the app's functions call. Only what cannot be derived MAY be declared.
- **FR-024**: The system MUST be able to check any agent against an app's requirements and report ready, or the exact missing items, each with where to fix it.
- **FR-025**: The person MUST be able to make any agent they may chat with the app's driver, whether ready or not. Functions and background work that need missing items MUST be turned off with a stated reason, and everything else MUST keep working.
- **FR-026**: When an app has no driver (deleted, or no longer allowed), its page and data MUST keep working, its chat and driver-dependent work MUST be paused with a stated reason, and the app MUST offer to choose a driver.
- **FR-027**: Deleting an agent MUST NOT delete any app it drives, or that app's data, versions or files.

**Functions, approvals and background work**

- **FR-028**: An app's functions MUST run with the current driver's tools and secrets, within time and frequency limits, and MUST be usable by the driver from the app's chat and background work.
- **FR-029**: The page MUST be able to call only functions the owner has approved for it. Approval MUST be asked again for functions added to that list and, on a driver switch, for functions that use secrets or have outside effects.
- **FR-030**: Function results and app data MUST reach any model as data, never as instructions.
- **FR-031**: An app MUST be able to have schedules that run its functions or ask its driver to act, without any open app or conversation. Each run's outcome MUST be recorded. A repeating failure MUST notify the owner once per episode.
- **FR-032**: An app MUST be able to set a count on its rail tile.

**Manifest tools for the open app**

- **FR-033**: Tools listed in an app's manifest MUST be offered to the driver in that app's chat while the app is open, answered by the open page, and treated as unavailable when the page has not registered them. Tools not in the manifest MUST never be offered.

**Removal and limits**

- **FR-034**: The owner MUST be able to remove an app after a confirmation that names what will be lost. Removal MUST delete its versions, data, files, approvals and schedules, and MUST leave its chat threads as ordinary conversations of their agents.
- **FR-035**: Every app activity that costs model or media usage MUST be counted in usage reports and attributed to the app and its driver.

### Key Entities

- **App**: a named, person-owned thing on the rail. It has an icon, a description, a current version, a driver (or none), a tile count, and a place in its owner's rail order. It is a **draft** until its first version is saved.
- **App version**: one saved state of the app's page, functions and manifest. Versions are ordered, and one is current.
- **Manifest**: the app's own description of itself: entry page, data collections, functions, page-callable functions, schedules, manifest tools and requirements.
- **Requirement**: something a driver must have for part of an app to work: a tool category, an addon, a connected server's tools, a named secret, or a model feature. It is derived or declared.
- **Driver**: the agent currently assigned to an app. It changes over time, and each driver has its own chat thread.
- **App data**: named collections of records belonging to the app, with size limits, shared by its page and its driver.
- **App file**: a file kept with the app, as opposed to an upload in one of its chat threads.
- **App function**: a saved script belonging to the app that runs with the driver's tools.
- **Page approval**: the owner's consent for the page to call a given function, kept per app and per driver.
- **App schedule**: a recurring run of a function or a request to the driver, with a recorded outcome per run.
- **Rail order**: one person's ordering of their agents and apps.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can go from asking for an app to having it on the rail in at most 2 actions after the agent has drawn it (Add to rail, confirm), and from the rail's + to typing into a new app's chat in at most 2 actions (+, New app).
- **SC-002**: Saving data from an app's page (for example ticking a habit) appears saved within 200 ms and costs no model call in the usage report.
- **SC-003**: 100% of app data and versions survive restarting the desktop app, deleting the conversation the app came from, switching driver and deleting the driving agent.
- **SC-004**: Switching an app's driver takes at most 3 actions (open picker, choose, confirm), and afterwards the new driver completes a requested change to the app on its first attempt in at least 9 of 10 trials with the offline test provider.
- **SC-005**: For every agent in the driver picker, the reported missing items match exactly what that agent lacks (no false ready, no false missing), across a test set of at least 10 requirement combinations.
- **SC-006**: No page-initiated function call runs without an approval on record; a test that attempts unapproved, newly added and post-switch calls finds 0 executions.
- **SC-007**: A chat feature added after this one ships (verified with at least one real later addition) works in an app's chat with 0 lines of app-specific code.
- **SC-008**: An app schedule runs within 1 minute of its due time while the desktop app is running, with no app window open.
- **SC-009**: In the server edition, an inventory of routes, tools and WebUI surfaces finds 0 app entries.

## Assumptions

- **Desktop only.** The feature ships in the desktop edition, which runs as one process on one machine with local storage. Running work that a second machine could also claim is not a concern here. Bringing apps to the server edition would be a separate feature.
- **Built on spec 063.** Apps reuse its sandboxed page host, the kit, edit-based versions, verification and the crash-loop pause. A page drawn in a reply and an app page look and behave the same.
- **Functions reuse the custom-tool engine's rules** (spec 061: validation, the derived list of called tools, sample runs) with the app, not an agent, as their owner.
- **Manifest tools follow the client-tool rules** of spec 019 (naming, turn-ending, deferral), with the manifest as the only source of declarations.
- **Export and import are out of scope.** The manifest and the self-contained rule (FR-004) are designed so they can be added later without reworking apps.
- **No sharing.** An app is seen and used only by its owner. Sharing with other people is out of scope.
- **The rail's + changes from spec 064** in the desktop edition: it opens a two-item menu instead of going straight to New agent. New agent is one click further away, which is accepted.
- **Every app is on the rail.** There is no hidden or archived app list in this feature: removing an app deletes it.
- **Icons come from a fixed icon set.** Custom drawn icons (for example an agent-written SVG) are out of scope until import exists, because they would need a sanitizer.
- **One open chat thread per app at a time**, belonging to the current driver. Previous drivers' threads are reachable from those agents' conversation lists.
- **Defaults chosen without asking:** the chat inset defaults to 380 px wide; data limits follow the existing scratchpad and page-state order of magnitude, to be fixed in planning; schedule time limits follow the detached-script limits (spec 058).
