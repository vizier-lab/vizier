# Feature Specification: Scratchpad Tools

**Feature Branch**: `020-scratchpad-tools`

**Created**: 2026-10-17

**Status**: Draft

**Input**: User description: "let's make \"scratchpad\" category tools for agent to write and read temporary files. consists of three tools: write_tmp(name, content), append_tmp(name, content), and read_tmp(name). all tmp will be deleted once all turns of a chat complete"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Keep working notes within a turn (Priority: P1)

An agent working through a multi-step request (gathering search results, drafting a long answer, collecting tool outputs across several scripts) needs somewhere to put intermediate text that is too big or too transient for its long-term memory and too awkward to carry in its own context. It writes a named scratch file, reads it back later in the same turn, and replaces it as its work evolves.

**Why this priority**: This is the core value: a place for working state that does not pollute memory and does not need to be re-sent through the model's context on every step.

**Independent Test**: With an agent on the offline test provider, call `write_tmp` with a name and content, then `read_tmp` with the same name in the same turn, and confirm the content returned is identical. Call `write_tmp` again with new content and confirm `read_tmp` returns only the new content.

**Acceptance Scenarios**:

1. **Given** a conversation with no scratch files, **When** the agent calls `write_tmp("plan", "step 1")` and then `read_tmp("plan")`, **Then** it receives `"step 1"`.
2. **Given** a scratch file `plan` containing `"step 1"`, **When** the agent calls `write_tmp("plan", "step 2")`, **Then** `read_tmp("plan")` returns `"step 2"` (the earlier content is replaced, not kept).
3. **Given** no scratch file named `notes`, **When** the agent calls `read_tmp("notes")`, **Then** it receives a clear "not found" result that also lists the scratch files that do exist in the conversation, and the call does not fail the turn or the script.

---

### User Story 2 - Accumulate output incrementally (Priority: P2)

An agent processing many items (for example, summarising each of twenty documents in a loop) appends each result to one scratch file as it goes, then reads the whole file once at the end to compose its reply.

**Why this priority**: Appending is the natural pattern for loops and batch work; without it the agent must read, concatenate and rewrite the full content on every step.

**Independent Test**: Call `append_tmp("log", "a")`, `append_tmp("log", "b")`, `append_tmp("log", "c")`, then `read_tmp("log")`, and confirm the result is `"abc"`.

**Acceptance Scenarios**:

1. **Given** no scratch file named `log`, **When** the agent calls `append_tmp("log", "first")`, **Then** the file is created and `read_tmp("log")` returns `"first"`.
2. **Given** a scratch file `log` containing `"first"`, **When** the agent calls `append_tmp("log", "\nsecond")`, **Then** `read_tmp("log")` returns `"first\nsecond"` — content is joined exactly as given, with no separator added.

---

### User Story 3 - Scratch space spans the whole chat burst, then disappears (Priority: P2)

A person sends several messages in quick succession; the agent's turns for them are queued and run one after another. Scratch files written in an earlier queued turn are still there for the later ones. Once the conversation has no running turn and nothing waiting, every scratch file in that conversation is deleted, so nothing stale leaks into a later exchange and no clean-up is needed by the agent.

**Why this priority**: Automatic clean-up is what makes the scratchpad safe to use freely; the cross-turn visibility within a burst is what makes it more than a per-call variable.

**Independent Test**: Send two messages to the same conversation so the second is queued behind the first; in the first turn write a scratch file, in the second read it (succeeds). After both turns finish, send a third message and confirm `read_tmp` reports the file as not found.

**Acceptance Scenarios**:

1. **Given** turn A is running and turn B is queued in the same conversation, **When** turn A writes `draft` and turn B reads `draft`, **Then** turn B receives turn A's content.
2. **Given** the conversation's last running turn finishes with nothing queued, **When** a new message later starts a turn, **Then** the conversation has no scratch files.
3. **Given** a turn that fails, times out or is aborted by the person, **When** no other turn for that conversation is running or queued, **Then** its scratch files are deleted just as after a normal completion.

---

### User Story 4 - Conversations never see each other's scratch files (Priority: P1)

Two people talk to the same agent at the same time in different conversations (or the same person in two topics), and each conversation's work uses a scratch file named `draft`. Neither conversation can see or overwrite the other's.

**Why this priority**: Without isolation, the feature would cause cross-conversation data leaks between people — a privacy defect, not just a bug.

**Independent Test**: In two concurrent conversations with the same agent, write different content to `draft` in each, then read `draft` in each and confirm each gets its own content.

**Acceptance Scenarios**:

1. **Given** conversations X and Y with the same agent, **When** X writes `draft` = `"x"` and Y writes `draft` = `"y"`, **Then** reading `draft` in X returns `"x"` and in Y returns `"y"`.
2. **Given** two different agents, **When** each writes a scratch file with the same name, **Then** neither sees the other's.

---

### Edge Cases

