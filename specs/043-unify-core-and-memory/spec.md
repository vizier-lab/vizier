# Feature Specification: Unify Core and Memory

**Feature Branch**: `043-unify-core-and-memory`

**Created**: 2026-11-01

**Status**: Draft

**Input**: User description: "unify core and memory and improve the memory system in general — by default an agent should have a core bundle with soul memory, both core and core/soul are permanent and can't be deleted, only core/soul is automatically injected to system context. agent may add and remove other memory to core. limit the size of a memory on write, to encourage more, smaller, chunkable memories. the soft cap should be a non-error warning on write, not wording in the operating doctrine. core/soul may be updated and deleted by the agent or the user."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One place to look: who an agent is lives in its memory (Priority: P1)

An agent's sense of self — its name, role, values, and how its people want it to work — currently lives in a separate document that is not part of its memory. The agent must learn two vocabularies: one tool set for identity, another for memory, with a different reading and writing shape and a different storage history. A person auditing an agent has to know to look in two places to see everything it thinks it knows.

After this change, an agent has a reserved bundle in its memory called **core**, and the document that defines it lives there as **soul**. The soul is ordinary memory: it is read, written, listed, linked, versioned and inspected exactly like every other memory, so an agent has one vocabulary and a person has one place to look. The soul is the only memory injected into every conversation automatically, because an agent must always know who it is.

**Why this priority**: This is the whole point of the unification — removing a second, parallel concept that costs the agent tool-selection effort and costs every reader of the system a second place to look. Everything else in this feature improves an existing behaviour; this one removes a whole category of confusion.

**Independent Test**: Give a fresh agent a first turn, then read its memory. Confirm a core bundle exists containing a soul document, that the soul's content was part of the agent's context for that turn, and that the same situation, values and history surfaces are available for it as for any other memory.

**Acceptance Scenarios**:

1. **Given** a newly created agent, **When** it takes its first turn, **Then** a core bundle exists in its memory holding a soul document, and the soul's content is present in its context for that turn.
2. **Given** an existing agent from before this change with an identity document already written, **When** the system starts after the change, **Then** that content is present in the agent's core bundle as its soul document, unchanged and without loss, and the agent sees the same identity in its context as it did before.
3. **Given** an agent with a core bundle, **When** it lists its memory, **Then** core is shown among its bundles with the soul as its only document.
4. **Given** an agent whose identity needs correcting, **When** it writes the corrected text to its soul, **Then** its context for the next turn reflects the correction.

---

### User Story 2 - The soul is permanent in practice but never a trap (Priority: P1)

An agent that loses its identity mid-conversation behaves like a different agent, so the soul must be very hard to remove by accident. At the same time, an agent's identity legitimately changes: a name is chosen, a role narrows, a mistaken value is dropped. And a person who manages agents must be able to correct a soul they disagree with.

The rule that satisfies both: the soul **can always be written and can be removed**, but is treated as special enough that careless deletion cannot silently damage the agent — the core bundle itself is reserved and cannot be removed, and other documents cannot be placed in the core bundle alongside it. A soul that is removed is replaced by a well-formed default identity, so the agent is never left with an empty identity.

**Why this priority**: A protection that is too strict makes the soul unmaintainable; one that is too loose lets an agent erase who it is. Getting the boundary exactly right is what makes the unification safe to ship.

**Independent Test**: Attempt, as both an agent and a person, to write the soul, to remove the soul, to remove the whole core bundle, and to add a second document to core. Confirm the first two succeed with the agent still healthy afterwards and the last two are refused with a clear reason.

**Acceptance Scenarios**:

1. **Given** an agent with a core bundle, **When** anyone removes the soul document, **Then** the removal succeeds and the agent's next turn carries a well-formed default identity rather than an empty or missing one.
2. **Given** a core bundle, **When** anyone attempts to remove the whole core bundle, **Then** the action is refused and the response explains that the core bundle is reserved.
3. **Given** a core bundle, **When** anyone attempts to create a document in it other than the soul, **Then** the action is refused and the response explains that the core bundle holds only the soul.
4. **Given** an agent whose soul was removed, **When** it writes an identity again, **Then** the soul is re-created with the written content and injected from the following turn.
5. **Given** any memory outside the core bundle, **When** anyone removes it, **Then** it is removed as before with no core-related restriction.

