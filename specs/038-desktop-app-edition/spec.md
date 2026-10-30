# Feature Specification: Vizier Desktop App Edition

**Feature Branch**: `038-desktop-app-edition`

**Created**: 2026-10-30

**Status**: Draft

**Input**: User description: "let continue this issue https://github.com/vizier-lab/vizier/issues/102, to build the tauri app"

## Context

Issue #102 splits vizier into a reusable engine and thin shells around it. Phases 0-4 (spec `037-core-extraction`) are merged: the engine is `vizier-core`, the HTTP adapter is `vizier-http`, and the server binary is `vizier`. This feature continues from there with phases 5-8: the **desktop app edition**, a native application that runs the same engine in-process, shows the same interface as the server's web interface, and adds capabilities only a desktop app can offer. The server edition keeps working exactly as it does today.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install and use vizier as a desktop app (Priority: P1)

A person who does not want to run a server downloads the app, opens it, and gets a working vizier: they create their agents, chat with them, and use memory, schedules, files and settings, all inside one window. There is no login screen, no port to pick and no separate process to start.

**Why this priority**: This is the product. Without it there is no desktop edition. Everything else builds on a window that opens, shows the interface and works against a local vizier.

**Independent Test**: On a clean machine, install the app, open it, complete first-run setup, create an agent on the offline test provider and hold a conversation that uses a tool. Delivers a usable vizier with no server.

**Acceptance Scenarios**:

1. **Given** a machine with no existing vizier data, **When** the person opens the app for the first time, **Then** they are guided through creating a local owner account and land in the main interface without ever seeing a login page.
2. **Given** a configured app, **When** the person sends a message to an agent, **Then** the reply streams into the window as it is produced, including tool steps, exactly as in the server's web interface.
3. **Given** the app is closed while an agent turn is running, **When** the person reopens it, **Then** the turn resumes or is shown as interrupted, the same way a server restart treats it.
4. **Given** the app is already open, **When** the person launches it a second time, **Then** the existing window is brought forward and no second engine starts on the same data.
5. **Given** a workspace that already holds vizier data from the server edition, **When** the app is pointed at it, **Then** the person's agents, memory and history appear unchanged.

---

### User Story 2 - Same interface, same rules as the server (Priority: P1)

Every screen and action available in the server's web interface works in the app, and the permission rules are the same. The app does not have a weaker or different set of checks just because it does not use the network.

**Why this priority**: The issue's central requirement is one interface and one rule set for both editions. A second frontend, or a path that skips permission checks, would make the app a liability.

**Independent Test**: Run the same set of requests (read, create, update, delete across agents, memory, tasks, integrations, secrets, artifacts, notifications, usage) through the app and through the server; the outcomes, including refusals, match.

**Acceptance Scenarios**:

1. **Given** the desktop owner, **When** they perform any action available in the web interface, **Then** it works and is recorded as done by that owner (revisions, ownership, notification audience, usage).
2. **Given** an action the person's role does not permit, **When** they attempt it in the app, **Then** it is refused with the same reason the server gives.
3. **Given** the web interface build, **When** it runs inside the app, **Then** no screens, state handling or components are duplicated or forked for the desktop.
4. **Given** images, logos, integration icons, uploaded files and audio, **When** the app shows or plays them, **Then** they display and download correctly.

---

### User Story 3 - Native desktop conveniences (Priority: P2)

The app behaves like a good desktop citizen: it lives in the menu bar or system tray, shows operating-system notifications for things that happened while the window was in the background, opens native file pickers and accepts files dropped onto the window, opens external links in the person's browser, and handles `vizier://` links.

**Why this priority**: These are the reasons to prefer the app over a browser tab, but the app is useful without them.

**Independent Test**: Close the window with a background task scheduled, wait for it to complete, and receive an operating-system notification; click it and land on the right screen.

**Acceptance Scenarios**:

1. **Given** the window is closed or in the background, **When** an agent finishes a notable event (task run, background job, reply), **Then** an operating-system notification appears and selecting it opens the related screen.
2. **Given** the window is closed, **When** the person has chosen to keep vizier running in the tray, **Then** scheduled tasks and channel integrations (Discord, Telegram) keep running until the person quits from the tray.
3. **Given** a file on disk, **When** the person drops it on the chat or picks it with the native dialog, **Then** it attaches exactly as a browser upload would.
4. **Given** a link in a message, **When** the person clicks it, **Then** it opens in their default browser, never inside the app window.

---

### User Story 4 - Optional access from a browser (Priority: P2)

The person can turn on web access in settings to reach the same vizier from a browser, another device on the network, or a script. It is off by default.

