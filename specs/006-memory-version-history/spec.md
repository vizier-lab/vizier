# Feature Specification: Version History for CORE.md and Memories

**Feature Branch**: `006-memory-version-history`

**Created**: 2026-09-15

**Status**: Draft

**Input**: User description: "i want core.md and memories to have a simple version management. where every save done by user or agent will be added to the history. user may see the history, diff/delta, and even rollback to previous version if needed"

## Clarifications

### Session 2026-09-15

- Q: Should content changes made directly to memory files on disk (outside Vizier) be detected and recorded as versions? → A: No — direct on-disk edits are ignored by this feature. Only saves that go through Vizier (agent tools, dream cycle, WebUI, API, import, rollback) are versioned.
- Q: Where does memory history live relative to the memory bundle on disk? → A: History is internal to Vizier, kept with the other embedded-database-backed entities; it is not written into the bundle as files, bundle export/import is unchanged, and history does not travel with the bundle.
- Q: Should the agent itself get tools to view or roll back its own history? → A: No — history, diff, and rollback are user-only (WebUI and API) in this feature; no agent-facing tools are added.
- Q: Should there be a cap on the number of versions retained per document? → A: No — every version is kept forever; no automatic pruning, no fixed or configurable cap in this feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every save is captured as a version (Priority: P1)

An agent's CORE.md and its memory documents change constantly — the agent rewrites them during conversations and during its unattended dream cycle, and the agent's owner edits them by hand in the WebUI. Today each save silently overwrites the previous content, so once an agent (or a person) writes something wrong, the prior content is gone for good. From now on, every save — whoever or whatever made it — is recorded as a new entry in that document's history, together with who made it, when, and through which path (a chat tool call, the dream cycle, the WebUI, the API, or a bundle import).

**Why this priority**: History is the foundation everything else in this feature stands on. Without a complete, automatically captured record there is nothing to view, compare, or roll back to. It must be captured passively — no one should have to remember to "save a version".

**Independent Test**: Have an agent rewrite its CORE.md through its tool, then have a user edit the same CORE.md in the WebUI, then have the agent write and later overwrite a memory. Confirm each document's history lists one entry per save, in order, each attributed to the correct actor and trigger, and that the stored content of every entry matches exactly what was saved at that moment.

**Acceptance Scenarios**:

1. **Given** an agent with an existing CORE.md, **When** the agent rewrites it through its CORE-writing tool during a conversation, **Then** a new history entry appears for CORE.md attributed to the agent, marked as originating from a conversation, with a timestamp and the full saved content.
2. **Given** the same agent, **When** its owner edits CORE.md in the WebUI and saves, **Then** a new history entry appears attributed to that user, marked as originating from the WebUI.
3. **Given** a memory document that already exists in a bundle, **When** the agent overwrites it via its memory-writing tool, **Then** a new history entry for that memory appears; earlier entries are untouched.
4. **Given** an agent running its dream cycle, **When** the dream cycle saves CORE.md or a memory, **Then** the resulting entry is attributed to the agent and marked as originating from the dream cycle, distinguishable from conversational saves.
5. **Given** a memory document, **When** it is deleted (by the agent or by a user), **Then** the deletion is itself recorded in that document's history as a terminal entry, and the pre-deletion content remains viewable in history.
6. **Given** a save whose content is byte-for-byte identical to the current content, **When** the save completes, **Then** no new history entry is created (the history contains no no-op duplicates).

---

### User Story 2 - A user browses history and sees what changed between versions (Priority: P2)

The agent's owner notices the agent behaving differently than yesterday and suspects its CORE.md or a memory was changed. They open the document in the WebUI, switch to its history, and see a chronological list of versions: who saved each one, when, and how. They pick a version and see exactly which lines were added, removed, or altered relative to the version before it — or compare any two versions of their choosing. They can also open any past version in full to read it as it was.