---

### User Story 3 - An agent is told when a memory is getting too big to be one thing (Priority: P2)

An agent's memory works best when one memory is one thing: a search returns a handful of excerpts drawn from across everything the agent knows, and a memory that bundles several unrelated subjects competes with itself for those slots. Left unchecked, agents write ever-growing documents — one agent's open-work note reached eleven kilobytes across three unrelated subjects, and a project note reached ten across ten sections — and each fact inside them becomes harder to find than it would be alone.

The engine cannot split these for the agent: the right seam is the agent's judgement, and splitting deliberate prose automatically fragments it. So the system **tells the agent** instead, at the moment of writing: a memory that grows past a comfortable size is still saved, and the result carries a warning that names the size, the comfortable size, and the reminder that one memory should be one thing. A memory that grows past a hard ceiling is refused, with the amount over and a concrete way to split it.

**Why this priority**: This is the "improve the memory system" half of the feature. It depends on the unification existing (the soul gets its own, tighter treatment under the same mechanism) but delivers value on its own for every other memory.

**Independent Test**: Write memories of increasing size and confirm: below the comfortable size, a clean success with no warning; above it, a success carrying a warning; above the ceiling, a refusal quoting the overage and how to split. Then confirm the agent still reads and edits everything it wrote.

**Acceptance Scenarios**:

1. **Given** a memory comfortably under the comfortable size, **When** it is written, **Then** the write succeeds and the result carries no warning.
2. **Given** a write that takes a memory past the comfortable size but at or under the ceiling, **When** it is written, **Then** the write succeeds, the memory is saved in full, and the result carries a warning stating the memory's size and the comfortable size.
3. **Given** a write that would take a memory past the ceiling, **When** it is written, **Then** the write is refused, the memory is left exactly as it was, and the response states how much the content exceeds the ceiling and how to split it into smaller memories that link to one another.
4. **Given** a memory that already exceeds the ceiling from before this change, **When** the agent reads it, **Then** it reads in full; when the agent shortens it, the shortened version is accepted.
5. **Given** a soul document, **When** it is written past its own, tighter comfortable size, **Then** the same warning behaviour applies, because the soul is paid for on every turn.
6. **Given** a warning-carrying success, **When** the agent reads the result, **Then** the warning is distinguishable from the success itself so the agent does not mistake the write for a failure and retry it.
7. **Given** a period that changed only the reserved bundle, **When** the agent reads the summary of that period, **Then** it sees those changes reported as identity changes rather than as memories written, and does not see them counted twice.

---

### User Story 4 - A person reviews an agent's identity where every other memory lives, and the separate area is gone (Priority: P3)

A person responsible for an agent can read its identity, see how it changed over time, and restore an earlier version — but the dedicated area that used to exist only for this has no reason to survive. Once identity is a memory, a page built solely to show one document is a second place to look for something already reachable from the first, and it carries its own editor, its own history view and its own navigation entry for a document that now has none of those privileges.

So the area is **removed, not redirected**: the reserved bundle is browsed, opened and edited in the memory surfaces like any other, and the navigation entry for it goes away. What must not be lost is the *behaviour* it carried — finding it quickly, its revision history, restoring a revision, and refusing a stale save.

**Why this priority**: This is what makes the unification real for people rather than cosmetic. Leaving a bespoke page behind would mean the system still has two ways to see identity, which is the thing the feature exists to remove.

**Independent Test**: Confirm the identity area no longer exists — its entry is gone from the agent's navigation and its address is not served. Then, from the memory surfaces alone, open the reserved bundle, open the identity document, view and compare its revisions, restore one, and confirm the restored text is what the agent sees next turn.

**Acceptance Scenarios**:

1. **Given** an agent, **When** a person looks at its navigation, **Then** no separate identity entry is offered, and the agent's memory is where identity is found.
2. **Given** an address for the former identity area, **When** it is visited, **Then** it is not served as a working page.
3. **Given** an agent with an identity document, **When** a person opens the reserved bundle in the memory view, **Then** they see the document listed and can open it.
4. **Given** an identity document with several revisions, **When** a person views its history, **Then** they see each revision with who or what wrote it, and can compare and restore as they can for any other memory.
5. **Given** a person editing the identity document from a stale view, **When** they save, **Then** the save is refused with the newer content shown, rather than overwriting it.

