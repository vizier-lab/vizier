# Feature Specification: Entity Wiki Links for Artifacts, Functions, Skills, and Tasks

**Feature Branch**: `066-wiki-entity-links`

**Created**: 2026-11-25

**Status**: Draft

**Input**: User description: "currently agent can link memories bundle with [[]] notation, can we do something like that for artifacts, functions, skills, and tasks? also i would like such links to be rendered on the webui too. and user could refer such thing on the composer as well with auto-completion/suggestion"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent replies contain clickable entity links (Priority: P1)

An agent that has just produced an artifact, saved or used a function, applied a skill, or run a task references it in its reply with the entity wiki-link syntax. In the WebUI the person sees that reference as a link labeled with the entity's name (or an alias) and clicks it to land directly on that entity's page.

**Why this priority**: Today only memory references are navigable, yet the things an agent actually works on — its artifacts, functions, skills, and tasks — must be found by hand in other parts of the UI. Navigable replies make the conversation self-documenting, and every later story builds on the same rendering.

**Independent Test**: Trigger an agent turn that references one entity of each kind with the link syntax; each reference in the resulting chat message renders as a link and lands on the correct page.

**Acceptance Scenarios**:

1. **Given** an agent that produced an artifact during a turn, **When** its reply references that artifact with the artifact link syntax, **Then** the message shows a link labeled with the artifact's name and clicking it navigates to that artifact's view.
2. **Given** replies containing one link of each of the four kinds, **When** each link is followed, **Then** each lands on the corresponding page: the artifact's view, the function's detail page, the skill's page, and the task's page with its runs.
3. **Given** a link-shaped reference inside a code block or inline code, **When** the message renders, **Then** it stays literal text (the same treatment memory wiki links get today).

---

### User Story 2 - The person writes links with suggestions in the composer (Priority: P2)

While typing in the chat composer, typing `[[` opens a suggestion list of that agent's artifacts, functions, skills, tasks, and memory bundles/documents. The person filters by typing, picks with keyboard or pointer, and the composer ends up holding a complete link. The sent message renders that reference as a clickable link.

**Why this priority**: Nobody can be expected to memorize entity names or the syntax; the suggestion list is both the discovery mechanism for the feature and a live inventory of the agent's work. It is the person-facing half of the feature and can ship right after rendering exists.

**Independent Test**: With an agent owning at least one entity of every kind, type `[[` in the composer, pick one entity of each kind, and send; every inserted link renders and navigates.

**Acceptance Scenarios**:

1. **Given** an agent with artifacts, functions, skills, tasks, and memories, **When** the person types `[[` in the composer, **Then** a suggestion list appears showing that agent's entities, each entry identifiable by kind.
2. **When** the person keeps typing after `[[`, **Then** the list narrows to matching entities, and typing a kind prefix (for example `[[skill:`) narrows the list to that kind.
3. **When** an entry is picked (arrow keys plus Enter or Tab, or pointer/touch), **Then** the partial token is replaced by a complete link; **When** Escape is pressed, **Then** the list closes without sending and typing resumes normally.
4. **When** the message is sent, **Then** the link part of the text renders as a clickable link in the conversation for everyone who can view it.

---

### User Story 3 - Uniform rendering and graceful breakage (Priority: P3)

The same link syntax renders the same way on every surface that already renders memory wiki links (memory documents, the conversation view), and it degrades gracefully when the target disappears.

**Why this priority**: The feature's value collapses if a link works in chat but is dead text in a memory document, or if a deleted entity garbles the surrounding text. This story is what makes the syntax trustworthy, but it can only be verified once links exist (P1) and are being written (P2).

**Independent Test**: Place one link of each kind in a memory document and in a chat message; delete one of the referenced entities; confirm the links render uniformly before, and display as visibly broken but harmless afterwards.

**Acceptance Scenarios**:

1. **Given** a memory document containing one link of each kind, **When** it is viewed, **Then** each renders as a link — including the optional alias form — matching how existing memory links render.
2. **Given** a linked entity that is later deleted or renamed, **When** the old reference is viewed, **Then** it displays in a recognizably broken state, the surrounding text is unaffected, and following it does not produce a broken page or an error screen.
3. **Given** a person who lacks the right to view the linked entity (or a reader of a shared conversation snapshot), **When** they follow the link, **Then** they get the existing sign-in/not-available experience, and nothing they cannot see leaks into the link's label or destination.

### Edge Cases

