# Feature Specification: Project Language Intelligence

**Feature Branch**: `054-project-lsp`

**Created**: 2026-11-07

**Status**: Draft

**Input**: User description, captured from the conversation and issue #156: attach language servers to a project so the agent can ask semantic questions about the code (where is this defined, who uses it, what is its type, what does this file declare) and learn whether its own edits broke anything. Vizier finds servers but does not install them. Detection is automatic per file, so a repository holding several projects works without setup. Changing code through a language server (rename, code actions, formatting) is explicitly out of scope.

## Context

In a project chat, the agent's only view of the code is text search plus whole-file reads, and its only feedback on an edit is running a build through the shell. Text search cannot tell two things with the same name apart or answer "who calls this", so the agent reads whole files to resolve one symbol. A build takes minutes and needs the shell switched on, so an agent often ends its turn without knowing that it broke a call site.

Language servers already hold these answers. This specification lets the agent use them: it learns right after an edit whether the code still holds together, and it can ask about meaning instead of matching text.

A language server is a program running on the host machine, and it can execute code from the project it opens (for example build scripts). It is on by default for new projects, because a person already chose the folder and a server only starts when it is needed. A person can turn it off per project.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The agent learns that its edit broke something (Priority: P1)

A person in a Build chat asks the agent to change a function's signature. The agent edits the file, and the result of the edit says that the change caused two errors, with their locations and messages. The agent fixes them in the same turn, without running a build and without the person pointing out the breakage.

**Why this priority**: This is the largest single gain. It turns an agent that ends its turn with a broken tree into one that corrects itself, and it works even with the shell off.

**Independent Test**: In a project with the feature on and a server ready, have the agent make an edit that introduces a type error. The edit's result lists the error. Have it fix the error; the next edit's result reports no errors.

**Acceptance Scenarios**:

1. **Given** a ready server for the file's language, **When** the agent edits or writes the file and the edit introduces an error, **Then** the result of that edit includes counts of errors and warnings and the individual problems with their locations.
2. **Given** a ready server, **When** an edit introduces no problems, **Then** the result says so explicitly.
3. **Given** a server that has not answered within the wait limit, **When** an edit completes, **Then** the edit still succeeds and its result says that diagnostics are not yet known, never that the file is clean.
4. **Given** no server is running or available for the file, **When** an edit completes, **Then** the edit behaves exactly as it does today, and the result does not imply that the file was checked.
5. **Given** an edit refused for any existing reason (the file was not read, or the text did not match), **When** the refusal returns, **Then** it is unchanged by this feature.

---

### User Story 2 - A person can see what it is doing and turn it off (Priority: P1)

A person opens a new project, and language intelligence is already on. They can turn it off for that project. They can see which servers are running for the project and whether each is starting, ready, failed or not installed. If a server is missing, they see which one and how to install it.

**Why this priority**: These are programs running on a person's machine, so they must be able to see and stop them. A missing server is the most common first experience, so it has to be clear rather than a silent nothing.

**Independent Test**: With the switch off, confirm no server process starts during a chat. Turn it on in a project containing a language with no server installed, and confirm the status names the missing server and gives an install hint. Turn it off and confirm all of that project's servers stop.

**Acceptance Scenarios**:

1. **Given** a new project, **When** it is created, **Then** language intelligence is on and no server runs until the agent reads, edits or asks about a file. A project created through the API is on unless the request turns it off.
1a. **Given** a project created before this feature, **When** Vizier is upgraded, **Then** language intelligence stays off for it until a person turns it on.
2. **Given** a person who can manage the project, **When** they turn it on, **Then** servers may start the next time the agent reads, edits or asks about a file, with no further step.
3. **Given** a person who cannot manage the project, **When** they try to change the switch, **Then** the change is refused.
4. **Given** the switch is on and a server is missing, **When** the agent touches a file of that language, **Then** the agent and the person are both told which server is missing and how to install it, and Vizier installs nothing.
5. **Given** running servers, **When** the switch is turned off, **Then** those servers stop, and the next turn no longer offers the capability.

---

### User Story 3 - The agent asks about meaning instead of matching text (Priority: P2)

A person asks the agent, in Plan or Build mode, how a function is used. The agent looks up its definition, finds every reference, reads its type signature and lists what a file declares, with no whole-file reads, and answers with exact locations.

**Why this priority**: It makes every turn in a large code base cheaper and more accurate, and it gives Plan mode real understanding. It comes after Story 1 because it improves work the agent can already do by other means.

**Independent Test**: In a fixture with two same-named functions in different files, ask for the references of one. Only its real uses come back, which a text search cannot do. Confirm the same works in Plan mode.

**Acceptance Scenarios**:

1. **Given** a ready server, **When** the agent asks for a definition, the references, the type signature, the symbols of a file or a symbol search, **Then** it receives locations relative to the chat's folder, with 1-based lines.
2. **Given** a Plan chat with the switch on, **When** the agent uses these capabilities, **Then** they work, and no file-changing capability becomes available.
3. **Given** the agent knows a file, a line and a symbol's name, **When** it asks about that symbol, **Then** it does not have to supply a column.
4. **Given** a symbol's name that does not appear on the given line, **When** the agent asks, **Then** it gets a clear "not found on that line" result rather than an answer about something else.
5. **Given** an answer longer than the result limit, **When** it returns, **Then** it is cut off at the limit, marked as truncated, and suggests how to narrow the question.
6. **Given** a server still starting, **When** the agent asks, **Then** it gets an immediate "starting" result rather than waiting into a timeout, and can continue its turn by other means.

---

### User Story 4 - A repository holding several projects just works (Priority: P2)

A person's repository holds two separate Rust workspaces and three TypeScript packages, like Vizier's own. They configure nothing. When the agent works in one package, the right server starts for that package, rooted correctly, and the others stay stopped until the agent touches them. A worktree gets its own servers, so answers reflect the worktree's code and not the main folder's.

**Why this priority**: Real repositories look like this, and rooting a server in the wrong place gives wrong or empty answers that look correct. It is second to the basic capabilities because single-root projects work without it.

**Independent Test**: In a fixture with two independent workspaces of one language and a package of another, have the agent touch one file in each. Exactly three servers run, each answering only about its own part. A chat that touches only one part starts only one.

**Acceptance Scenarios**:

1. **Given** a file inside a nested package, **When** the agent touches it, **Then** the server is rooted according to that language's rule, including at the workspace root rather than the member package where the language has workspaces.
2. **Given** two files under one root, **When** the agent touches both, from one chat or from two chats and their subagents on the same folder, **Then** they share one server.
3. **Given** a project with no marker above a file, **When** the agent touches it, **Then** the chat's folder is the root.
4. **Given** a chat in a worktree, **When** it asks a question, **Then** the answer reflects the worktree's files and never the main folder's or another worktree's.
5. **Given** the limit on running servers is reached, **When** another is needed, **Then** the least recently used one stops, and it starts again if it is needed later.

---

### User Story 5 - A person points a language at a specific server (Priority: P3)

A person wants a different server for a language than the built-in choice, or has one installed outside the usual search path. They set the command for that language on the project.

**Why this priority**: The built-in table and the search path cover most cases, but without an override a person with an unusual setup has no way in.

**Independent Test**: Set an override for a language to a fixture server and confirm that one starts instead of the default. Remove it and confirm the default returns.

**Acceptance Scenarios**:

1. **Given** an override for a language, **When** a server is needed for it, **Then** the override is used before the built-in table.
2. **Given** an override whose command cannot be found, **When** a server is needed, **Then** the status names that command as the problem, with no silent fallback to the default.
3. **Given** an override is changed, **When** the change is saved, **Then** running servers for that language stop and the next use starts the new one.
4. **Given** files in the repository or an agent tool, **When** either tries to define a server command, **Then** it has no effect.

---

### Edge Cases

- **A server crashes**: it restarts automatically, at most 3 times in 10 minutes. After that it shows as failed, and calls say so, until a person restarts it, the switch is turned off and on again, the override changes or Vizier restarts.
- **A command is found but cannot run** (for example a `rustup` proxy whose component is not installed): this reads as not installed, with the install hint, not as a crash.
- **A server hangs**: calls to it end with a clear result within the tool's time limit, and other agents, chats and the desktop app are unaffected.
- **A file changes behind the server's back** (a shell command, a formatter, a code generator, a branch switch): the next answer must reflect the current bytes on disk for the file asked about, and the server must be told about changes made through the shell or a branch switch.
- **A definition lies outside the project** (a standard library or dependency): the location is marked as outside the project and names the file without exposing an absolute host path. Hover still gives the signature.
- **Worktree areas**: results from a main-folder chat never point into the project's `.vizier/` area.
- **Non-ASCII text**: lines and symbol positions stay correct in files with accented letters, CJK text or emoji, and in files with Windows line endings.
- **A large folder is requested for diagnostics**: the answer is capped and marked as truncated, as with any other result.
- **A file with no known language**: the agent gets a "no language server for this file type" result, and edits behave as today.
- **The project folder goes missing**: the capability stops being offered, as with other project tools, and the servers stop.
- **Project removed, worktree deleted, agent deleted, Vizier shut down**: every server belonging to it stops.
- **The switch is turned off during a turn**: the turn keeps running, and its later calls get the same clear refusal as FR-003. Edits still succeed, without diagnostics.
- **A second chat turns up while a server is indexing**: it shares that server and sees the same "starting" state.
- **A subagent piece launched from a project chat**: it shares the launching chat's servers and is under the same mode.
- **Dreams, tasks and ordinary conversations**: the capability does not exist there, like every other project tool.

