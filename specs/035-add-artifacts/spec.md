# Feature Specification: Permanent Artifacts

**Feature Branch**: `035-add-artifacts`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "Handle issue #115 — Add artifacts: permanent files promoted from transient session files."

## Context

Today everything a conversation produces lands in the same bucket: **session files**. A session file belongs to one conversation, is listed in the chat side panel, can be read by the agent, can be attached to an outgoing reply, and can be attached to a memory. Nothing distinguishes a throwaway upload from a file the agent deliberately produced, and nothing is ever cleared, so session files accumulate forever.

This feature introduces a second, permanent tier: **artifacts**. Session files become explicitly **transient** — per-conversation scratch that a person can clear. An agent or a person can **promote** a session file into an artifact: a named, agent-owned file that lives until a person deletes it. Artifacts can be **replaced** with newer content, attached to outgoing messages and memories, and browsed in a new **Artifacts gallery**.

Two decisions keep the first release simple:

- **No automatic collection.** Session files are cleared only by an explicit action — the chat side panel's Clear session files, the `/clear-session-file` channel command, deleting the conversation, or deleting the agent. There is no retention period, background collector or size-based cleanup in this release. This is deliberately deferred so the permanence model can settle first.
- **Promotion copies, it does not link.** The artifact gets its own copy of the bytes, and the source session file stays where it is. Nothing needs to mark which session files are artifacts, and clearing session files can never damage an artifact. The source is not hidden or flagged after promotion; it remains an ordinary session file until it is cleared.

The feature deliberately does not turn every existing session file into an artifact. Permanence is an explicit step. Existing messages and memories that reference session files keep working while those files exist; clearing the files makes those references unavailable. Artifacts are not shared across agents, are not public links, and do not keep a restorable history of earlier versions in this release. The generated-UI media surface continues to use the conversation's session files.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Promote a file into an artifact that outlives the conversation (Priority: P1)

An agent builds a quarterly report in a conversation and saves it as a session file. It promotes that file into an artifact named `Q3-report.pdf`. Days later the person clears the conversation's session files. The artifact is still in the agent's gallery, unchanged and downloadable, because promotion copied the bytes. When a newer report is ready, the agent replaces the artifact's content from a new session file; the name stays the same and everyone who refers to the artifact sees the new content. If the replacement fails, the previous content is still there.

**Why this priority**: Promotion and replacement are the feature. Without a durable object there is no artifact, and without replacement the object would be a one-off snapshot rather than a maintained file.

**Independent Test**: On the offline `dummyplug` provider, create a session file in a conversation, promote it, clear the conversation's session files, and download the artifact; confirm its bytes match. Then replace it from a different session file and confirm the name, version and content changed while the old content is gone. Attempt a replacement from a missing session file and confirm the artifact is unchanged.

**Acceptance Scenarios**:

1. **Given** a session file in the agent's current conversation, **When** the agent promotes it with a valid name, **Then** an artifact exists for that agent with that name, the source file's media type and size, version 1, and the conversation and source filename recorded as its origin.
2. **Given** a promoted artifact, **When** the person clears the origin conversation's session files, **Then** the artifact remains listed and its content is byte-for-byte identical when downloaded.
3. **Given** a promoted artifact, **When** the origin conversation is deleted, **Then** the artifact remains listed and downloadable.
4. **Given** an existing artifact and a newer session file, **When** the agent replaces the artifact's content, **Then** the artifact keeps its name, its version increases by one, and every reference shows the new content.
5. **Given** a failed replacement because the source file is missing or unreadable, **When** the attempt ends, **Then** the artifact still holds its previous content and version, and the agent receives a readable error.
6. **Given** an artifact named `Q3-report.pdf`, **When** the agent tries to create another artifact with the same name, **Then** the create is refused with a message pointing to replacement, and the original artifact is unchanged.
7. **Given** a person viewing a conversation's files, **When** they promote one, **Then** the artifact is created for that agent with the person recorded as the actor and the source file's name as the default name.
8. **Given** a session file from another agent or another conversation, **When** promotion is attempted, **Then** it is refused and no artifact is created.
9. **Given** a promoted session file, **When** the conversation's files are listed again, **Then** the source still appears as an ordinary session file with no artifact marker, and the artifact is found in the gallery.
10. **Given** a zero-length or otherwise unreadable source, **When** promotion is attempted, **Then** it is refused with a clear explanation and no artifact is created.