### Edge Cases

- An agent removes its soul, then takes several turns, then writes a new one: the agent must never have had a turn with no identity section at all, only with the default.
- A person removes the soul through the management interface: the same default applies, and the removal is recorded in the soul's history so it is visible and recoverable.
- A memory is written to the core bundle at a path other than the soul: refused, and the refusal must say which path the core bundle accepts.
- A bundle import or export involves the core bundle: an import that would place anything but a soul there is refused; an export of the core bundle is permitted.
- A write leaves a memory exactly at the ceiling: accepted — only exceeding the ceiling is refused.
- A memory's size grows because of an import or a restore rather than a direct write: the ceiling applies to the resulting content, and a legacy document above it must remain readable and shorten-able.
- An agent's pre-existing identity content is larger than the soul's own ceiling: the upgrade must still succeed. The content is migrated as it is — never refused and never split — and the agent's next edit of it is what brings it within the limit.
- The comfortable size and ceiling are changed for an agent after the fact: subsequent writes are judged against the new values, and no existing memory becomes unreadable.
- The core bundle is empty because its soul was removed: memory listing still shows the core bundle as reserved rather than dropping it.
- An install still holds files written by the earlier per-document model: nothing reads them, nothing is promised about them, and no upgrade path is offered for them — the only identity document an agent has is the one in the reserved bundle.
- A person has the former identity area bookmarked or open in a tab: the address no longer serves it, and reaching identity takes the memory surfaces instead.
- An agent writes a memory just under the ceiling that is a single indivisible block, such as a large pasted table: it is accepted, and the warning still applies if it is above the comfortable size.

## Requirements *(mandatory)*

### Functional Requirements

**Unification**

- **FR-001**: Every agent MUST have a reserved bundle named `core` in its memory, holding a single document at path `soul`.
- **FR-002**: A newly created agent MUST be given a soul document containing a well-formed default identity.
- **FR-003**: An agent's pre-existing identity content MUST be copied into its core bundle as the soul document on upgrade, byte for byte and without splitting it. The old record is not converted and not deleted; it is kept as a read-only source until it is retired before 1.0.0, so the upgrade is reversible.
- **FR-004**: Identity MUST be readable and writable through the same memory operations, address shape, and result shapes as every other memory, so that no identity-specific read or write operation remains.
- **FR-005**: The soul document MUST be listed, searched, linked, and versioned like every other memory, and MUST appear in the agent's memory listing.
- **FR-006**: The soul document MUST be the only memory automatically injected into the agent's system context; no other memory is injected there.
- **FR-007**: The identity document's wrapper MUST be byte-stable across consecutive turns, so the prompt prefix carrying it stays cacheable. It names no slot in the assembled prompt: where it sits relative to the other instructions is not observable to the agent and MUST NOT be treated as an assumption.
- **FR-008**: A removed or absent soul MUST be treated as a well-formed default identity; the agent MUST never receive an empty identity section.
- **FR-009**: The injected soul MUST carry its own address — the bundle and path it came from — in the text the agent receives, so that the agent can tell it is reading a memory it can also write, without being told separately or needing to search for it.
- **FR-010**: The agent MUST be told, in the doctrine it receives, that its identity is a memory at that address and which operations read and write it, because the old discovery route — a dedicated identity operation named in the tool list — no longer exists.

**Protection**

- **FR-011**: The soul document MUST be creatable, updatable, and removable by both the agent and a person.
- **FR-012**: Removal of the whole `core` bundle MUST be refused, with a response stating that the bundle is reserved.
- **FR-013**: Creation of any document in the `core` bundle other than `soul` MUST be refused, with a response stating which path the bundle accepts.
- **FR-014**: Importing a bundle into `core` MUST be refused unless it would place only a soul document there.
- **FR-015**: Removal of the soul MUST be recorded in its version history so that it is visible and reversible, and MUST NOT remove the `core` bundle.

**Size limits**