## Requirements *(mandatory)*

### Functional Requirements

**Control and consent**

- **FR-001**: Each project MUST have a language intelligence switch, changeable only by someone who can manage the project. It MUST be on for projects created after this feature ships, and off for projects that already exist.
- **FR-002**: *(Removed by decision: the interface shows no warning when the switch is turned on. See Assumptions and FR-042.)*
- **FR-003**: With the switch off, no server MUST start for the project and the capability MUST NOT be offered; a script calling it by name MUST get a clear refusal.
- **FR-004**: Turning the switch off MUST stop that project's servers.
- **FR-005**: The switch MUST be independent of the shell switch: neither implies the other.
- **FR-006**: The agent MUST learn the switch's current state with each message, and changing it MUST NOT restart the agent or disturb a running turn.

**Finding servers**

- **FR-007**: Vizier MUST NOT download or install language servers.
- **FR-008**: Vizier MUST ship a built-in table covering at least Rust, TypeScript/JavaScript, Python, Go, C/C++ and Lua, giving each language its server, its file types, its root rule and its install hint.
- **FR-009**: The server for a language MUST be resolved from the project's override first, then the built-in table, located on the host's search path.
- **FR-010**: A person who can manage the project MUST be able to set and remove a per-language override of command, arguments and environment.
- **FR-011**: Server commands MUST NOT be taken from files in the repository, MUST NOT be changeable by an agent's tools, and MUST NOT resolve to an executable inside the project folder unless a person set that override.
- **FR-012**: A missing server MUST produce a distinct result that names it and gives its install hint, to both the agent and the person, never a silent no-op.
- **FR-013**: A desktop app opened from the operating system's launcher MUST find servers a person installed in their usual tool locations, even when the launcher gives it a minimal search path.

**Detection and ownership**

- **FR-014**: Detection MUST be automatic and per file: the file type selects the language, and that language's root rule, searching upward and stopping at the chat's folder, selects the root. The chat's folder is the fallback root.
- **FR-015**: Root rules MUST be per language, and MUST place a member of a multi-package workspace at the workspace's root where the language has that concept.
- **FR-016**: There MUST be one server per project, root and language, shared by every chat and subagent piece on that folder. Each worktree MUST have its own.
- **FR-017**: A server MUST start only when the agent reads, edits or asks about a file of its language under its root. Listing or searching files MUST NOT start one. A read MUST begin the start in the background, so that the server is usually ready by the first question. Turning on the switch MUST NOT scan the project or start anything.
- **FR-018**: Vizier MUST limit the number of running servers across the whole instance, 6 by default and changeable by the operator, stopping the least recently used server when a new one is needed.
- **FR-019**: A server idle for 10 minutes MUST stop.

**What the agent can ask**

- **FR-020**: The agent MUST be able to get, in Plan and Build modes: the problems in a file or folder; the definition of a symbol; its references; its type signature and documentation; the symbols a file declares; and a search for symbols by name.
- **FR-021**: The agent MUST be able to identify a position by file, line and symbol name, and MUST NOT be required to supply a column.
- **FR-022**: Results MUST be compact structured data usable from a script: paths relative to the chat's folder, 1-based lines, and no absolute host paths.
- **FR-023**: Results MUST be limited to about 50 items and 16 KiB, marked as truncated beyond that, with a hint on how to narrow the question.
- **FR-024**: A location outside the project MUST be marked as such and MUST NOT reveal an absolute host path.
- **FR-025**: The capability MUST NOT change any file: renaming, code actions and formatting through a server are out of scope, and a server's request to apply an edit MUST be refused.
- **FR-026**: The agent MUST be told when to prefer these capabilities over text search.

**Diagnostics after an edit**

- **FR-027**: When a server is ready for a file, the result of a successful edit or write MUST include error and warning counts and the individual problems, capped as in FR-023.
- **FR-028**: Vizier MUST wait at most 3 seconds for those diagnostics. If they have not arrived, the edit MUST still succeed and its result MUST say that they are not yet known.
- **FR-029**: A result MUST distinguish "checked and clean" from "not known". It MUST NOT present stale diagnostics from before the edit as current.
- **FR-030**: Diagnostics MUST NOT block the end of a turn: no turn is refused or held because problems remain.
- **FR-031**: The read-before-edit rule and the existing write path MUST be unchanged.

**Staying correct**

- **FR-032**: Before answering about a file, Vizier MUST make sure the server has that file's current bytes on disk.
- **FR-033**: After a shell command or a branch switch in a folder with running servers, those servers MUST be told that files may have changed.
- **FR-034**: Positions MUST be correct for any text the file contains, whichever text encoding a server uses for positions.

