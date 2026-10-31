# Feature Specification: Project Coding Workspace

**Feature Branch**: `041-project-coding-workspace`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description, captured from the conversation: enhance projects (040) with dedicated file tools (read with an optional line range, write, exact-text edit, list, grep, glob) in a new `coding` category that also takes the workspace shell; a plan/build mode toggle where only build mode may write files or run commands; a dedicated project conversation type instead of reusing the WebUI's; `@file` mentions in the message box; a git interface (see diffs, switch branches); and worktrees under `.vizier/<agent-id>/<name>`, each one being one chat, deletable together. Line-range writes, GitHub (`#123`) references, merging/rebasing from the UI, agent-created worktrees and a worktree setup command are explicitly out of scope.

## Context

Spec 040 gave an agent a folder and a shell. Everything the agent does there goes through that shell: reading a file is `cat`, searching is `grep`, editing is a heredoc. That is slow to read in the conversation, easy to get wrong (a heredoc that silently mangles a file), and all-or-nothing: an agent that may look at a project may also change it.

This specification makes a project a place to do coding work: purpose-built file tools, a read-only mode for planning, a way to point the agent at a file, a view of what changed, and isolated worktrees for parallel lines of work. It builds on 040 and changes nothing about how a project is opened, shared or removed.

Approval of individual actions (ask before running this command) is **not** part of this specification; it belongs to the guardrails work in issue #119.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The agent reads, searches and edits files with dedicated tools (Priority: P1)

A person in a project chat asks the agent to change something. The agent lists files, searches the code, reads the relevant part of a file, and edits it by naming the exact text to replace. The conversation shows each step in plain terms (which file, which lines, how many matches, what changed) instead of raw shell commands.

**Why this priority**: Every other story builds on the agent being able to work with files reliably. It is also the largest single improvement in reliability: an edit that names the exact text either applies or is refused, never silently corrupts a file.

**Independent Test**: In a project with a small code base, ask the agent to rename a function used in three files. The trail shows a search, reads and three edits; the files on disk are correct; no shell command was used.

**Acceptance Scenarios**:

1. **Given** a project chat, **When** the agent reads a file with a line range, **Then** it receives only those lines, numbered as in the file, and is told when the file has more.
2. **Given** a large file, **When** the agent reads it without a range, **Then** it receives a bounded portion and is told where to continue.
3. **Given** an edit whose text to replace appears exactly once, **When** the agent applies it, **Then** the file changes in that one place.
4. **Given** an edit whose text to replace is absent or appears more than once (and replacing every occurrence was not asked for), **When** the agent applies it, **Then** it is refused with the reason and the file is unchanged.
5. **Given** a file changed on disk after the agent last read it in this chat, **When** the agent tries to edit it, **Then** the edit is refused and the agent is told to read it again.
6. **Given** a project with ignored files (dependencies, build output), **When** the agent lists or searches files, **Then** ignored files are skipped unless the agent asks for them by path.
7. **Given** a path outside the project folder (absolute, or escaping through `..` or a link), **When** any file tool is used with it, **Then** it is refused.
8. **Given** the agent's tool categories, **When** it looks for the file tools and the workspace shell, **Then** they are all in one `coding` category, and the shell is no longer listed under shell.

---

### User Story 2 - Plan before building (Priority: P1)

A person starts a chat in **Plan** mode. The agent can look at everything (read, list, search, see the git state) but cannot change a file or run a command. It answers with a plan. The person reviews it and switches the chat to **Build**, or presses **Build this plan**, and the agent carries it out.

**Why this priority**: It is the main safety and trust control for coding work, and the person's way to check the agent's understanding before anything changes on disk.

**Independent Test**: In Plan mode, ask the agent to fix a bug. Confirm it reads files and proposes a plan, and that no file changes and no command runs. Press Build this plan; confirm the agent now edits files.

**Acceptance Scenarios**:

1. **Given** a new project chat, **When** it opens, **Then** it is in Plan mode.
2. **Given** a chat in Plan mode, **When** the agent works, **Then** it is offered only the tools that read (files, listings, searches, git state), and is told it is planning.
3. **Given** a chat in Plan mode, **When** the agent proposes a plan, **Then** it is shown as a plan with a **Build this plan** action.
4. **Given** a proposed plan, **When** the person chooses Build this plan, **Then** the chat switches to Build mode and the agent starts on the plan.
5. **Given** a chat in Build mode, **When** the agent works, **Then** it may also write and edit files and, if the project's shell switch is on, run commands.
6. **Given** a project whose shell switch is off, **When** a chat is in Build mode, **Then** the agent may edit files but is not offered commands.
7. **Given** a turn running in one mode, **When** the person switches mode, **Then** the running turn keeps its tools and the new mode applies from the next turn.
8. **Given** two chats in the same project, **When** one is switched to Build, **Then** the other keeps its own mode.
9. **Given** a chat is reopened later, or a turn resumes after a restart, **When** the agent works, **Then** the chat's saved mode applies.

---

### User Story 3 - Work in an isolated worktree (Priority: P2)

A person wants to try a change without touching the main folder. From the project, they create a worktree: they name it, choose the branch to start from, and a new chat opens inside it. The agent's files, commands and git view in that chat are the worktree's, not the main folder's. When they are done, they delete the chat and its worktree goes with it.

**Why this priority**: It lets several lines of work run in one project at once, on separate branches, without stepping on each other or on the person's own checkout.

**Independent Test**: Create a worktree from the main branch, ask the agent to change a file there, and confirm the main folder is unchanged. Delete the chat and confirm the worktree folder is gone.

**Acceptance Scenarios**:

1. **Given** a project in a git repository, **When** the person creates a worktree named `fix-ps` from `master`, **Then** a worktree is created at `.vizier/<agent-id>/fix-ps` inside the project on a new branch `fix-ps`, and one chat opens in it.
2. **Given** a worktree chat, **When** the agent reads, edits, searches or runs commands, **Then** it acts in the worktree, and a path that leaves the worktree (including into the main folder) is refused.
3. **Given** a project with worktrees, **When** the sidebar is shown, **Then** the project lists its main-folder chats and each worktree chat with its branch, and the main folder's own listings and searches skip `.vizier/`.
4. **Given** a worktree is created, **When** the person runs `git status` in the main folder, **Then** the `.vizier/` folder does not appear, and no tracked file of the project was changed to achieve that.
5. **Given** a worktree name that is already used in this project, or is not a valid folder and branch name, **When** the person tries to create it, **Then** it is refused with the reason before anything is created.
6. **Given** a worktree chat with a turn running, **When** the person tries to delete it, **Then** deletion is refused until the turn ends.
7. **Given** a clean worktree, **When** the person deletes its chat, **Then** after one confirmation the chat and worktree are removed.
8. **Given** a worktree with uncommitted or untracked changes, **When** the person deletes its chat, **Then** the confirmation names how many files will be lost, and nothing is removed unless they confirm.
9. **Given** a worktree deletion, **When** the person confirms, **Then** they may also delete its branch; this is pre-selected only when the branch is merged into the branch it started from.
10. **Given** a worktree whose folder was deleted outside Vizier, **When** the chat is opened, **Then** it is marked unavailable, history stays readable, and deleting it needs no confirmation about changes.
11. **Given** a project that is not a git repository, **When** the person looks for worktrees, **Then** worktree creation is not offered.

---

### User Story 4 - See what changed and switch branches (Priority: P2)

From a project chat, the person sees which branch the folder is on and which files have uncommitted changes, opens any changed file to see its diff, and switches the folder to another branch, or creates a new one.

**Why this priority**: The person needs to review the agent's work without leaving Vizier, and to put the folder on the right branch before the agent starts.

**Independent Test**: After the agent edits two files, open the changes view and confirm both are listed with added/removed line counts and a readable diff. Switch to another branch and confirm the header and the agent's next turn see the new branch.

**Acceptance Scenarios**:

1. **Given** a project or worktree chat in a git repository, **When** it is shown, **Then** the header shows the current branch and the number of changed files.
2. **Given** uncommitted changes, **When** the person opens the changes view, **Then** each changed, added, deleted or untracked file is listed with its added/removed line counts.
3. **Given** a changed file, **When** the person selects it, **Then** its diff against the last commit is shown.
4. **Given** the branch switcher, **When** the person picks another branch, **Then** the folder is switched to it and the header updates.
5. **Given** a branch already checked out in a worktree, **When** the person looks at it in the switcher, **Then** it is marked with that worktree's name and cannot be picked.
6. **Given** uncommitted changes that would be overwritten, **When** the person switches branch, **Then** the switch is refused with a plain explanation and nothing changes.
7. **Given** any turn running in that folder, **When** the person tries to switch branch, **Then** it is refused until the turn ends.
8. **Given** the switcher, **When** the person creates a new branch from the current one, **Then** the folder is switched to it.
9. **Given** a project folder that is not a git repository, or a machine without git, **When** the chat is shown, **Then** the branch and changes views are absent and everything else works.
10. **Given** a chat in either mode, **When** the agent needs the git state, **Then** it can read status, diffs and recent history through dedicated read-only tools.