**Why this priority**: Being able to *see* what changed is what turns a raw list of snapshots into something a person can act on. It depends on Story 1's captured history but is valuable on its own even before rollback exists — most of the time understanding what changed is enough.

**Independent Test**: Seed a document with three saves of differing content. Open its history in the WebUI: confirm all three versions are listed newest-first with actor, trigger, and timestamp; open the middle version and confirm its full content is shown; view its diff against the earliest version and confirm added/removed lines are correctly highlighted; then select the newest and earliest versions and confirm the two-version comparison is correct.

**Acceptance Scenarios**:

1. **Given** a CORE.md with multiple versions, **When** a user with access to the agent opens the CORE page's history view, **Then** every version is listed newest-first, each showing actor, trigger, and timestamp, and the current version is clearly marked.
2. **Given** a memory document with multiple versions, **When** a user opens that memory's detail in the WebUI, **Then** they can reach the same history view for that memory.
3. **Given** a history list, **When** the user selects a version, **Then** the full content of that version is displayed read-only.
4. **Given** a selected version that is not the first, **When** the user asks to see its changes, **Then** a line-level diff against the immediately preceding version is shown, with additions and removals visually distinguished.
5. **Given** a history list, **When** the user chooses any two versions to compare, **Then** a line-level diff between exactly those two versions is shown, regardless of how many versions lie between them.
6. **Given** a document with a large history, **When** the user opens the history view, **Then** the list is paginated or incrementally loaded so it stays responsive.
7. **Given** a user who does not have access to the agent, **When** they attempt to view any of its document history, **Then** the request is refused exactly as viewing the document itself would be.

---

### User Story 3 - A user rolls a document back to a previous version (Priority: P3)

Having found the version where things went wrong, the user restores an earlier version of CORE.md or a memory. The restore is itself just another save: the chosen version's content becomes the current content, and a new history entry is recorded attributed to the user, noting which version it was restored from. Nothing is ever erased from history — a rollback can itself be rolled back. Restoring a memory that had been deleted brings it back.

**Why this priority**: Rollback is the recovery action the whole feature is aimed at, but it is only safe and useful once history is reliably captured (Story 1) and a user can identify the right version to return to (Story 2).

**Independent Test**: Seed a document with versions A, B, C. Roll back to A and confirm the current content equals A, a new version D (content = A, attributed to the user, referencing A) is added, and versions A/B/C are still present. Confirm the agent sees the restored content on its next read. Delete a memory, then restore it from history and confirm it is listed and readable again.

**Acceptance Scenarios**:

1. **Given** a document with versions A, B, C (C current), **When** the user rolls back to A, **Then** the document's current content equals A's content and a new version D is appended, attributed to the user, marked as a rollback, and referencing A as its source.
2. **Given** a rollback has been performed, **When** the user views the history, **Then** the versions that were "skipped over" (B and C) are still present and viewable — history is append-only.
3. **Given** CORE.md was rolled back, **When** the agent next reads its CORE (during a conversation, at its next startup, or in its dream cycle), **Then** it sees the restored content, exactly as if the user had saved that content by hand.
4. **Given** a memory was rolled back, **When** the agent next searches or reads that memory, **Then** it gets the restored content, and any listings, links, or related-memory relationships derived from its content reflect the restored version.
5. **Given** a memory whose latest history entry is a deletion, **When** the user restores a content version from its history, **Then** the memory exists again with that content and a rollback entry is recorded.
6. **Given** the user attempts to roll back to the version that is already current, **When** they confirm, **Then** the system reports there is nothing to do and records no new version.
7. **Given** a rollback, **When** the user initiates it, **Then** they are asked to confirm before it takes effect, and the confirmation shows which version will become current.

---

### Edge Cases