- **FR-016**: Every memory write MUST be judged against a per-agent hard ceiling on the content's size, excluding metadata such as title, tags, and links.
- **FR-017**: A write that would leave content above the hard ceiling MUST be refused, MUST leave the existing memory unchanged, and MUST NOT truncate or otherwise save partial content.
- **FR-018**: The refusal MUST state the content's size, the ceiling, the amount by which it exceeds the ceiling, and a concrete way to split the content into smaller memories that link to one another.
- **FR-019**: A write that leaves content above a per-agent soft limit but at or below the hard ceiling MUST succeed in full, and its result MUST carry a non-error warning stating the content's size and the soft limit.
- **FR-020**: The warning MUST be distinguishable from the success result itself, so that it cannot be read as a failure.
- **FR-021**: A memory whose content already exceeds the hard ceiling MUST remain readable in full, and MUST remain editable when the edit reduces its content.
- **FR-022**: The hard ceiling and soft limit MUST be configurable per agent, and MUST apply to writes arriving from any path: an agent's own write, a person's edit, a restore, or an import.
- **FR-023**: The soul document MUST be subject to a tighter pair of limits than ordinary memories.
- **FR-024**: The operating doctrine given to agents MUST NOT state a size figure, so the numbers live only where they are enforced and cannot drift from the configuration.
- **FR-025**: Content carried over by the upgrade MUST be exempt from the hard ceiling, so that no upgrade can be blocked or made lossy by a limit introduced in the same change. The exemption MUST apply only to that carry-over, not to any later write.
- **FR-026**: Any temporary compatibility mechanism introduced to carry pre-existing identity forward MUST be recorded for removal before the first stable release, naming what depends on it and what must be checked before it is removed.

**Interface**

- **FR-027**: A person MUST be able to read the identity document, view its revision history, compare revisions, and restore one, entirely through the memory surfaces — the bundle list, the document view, and the revision history already used for memory revisions. No separate area of the interface for identity MUST remain: the reserved bundle is browsed and edited exactly like any other.
- **FR-028**: A concurrent edit from a stale view MUST be refused with the newer content shown, rather than overwriting it.
- **FR-029**: Removing the separate identity area MUST NOT lose behaviour it carried: choosing the identity from the agent's navigation, viewing and comparing its revisions, restoring one, and refusing a save made from a stale view MUST all remain reachable from the memory surfaces. The separate area itself MUST be removed, not redirected.
- **FR-030**: Every place that refers to the old arrangement — the doctrine and tool guidance the agent receives, the tool-name lists that decide what an agent may call, the progress labels a person sees, the navigation entry and pages that exist only for identity, the templates a new agent is seeded from, and the documentation and website copy — MUST be updated or removed in the same change, so that no surface offers an agent or a person something that no longer exists. The product-facing term for identity does not change: what changes is where it lives.
- **FR-031**: A change to any document in the reserved bundle MUST be classified as a change to the agent's identity, and MUST be classified that way because of the bundle it lives in rather than the record it was written to. The identity document needs no special case: it is a member of that bundle.
- **FR-032**: A change to the reserved bundle MUST NOT be counted twice — it is reported as an identity change and MUST NOT also be counted among the memories written, revised or removed. A revision of the identity document is not also a memory revision for counting purposes.
- **FR-033**: A summary of a period's changes MUST report the number of identity changes rather than merely whether any happened, and MUST still say plainly when a period changed the agent's identity.
- **FR-034**: The interfaces that read, write and version identity without an agent's memory in the address MUST keep working and keep their existing response shapes, but MUST be marked deprecated wherever they are described to a consumer, and MUST state what replaces them. Their removal is deferred, not cancelled.
- **FR-035**: A deprecated identity interface MUST read and write the same underlying document as the memory surfaces, so the two can never disagree — there is one identity document, not a compatibility copy.

### Key Entities

