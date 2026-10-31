# Feature Specification: Projects Mode Foundation

**Feature Branch**: `040-projects-mode`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "make the spec, this spec focus on the foundation, no need to consider things about git integration, file diff view, etc." The feature captured from the conversation: a project is a folder mounted to an agent, with its own sessions, a workspace shell, a project list above Sessions in the sidebar, desktop-only creation, and browser access to existing projects gated by a `projects` permission.

## Context

Projects mode turns a conversation with an agent into work in a real folder. A person opens a folder from the desktop app; the agent receives that folder as its workspace and can run commands there. The project is a long-lived thing: it keeps its own sessions, can be reopened, and can be reached from a browser when web access is enabled.

This specification covers the **foundation**: opening a project, its sessions, the workspace shell, the sidebar, permissions, and remote use. Git integration, diff review, a file editor, per-session worktrees, a terminal UI, and sandboxing are deliberately out of scope.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open a project and work in its folder (Priority: P1)

A person is in the desktop app with an agent. They choose **Open a Project**, pick a folder, and confirm that they trust the agent to work in it. A project session opens. They ask the agent to inspect a file, change it, or run a command, and the agent does so inside that folder. The command, its output and whether it succeeded appear in the conversation. The work continues even while the person switches to another screen or another conversation.

**Why this priority**: This is the product. Without a mounted folder and a working shell there is no projects mode.

**Independent Test**: Open a temporary folder on the desktop app, ask the agent to run a simple command and create a file. The conversation shows the command, output and success status, and the file exists on disk.

**Acceptance Scenarios**:

1. **Given** the desktop app and an agent with no projects, **When** the person chooses Open a Project and picks a folder, **Then** a trust disclosure is shown before any command can run, and the project appears with a new session.
2. **Given** a project session, **When** the person asks the agent to run a command in the folder, **Then** the reply shows the command, its output, its exit status and its duration.
3. **Given** a command that exits with an error, **When** it finishes, **Then** the agent receives the output and the non-zero status as a normal result rather than a failed tool call.
4. **Given** a turn is running, **When** the person navigates to another screen, **Then** the turn continues and the shell call still completes.
5. **Given** an empty conversation on the desktop app with no projects, **When** it opens, **Then** it offers Open a Project.
6. **Given** a project whose folder is no longer available, **When** the person opens it, **Then** it is marked unavailable and commands are refused until the folder is available again.
7. **Given** a folder that is already open as a project, **When** the person chooses it again, **Then** the existing project is reused and no duplicate appears.
8. **Given** the application's own data folder, **When** the person tries to open it as a project, **Then** it is refused with an explanation.
9. **Given** a project session that is not on screen, **When** the agent finishes a reply, **Then** the person receives the normal notification and the reply is attributed to them and the project in usage.

---

### User Story 2 - Keep several sessions in one project (Priority: P2)

A person has a project with several lines of work. They start a new session for each, switch between them, and see them grouped under the project in the sidebar above the regular Sessions list. When they have no projects, the Projects section is not shown.

**Why this priority**: A project is a long-lived workspace; one session is not enough. The sidebar is how the person moves between workstreams.

**Independent Test**: Create three sessions in one project, switch between them and confirm each keeps its own history. Confirm the regular Sessions list does not contain them and that the Projects section disappears when the last project is removed.

**Acceptance Scenarios**:

1. **Given** a project, **When** the person starts a new session from the project row, **Then** a new empty session opens and is listed under that project.
2. **Given** several sessions in a project, **When** the person opens one, **Then** it is highlighted and the others remain listed, with a live marker on any running session.
3. **Given** no projects, **When** the sidebar is shown, **Then** the Projects section is hidden.
4. **Given** a project session, **When** the regular Sessions list is shown, **Then** that session is not listed there.
5. **Given** a project, **When** its owner turns its shell switch off, **Then** the agent in that project is not offered workspace commands.
6. **Given** a project with sessions, **When** its owner removes it, **Then** it and its sessions leave the sidebar, its files remain, and re-opening the same folder restores the sessions.
7. **Given** two sessions in one project, **When** both run commands, **Then** both run without waiting for each other.