- **Concurrent saves**: the agent saves a memory in the same instant a user saves it from the WebUI. Both saves are recorded as separate versions in the order they were applied; the later one is current. No version is lost or merged.
- **Very large documents or very long histories**: history storage grows with every save. Every version is retained (no automatic pruning in this feature), so a document that is rewritten thousands of times has thousands of versions; the history view must remain usable (paginated) at that scale.
- **Memory renamed or moved to another path/bundle**: a memory written under a new path is a new document with its own history starting from that write; the old path's history remains under the old path (with its deletion recorded, if it was deleted). History is not transferred across paths in this feature.
- **Agent deleted**: when an agent is deleted, all of its document history is removed with it — history does not outlive the agent it belongs to.
- **Bundle deleted**: deleting a whole bundle records a deletion entry for each memory it contained, exactly as individual deletions do, so they remain restorable individually.
- **Bundle import/export**: exporting a bundle produces the plain documents only — history is not part of the exported bundle. Importing a bundle records the imported content as a version of each document attributed to the importing user and marked as originating from import.
- **Legacy content at upgrade time**: documents that exist before this feature ships have no history. On first access after upgrade, the current content is recorded as the initial version (attributed to "system", marked as the pre-existing baseline) so that the first real change afterward has something to diff against.
- **Rollback of a memory whose restored content links to memories that no longer exist**: the restore succeeds; any links to missing targets are treated as broken links exactly as they would be for a fresh save with that content.
- **History entry references a version that failed to save**: a version is only recorded when the underlying save succeeds; a failed save records nothing.
- **Document edited on disk, then rolled back in Vizier**: the rollback restores the chosen recorded version's content regardless of what is currently on disk; the on-disk edit is overwritten and was never recorded.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST record a new version entry for a document every time its CORE.md or a memory concept document is successfully saved, regardless of who or what performed the save.
- **FR-002**: Each version entry MUST capture the full content as saved, the time of the save, the actor (a specific user, the agent itself, or "system"), and the trigger (conversation tool call, dream cycle, WebUI, API, bundle import, rollback, or upgrade baseline).
- **FR-003**: System MUST NOT record a new version when a save's content is identical to the current version's content.
- **FR-004**: System MUST record a deletion of a memory document as a terminal history entry, retaining all prior versions of that document.
- **FR-005**: System MUST NOT attempt to detect or version content changes made to memory documents outside of Vizier (directly on disk); such edits are out of scope. A subsequent in-Vizier save simply records the newly saved content as the next version, and a diff against the prior version will include whatever the external edit changed.
- **FR-006**: Users MUST be able to list the version history of an agent's CORE.md and of any memory document, ordered newest-first, showing actor, trigger, and timestamp for each entry, with the current version identified.
- **FR-007**: Users MUST be able to view the full content of any individual version.
- **FR-008**: Users MUST be able to view a line-level diff between any version and its immediately preceding version.
- **FR-009**: Users MUST be able to view a line-level diff between any two versions of the same document.
- **FR-010**: Users MUST be able to roll a document back to any previous version.
- **FR-011**: A rollback MUST make the chosen version's content the document's current content, and MUST record a new version entry attributed to the user who performed it, marked as a rollback, and referencing the version it was restored from. Rollback MUST NOT delete or alter any existing history entry.
- **FR-012**: A rollback MUST behave, from the agent's perspective, identically to a user saving that content by hand: everything derived from the document (the agent's loaded CORE, memory listings, links and related-memory relationships, search results) MUST reflect the restored content.
- **FR-013**: Users MUST be able to restore a deleted memory document by rolling back to one of its content versions.
- **FR-014**: History viewing and rollback MUST be subject to the same access rules that govern viewing and editing the document itself; users without access to an agent MUST NOT be able to see or restore its history.
- **FR-015**: System MUST require explicit user confirmation before a rollback takes effect.
- **FR-016**: History MUST be available through the same user-facing surfaces as the documents themselves — the WebUI's CORE page and memory detail, and the HTTP API used by that WebUI.
- **FR-017**: History lists MUST be paginated or incrementally loaded so that documents with a very large number of versions remain usable.
- **FR-018**: Documents that predate this feature MUST have their current content recorded as an initial baseline version on first access after upgrade, attributed to "system".
- **FR-019**: Deleting an agent MUST remove all of that agent's document history.
- **FR-020**: History MUST be retained indefinitely; this feature introduces no automatic pruning or retention limit.
- **FR-021**: History MUST NOT be written into the memory bundle's on-disk document set; the bundle's files, layout, and export format MUST remain unchanged by this feature.
- **FR-022**: System MUST NOT expose history, diff, or rollback to agents as tools in this feature; these capabilities are available only to users through the WebUI and its API.