- **Invalid names**: an empty name, a name longer than 128 characters, or one containing path separators, `..`, control characters or other characters outside letters, digits, `-`, `_` and `.` is refused with a clear message stating the allowed form; nothing is written.
- **Size limit**: a write or append that would make a single scratch file exceed 1 MiB is refused with a message giving the limit and the file's current size; the existing content is left unchanged (no partial write).
- **Count limit**: creating a new scratch file when the conversation already has 64 is refused with a message listing the existing names; overwriting or appending to an existing file is still allowed.
- **Empty content**: `write_tmp(name, "")` creates or empties the file; `append_tmp(name, "")` is accepted and changes nothing (but creates the file if absent).
- **Non-text content**: content is text; the scratchpad does not accept or return binary data.
- **Background job reports**: a background job's report arrives as a new turn after the launching conversation has typically gone idle, so scratch files written before launch are normally gone by then. Agents must not rely on the scratchpad to carry state to a woken report turn (this is stated in the tool descriptions).
- **Subagent and delegated work**: subtasks and delegated agents run in their own conversations and therefore have their own, separate scratchpads; they do not see the launcher's scratch files.
- **Server restart**: scratch files are kept in durable storage, so a crash or restart can leave some behind. Since no turn can be running before the server starts, every scratch file left over is deleted at startup; after a restart every conversation starts with an empty scratchpad.
- **Dream cycle**: a dream run is a conversation of its own, so it has its own scratchpad, separate from every person's conversation, deleted when the dream run ends.
- **Agent update/restart or deletion**: when an agent instance is stopped (update, restart, delete), its conversations' running turns end; the scratchpads of conversations left with nothing running or queued are deleted. Deleting an agent deletes all its scratch files.
- **Conversation deleted** while it has scratch files: the scratch files are deleted with it.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a tool category named `scratchpad`, available to every agent, containing exactly three tools: `write_tmp`, `append_tmp` and `read_tmp`.
- **FR-002**: `write_tmp(name, content)` MUST create the named scratch file with the given content, or replace its entire content if it already exists, and MUST report the resulting size.
- **FR-003**: `append_tmp(name, content)` MUST add the given content to the end of the named scratch file exactly as given (no separator inserted), creating the file if it does not exist, and MUST report the resulting size.
- **FR-004**: `read_tmp(name)` MUST return the full current content of the named scratch file; if no such file exists it MUST return a non-fatal "not found" result listing the names of the conversation's existing scratch files.
- **FR-005**: Scratch files MUST be scoped to one conversation of one agent: a scratch file is visible to, and changeable by, only the turns of the conversation in which it was written.
- **FR-006**: Scratch files MUST remain available across all turns of a conversation that run back-to-back, i.e. while any turn of that conversation is running or queued.
- **FR-007**: When a conversation has no running turn and no queued turn, the system MUST delete all of that conversation's scratch files, whether the last turn completed, failed, timed out or was aborted.
- **FR-008**: Scratch files MUST be kept in the system's durable storage, not held in process memory, so that their size never adds to the server's memory use. Any scratch files left over from before a restart MUST be deleted at startup, before any turn runs, so no scratch content from before a restart is readable after it.
- **FR-009**: Deleting a conversation or an agent MUST delete its scratch files.
- **FR-010**: The system MUST validate names (1–128 characters; letters, digits, `-`, `_`, `.`; not `.` or `..`) and refuse invalid ones with a message stating the allowed form.
- **FR-011**: The system MUST cap each scratch file at 1 MiB and each conversation at 64 scratch files, refusing over-limit writes atomically (existing content unchanged) with a message stating the limit.
- **FR-012**: All scratchpad refusals (invalid name, limits, not found) MUST be returned as data the agent can read and recover from, never as a failure that ends the turn or the script.
- **FR-013**: The tool descriptions MUST tell the agent that scratch files are temporary, private to the conversation, deleted once the conversation goes idle, and not suitable for anything that must persist (memory) or reach a later background report.
- **FR-014**: Scratch files MUST NOT be stored in or affect the agent's long-term memory, memory search, version history, or the conversation's visible history beyond the record of the tool calls themselves.
- **FR-015**: The scratchpad tools MUST be reachable the same way as the agent's other categorised tools (listed under their category and callable from the agent's scripts).
- **FR-016**: The scratchpad tools MUST be available to the dream cycle, scoped to the dream run's own conversation and deleted when that run ends, under the same rules as any other conversation.

### Key Entities

- **Scratch file**: a named piece of text belonging to one conversation of one agent. Attributes: name, content, size. Lifetime: from first write or append until its conversation goes idle, the conversation or agent is deleted, or the next server startup.
- **Conversation scratchpad**: the set of scratch files of one conversation (at most 64), created implicitly on first write and removed entirely when the conversation goes idle.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In an end-to-end check with the offline test provider, 100% of write → read, append → read and overwrite → read sequences in one turn return exactly the expected content.
- **SC-002**: After a conversation's last turn ends with nothing queued, a scratch file written during it is readable by 0 later turns (verified across completed, failed and aborted turns).
- **SC-003**: Across two concurrent conversations using identical scratch file names, 0 reads return the other conversation's content.
- **SC-004**: After a server restart, including one that interrupted running turns, 0 scratch files from before the restart are readable.
- **SC-007**: A dream run can write and read scratch files, and 0 of them are readable once it ends.
- **SC-005**: Every refused call (invalid name, over size, over count, not found) returns a message an agent can act on, and 0 of them end the turn or the script.
- **SC-006**: Reading or writing a scratch file of up to 1 MiB completes fast enough that it does not noticeably lengthen a turn (well under one second).

## Assumptions

- "A chat" means one conversation (session/topic) with one agent; "all turns complete" means the conversation has no running turn and no queued turn. Queued turns from the same burst therefore share one scratchpad.
- Scratch content is plain text; binary data, attachments and session files are out of scope (the existing session-file tools cover files people send).
- No `list_tmp` or `delete_tmp` tool is provided, per the requested three-tool set; listing is available through the "not found" result of `read_tmp`, and overwriting with empty content serves as clearing.
- Scratch files are kept in durable storage, not in RAM, because their total size across conversations is unbounded in practice. Leftovers from before a restart are cleared at startup.
- The dream cycle's restricted tool set gains the three scratchpad tools.
- Limits of 1 MiB per file and 64 files per conversation are reasonable defaults for working notes and are not configurable in this feature.
- People do not see scratch files in the WebUI or any channel; they are an agent's private working space.