---

### User Story 3 - Reach existing projects from a browser (Priority: P2)

A person is away from their desk. They open the web interface, sign in, and — if they have the `projects` permission — see the projects that already exist. They open a session, continue the conversation, start a new session, and run commands. They can tell that the commands run on the host machine. They cannot create a new project from the browser, because there is no local folder to choose there.

**Why this priority**: This is what makes projects useful for remote work. It is not required to prove the desktop foundation, so it follows the core stories.

**Independent Test**: With web access enabled, sign in from a browser as a person with the permission. Open an existing project session, ask for a command, and see the output. Confirm there is no way to create a project, and that a direct attempt to create one is refused.

**Acceptance Scenarios**:

1. **Given** a person with the `projects` permission and web access enabled, **When** they open the web interface, **Then** the Projects section is visible and lists existing projects.
2. **Given** such a person, **When** they open an existing project session and run a command, **Then** the command runs on the host and its result appears in the browser.
3. **Given** the web interface, **When** the person looks for a way to add a project, **Then** none is offered and any attempt to create one is refused.
4. **Given** a person without the `projects` permission, **When** they use the web interface, **Then** they see no Projects section and cannot reach a project by direct address.
5. **Given** a remote project session, **When** it is open, **Then** the interface states that commands run on the host machine.
6. **Given** a browser user who is not the project's owner and lacks the whole-workspace project permission, **When** they try to rename or remove the project, **Then** the action is refused.
7. **Given** two people with the `projects` permission and the same project, **When** each opens it, **Then** each sees only their own sessions.

---

### User Story 4 - Give the agent the project's context (Priority: P3)

In a project session the agent knows which folder it is working in and reads the folder's own instructions, so it follows the project's conventions. In an ordinary conversation it receives neither.

**Why this priority**: It turns a shell into a useful coding partner, but the foundation is already useful without it.

**Independent Test**: Put an instruction document in the folder, ask the agent in a project session where it is working and what instructions it follows, and confirm it names the folder and the instruction. Ask the same agent in an ordinary conversation and confirm it does not claim a project context.

**Acceptance Scenarios**:

1. **Given** a project session, **When** the person asks where the agent is working, **Then** the agent names the project's folder.
2. **Given** a project folder with an instruction document, **When** a project turn runs, **Then** the agent follows that document's guidance.
3. **Given** an ordinary conversation, **When** it runs, **Then** it has no project folder, no project instructions and no project workspace shell.

---

### Edge Cases