- **Core bundle**: a reserved memory bundle, exactly one per agent, containing one document. It cannot be removed while it exists, and accepts no document other than the soul.
- **Soul document**: the agent's identity — name, role, values, and how its people want it to work. Ordinary memory in every mechanical respect; special only in that it is injected every turn, is protected from bundle-level removal, and has tighter size limits.
- **Memory size limits**: a hard ceiling and a soft limit, per agent, measured on content only. The ceiling refuses; the soft limit warns.
- **Write warning**: a non-error note attached to a successful write whose content exceeded the soft limit. It never blocks a write.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of agents have a core bundle with a soul document after upgrading, including agents that existed before the change, with zero loss of previously written identity content.
- **SC-002**: An agent's context cost for its identity is unchanged by the upgrade beyond the fixed wrapper that marks it as memory, verified as a cached prefix that is stable across consecutive turns for sampled agents.
- **SC-003**: An agent sees its identity on 100% of turns, including after the soul has been removed — measured as zero turns with an empty or missing identity section.
- **SC-004**: An agent no longer has a second vocabulary to choose from: the number of operations that exist only for identity is zero, and the existing ability to read and write identity is reachable through the memory operations.
- **SC-005**: A person can complete the task "find an agent's identity, compare it with last week's, and restore last week's" from the memory surfaces alone in under a minute, and there are zero interface entries, routes or pages that exist only for identity.
- **SC-006**: Zero successful writes are silently truncated: every write either saves the caller's content in full or fails with a refusal that names the overage.
- **SC-007**: Memories stop growing without bound: over a sample of at least 100 memory writes by agents after the change, the share of writes receiving a soft-limit warning is measurably non-zero and the share refused by the ceiling is small, showing the warning guides rather than the refusal.
- **SC-008**: No memory becomes unreadable or uneditable as a result of the limits, verified against a corpus containing documents above the ceiling.
- **SC-009**: A refused write leaves the memory exactly as it was, verified across the ceiling, core-bundle, and stale-edit refusals — zero cases of partial or corrupted state.
- **SC-010**: An agent that reads a warning-carrying success does not retry the write: over a sample of warnings, zero immediate duplicate writes of the same content.
- **SC-011**: Identity changes are never invisible and never double-counted: over a sample of periods that changed the reserved bundle, 100% report an identity change with the right count, and zero of those revisions also appear among the memories written, revised or removed.
- **SC-012**: The behaviour the removed identity area carried is fully reachable from memory: for 100% of sampled agents, finding identity, viewing its history, comparing two revisions and restoring one each succeed without any identity-only interface.
- **SC-013**: A retained deprecated interface and the memory surfaces never disagree: for 100% of sampled writes to identity through either, the other reads back the same content immediately, and zero cases exist of two different identity documents.

## Assumptions

- The reserved bundle is named `core` and the identity document lives at path `soul`, giving the address `core/soul`. `core` is reserved and is not a valid destination for general memories. The path segment is an implementation detail the agent addresses and the product never names: **Core is the only product-facing term**, and it stays exactly as it is today.
- The old-concept cleanup removes code, not vocabulary: the retired document model survived as an unreachable interface that nothing called and nothing documented, and it is removed here because a second, unguarded identity surface is a trap. No user-visible label, route, anchor or template name changes as a result, because the product-facing term never changed.
- The default agent bundle for ordinary memories is unchanged, so no existing memory is re-addressed and no existing search or link path is invalidated.
- Identity is injected as its own instruction block alongside the other instructions the agent receives, not inside the per-turn context block that carries retrieved excerpts. Its ordering relative to those instructions is an implementation choice rather than a requirement: nothing an agent can observe depends on it, and explicit placement is settled during planning.
- Default limits: 8 KiB hard ceiling and 4 KiB soft limit for ordinary memories; 4 KiB hard ceiling and 1.5 KiB soft limit for the soul. These come from measurement of a real agent corpus of 34 memories (median 3.2 KiB, four documents above 8 KiB) rather than from a round number, and are expected to be adjusted after real use.
- The hard ceiling exists to enforce addressing — one memory, one thing — not to bound per-turn cost, which is already bounded separately by how much automatic context may inject. This is recorded so the ceiling is not later tightened on cost grounds it does not serve.
- Splitting an over-ceiling memory is the agent's judgement, not the system's: the system states the overage and the pattern and does not pick the seam. Automatic splitting of deliberate prose is explicitly rejected as fragmenting it.
- The soft limit is enforced as a warning rather than as doctrine text, because a warning arrives when the agent can act on it, can quote the agent's own configured limit, and can be counted afterwards; doctrine text cannot.
- Identity revision history is retained under the same retention and access rules that already apply to memory revisions.
- Existing deployments upgrade in place with no operator action beyond a normal restart.
- Automatic surfacing of remembered excerpts for a turn is out of scope; this feature changes what identity is, not how excerpts are retrieved.