- A link with an unknown kind (for example `[[flavour:vanilla]]`) renders as literal text or a broken link — never as a message failure or a crash.
- Several of an agent's entities of one kind share a name: resolution follows one documented order (newest first), and the suggestion list shows enough context (kind, order) to tell them apart.
- `[[` typed inside a code block or inline code offers no suggestions, matching rendering.
- `[[` typed with an empty query shows the agent's entities immediately rather than waiting for text.
- Names containing the syntax's own characters (`|`, `]]`) are never produced by the suggestion list; a pasted ambiguous link renders as plain text.
- A task link points at the task definition's page (which lists its runs); there is deliberately no per-run link syntax in v1.
- A skill link to a skill that was uninstalled (from any of its scopes) displays as broken state.
- In a shared conversation snapshot or the exported offline file, entity links reveal nothing about the live agent; following one behaves like any other sign-in-required page (or renders non-clickable in the offline viewer).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-1**: The system MUST support an entity wiki-link syntax `[[kind:name]]` where kind identifies exactly one of: artifact, function, skill, task; name addresses the entity within the conversation's agent. The alias form `[[kind:name|label]]` MUST be supported wherever links render. Existing memory link forms (`[[bundle/slug]]`, `[[bundle]]`, bare `[[slug]]`) MUST continue to work unchanged.
- **FR-2**: The WebUI conversation view MUST render entity links as clickable links — label from the alias or the entity's name, with the kind visually distinguishable — and MUST NOT linkify text inside code blocks or inline code.
- **FR-3**: Entity links MUST render identically on every surface that renders memory wiki links today (memory documents at minimum), so the same reference reads the same everywhere.
- **FR-4**: The composer MUST offer suggestions when `[[` is typed: that agent's artifacts, functions, skills, tasks, and memory bundles/documents; narrowing as the person types, including narrowing by kind prefix; keyboard selection (arrows, Enter/Tab, Escape) and pointer/touch selection; insertion replaces the partial token with a complete link. The interaction MUST match the existing `@` file-mention suggestions in the same composer (debounce, dismissal, and re-open behaviour).
- **FR-5**: Suggestions and link resolution MUST be scoped to the current conversation's agent — its own artifacts, functions, and tasks, the skills in the scopes it uses, and its memory — and MUST NOT offer entities the person has no rights to.
- **FR-6**: Agents MUST be able to emit links of every kind in replies and memory documents as plain text, guided where the memory-link guidance is given today; emitting a link requires no tool call.
- **FR-7**: Following a link MUST land on the entity's existing page — artifact → its view, function → its detail page, skill → its page, task → its page including runs, memory → the document or bundle view — with no new pages required.
- **FR-8**: A link MUST NOT grant any permission: destination pages enforce existing access rules, and unauthorized access follows the existing not-available behaviour.
- **FR-9**: A link MUST be a reference only: it does not attach the entity's content to the turn, change steering or queueing behaviour, or trigger anything on send.
- **FR-10**: Links in saved conversations and memory documents MUST keep resolving after reload and restart with no data migration; a target that no longer exists renders as broken state and never breaks the surrounding text.
- **FR-11**: When several entities of one kind share a name, resolution MUST follow one documented order (newest first) and the suggestion list MUST provide context to distinguish them.
- **FR-12**: A malformed or unknown-kind link MUST be inert: rendered literally or as broken state, never a message failure, an error report, or a crash.

### Key Entities *(include if feature involves data)*

- **Wiki link**: the reference itself — a kind, a target name, an optional alias — carried inline in message text and memory documents, resolved within one agent's scope. Derived for rendering; v1 stores no separate link inventory.
- **Artifact**: a named, versioned file an agent owns; a link target resolves to its current version.
- **Function**: a named, versioned custom tool saved by an agent or its people; a link target resolves to its detail page.
- **Skill**: a named reusable behavior from one of the skill scopes (project, agent, global); a link target resolves to the skill's page.
- **Task**: a named scheduled task of an agent with its run history; a link target resolves to the task's page with its runs.
- **Memory bundle/document**: an existing link target; unchanged by this feature.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person moves from an agent reply to any referenced entity in a single click — no searching, name copying, or menu navigation — verified for all four kinds.
- **SC-002**: From typing `[[` to a complete inserted link takes at most three interactions (type, filter, pick), with suggestions appearing within one second of pausing typing on a normal connection.
- **SC-003**: A fixture set holding one link of each kind in a chat message and in a memory document renders 100% correctly: right label, right destination, zero misrouted or dead links.
- **SC-004**: After a linked entity is deleted or renamed, every existing reference displays visibly broken but harmless — no crash, no lost text, no wrong destination — in both the conversation view and memory documents.
- **SC-005**: In a scripted test session where an agent is asked to cite its work, at least 90% of the entity references it emits use the link syntax and resolve to real entities.

## Assumptions

- **Syntax**: the kind-prefixed form `[[kind:name]]` is chosen because bare `[[name]]` already means a memory bundle/concept reference and must keep that meaning; prefixes keep the kinds unambiguous and leave room for future kinds (conversations, other agents) without reinterpreting old links. This is a product-facing decision recorded here rather than asked, because the alternative (bare-name guessing with a resolution order) is strictly worse for people and for agents.
- **Kinds in v1**: artifacts, functions (custom tools), skills, tasks, plus the existing memory targets. Linking conversations, other agents, integrations, or MCP servers is out of scope.
- **References, not attachments**: a link never pulls the entity's content into the turn — that remains the job of the composer's attachment and `@` file-mention flows.
- **Rendering is derived**: links are rendered from the stored message/memory text at display time; v1 builds no link inventory, backlink graph, or index for the new kinds (memory keeps its existing relations).
- **Suggestions are per-agent**: the list shows the conversation agent's entities and the skills/memory visible to it; there is no cross-agent search.
- **Channels**: rendering and suggestions are WebUI features; Discord/Telegram message rendering is untouched (external renderers). Agent-side link emission works on every channel because it is plain text.
- **Memory editor**: people with rights to edit a memory document can write these links by hand today; composer suggestions cover chat messages only (the memory editor may adopt the same suggestions later).