---

### User Story 5 - Point the agent at a file with `@` (Priority: P3)

While typing a message, the person types `@` and part of a file name; a list of matching files and folders from the chat's folder appears. They pick one and it becomes a mention. When the message is sent, the agent receives that file's current contents (or, for a folder, its listing) with the message.

**Why this priority**: A convenience that saves a round of searching and makes the person's intent unambiguous; the agent can already find files without it.

**Independent Test**: Type `@agent` in a project chat, pick a file from the list, send "explain this". The agent's reply refers to the file's actual contents without having read it first.

**Acceptance Scenarios**:

1. **Given** a project or worktree chat, **When** the person types `@` followed by text, **Then** matching files and folders from that chat's folder are suggested, ignored files excluded.
2. **Given** a picked file, **When** the message is sent, **Then** the agent receives the file's contents as they are when the turn starts, with line numbers.
3. **Given** a mentioned file larger than the size limit, **When** the message is sent, **Then** the agent is told the file is too large to include and to read it with the file tools.
4. **Given** a mentioned folder, **When** the message is sent, **Then** the agent receives a shallow listing of it.
5. **Given** a sent message with mentions, **When** the conversation is reopened, **Then** the message shows the mentions as they were typed, and the file contents are not stored in the conversation.
6. **Given** a mention of a path that no longer exists when the turn starts, **When** the agent works, **Then** it is told that file is missing.

---

### User Story 6 - Project chats are their own kind of conversation (Priority: P3)

Project chats are recognised as project chats everywhere: usage reports list them as project work, notifications link to them, and they never appear in the regular Sessions list or search. Chats created before this change keep their history and keep working.

**Why this priority**: Mostly invisible to the person, but it removes a class of bugs where a project chat is mistaken for an ordinary WebUI conversation, and every other story relies on it.

**Independent Test**: Upgrade an install that has project chats from 040. Confirm they open with full history, appear only under their project, are reported as project activity in usage, and their notifications link to them.

**Acceptance Scenarios**:

1. **Given** project chats created before this change, **When** Vizier starts after upgrading, **Then** they keep their history, files, pending turns and background jobs, under their project.
2. **Given** a project chat, **When** usage is reported, **Then** its activity is reported as project work, separately from ordinary chat.
3. **Given** a project chat reply while the person is elsewhere, **When** the notification is shown, **Then** it links to that project chat.
4. **Given** a project chat, **When** the regular Sessions list, search or picker is shown, **Then** it is not listed.
5. **Given** a person's project chat, **When** another person without access to that project looks for it, **Then** it is not visible to them.
6. **Given** project and worktree chats with activity since the last dream, **When** the agent's dream cycle runs, **Then** it reflects on them like any other person's conversation, and the dream itself is offered no coding tools and does not act in the project folder.

### Edge Cases

- A file tool is given a binary file: reading reports it as binary with its size rather than returning bytes; searches skip it.
- A file has Windows line endings or no trailing newline: edits keep the file's existing line endings and final-newline state.
- The agent writes a file in a folder that does not exist yet: the folders are created, inside the project only.
- Two chats edit the same file: the stale-read refusal makes the second editor re-read before its edit lands.
- A search matches thousands of lines: results are bounded and the agent is told they were cut off.
- A symbolic link inside the project points outside it: reading or writing through it is refused.
- The person switches the main folder's branch while a worktree chat is running: allowed, since the worktree is a separate folder; only turns in the main folder block the switch.
- Git is installed but the project is a subfolder of a repository: the git views reflect the enclosing repository; worktrees are still created under the project folder's `.vizier/`.
- A worktree folder exists but is no longer registered with git (or the reverse): Vizier shows it as unavailable and deletion cleans up what remains.
- A worktree's tools see build tools or file watchers in the main folder reacting to `.vizier/` contents: documented as a known limitation (see Assumptions).
- The project is removed (soft removal, 040) while it has worktrees: the worktree folders stay on disk and the removal confirmation says so.
- The person sends `@` text that matches nothing: it is sent as plain text, not a mention.
- Subagents launched from a project chat (in the background or blocking): each piece works in the same folder (the worktree, for a worktree chat) with the coding tools of the launching turn's mode; a piece never gets `propose_plan`, and two pieces editing the same file are kept apart by the stale-read refusal. The report a background job wakes in the project chat uses the chat's saved mode.
- Work delegated to a different agent, scheduled tasks and dreams get no coding tools: a project belongs to one agent.