---

### User Story 2 - Browse and manage artifacts in the gallery (Priority: P1)

A person opens the selected agent's **Artifacts** page. They see every permanent file the agent owns, newest first, with its name, type, size, version and when it last changed. They search by name, filter by type and date, and open one to preview it. A contract PDF opens in the viewer; a CSV shows its rows; an image shows inline; anything unsupported offers download. They see which conversation it came from, who created or last replaced it, and the messages and memories that use it. They delete an old artifact after a confirmation that names it and says how many places use it. Someone without the right to manage the agent does not get a delete action, and a direct attempt is refused.

**Why this priority**: A permanent object nobody can find or manage is just hidden storage. The gallery is how people use, audit and clean up the new tier.

**Independent Test**: With an agent holding several artifacts of different types, open the gallery, verify ordering and metadata, search and filter, preview each supported type, download one, open its detail view, and delete one after confirming the dialog. Repeat as a person without management rights and confirm delete is unavailable and refused. Check the empty state with an agent that has no artifacts.

**Acceptance Scenarios**:

1. **Given** a selected agent, **When** the person opens the app's navigation, **Then** an Artifacts entry is offered for that agent and opens its gallery.
2. **Given** several artifacts, **When** the gallery renders, **Then** they are listed newest first with name, type, size, version and last change time, and the total count and total size are shown.
3. **Given** a long list, **When** the person searches by name, filters by type or date, or loads more, **Then** the list narrows or pages without duplicates or gaps and keeps the chosen filters.
4. **Given** an image, PDF, CSV, markdown or text artifact, **When** it is previewed, **Then** it is shown in the matching viewer; **Given** any other type, **Then** the person is offered download instead of a broken preview.
5. **Given** any artifact, **When** the person downloads it, **Then** the bytes match the artifact's current content and the download carries its name.
6. **Given** an artifact's detail view, **When** it opens, **Then** it shows the origin conversation, the actor and time of the current version, the change history, and every message and memory that references the artifact.
7. **Given** a permitted person, **When** they delete an artifact and confirm, **Then** it disappears from the gallery, its content is removed, and references show that it was deleted.
8. **Given** a person without permission to manage the agent, **When** they open the gallery, **Then** they may browse and download what their access allows but are offered no delete action, and a direct delete attempt is refused.
9. **Given** an agent with no artifacts, **When** the gallery opens, **Then** a specific empty state explains what an artifact is and how one is made.
10. **Given** a phone viewport, keyboard-only use or a screen reader, **When** the gallery, filters, preview and delete confirmation are used, **Then** every action is reachable, labelled and operable without relying on colour alone.
11. **Given** the gallery, **When** the person opens its activity view, **Then** promotions, replacements and deletions are listed newest first with the actor, action, artifact name, time and source, including records of artifacts that no longer exist.

---

### User Story 3 - Clear a conversation's session files explicitly (Priority: P2)

A conversation is finished with. Its uploaded spreadsheets, generated drafts and images have served their purpose. The person opens the chat details panel and chooses **Clear session files**. A confirmation says it will remove this conversation's transient files, that artifacts are copies and are not affected, and that older messages or memories that still point at these files will no longer open them. They confirm and the panel reports how many files were removed. On Discord or Telegram they send `/clear-session-file` (shown as `/clear_session_file` on Telegram) and confirm in the same way. A turn in progress is never cleared underneath it.

**Why this priority**: Explicit clearing is the storage half of the feature. Without it, session files still accumulate forever, but automatic collection is intentionally deferred to a later change.

**Independent Test**: On a running binary, create files in two conversations, promote one file, and attach an artifact to a message and a memory. Clear the first conversation from the side panel and the second with the channel command; confirm the counts, that each conversation only affected its own files, that every artifact still downloads, and that older message/memory references show an unavailable placeholder. Then check cancelling, clearing an empty conversation, and clearing while a turn is running.

**Acceptance Scenarios**:

1. **Given** an open conversation with session files, **When** the person chooses Clear session files in the side panel and confirms, **Then** every session file in that conversation is removed, the action reports how many were removed, and no other conversation is affected.
2. **Given** a Discord or Telegram conversation with session files, **When** the person sends `/clear-session-file` and confirms, **Then** the same clear happens for that conversation, with the same count and safety rules.
3. **Given** any clear action, **When** the confirmation is shown, **Then** it says artifacts are unaffected and that older messages or memories referencing the cleared files will no longer open them.
4. **Given** a promoted artifact, **When** its source conversation is cleared, **Then** the artifact and its content are unchanged and it remains usable in the gallery, messages and memories.
5. **Given** a conversation with a turn running or queued, **When** clear is attempted, **Then** it is refused with a message explaining that the turn must finish first, and no file is removed.
6. **Given** a clear confirmation that is cancelled, or a conversation with no files, **When** the action ends, **Then** nothing is removed and the person is told what happened.
7. **Given** two conversations with session files, **When** one is cleared, **Then** the other conversation's files, artifacts and history are unchanged.
8. **Given** an older message or memory that references a cleared session file, **When** it is shown or read, **Then** the missing file is shown as unavailable without breaking the rest of the message or memory.
9. **Given** a conversation with no turn running or queued, **When** it is deleted, **Then** its session files are removed and its artifacts remain.
10. **Given** an agent is deleted, **When** the deletion completes, **Then** its session files, artifacts and artifact records are gone and no orphaned content remains.
11. **Given** a file is being promoted while the conversation is cleared, **When** both complete, **Then** either the artifact was created from the complete copy and survives, or the promotion fails cleanly and no artifact exists.

---

### User Story 4 - Attach only artifacts to replies and memories (Priority: P2)

An agent finishes a chart and wants to send it to the person. It promotes the chart, then attaches the artifact to its reply. The reply carries the artifact's current content. Later the artifact is replaced, and the same message shows the newer file; if the artifact is deleted, the message shows a clear "artifact deleted" placeholder instead of a broken download. The agent can also attach artifacts to memories, and those memories still resolve after the source conversation's files are cleared. Files people upload into a conversation remain transient inputs. On Discord and Telegram an artifact larger than the platform allows fails with a clear notice while the reply text is still delivered.

**Why this priority**: Attachments are where artifacts become visible in normal use. Restricting new outgoing attachments to artifacts is what makes message and memory references trustworthy.

**Independent Test**: On `dummyplug`, promote an image, attach it with `send_attachment`, and confirm the reply carries it. Replace the artifact and confirm the same message now serves the new bytes; delete it and confirm a tombstone. Write a memory using an artifact attachment, clear the source conversation's session files, then read the memory and confirm the attachment still resolves. Try to attach a plain session file and confirm the refusal names promotion. Send an oversized artifact to Discord/Telegram and confirm the text arrives with an explicit failure notice.

**Acceptance Scenarios**:

1. **Given** an artifact, **When** the agent attaches it to a reply, **Then** the outgoing message carries the artifact's current content and name.
2. **Given** a session file that is not an artifact, **When** the agent tries to attach it, **Then** the attempt is refused with guidance to promote it first, and the reply is not given a broken attachment.
3. **Given** a message that attached an artifact, **When** the artifact is replaced, **Then** the message resolves the current content the next time it is shown or delivered.
4. **Given** a message that attached an artifact, **When** the artifact is deleted, **Then** the message shows a clear deleted-artifact placeholder naming it, and the rest of the message still renders.
5. **Given** an artifact, **When** a memory is written or updated with it attached, **Then** the memory records the artifact reference and the reference survives the source conversation's files being cleared.
6. **Given** a memory that references a deleted artifact, **When** the memory is read, **Then** the missing attachment is named as deleted without failing the rest of the memory.
7. **Given** a person uploading a file into a conversation, **When** the agent reads it, **Then** it is still a transient session file and does not need to be an artifact to be read.
8. **Given** an agent attaching an artifact it does not own, **When** the attempt is made, **Then** it is refused.
9. **Given** an artifact larger than a channel's delivery limit, **When** it is attached to a reply there, **Then** the reply text is still delivered and the attachment failure is reported explicitly, with no silent drop.
10. **Given** an artifact whose content is missing from storage, **When** it is attached or downloaded, **Then** the failure is reported as unavailable and no partial or empty file is passed off as the artifact.

---

### User Story 5 - Provenance is recorded and boundaries hold (Priority: P3)