### Key Entities

- **Versioned Document**: a CORE.md or a memory concept document, identified by its owning agent plus (for memories) its bundle and path. The unit to which a history belongs.
- **Version Entry**: one immutable snapshot in a document's history — sequence position, full content, timestamp, actor, trigger, optional reference to a source version (for rollbacks), and a flag for deletion entries. Append-only.
- **Actor**: who performed the save — a specific user (by identity), the agent itself, or "system" (for upgrade baselines).
- **Trigger**: the path through which the save occurred — conversation tool call, dream cycle, WebUI, API, bundle import, rollback, or upgrade baseline.
- **Diff**: a line-level comparison between two version entries of the same document, showing additions and removals.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of successful saves to CORE.md and memory documents — from every save path (agent conversation tools, dream cycle, WebUI, API, import) — appear in the document's history with the correct actor and trigger.
- **SC-002**: A user can find the version in which a given line was changed, using the history and diff views, in under 2 minutes for a document with up to 50 versions.
- **SC-003**: A user can complete a rollback (open history, pick a version, confirm) in 3 or fewer interactions after opening the document.
- **SC-004**: After a rollback, the agent's next read of the document returns the restored content in 100% of cases, with no restart required.
- **SC-005**: The history view for a document with 1,000 versions opens and is interactive within 2 seconds.
- **SC-006**: No history entry is ever lost or altered by any later save, deletion, or rollback: for any document, the set of versions present before an operation is a subset of the set present after it (except when the owning agent is deleted).
- **SC-007**: Repeatedly saving unchanged content produces zero new history entries.

## Assumptions

- **Scope of "memories"** is the agent's memory concept documents — the per-bundle markdown documents an agent writes and reads. The bundle's auto-maintained index and log documents are derived from concept documents and are not independently versioned. Session files, dream journal entries, and skills are out of scope.
- **Users only (confirmed)**: history, diff, and rollback are user-facing capabilities exposed through the WebUI and its API. The agent gets no tools to browse or roll back its own history in this feature; that could be added later on top of the same history.
- **Full snapshots**: each version stores the complete content as saved rather than a delta, so any version can be displayed or restored on its own without replaying earlier ones. Diffs are computed on demand from two snapshots.
- **Line-level diffs** are sufficient; word- or character-level highlighting is a nice-to-have, not required.
- **Actor identity for agent saves**: when the agent saves through a tool during a conversation, the actor is the agent; the user it was talking to is not recorded as the actor (they did not author the content). If the conversational context is cheaply available it may be recorded as additional context, but is not required.
- **Rollback is a save, not an undo**: history is strictly append-only; rolling back never removes entries.
- **No retention limits (confirmed)**: every version is kept forever; there is no fixed or configurable cap. Storage growth from full snapshots is accepted; pruning/compaction can be a follow-up.
- **Storage location**: history is internal to Vizier (alongside the other embedded-database-backed entities), not written into the memory bundle on disk. Bundle export stays a plain open-format bundle without history; history does not travel between deployments.
- **Access control** reuses the existing per-agent visibility rules; no new roles or permissions are introduced.
- **Concurrency**: saves to the same document are applied in some serial order; the version sequence reflects that order. No merge or conflict-resolution behavior is introduced.
- **Direct on-disk edits are out of scope**: a developer editing a memory file directly on disk does not produce a history entry. The content the agent reads may therefore temporarily differ from the latest recorded version; this is accepted. The next in-Vizier save captures the then-current content as a new version.