## Requirements *(mandatory)*

### Functional Requirements

**Coding tools**

- **FR-001**: A project chat MUST offer the agent tools to read a file (optionally a line range), list files, search file contents with a pattern, find files by name pattern, write a whole file, and edit a file by replacing exact text.
- **FR-002**: There MUST be no tool that writes by line range.
- **FR-003**: Read results MUST carry the file's line numbers, be bounded in size, and say how to continue when cut off.
- **FR-004**: An edit MUST be refused when its text is absent, when it matches more than once without the replace-all option, or when the file changed on disk since the agent last read it in that chat; a refused edit MUST leave the file unchanged.
- **FR-005**: Listing and searching MUST skip files the project's ignore rules exclude, and, in the main folder, the `.vizier/` folder.
- **FR-006**: Every file tool MUST refuse a path that resolves outside the chat's folder, including through `..` or symbolic links.
- **FR-007**: The file tools and the workspace shell MUST be in one tool category named `coding`; the workspace shell MUST no longer be in the shell category. The tools MUST remain available only in project chats and in subagent pieces launched from one, never in ordinary conversations, scheduled tasks, dreams, or work delegated to another agent.
- **FR-007a**: A subagent piece launched from a project chat MUST work in that chat's folder (its worktree, when it has one) with the coding tools of the launching turn's mode, minus `propose_plan`, and MUST keep them if the piece resumes after a restart. The project owner's permission MUST be re-checked when the piece starts.
- **FR-008**: Read-only git tools (status, diff, recent history) MUST be offered in the `coding` category when the chat's folder is in a git repository and git is available.

**Plan and Build modes**

- **FR-009**: Every project chat MUST have a mode, Plan or Build, saved with the chat; new chats MUST start in Plan.
- **FR-010**: In Plan mode the agent MUST be offered only tools that do not change the folder or run commands, and MUST be told it is planning.
- **FR-011**: In Build mode the agent MUST additionally be offered writing and editing, and the workspace shell when the project's shell switch is on.
- **FR-012**: The agent in Plan mode MUST be able to present a plan that the person can turn into a Build-mode request with one action.
- **FR-013**: A mode change MUST apply from the next turn; a running turn MUST keep the tools it started with. Resumed turns and turns woken by background reports MUST use the chat's saved mode.

**Project conversations**

- **FR-014**: Project chats MUST be a distinct kind of conversation, recognised as such by session listings, usage accounting, notifications, the dream cycle and access checks, without relying on the conversation's name.
- **FR-015**: Project chats created before this change MUST be carried over at startup with their history, files, pending turns, background jobs, usage and notifications intact.
- **FR-016**: Usage MUST report project chat activity as its own activity.
- **FR-016a**: Project and worktree chats MUST be among the conversations the dream cycle reflects on, as they are before this change; the dream cycle MUST NOT be offered the coding tools or act in a project folder.

**Mentions**

- **FR-017**: The message box in a project or worktree chat MUST suggest files and folders from that chat's folder when the person types `@`, excluding ignored files.
- **FR-018**: When a turn starts, each mentioned file's current contents (bounded per file and per message) or each mentioned folder's shallow listing MUST be given to the agent with the message; files over the limit or missing MUST be reported to the agent instead.
- **FR-019**: The saved conversation MUST keep the message as typed and the list of mentions, not the mentioned contents.

**Git interface**

- **FR-020**: A project or worktree chat in a git repository MUST show the folder's current branch and its number of changed files.
- **FR-021**: The person MUST be able to see every uncommitted change in the folder with added/removed line counts, and the diff of each changed file against the last commit.
- **FR-022**: The person MUST be able to switch the folder to an existing branch and create a new branch from the current one.
- **FR-023**: A branch switch MUST be refused while a turn is running in that folder, and when git refuses it because of uncommitted changes; the refusal MUST be explained in plain words. A branch checked out in another worktree MUST be shown as such and not be selectable.
- **FR-024**: When git is unavailable or the folder is not in a repository, the git interface and git tools MUST be absent and nothing else MUST be affected.