A person wants to know where an artifact came from and who changed it. The gallery's detail view shows who promoted it, from which conversation and file, when it was replaced and by whom, and when it was deleted. An agent can only ever create, list, read and replace its own artifacts; it cannot delete any. A shared viewer cannot delete; only the owner or someone allowed to manage the agent can. When an agent is deleted, its artifacts and their change records go with it.

**Why this priority**: Accountability and boundaries make permanent files safe to keep, but the feature is useful before every audit detail is exposed.

**Independent Test**: Promote as the agent, replace as a person, and confirm both appear in the change history with actor, time and source. Attempt a cross-agent read, replace, attach and delete and confirm each is refused. Delete the agent and confirm its artifacts, content and records are gone.

**Acceptance Scenarios**:

1. **Given** an artifact is promoted or replaced, **When** its detail view opens, **Then** the change history names the actor (a person or the agent), the action, the time and the source conversation and file.
2. **Given** an artifact is deleted, **When** its audit is inspected by a person who may manage the agent, **Then** the deletion record remains until the agent is deleted, without preserving the content.
3. **Given** agent A and agent B, **When** A lists, reads, replaces or attaches B's artifact, **Then** every attempt is refused and A cannot tell whether the name exists on B.
4. **Given** an agent, **When** it is offered its tools, **Then** it has no way to delete an artifact; a direct delete attempt is refused.
5. **Given** the gallery, **When** a person views or downloads an artifact, **Then** only artifacts of agents they may see are offered, regardless of which conversation produced them.
6. **Given** a person loses access to an agent while the gallery is open, **When** they next read, download or delete, **Then** the operation is refused and nothing changes.
7. **Given** an agent is deleted, **When** deletion completes, **Then** its artifacts, content, references and change records are removed and no other agent's artifacts are affected.

---

### Edge Cases