**Why this priority**: Valuable for people who want phone or remote access, but not needed for the app to be useful.

**Independent Test**: Turn on web access, sign in from a browser with the account's password, chat with an agent, and confirm the app window shows the same conversation.

**Acceptance Scenarios**:

1. **Given** a fresh install, **When** the person looks at network behavior, **Then** nothing listens for outside connections.
2. **Given** the desktop account has no password, **When** the person tries to enable web access, **Then** it is blocked and they are walked through setting a password first; no credential is created silently.
3. **Given** web access is on, **When** a browser connects, **Then** it must sign in, including on the same machine.
4. **Given** web access is on, **When** the person chooses to expose it beyond the local machine, **Then** an explicit switch and a clear warning are required.
5. **Given** any request over the network, **When** it presents desktop-app identity, **Then** it is rejected.

---

### User Story 5 - Native tools for agents (Priority: P3)

Agents can use abilities of the person's computer that a server cannot offer, such as showing a notification, picking or saving a file with the person's consent, or reading the clipboard, and the person can see and control what is allowed. Credentials can optionally be kept in the operating system's keychain.

**Why this priority**: Differentiates the app, but depends on everything above.

**Independent Test**: Ask an agent to notify the person when something is done; the notification appears. Revoke the permission in settings; the same request is declined.

**Acceptance Scenarios**:

1. **Given** native tools are enabled, **When** an agent calls one, **Then** it works and the call appears in the conversation like any other tool call.
2. **Given** a native tool is disabled, **When** an agent tries it, **Then** it gets a clear refusal and nothing happens on the machine.
3. **Given** a headless server turn, background job or scheduled run with no window, **When** it would need a native tool, **Then** the call fails cleanly rather than hanging.

---

### User Story 6 - Ship and update the app (Priority: P2)

Maintainers publish installable macOS and Linux builds from the release process, and installed apps can update themselves. macOS ships as a single download that runs on both Apple Silicon and Intel Macs.

**Why this priority**: People cannot use an app they cannot install, but this can follow a working build.

**Independent Test**: Run the release process in dry-run mode and verify the produced artifacts for each supported platform, including that the macOS artifact contains both processor architectures.

**Acceptance Scenarios**:

1. **Given** a release, **When** the pipeline finishes, **Then** a signed installer exists for each supported platform.
2. **Given** an installed older version on either Mac architecture, **When** a newer release is published, **Then** the app offers and applies the update.
3. **Given** the server edition's release artifacts, **When** the app is released, **Then** the server's packages, container image and install script are unaffected.

---

### Edge Cases

- Two people on one machine use different operating-system accounts: each gets their own workspace; the app never shares data across OS accounts.
- The workspace holds more than one administrator account: the app asks once which one is "me" and remembers.
- The workspace is locked by a running server edition: the app refuses to open it and says why.
- The engine stops or fails after launch: the window shows the failure with a way to restart, rather than a blank screen.
- A large file or long audio recording goes through the app: it works and does not freeze the window.
- Deep links or reloading on a nested screen: the app returns to the right screen instead of an error.
- The operating system's webview lacks a feature (audio recording, file downloads): the app degrades to a clear message or a native equivalent.
- The app is quit while web access is serving a browser session: the session ends cleanly.
- Windows is not supported in this release.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST run the vizier engine inside the application process, with no separately launched server and no network port open by default.
- **FR-002**: The app MUST store its data in the current operating-system user's application data location by default, and MUST be able to open an existing vizier workspace.
- **FR-003**: The app MUST refuse to open a workspace already in use by another running vizier engine, and MUST tell the person which one.
- **FR-004**: Launching the app a second time MUST focus the existing window instead of starting another engine.
- **FR-005**: The app MUST show the same interface as the server edition from the same build output, with no forked screens or duplicated state handling.
- **FR-006**: The app MUST sign the person in automatically as a real owner account created at first run or chosen from existing administrators, and MUST NOT show a login screen or a sign-out action.
- **FR-007**: Actions in the app MUST be attributed to the desktop owner in revisions, ownership, notification audience and usage records.
- **FR-008**: Every action available through the server's web interface MUST be available in the app and MUST be subject to the same permission checks, applied in one place shared by both editions.
- **FR-009**: The same request, sent through the app and through the server, MUST produce the same result, including the same refusal reasons; this MUST be verified by an automated suite.
- **FR-010**: Live turn output, background job changes, reactions and notifications MUST reach the window in real time, including after the window is reopened mid-turn.
- **FR-011**: Images, logos, icons, session files, artifacts and audio MUST display, play and download in the app.
- **FR-012**: The app MUST support attaching files by native dialog and by drag-and-drop, producing the same result as a browser upload.
- **FR-013**: External links MUST open in the person's default browser and MUST NOT navigate the app window away from the interface.
- **FR-014**: The app MUST show operating-system notifications for notable events while the window is not focused, and selecting one MUST open the related screen.
- **FR-015**: The app MUST offer a system-tray presence and let the person choose whether closing the window keeps vizier running in the background.
- **FR-016**: The app MUST register a `vizier://` link scheme that opens the matching screen.
- **FR-017**: Web access MUST be off by default; enabling it MUST require the desktop account to have a password or API key, and MUST NOT create credentials silently.
- **FR-018**: When web access is on, every network request MUST authenticate through the normal sign-in, including requests from the same machine, and the desktop identity MUST be rejected on the network.
- **FR-019**: Exposing web access beyond the local machine MUST require an explicit switch with a visible warning, and cross-origin access MUST be limited to the origins the app actually serves.
- **FR-020**: Agents MUST be able to use native capabilities (at minimum notifications and file pick/save with consent) as tools, individually switchable by the person, and unavailable tools MUST fail cleanly in server, background and scheduled contexts.
- **FR-021**: The app MUST support keeping credentials in the operating-system keychain as an optional alternative to the encrypted store.
- **FR-022**: If the engine or any of its subsystems stops, the app MUST show the failure and offer restart; it MUST NOT exit silently or show a blank window.
- **FR-023**: The release process MUST produce installable, signed builds for macOS and Linux, with macOS delivered as one download covering Apple Silicon and Intel (macOS 11 or later), and installed apps MUST be able to update themselves.
- **FR-024**: The server edition's behavior, packages, container image and install paths MUST remain unchanged, and the app MUST NOT add desktop-toolkit requirements to the server's build or test runs.
- **FR-025**: This work MUST NOT change the pinned embedding and speech dependency versions.
- **FR-026**: The documentation MUST describe installing the app, the security boundary (operating-system account equals workspace boundary), web access, and how third-party interfaces can build on the engine.