**Worktrees**

- **FR-025**: In a project in a git repository, a person who may use the project MUST be able to create a worktree by giving a name, a starting branch, and a branch name (defaulting to the worktree name).
- **FR-026**: A worktree MUST be created at `.vizier/<agent-id>/<name>` inside the project folder, and `.vizier/` MUST be hidden from the project's git status without changing any tracked file.
- **FR-027**: Creating a worktree MUST open exactly one chat in it; a worktree and its chat MUST be one-to-one.
- **FR-028**: In a worktree chat, every file tool, the workspace shell, mentions and the git interface MUST act on the worktree folder and refuse paths outside it.
- **FR-029**: The sidebar MUST show a project's worktree chats under the project, each with its branch and whether it has changes.
- **FR-030**: Deleting a worktree chat MUST also remove its worktree; it MUST be refused while a turn is running; with uncommitted or untracked changes the confirmation MUST state how many files will be lost.
- **FR-031**: Deletion MUST offer to delete the worktree's branch, pre-selected only when the branch is merged into its starting branch.
- **FR-032**: A worktree whose folder is missing MUST be shown as unavailable with history readable, and its deletion MUST clean up what remains without asking about changes.
- **FR-033**: Removing a project MUST leave its worktree folders on disk and say so in the confirmation.

**Access**

- **FR-034**: The git interface, mentions and worktree creation MUST be available to whoever may use the project, from the desktop app and from a browser with web access, under the existing project permissions; switching the main folder's branch and deleting another person's worktree chat MUST require managing the project.

### Key Entities

- **Project chat**: a conversation belonging to one project and one person, optionally bound to one worktree; carries its mode (Plan or Build).
- **Worktree**: an isolated checkout of the project's repository on its own branch, belonging to one project and created by one person; has a name, branch, starting branch, location, creation time, and an availability state. One-to-one with a project chat.
- **Mention**: a reference in a sent message to a file or folder in the chat's folder, resolved to contents when the turn starts.
- **Change**: an uncommitted difference in a chat's folder (path, kind, added and removed lines, diff), always read live, never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a project chat, an agent completes a multi-file rename or similar edit with no shell command used for reading, searching or editing files.
- **SC-002**: No edit through the file tools ever leaves a file partially changed: across a test suite of refused edits (absent text, ambiguous text, stale read), 100% leave the file byte-identical.
- **SC-003**: In Plan mode, across every tool the agent can reach, zero can change a file or run a command.
- **SC-004**: A person can go from a proposed plan to the agent building it with one action.
- **SC-005**: A person can create a worktree and be typing in its chat in under 30 seconds on a repository the size of Vizier's.
- **SC-006**: After the agent edits files, the person can see every change and its diff without leaving Vizier.
- **SC-007**: 100% of project chats from an existing 040 install remain reachable with full history after upgrading.
- **SC-008**: File suggestions for `@` appear while typing in under half a second on a repository of 10,000 tracked files.
- **SC-009**: A project without git, or a machine without git, shows no git-related controls and no errors.

## Assumptions

- The `.vizier/` location inside the project is accepted with its known side effect: build tools or file watchers running in the main folder (for example a hot-reload watcher) may react to changes inside worktrees. Making the location configurable is out of scope.
- A fresh worktree contains only tracked files (no submodules, dependencies or untracked local files). Preparing it is left to the agent in Build mode, guided by the project's own instructions file; a per-project setup command is deferred to a separate issue.
- Per-call approval (ask before acting) is out of scope and will be provided by the guardrails work (#119); Plan mode is the only gate this specification adds.
- Only people create worktrees; agent-created worktrees are deferred to a separate issue.
- GitHub pull request and issue references (`#123`), and merging or rebasing a worktree from the UI, are deferred to separate issues.
- Committing is done by the agent through the workspace shell in Build mode or by the person outside Vizier; there is no commit UI in this specification.
- File size and result limits follow the existing workspace shell's style (bounded output with a truncation notice); exact numbers are chosen during planning.
- Diffs are shown against the last commit only; comparing branches is out of scope.
- New UI (mode toggle, plan card, changes panel and diff view, branch switcher, worktree dialog and sidebar grouping, mention suggestions) is designed in FigCraft before implementation, per the project's design-first rule.