- A person without the `projects` permission: no section, no CTA, no project access, even by direct address.
- No projects: the Projects section is hidden; the desktop empty conversation offers Open a Project; the web interface offers nothing.
- The folder was moved, renamed or deleted: the project is shown as unavailable, its history remains readable, and commands are refused until the folder is available again. Re-opening the same path restores the project; a folder moved to a new path is a new project, because relinking is out of scope for this foundation.
- The chosen folder is the application's own data folder: it is refused, because the agent could damage the application's data.
- The same folder is opened twice: the existing project is reused; no duplicate appears.
- Two projects have the same name: both are allowed; they are distinguished by their folders.
- A project is removed while a turn is running: removal is refused until the turn ends.
- A project is removed with no turn running: it and its sessions disappear from every sidebar, but its history is kept and no file in the folder is deleted. Re-opening the same folder restores the sessions.
- The workspace shell is turned off for a project: the agent has no project shell at all; it is not offered a tool that is guaranteed to fail.
- A command runs longer than its limit: it and its child processes are stopped, and the agent receives the partial output with a clear timeout result.
- A command writes a very large amount of output: the result is capped and marked as truncated.
- A command exits non-zero: this is a normal result, not a tool failure.
- The requested working directory is outside the project folder: it is refused; the command itself is still not sandboxed and can reach other paths, which the trust disclosure says.
- Two sessions of one project run commands at the same time: both run; neither waits for the other; file changes can race.
- A scheduled task or a subagent piece runs: it does not receive the project folder or its shell. Only a background report that wakes inside the project session does.
- A browser without the permission opens a bookmarked project address: the project is refused, not shown read-only.
- A browser user with the permission tries to rename, remove or disable a project: only the project's owner or a person with the whole-workspace project permission may do so.
- The `projects` permission is revoked while a session is open: new turns are refused; a turn already running may finish.
- The app is closed while a project turn runs: the turn is shown as interrupted on return, the same as any other conversation.
- The app is in the tray with the window hidden: project turns continue to run and their shell calls complete.
- Web access is off: projects are reachable only in the desktop app.
- A project folder is read-only or permissions change under the app: commands fail with the system's error, and the project remains listed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The desktop app MUST let a person open a project by choosing a local folder, and MUST verify that the folder exists and is a directory.
- **FR-002**: Opening a project MUST show, before the first command can run, that commands run with the person's operating-system account, that there is no sandbox, and that only trusted folders should be opened.
- **FR-003**: A folder MUST have at most one project per workspace; opening the same folder again MUST reuse the existing project rather than create a duplicate.
- **FR-004**: Projects MUST persist across app restarts and MUST be available to any agent the person can view. A session inside a project MUST belong to one agent and one person.
- **FR-005**: The sidebar MUST show a Projects section above Sessions, listing each project and that person's sessions for the current agent; the section MUST be hidden when there are no projects.
- **FR-006**: Project sessions MUST NOT appear in the regular Sessions list.
- **FR-007**: A project MUST support multiple independent sessions, each with its own history and files; switching between them MUST not mix their content.
- **FR-008**: A person MUST be able to start a new session in a project from the project row and from a project with no sessions.
- **FR-009**: In a project session the agent MUST have a workspace shell that runs commands with the project folder as its default working directory; an optional relative working directory MUST stay inside the folder.
- **FR-010**: The workspace shell MUST return the command's standard output, standard error, exit status and duration. A non-zero exit MUST be a normal result, not a failed tool call.
- **FR-011**: The workspace shell MUST enforce a command time limit, stop the command and its child processes when it is exceeded, and return the partial output with a clear timeout outcome.
- **FR-012**: The workspace shell MUST cap output size and mark any truncated result.
- **FR-013**: The workspace shell MUST work for a project turn even when the conversation is not the screen the person is looking at; it MUST NOT require the person to keep the chat open or focused.
- **FR-014**: The project folder, project instructions and workspace shell MUST be available only inside project sessions. Ordinary conversations, scheduled task runs and subagent work MUST NOT receive them.
- **FR-015**: In a project session the agent MUST be told the project's name and folder, and MUST be told that its shell runs there.
- **FR-016**: If the project folder contains a project instruction document, the agent MUST receive its guidance up to a bounded size; if the document is longer, the agent MUST be told that it was truncated. A missing document MUST be harmless.
- **FR-017**: Each project MUST have a switch that turns the workspace shell on or off. When off, the agent MUST NOT be offered the project shell.
- **FR-018**: A `projects` permission MUST gate the Projects section, the project list, opening project sessions, starting project turns and the workspace shell. Without it, every project access MUST be refused and nothing project-related may be shown.
- **FR-019**: The desktop workspace owner MUST hold the `projects` permission by default. It MUST be grantable through roles, and a revocation MUST affect new turns immediately.
- **FR-020**: A person with the `projects` permission and web access MUST be able to list existing projects, open their sessions, start a new session, chat and run the workspace shell from the web interface.
- **FR-021**: Creating a project MUST require the desktop app. The web interface MUST NOT offer creation, and a direct creation attempt over the web MUST be refused.
- **FR-022**: Renaming a project, removing it and changing its shell switch MUST be limited to the project's owner or a person with the whole-workspace project permission.
- **FR-023**: A project session reached through web access MUST visibly indicate that commands run on the host machine.
- **FR-024**: Project sessions MUST belong to one person. Another permitted person MAY see the project, but MUST NOT see or continue that person's sessions.
- **FR-025**: Removing a project MUST hide it and its sessions from every sidebar, MUST NOT delete any file in the folder, and MUST keep the history so that re-opening the same folder restores the sessions.
- **FR-026**: If a project's folder is missing or unreadable, the project MUST be shown as unavailable, its history MUST remain readable, commands MUST be refused, and re-opening the same folder MUST restore normal operation.
- **FR-027**: Two sessions of one project MAY run commands at the same time; the system MUST NOT serialise them or make one wait for the other. The interface MUST show which sessions are running.
- **FR-028**: The application's own data workspace MUST be refused as a project folder.
- **FR-029**: Project sessions MUST participate in the existing conversation features: history, files and attachments, search, notifications and usage. Replies MUST notify the person who owns the session, and usage MUST be attributable to that person and project.
- **FR-030**: The foundation MUST NOT include git integration, diff review, a file editor, per-session worktrees, a terminal UI or sandboxing.