**Lifecycle and safety**

- **FR-035**: A call against a server that is starting MUST return a distinct "starting" result immediately. No call may fail with a bare timeout because a server is slow to start.
- **FR-036**: Server lifecycle work MUST NOT hold up a turn, another agent or the desktop app's interface. A stuck server affects only calls to that server.
- **FR-037**: A crashed server MUST restart automatically at most 3 times in 10 minutes and then show as failed, with the reason available to the agent and the person.
- **FR-038**: Removing a project, deleting a worktree, deleting an agent, losing the folder and shutting Vizier down MUST stop the servers involved. A server MUST NOT outlive Vizier, including after Vizier exits abnormally, on every supported platform where the operating system allows it.
- **FR-039**: Servers MUST NOT receive Vizier's own secrets through their environment or their startup messages.
- **FR-040**: Language intelligence MUST work on the platforms the desktop app supports, including Windows.

**Status**

- **FR-041**: A person who can use a project MUST be able to see, for each of its servers: language, root, state (starting, ready, failed, not installed) and the reason for a failure. Someone who can manage the project MUST be able to restart a failed server and check again for a missing one. The status MUST update without a page reload. The status lives in a popup from the chat's status strip, which also holds the switch and the overrides (`mockups/lsp-status-popup.html`).
- **FR-042**: Project documentation MUST cover installing a server per language, the switch and what it permits (including that a server may execute code from the project, in Plan mode too), overrides, and the not-installed state.

### Key Entities

- **Language intelligence setting**: per project; whether the feature is on, plus any per-language overrides. Survives restarts.
- **Server override**: on a project, for one language; command, arguments and environment set by a person.
- **Language definition**: built in; a language's file types, default server, root rule and install hint.
- **Server instance**: running, in memory only; identified by project, root and language, with a state, a last-used time and a failure reason.
- **Diagnostic**: a problem a server reports in a file; severity, location and message. Belongs to one version of a file's content and is never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With a server ready, an edit that introduces an error reports that error in its own result within 3 seconds, in at least 95% of attempts on the verification fixture.
- **SC-002**: No question or edit made while a server is starting fails with a timeout; every question gets an answer or a "starting" result within 1 second.
- **SC-003**: With the switch off, a project chat starts no extra processes, and every existing project behaviour is unchanged.
- **SC-004**: Finding every real use of a function costs at least 80% less text in the conversation than the text search and file reads needed to get the same answer, measured on the verification fixture, and same-named but unrelated functions are never included.
- **SC-005**: In a fixture with at least two independent workspaces of one language and one package of another, a person sets nothing up, the correct number of servers runs, and every answer is correct for its part of the repository.
- **SC-006**: No server process remains 10 seconds after any of: switch off, project removal, worktree deletion, agent deletion, normal shutdown.
- **SC-007**: A deliberately hung or crashing server never delays another chat, another agent or the desktop interface.
- **SC-008**: Every location reported for a fixture containing non-ASCII text and Windows line endings points at the correct line and symbol.
- **SC-009**: A person with no server installed learns which one to install and how, within one attempt, from the agent's message or the project status, without reading documentation.

## Assumptions

- **Where it runs**: anywhere projects exist today, on the host holding the project folder. No new way of creating projects, and no server is shipped in the Docker image or the desktop app.
- **Server installation is the person's job**, either by hand or by asking the agent in Build mode with the shell on. A managed download could come later as an opt-in feature.
- **Choosing the folder is the consent**: new projects are on by default, by decision, with no separate prompt. Per-call approval is out of scope and belongs to the guardrails work (#119). Language servers are not sandboxed, and they write their own caches into the project (for example build output), as a build would. This is an exception to "Plan mode changes nothing". By decision, it is covered in the project documentation (FR-042) and not shown in the interface.
- **No cross-language or cross-root answers**: references are searched within the file's own root, so a call from another workspace or language is not found. This is a known limit for repositories like Vizier's.
- **No code viewer**: the first consumer is the agent. A person sees the switch, overrides and server status only; a problems panel or gutter marks are out of scope.
- **No hard gate**: an agent is shown problems but is free to finish with them, since a multi-step change is often broken partway.
- **Limits are defaults chosen from the issue and from experience** (10-minute idle stop, 6 servers, 3-second wait, 50 items / 16 KiB) and may be tuned during planning without changing this specification's intent.
- **Heavy servers are accepted**: a worktree starts a fresh index for its own servers, which is slow and memory-hungry in large repositories. The limit on running servers and the switch are the protections in v1.
- **Text search is not replaced**: tree-sitter or other local parsing is a possible later complement, not part of this work.
- **UI work follows the design rule**: the switch, overrides and status are drawn before they are coded.
- **Dependencies**: the project coding tools and their mode rules (041), projects and the shell switch (040), and worktrees (041).