- **Name rules**: an empty or whitespace-only name, a name longer than 255 characters, a name containing `/`, `\` or control characters, or a name of `.` or `..` is refused with a message explaining the rule; nothing is created.
- **Name matching**: artifact names are case-sensitive within one agent, so `Report.pdf` and `report.pdf` are different artifacts; the same name on two agents is allowed and unrelated.
- **Duplicate create**: creating an artifact with a name that already exists never overwrites it; replacement is a separate, explicit action.
- **Replacing with identical content**: the replacement succeeds, the version increases once, and one change record is written.
- **Concurrent replacement**: two replacements at the same time leave one complete version and two change records; the artifact never holds mixed or partial content.
- **Concurrent promotion and clear**: a file being promoted is either copied completely before the clear reaches it or the promotion fails cleanly; a completed artifact is never affected by the clear.
- **Source disappears mid-promotion**: promotion fails with a clear message and creates no artifact.
- **Promoted source stays put**: promotion copies; the source session file is still listed, can still be read, and can be promoted again under a different name, or cleared like any other session file.
- **Content missing on disk**: the gallery marks the artifact unavailable, download and attach report an explicit error, and no empty file is substituted.
- **Artifact referenced by several messages and memories**: the detail view lists all of them, and deletion warns with the count.
- **Artifact deleted while referenced**: every reference keeps rendering, showing a deleted placeholder; memories keep the name and report it as deleted when read.
- **Artifact replaced while a message is open**: the open message shows the new content on its next load; a download already in progress completes with a whole file, never a mix.
- **Artifact larger than a channel limit**: the reply text is delivered and the attachment failure is reported; the artifact itself remains usable in the gallery and other channels.
- **Very large artifact**: promotion and download report a clear failure where the source cannot be read completely; an interrupted read never truncates the stored artifact.
- **Clock or time-zone change**: ordering uses recorded event times, not the browser's clock.
- **Conversation deleted while a turn is running**: deletion is refused until the turn ends, as is clearing; the turn keeps its files while it runs.
- **Clear with zero files**: the action says there was nothing to clear and changes nothing.
- **Clear confirmation cancelled or expired**: nothing is removed.
- **Clear while a turn is running or queued**: refused; the person is told to wait for the turn to finish.
- **Clear in one conversation**: no other conversation's files, artifacts or history change.
- **Clearing a file referenced by an older message or memory**: the reference becomes unavailable; the rest of the message or memory still renders.
- **No automatic expiry**: a session file that nobody clears remains available indefinitely; storage growth is visible and accepted until automatic collection is added later.
- **Agent updated or restarted**: artifacts and their references survive without migration.
- **Existing files at first availability**: no artifact is created retroactively; existing referenced content keeps working while the files exist.
- **Dream cycle**: it cannot create, replace or delete artifacts, so unattended reflection cannot make permanent content.
- **Scheduled task or background job**: they can list, read, create and replace their agent's artifacts under the same rules as an ordinary turn.
- **No permission to manage the agent**: the gallery is read-only and delete is refused, even if the person promoted the artifact.
- **Two people act at once**: one person's delete and another's replace resolve consistently — either the replace succeeds before the delete and is then deleted, or the delete succeeds and the replace reports not found.
- **Platform command naming**: `/clear-session-file` is the canonical name; Discord accepts it as written, while Telegram's command rules turn it into `/clear_session_file`.

## Requirements *(mandatory)*

### Functional Requirements

**Artifact model and ownership**

- **FR-001**: An artifact MUST belong to exactly one agent and MUST be a permanent object until a permitted person or an agent deletion removes it.
- **FR-002**: An artifact MUST have a name that is unique within its agent and case-sensitive, and a stable identity independent of its name's content or current version.
- **FR-003**: Artifact names MUST be validated before any change: 1–255 characters after trimming, not empty, no `/`, `\`, control characters, `.` or `..`; a violation MUST be refused with the rule explained and MUST NOT create or change anything.
- **FR-004**: An artifact MUST record its media type, byte size, current version (starting at 1), origin conversation, source session filename, the actor of its creation, and created and last-updated times.
- **FR-005**: An artifact's bytes MUST be preserved exactly as promoted; no transcoding, recompression or text conversion may change them.
- **FR-006**: Artifacts MUST persist across agent updates, agent restarts, vizier restarts, conversation deletion and session-file clearing.
- **FR-007**: Deleting an agent MUST delete its artifacts, their content, their references and their change records, and MUST NOT affect another agent's artifacts.
- **FR-008**: An artifact MUST NOT be readable, replaceable, attachable or deletable by any other agent.

**Promotion and replacement**

- **FR-009**: An agent MUST be able to promote a session file from its current conversation into a new artifact, with the file's name as the default artifact name and an explicit name allowed.
- **FR-010**: A person viewing a conversation's session files MUST be able to promote one from the chat side panel.
- **FR-011**: Promotion MUST COPY the source file's bytes into new artifact-owned content; it MUST NOT link, move or mark the source. The source MUST remain an ordinary session file, and clearing session files MUST NOT be able to affect the artifact.
- **FR-012**: Promotion MUST fail atomically, creating no artifact, when the name is invalid, the name already exists for that agent, the source file is missing or unreadable, the source is empty, or the agent may not access the source conversation.
- **FR-013**: An agent MUST be able to replace an existing artifact's content from a session file in its current conversation; the name and identity MUST stay the same, the version MUST increase by one, and the change MUST be recorded.
- **FR-014**: A failed replacement MUST leave the previous content, version and metadata intact and MUST report a readable error.
- **FR-015**: After a successful replacement, every reference to the artifact MUST resolve the new content; there is no per-reference version pinning in this release.
- **FR-016**: Concurrent replacements MUST leave exactly one complete version; the artifact MUST never contain mixed or partial content, and each successful replacement MUST add one change record.
- **FR-017**: An artifact's content MAY be text or binary and MUST support every file type the product can already upload and read; the gallery decides what it can preview from the type.
- **FR-018**: Agents MUST be able to list their own artifacts with name, type, size, version and last change, and to load an artifact into the current conversation for reading or processing.
- **FR-019**: Agents MUST NOT be able to delete artifacts; a delete attempt MUST be refused.
- **FR-020**: Artifact tools MUST be available in ordinary, scheduled-task and background turns and MUST NOT be offered to the dream cycle.
- **FR-021**: Artifact operations MUST act only on the calling agent's own artifacts; a name that exists only on another agent MUST behave exactly like a name that does not exist.

**Gallery**

- **FR-022**: The WebUI MUST offer an Artifacts page for the selected agent from the main navigation.
- **FR-023**: The gallery MUST list artifacts newest first with name, type, size, version, last change time and whether the content is currently available, plus the agent's total artifact count and total bytes.
- **FR-024**: The gallery MUST support search by name, filtering by type and date, and paging without duplicates or gaps, preserving the chosen filters while paging.
- **FR-025**: The gallery MUST preview images, PDFs, CSV, markdown and text artifacts, and MUST offer download for every artifact, including types it cannot preview.
- **FR-026**: An artifact's detail view MUST show its origin conversation, source filename, the actor and time of the current version, its change history, and every message and memory that references it.
- **FR-027**: Deleting an artifact MUST require a confirmation that names the artifact and states how many references use it, and MUST remove the artifact, its content and its references immediately on confirmation.
- **FR-028**: A person MUST be able to view and download artifacts for agents they may see, and delete an artifact only if they own the agent or hold `all_agents:edit`. The system role retains its existing bypass.
- **FR-029**: An artifact's visibility MUST follow the agent-visibility rule alone; which conversation produced it MUST NOT grant or withhold access beyond that rule.
- **FR-030**: The gallery MUST provide loading, empty, unavailable-content and failed-load states with retry, and MUST remain usable on a phone and by keyboard and screen-reader users.
- **FR-031**: No per-agent storage quota is enforced in this release; the gallery MUST make growth visible through count and total bytes.
- **FR-032**: The gallery MUST provide an activity view, newest first, that lists promotion, replacement and deletion records with the actor, action, artifact name, time and source, and that keeps deletion records until the agent is deleted.

**Session files and explicit clearing**

- **FR-033**: Session files MUST be transient and MUST be removed only by an explicit action: the chat side panel's Clear session files, the `/clear-session-file` channel command, deleting the conversation, or deleting the agent. No retention timer, background collector or size-based cleanup may remove them in this release.
- **FR-034**: The chat side panel MUST offer a Clear session files action for the current conversation; it MUST require confirmation, report how many files were removed, and affect only that conversation.
- **FR-035**: Discord and Telegram conversations MUST offer the same clear as a `/clear-session-file` command, with the same confirmation, count and safety rules; on a platform whose command naming rules do not accept hyphens, the command MUST be registered in its valid form (Telegram's `/clear_session_file`).
- **FR-036**: A conversation with a turn running or queued MUST NOT have its session files removed by a clear; the attempt MUST be refused with an explanation that the turn must finish first.
- **FR-037**: Clearing MUST remove the conversation's session file records and their content, MUST NOT remove or change any artifact or artifact content, and MUST NOT affect another conversation.
- **FR-038**: The clear confirmation MUST state that artifacts are unaffected and that older messages or memories still referencing the cleared files will no longer open them.
- **FR-039**: A cancelled confirmation or a conversation with no files MUST remove nothing and MUST tell the person what happened.
- **FR-040**: Deleting a conversation MUST be refused while a turn is running or queued; otherwise it MUST remove the conversation's session files and MUST leave its artifacts intact.
- **FR-041**: Deleting an agent MUST remove its session files and artifacts together, leaving no orphaned content.
- **FR-042**: Promotion and clearing MUST be safe against each other: a completed artifact MUST survive a clear, and a promotion whose source is cleared mid-copy MUST fail cleanly rather than produce a partial artifact.
- **FR-043**: Clear MUST treat promoted and unpromoted session files identically; the product MUST NOT need a promotion marker, link or reference count to protect artifacts from clearing.

**Attachments and memory**

- **FR-044**: A newly created outgoing message attachment MUST reference an artifact by identity; attaching a plain session file MUST be refused with guidance to promote it first.
- **FR-045**: A message attachment MUST show the artifact's current content, MUST reflect a replacement on its next load or delivery, and MUST show a clear deleted-artifact placeholder naming it after deletion.
- **FR-046**: New or updated memory attachments MUST reference artifacts only; an existing memory attachment that already references a session file remains valid while that file exists, and a memory's artifact attachment MUST survive session-file clearing and conversation deletion.
- **FR-047**: Reading a memory with attachments MUST make those artifacts available to the agent in the current conversation, and MUST report a deleted attachment by name without failing the rest of the memory.
- **FR-048**: Files a person uploads into a conversation MUST remain transient session files and MUST be readable by the agent without promotion; they become permanent only through an explicit promotion.
- **FR-049**: An agent MUST NOT attach another agent's artifact, and a missing or deleted artifact MUST be refused as an attachment.
- **FR-050**: Channel delivery MUST respect each platform's attachment size limit: when an artifact is too large, the reply text MUST still be delivered, the attachment failure MUST be reported explicitly, and the artifact MUST remain usable elsewhere.
- **FR-051**: Existing messages and memories that reference session files MUST keep working while those files exist; after a clear removes them, they MUST render an unavailable placeholder without breaking the rest of the message or memory, and no retroactive promotion or bulk rewrite is required.

**Provenance, permissions and documentation**

- **FR-052**: Every promotion, replacement and deletion MUST record the actor (a person or the agent), the action, the artifact name and identity, the time, and the source conversation and filename where applicable, and MUST NOT record the content.
- **FR-053**: Artifact change records MUST remain available to permitted people after the artifact is deleted, until the agent is deleted, and MUST be visible in the gallery's activity view as well as in the artifact's detail view while it exists.
- **FR-054**: Every read, promotion, replacement, attachment, download and delete MUST be authorised independently; hiding a control in the interface MUST NOT be the protection.
- **FR-055**: Permission changes MUST take effect on the next request, even if the person's gallery is already open.
- **FR-056**: Artifacts MUST NOT be shared across agents and MUST NOT be reachable through a public link in this release.
- **FR-057**: User documentation MUST explain that session files are transient and cleared explicitly (including the side-panel action and `/clear-session-file`), who may view and delete artifacts, and what messages and memories show after an artifact is replaced or deleted.

### Key Entities

- **Artifact**: A named, permanent file owned by one agent. It has an identity, a name unique within the agent, a media type, a byte size, a current version, its own copy of the content, its origin conversation and source filename, who created it, and when it was created and last changed.
- **Artifact change record**: One promotion, replacement or deletion of one artifact, naming the actor, the action, the time and the source. It survives the artifact's deletion and never contains the content.
- **Artifact reference**: A message attachment or memory attachment that points at an artifact by identity rather than at a snapshot of its bytes. It follows the current version and becomes a tombstone when the artifact is deleted.
- **Session file**: A transient file scoped to one conversation, with its filename, media type, size, who sent it and the message it arrived with. It is readable by the agent, may be copied into an artifact, and remains until a person clears it or its conversation or agent is deleted. It carries no promotion marker.
- **Agent visibility**: The existing rule that decides which agents a person may see. It alone governs gallery listing, preview and download; the conversation that produced an artifact does not add or remove access. Delete additionally requires ownership of the agent or `all_agents:edit`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of promoted artifacts remain listed and byte-identical after their origin conversation's session files are cleared, the conversation is deleted, and vizier restarts.
- **SC-002**: A person can promote a file, find it in the gallery, preview it and download it in under 2 minutes from the conversation.
- **SC-003**: A person can find a known artifact by name, type and date in under 30 seconds and can tell its origin, current version and every place it is used from one detail view.
- **SC-004**: A clear removes 100% of the current conversation's session files, reports the correct count, and changes no other conversation's files.
- **SC-005**: A clear leaves 100% of artifacts intact, downloadable and usable in messages and memories.
- **SC-006**: A replacement is visible through every reference on its next load or delivery in 100% of checks, and a failed replacement leaves the previous content and version unchanged in 100% of checks.
- **SC-007**: A deleted artifact produces a readable placeholder in 100% of messages and memories that referenced it, with no crash or broken download.
- **SC-008**: 100% of cross-agent read, replace, attach and delete attempts are refused, and 100% of delete attempts by people without management rights are refused.
- **SC-009**: Deleting an agent leaves zero session files, artifacts, artifact content, references or change records for that agent.
- **SC-010**: 100% of promotion, replacement and deletion actions appear in the change history or activity view with actor, action, time and source, and none records content.
- **SC-011**: With 500 artifacts for one agent, the gallery's first page loads in under 1 second for 95% of loads, and search and filters return the correct set with no duplicates or gaps.
- **SC-012**: 100% of artifact attachments too large for a channel are reported explicitly while the reply text is still delivered, and the artifact remains usable in the gallery.
- **SC-013**: The feature documentation covers explicit session clearing, permissions and reference behaviour, and a reader can answer what happens to a message or memory after a clear, replace or delete without reading the code.
- **SC-014**: 100% of session files remain until an explicit clear, conversation deletion or agent deletion; no timer, background job or size threshold removes one.
- **SC-015**: 100% of messages and memories whose session-file references were cleared show an unavailable placeholder and still render the rest of their content.

## Assumptions

- **Automatic collection is deferred**: this release adds no retention period, background garbage collector, size threshold or automatic expiry for session files. They are removed only by an explicit clear, conversation deletion or agent deletion. Storage growth is accepted until a later change adds collection; the gallery and clear action make the transient tier manageable in the meantime.
- **Promotion copies**: the artifact owns an independent copy of the bytes, and the source session file is left untouched and unmarked. This is what removes the need to track which session files are artifacts or to protect them from clearing.
- **Clear is destructive for legacy references**: after a clear, an older message or memory that still points at one of the cleared session files shows an unavailable placeholder. New messages and memories avoid this by referencing artifacts. The confirmation says so before anything is removed.
- **Clear is conversation-scoped**: the side panel clears only the open conversation; the channel command clears only the conversation it is sent in. Anyone who can participate in that conversation may clear its files. No other conversation, memory, artifact or agent is affected.
- **Channel command naming**: `/clear-session-file` is the canonical name. Discord accepts it as written; Telegram's rules do not accept hyphens, so the same command is registered there as `/clear_session_file`. Both do the same thing.
- **Agent-owned, not conversation-owned**: an artifact belongs to the agent workspace, not to the conversation that produced it. It outlives the conversation and is visible wherever the agent's own visibility allows.
- **Permanence is explicit**: session files are not promoted automatically, and existing session files are not backfilled into artifacts. Existing references keep working while the files exist; they are not rewritten.
- **Replacement is overwrite-in-place**: a replacement changes the artifact's current content and increments a version counter. Earlier versions are not retained, restorable or pinnable; references always follow the latest. This keeps the storage model and gallery lean for the first release.
- **Agents promote on their own**: a promotion does not require a person's confirmation. The agent decides that a file is worth keeping; people retain deletion rights and see the provenance. Scheduled tasks and background jobs may promote, but the dream cycle may not.
- **Agents never delete**: deletion is a person's action only. An agent can create, list, read and replace its own artifacts, so unattended or prompt-driven work cannot erase permanent files. Agents also have no tool to clear a conversation's session files.
- **Text and binary**: artifacts are not limited to media or to documents; any file the product can upload can be an artifact. The gallery previews what it already knows how to preview and downloads everything else.
- **Attachment direction**: only new outgoing agent attachments and new memory attachments are restricted to artifacts. Files a person uploads into a conversation are transient input and must stay usable without promotion.
- **Reference behaviour**: a reference points at the artifact, not at a copy of its bytes. Replacements therefore reach every existing reference; deletion leaves a named tombstone. This is intentional and is the simplest trustworthy behaviour.
- **Permissions**: gallery view and download follow existing agent visibility, regardless of the origin conversation. Delete requires ownership of the agent or `all_agents:edit`; there is no new artifact-specific permission in this release. A sharee with chat-only access may therefore browse and download artifacts but cannot delete them.
- **No quotas**: no per-agent artifact or session-file quota is enforced in this release and artifacts do not count toward model-token usage reporting. The gallery shows artifact count and bytes so growth is visible.
- **No public sharing**: artifacts are not public links, are not shared between agents and are not exported as a gallery feed. Bundle export continues to cover memories only.
- **Generated UI unchanged**: generative-UI media continues to refer to the conversation's session files. Extending it to artifacts is a separate change.
- **No direct gallery upload**: people create artifacts by promoting a session file, not by uploading straight into the gallery. This keeps the origin and the audit trail meaningful.
- **Deletion is final**: a deleted artifact's content is gone. There is no trash, restore or retention period for deleted artifacts; only its change records remain until the agent is deleted.
- **Documentation**: the existing docs set gains a page for artifacts and updates the files, memory, tools and channel-command pages so explicit clearing, permissions and reference behaviour are stated in one place.
- **Dependencies**: the existing agent visibility and permission rules, the existing file upload and preview surfaces, the existing memory document format, the existing Discord/Telegram command registration, the existing chat side panel and the existing storage abstraction are reused rather than replaced.