### Key Entities

- **Desktop owner**: The real account the app signs in as; owns agents created in the app and is the audience for chat-reply notifications. Bound to the workspace in app settings.
- **Desktop workspace**: The folder holding agents, memory, history and the single-engine lock for one operating-system user.
- **Caller / identity**: Who is acting (desktop owner, signed-in browser user, API key, system), carrying the permissions every action is checked against.
- **Event stream**: The live feed of turn frames, job changes, reactions and notifications that the window and optional network listener both subscribe to.
- **Native capability**: An ability of the person's computer offered to agents as a tool, with a per-capability on/off setting.
- **Web access setting**: Off by default; holds whether the network listener is on, whether it is local-only or exposed, and its port.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new user can go from opening the installed app to receiving their first agent reply in under 5 minutes, without touching a terminal.
- **SC-002**: 100% of the actions available in the server's web interface are available in the app, confirmed by the parity suite.
- **SC-003**: The parity suite shows identical outcomes, including refusals, for every request vector run through both the app and the server.
- **SC-004**: With default settings, a port scan of the machine finds nothing listening on behalf of the app.
- **SC-005**: A reply starts appearing in the window within 1 second of the model producing its first output, matching the server's web interface.
- **SC-006**: Relaunching the app after a forced quit resumes or marks every interrupted turn, with zero lost conversations.
- **SC-007**: A single macOS download installs and launches on both Apple Silicon and Intel Macs running macOS 11 or later.
- **SC-008**: An installed app updates to a newer release on both Mac architectures and on Linux in a release rehearsal.
- **SC-009**: Server-edition tests, packaging and release dry run pass unchanged, and a server build needs no desktop toolkit installed.
- **SC-010**: No network request carrying desktop identity is ever accepted, confirmed by an automated check.

## Assumptions

- Supported platforms for this release are macOS 11+ and Linux; Windows is deferred (per issue #102).
- The service layer (phase 5) is part of this feature's delivery as the enabling step: permission checks move to one shared place before the app's commands are built, so the app never reimplements them.
- The app is local-first. Connecting the app window to a remote vizier is out of scope but must not be made harder.
- One desktop owner is bound per workspace; an app lock (PIN/biometric) and multi-user sharing of one workspace are follow-ups.
- Web access over the network uses plain HTTP; the exposed mode is documented as trusted-network only until encryption is added.
- Retiring local speech models (#101) and upgrading the embedding library are separate projects.
- The web interface is the single frontend; the app adds only a transport layer under it.
- The model catalogue stays inside the engine for now.
- Maintainers, not this feature, make CI checks required on `master` and publish the crates.