### Key Entities

- **Project**: A folder on the host machine that an agent may work in. It has a name, a folder path, a shell switch, an owner, a created time and a removed state. It is shared across agents; the person who created it is recorded.
- **Project session**: A conversation with one agent inside one project, owned by one person. It has its own history and files, and can run the workspace shell.
- **Project instructions**: An optional document inside the project folder that guides the agent in that project.
- **Project permission**: A role permission that allows a person to see and use projects. It is available in a whole-workspace scope and an owned-agents scope, like the other agent permissions.
- **Workspace shell call**: One command run in the project folder, with a working directory, output, error output, exit status, duration, timeout outcome and truncation marker.
- **Remote project session**: A project session reached through the web interface, where commands still run on the host machine.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A desktop user can open a folder, start a session and receive a first shell result in under 2 minutes.
- **SC-002**: 100% of completed shell calls report an exit status, output and duration; 100% of commands over their limit are stopped and reported as timed out.
- **SC-003**: A project with three sessions lists all three, switching preserves each session's content, and 0 project sessions appear in the regular Sessions list.
- **SC-004**: A person without the `projects` permission sees no Projects section and 100% of their project access attempts are refused.
- **SC-005**: A remote user with the permission can open an existing project and receive a shell result in under 3 minutes; 100% of web creation attempts are refused.
- **SC-006**: Removing a project deletes 0 files from its folder, and re-opening the same folder restores its prior sessions in 100% of tested cases.
- **SC-007**: A project turn started before the person navigates away completes with shell access in 100% of tested cases; no test requires the chat to remain open.
- **SC-008**: Two sessions running commands in one project both complete without waiting on each other; measured wall-clock time is not the sum of the two commands.
- **SC-009**: 100% of remote project sessions display the host-execution indicator.
- **SC-010**: In permission tests, 0 project sessions appear in another person's session list or search results.
- **SC-011**: In 100% of tested project sessions, the agent names the correct folder; in ordinary conversations, 0 project folders or project instructions are supplied to the agent.

## Assumptions

- Project hosting is a desktop-app capability. Remote access is through the desktop app's opt-in web interface; a standalone server installation is not a project host in this foundation.
- The workspace has one desktop owner. The owner holds every permission, including `projects`; other people receive it through their roles.
- Projects are shared across agents, while sessions are personal and per agent. Two people in the same project do not share a session.
- A project folder is an ordinary local folder on the host. Cloud drives, network shares and remote mount protocols are not special-cased.
- Commands run with the host user's operating-system account and are not sandboxed. The trust disclosure is the safeguard.
- A sensible command limit, output cap and instruction-document size bound are configuration defaults; the exact values are implementation choices.
- Removing a project is a soft removal: history is kept and no file is deleted. A hard delete is out of scope.
- Interaction between projects and scheduled tasks or subagents is deferred. Only a background report that wakes inside a project session inherits the project.
- Concurrent sessions in one project are allowed and unsynchronised; the person is responsible for commands that conflict.
- Two people may hold the `projects` permission at the same time; the permission is the trust boundary for remote command execution on the host.
- This foundation excludes git integration, diff review, a file editor, per-session worktrees, a terminal UI, sandboxing and a remote/cloud workspace provider.
- The design follows the existing desktop and web interface conventions, including the same screens for both editions where the capability is available.
