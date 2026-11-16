# Feature Specification: HTML Generative UI

**Feature Branch**: `063-html-generative-ui`

**Created**: 2026-11-16

**Status**: Draft

**Input**: User description: "Generative UI built on json-render (spec 026) is too strict, looks samey and its interaction is limited. Before it, agents wrote raw HTML and could build a chess game or a Doom clone; since json-render those either can't be built, break, or look bad. Rebuild generative UI as one tool (no second UI tool) that takes HTML again, with a Vizier kit inside the page so a plain page looks like Vizier, versioned pages with patch edits so iterating is fast, and script errors reported back to the agent. Move the tool from a WebUI client tool to a built-in tool, offered only in WebUI conversations (not tasks, dreams, etc.), and available even when no WebUI tab is open. Not in Workspaces for now. This is the foundation for agents creating subapps and mods inside the app edition later."

## Context

Spec 026 replaced `render_interactive_ui` (the agent wrote a whole HTML page) with `ui_render` (the agent describes a page with components from a fixed catalogue). It fixed what it set out to fix: simple dashboards became faster and always matched the app's look. It also took away what made the old tool valuable. The catalogue is a fence. Anything it does not foresee (a chess game, a physics toy, a drawing tool, a small arcade game) either cannot be built, comes out broken, or looks poor. Every page also looks like every other page.

The old tool's real problems were narrower than they seemed:

- **Slow iteration.** Every change meant writing the whole page again.
- **Fragility.** One script error left a blank frame, and only the person could notice it.
- **No look by default.** Every page brought its own styles.

This feature keeps HTML's freedom and fixes those three problems directly:

- **Edits instead of rewrites.** A page is kept with every version, and a change is a small edit to the last version.
- **Errors reported back.** Script errors go back to the agent so it can repair the page in the same turn.
- **A Vizier kit inside every page.** Theme, base styles and optional ready-made building blocks.

There is still exactly **one** generative UI tool.

This is phase 1 of a longer path. Phase 2 (pages saved as named **subapps** opened outside a conversation) and phase 3 (**mods** placed into the desktop app's own surfaces) are out of scope here. Phase 1's decisions (pages as versioned records owned by an agent, a bridge whose abilities are named and versioned, one sandbox host that is not tied to the chat) are made so those phases add places and abilities without reworking this one.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The agent builds anything a web page can be (Priority: P1)

A person asks their agent in a WebUI conversation for something interactive and unusual: "make me a chess game I can play against you", "a small Doom-like maze", "a pendulum simulation with sliders". The agent writes a complete HTML page (markup, styles, scripts) and hands it to the one generative UI tool. The page appears inside the agent's reply, isolated from the rest of the WebUI, and the person plays with it right there. A page can also go full screen within the chat panel.

**Why this priority**: Expressiveness is the reason for the feature. Without it, nothing else here matters.

**Independent Test**: In a WebUI conversation, ask for a playable chess board with legal-move checking. The agent produces a page in one turn. Pieces move by click or drag, illegal moves are refused, and the page runs with no network access and no access to the WebUI.

**Acceptance Scenarios**:

1. **Given** a WebUI conversation, **When** the agent renders an HTML page containing a canvas game loop, **Then** the game runs inside the reply at interactive speed, and keyboard and pointer input reach it while it has focus.
2. **Given** a rendered page, **When** its script tries to read the WebUI's storage, cookies, parent document or sign-in token, or tries to make a network request, **Then** the attempt fails and the WebUI is unaffected.
3. **Given** a page that shows an image, audio or video from an `https` address, **When** it is drawn, **Then** the media loads with no prompt. The same media at a plain `http` address does not load, and the block is reported in verification.
4. **Given** a rendered page, **When** the person chooses full screen, **Then** the page fills the chat panel and can be returned to its place in the reply.
5. **Given** a successful render, **When** the turn finishes, **Then** the page is the turn's final answer: no further model call follows it, as with today's `ui_render`.
6. **Given** a page whose opponent AI thinks for several seconds through the kit's background helper, **When** the person waits for its move, **Then** the page keeps animating and the WebUI keeps responding.

---

### User Story 2 - Changing a page is fast (Priority: P1)

The person says "make the board wooden and add a move history". The agent does not write the page again. It sends a few small edits, each one replacing an exact piece of the latest version with new text. A new version of the same page is drawn in the reply, and the earlier version stays where it was in the history.

**Why this priority**: The person named slow iteration as the reason pure HTML was not enough. Without this, the feature is the old tool again.

**Independent Test**: After the chess page exists, ask for a colour change and a small new panel. The agent's change is a short list of edits (a small fraction of the page's size), version 2 appears in the new reply, and version 1 still shows in the earlier reply.

**Acceptance Scenarios**:

1. **Given** a page at version N, **When** the agent sends edits based on version N, **Then** version N+1 is created from N with every edit applied, and it is drawn in the agent's reply.
2. **Given** an edit whose search text appears zero times or more than once in the version it is based on, **When** the agent sends it, **Then** no version is created, and the agent is told which edit failed and why (not found, or found K times).
3. **Given** a page whose latest version is N+1, **When** the agent sends edits based on version N, **Then** the edits are refused and the agent is told the latest version number, so it never overwrites a change it has not seen.
4. **Given** a page with several versions, **When** the person scrolls the history, **Then** each reply still shows the version it produced.
5. **Given** a large page, **When** the agent needs a sweeping change, **Then** it can still send a complete new HTML document as the next version of the same page.

---

### User Story 3 - A broken page is repaired before the person sees it (Priority: P2)

The agent's first draft throws an error on load. With a WebUI tab open on the conversation, the page is mounted, and the errors raised in its first moments (uncaught errors, rejected promises, logged errors, blocked requests) are sent back to the agent as the tool's result. The turn keeps running, and the agent fixes the page with an edit. The person sees the working version, not a blank frame.

**Why this priority**: Fragility was the second main complaint about the old tool, and the person cannot be the only error detector. It depends on Stories 1 and 2.

**Independent Test**: Make an agent (offline test provider) render a page that throws on load, with a tab open. The tool's result lists the error with its message and line. A follow-up edit that fixes it yields a page reported clean, which ends the turn.

**Acceptance Scenarios**:

1. **Given** a tab open on the conversation, **When** a rendered version throws during its first moments, **Then** the tool reports failure with each error's message and location, the turn continues, and the version is still saved.
2. **Given** a tab open on the conversation, **When** a rendered version raises no error in its first moments, **Then** the tool reports success, marked as verified, and the turn ends with the page.
3. **Given** no tab open on the conversation, **When** the agent renders a page, **Then** the version is saved, the tool reports success marked as not verified, the turn ends, and the page is drawn when the person next opens the conversation.
4. **Given** several tabs open on the conversation, **When** a page is rendered, **Then** exactly one verification report is used and the tool does not wait on the others.
5. **Given** a tab that never answers the verification, **When** the verification limit passes, **Then** the tool reports success marked as not verified, rather than holding the turn.

---

### User Story 4 - Plain pages look like Vizier, and common pieces are ready-made (Priority: P2)

Every page starts with a Vizier kit already present:

- the colours of the person's current theme, light or dark, changing with it live;
- the app's typography and spacing;
- base styling for ordinary buttons, inputs, tables and headings;
- optional ready-made building blocks for the things agents draw most (a card, a stat, a table, a chart, tabs, a form field), usable as ordinary HTML tags.

An agent writing a dashboard needs only a few tags. An agent writing a game ignores the blocks and keeps the theme. Any style can be overridden.

**Why this priority**: It answers "looks bad" and "samey" without a fence, and it keeps simple pages short, which keeps them fast. The feature works without it, so it ranks after Stories 1 and 2.

**Independent Test**: Render a page with only a heading, a paragraph, a button and a table, and no styles of its own. It matches the WebUI's look in both themes, and switching the theme with the page open recolours it without reloading. Render a page with three stat blocks and a chart block: it draws correctly from a few lines of HTML.

**Acceptance Scenarios**:

1. **Given** a page with no styles of its own, **When** it is drawn, **Then** its text, colours, buttons, inputs and tables follow the current Vizier theme.
2. **Given** a page is open, **When** the person switches the theme or colour scheme, **Then** the page recolours without reloading and without losing its state.
3. **Given** a page that uses a kit block with an invalid attribute, **When** it is drawn, **Then** the block shows a visible, readable problem in place, and the problem is included in the verification report.
4. **Given** the agent wants to know the kit, **When** it asks the tool for its reference, **Then** it receives the full list of blocks, their attributes, examples and the bridge abilities, on demand and never in the system prompt.
5. **Given** a page that needs 3D rendering or charts, **When** it loads one of the libraries shipped with the kit, **Then** the library is available with no internet access.

---

### User Story 5 - Pages remember and talk back (Priority: P2)

A page can keep its own state, such as a game position, a form draft or a selected tab. That state survives reloading the conversation, reopening it later and having a second tab open (both tabs converge). A page can also send a message into the conversation as the person's turn ("I moved e2-e4") when the person really interacts with it. This is how a game against the agent continues.

**Why this priority**: Interaction is one of the three complaints. Today's pages already have these abilities, and dropping them would be a regression.

**Independent Test**: Play two chess moves and reload the WebUI: the position is the same. Make a move that sends it to the agent: the agent answers with its move, applied as an edit or through the page's state.

**Acceptance Scenarios**:

1. **Given** a page saved some state, **When** the conversation is reloaded or opened in another tab, **Then** the page starts from the saved state.
2. **Given** a page sends a message in response to a click or key press, **When** that happens, **Then** the message becomes the person's next turn in the conversation, carrying any structured data the page attached.
3. **Given** a page tries to send a message with no recent real interaction, or more often than allowed, **When** it tries, **Then** the message is not sent and the page is told why.
4. **Given** a new version of a page, **When** it is drawn, **Then** it starts from the state the previous version saved, so an edit does not reset a game in progress.
5. **Given** a page's state written in one tab, **When** another tab has the page open, **Then** the other tab receives the change.

---

### User Story 6 - The tool is offered only where a page can be shown (Priority: P3)

The tool is part of every WebUI conversation of an agent, including project chats in the desktop app, whether or not a tab is open at that moment. It does not exist in scheduled tasks, dream cycles, side chats, background (detached) scripts, subagent workers, Discord, Telegram or Workspaces. An agent there does not see it, and its category list does not mention it.

**Why this priority**: Without it, pages would be created where nobody can see them. It is a rule applied across the stories above.

**Independent Test**: List the agent's tools in a WebUI conversation (present), a scheduled task run (absent), a dream (absent) and a Discord conversation (absent).

**Acceptance Scenarios**:

1. **Given** a WebUI or project conversation with no tab open, **When** the agent's turn runs, **Then** the tool is offered.
2. **Given** a scheduled task, dream, side chat, detached script, subagent worker, Discord, Telegram or Workspace conversation, **When** the agent's turn runs, **Then** the tool is neither offered nor callable from a script.
3. **Given** a WebUI conversation, **When** the agent lists tool categories at different moments of the conversation, **Then** the listing does not change with whether a tab is open.

---

### User Story 7 - Existing pages keep working (Priority: P3)

Conversations already hold pages drawn by `render_interactive_ui` and by the catalogue-based `ui_render`. They still draw exactly as before. New turns offer only the new tool.

**Why this priority**: It protects saved history. It is low effort if planned from the start, and costly to repair later.

**Independent Test**: Open a conversation with an old HTML page and one with a catalogue page: both draw and keep their saved state. A new turn in either conversation offers only the new tool.

**Acceptance Scenarios**:

1. **Given** a conversation with pages from either retired tool, **When** it is opened, **Then** each page draws and behaves as it did before this feature.
2. **Given** any new turn, **When** the agent's tools are listed, **Then** neither retired tool is offered, and calling a retired name returns guidance pointing to the new tool.

---

### Edge Cases

- An edit's search text is found in the base version more than once, or not at all. The whole edit set is refused, and no partial version is saved (Story 2).
- Two edit sets arrive based on the same version, for example from a retry. The first creates the next version. The second is refused as out of date and names the latest version.
- The agent edits a page from another conversation. This is refused. In this phase, a page can be edited only in the conversation where it was created, because its content may hold that conversation's private material.
- A version would exceed the size limit. It is refused with the size and the limit, and the previous version stays current.
- The page loops forever on load:
  - On browsers that run embedded pages in their own process, the WebUI stays responsive, and verification reports the page as unresponsive.
  - Elsewhere, the tab may freeze until it is reloaded. After the reload, that version is shown paused, and the agent's next turn is told why.
- The page runs a heavy but finite computation (for example a chess engine searching several moves ahead) through the kit's background helper. The page and the WebUI stay responsive while it runs. A computation that passes its time limit is stopped and reported to the page as timed out.
- The page throws only later, on a click. That error is not part of the tool's result, because the turn has ended. The person sees a small error indicator on the page.
- A tab is closed between the start of verification and its report. The tool reports success marked as not verified.
- The conversation is deleted. All its pages, versions and states are deleted with it. Deleting the agent deletes all of its pages.
- The person opens a conversation on a phone. The page fits the narrow width and full screen is available.
- The agent is updated or restarted mid-render. The turn resumes as any interrupted turn does, and a version saved before the stop is not saved twice.
- A remote `https` image is slow, missing or not an image. The page shows the browser's usual broken-image state, and nothing else on the page is affected.
- A page refers to a session file by name. It loads that file as today's pages do. A missing file shows a placeholder.

## Requirements *(mandatory)*

### Functional Requirements

**The one tool**

- **FR-001**: The system MUST offer exactly one generative UI tool to agents. It renders a page either from a complete HTML document (a new page, or a full replacement version of an existing page) or from a set of edits to an existing page.
- **FR-002**: The tool MUST be a built-in tool of the agent, not one declared by a connected WebUI tab. Its pages and versions MUST be stored by the server, independently of whether any tab is open.
- **FR-003**: A successful render MUST end the turn, with the page as the final answer, following the existing turn-ending rule. A refused render or a failed verification MUST leave the turn running.
- **FR-004**: The tool MUST provide its full reference (kit blocks, attributes, examples, bridge abilities, limits) to the agent on demand. The system prompt and the tool's own description MUST stay the same across turns.

**Pages and versions**

- **FR-005**: Each page MUST belong to the agent and to the conversation in which it was created, MUST have a stable identifier and a title, and MUST keep every version in order (1, 2, 3, …).
- **FR-006**: Each reply that renders a page MUST keep showing the version that reply created, even after later versions exist.
- **FR-007**: An edit set MUST name the page and the version it is based on, and MUST consist of one or more edits. Each edit replaces one exact occurrence of a search text with replacement text.
- **FR-008**: An edit set MUST be applied completely or not at all. A search text found zero times, or more than once, MUST refuse the set and say which edit failed and the count found.
- **FR-009**: An edit set based on a version that is no longer the latest MUST be refused, and the refusal MUST name the latest version.
- **FR-010**: Editing a page MUST be refused outside the conversation where the page was created.
- **FR-011**: A version MUST be refused when it exceeds the page size limit, and the refusal MUST state the size and the limit.
- **FR-012**: Pages, versions and saved states MUST be deleted with their conversation and with their agent.
- **FR-013**: Every refusal MUST be returned to the agent as an explanatory result it can act on, never as a failure that ends the turn.

**Isolation**

- **FR-014**: A page MUST run isolated from the WebUI: no access to its storage, cookies, sign-in token, parent document or other pages.
- **FR-015**: A page's scripts MUST NOT reach the network: no requests made by script, no live connections, and no scripts, styles or fonts loaded from outside Vizier. The libraries shipped with the kit MUST be served by Vizier itself and work offline.
- **FR-015a**: A page MUST be able to show images, audio and video from any `https` address, with no approval step. Other schemes (plain `http` included) MUST be refused for remote media.
- **FR-016**: A page MUST NOT be able to freeze the WebUI on browsers that run embedded pages in their own process.
- **FR-016a**: On other browsers (where an embedded page shares the WebUI's thread):
  - The kit MUST offer a one-call way to run heavy computation off the page's main thread, with a time limit. The tool's guide MUST tell agents to use it for heavy work (search, AI, simulation steps) and to drive animation and game loops by frames, never by blocking loops.
  - Verification MUST report a page that stops responding.
  - A page version that froze the WebUI MUST be shown paused, with a way to run it, the next time the conversation is drawn.

**Verification**

- **FR-017**: When at least one tab is open on the conversation, the system MUST mount the new version in one tab and collect the problems raised during a short observation period: uncaught errors, rejected promises, logged errors, blocked requests, kit attribute problems, and no response. It MUST report these to the agent with each problem's message and location.
- **FR-018**: A verified version with problems MUST be reported as a failure and keep the turn running. A verified version without problems MUST be reported as success, marked verified.
- **FR-019**: When no tab is open, when the tab closes during verification, or when no report arrives within the verification limit, the render MUST be reported as success, marked not verified, without holding the turn.
- **FR-020**: The version MUST be saved whatever the verification outcome, so the agent can edit it to fix problems.
- **FR-021**: When a page raises an error after verification, the person MUST see a visible error indicator on that page.

**The kit**

- **FR-022**: Every page MUST start with the person's current Vizier theme (colours for light and dark, typography, spacing) and base styling for standard HTML elements. The page's own styles MUST be able to override all of it.
- **FR-023**: A theme or colour-scheme change MUST reach open pages live, without reloading them and without losing their state.
- **FR-024**: The kit MUST offer ready-made blocks usable as HTML tags, covering at least: card, stat, table, chart, tabs, form field (text, number, select, checkbox, slider), badge, callout and progress. Each MUST follow the theme.
- **FR-025**: The kit MUST include a curated set of libraries shipped with Vizier, covering at least 3D rendering and charting. Each MUST be loadable by a page without internet access.
- **FR-026**: The tool's reference MUST list every kit block and library, and a check MUST fail when a block exists without a reference entry, or a reference entry without a block.

**Bridge (abilities a page has)**

- **FR-027**: A page MUST be able to read and write its own saved state. The state MUST be kept per page across versions, survive reloads and restarts, reach the page's other open tabs, and be saved at most twice a second.
- **FR-028**: A page MUST be able to send a message, with optional structured data, into its conversation as the person's next turn. This MUST happen only shortly after a real interaction with the page, and at most once a second. A refused send MUST tell the page why.
- **FR-029**: A page MUST be able to load the conversation's session files by name, as today's pages can.
- **FR-030**: The bridge MUST carry a version, and a page's abilities MUST be named, so later phases can add abilities (for example calling the agent's tools, or app data) without changing how existing pages work.

**Where it is offered**

- **FR-031**: The tool MUST be offered in WebUI conversations and desktop-app project conversations, whether or not a tab is open. It MUST NOT be offered in scheduled tasks, dreams, side chats, detached scripts, subagent workers, Discord, Telegram or Workspaces. Where it is not offered, it MUST be absent from the agent's tool listings and categories, and uncallable from scripts.
- **FR-032**: Whether the tool is offered MUST depend only on the kind of conversation, so the agent's tool listing stays the same for the whole conversation.

**Presentation**

- **FR-033**: A page MUST draw inside the reply at a height that fits its content, up to a limit, and offer full screen within the chat panel. It MUST fit phone widths.
- **FR-034**: The person MUST be able to download a page version as a standalone HTML file that works on its own, as today's pages can.
- **FR-035**: Each drawn page MUST show its title and version number.

**Retiring the current tools**

- **FR-036**: Pages saved by `render_interactive_ui` and by the catalogue-based `ui_render` MUST keep drawing and keep their saved state.
- **FR-037**: Neither retired tool MUST be offered to new turns, and a call to a retired name MUST return guidance naming the new tool. The compatibility kept for old pages MUST be recorded for removal before 1.0.0 (issue #82).

### Key Entities *(include if feature involves data)*

- **Page**: an interactive document an agent made in one conversation. It has an identifier, a title, an owning agent and conversation, its versions in order, and its saved state.
- **Page Version**: one complete HTML document of a page. It has a number, its size, its time, how it was produced (a full document or an edit set based on version N), and its latest verification outcome.
- **Edit Set**: the change from one version to the next. It names the page and its base version, and holds an ordered list of exact search-and-replace pairs.
- **Verification Report**: what one tab observed when it mounted a version: verified or not, and the problems found, each with a kind, message and location.
- **Page State**: the data a page saved through the bridge. There is one per page, shared by its versions.
- **Kit**: the theme, the base styles, the ready-made blocks and the shipped libraries every page starts with, plus the reference describing them.
- **Bridge**: the named and versioned abilities a page has (state, send message, session files), designed to grow in later phases.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Each of these can be built by an agent in one WebUI conversation and works when tried: a two-player chess board with legal moves, a first-person maze with keyboard movement, a physics simulation with sliders, and a dashboard with stats and a chart. Under json-render, none of the first three could be built.
- **SC-002**: A small change to an existing page (a colour, a label, one new control) takes the agent no more than 10% of the output that writing the whole page again would.
- **SC-003**: A small change appears in the conversation in under 15 seconds, measured on the same model that takes over a minute to write the whole page.
- **SC-004**: In a set of at least 20 first drafts that fail on load with a tab open, at least 90% are fixed by the agent in the same turn, with no message from the person.
- **SC-005**: A page with no styles of its own is indistinguishable in colour and typography from the surrounding WebUI, in both themes, under a side-by-side review.
- **SC-006**: 100% of saved pages from the two retired tools in an existing test workspace still draw and keep their state after the upgrade.
- **SC-007**: In none of the excluded conversation kinds (FR-031) can the agent see or call the tool.
- **SC-008**: In a hostile-page test suite covering storage, cookies, token, parent document, script network requests, outside scripts and frame-escape attempts, no page can read anything from the WebUI or make a network request from script. `https` images, audio and video are the only remote loads that succeed.

## Assumptions

- **Scope is phase 1 only.** Named subapps opened outside a conversation, pinning, mods in the desktop app's surfaces, and bridge abilities that reach agent tools or app data are later specs. This spec only keeps the door open for them (FR-005, FR-030).
- **People do not edit pages by hand** in this phase, and there is no page gallery. A page is seen where it was rendered.
- **Limits:**
  - 1 MiB per version;
  - no limit on the number of versions while the conversation exists;
  - a verification observation period of about two seconds within an overall limit of about ten seconds;
  - the existing interaction window and send rate for messages.
  These may be tuned in planning.
- **The kit's blocks** start from the components of the current catalogue, so the WebUI's look stays the reference. The shipped libraries are a short curated list chosen in planning (at least 3D and charts). A page's scripts still cannot reach the internet, for safety and so pages work in the desktop app offline.
- **Remote `https` media loads with no approval** (decided 2026-11-16). This is an accepted risk: each load tells the remote host the person's address, and a page's script can put data into a media address, so that address is a narrow way out. Script requests, live connections and outside scripts stay blocked. For an image that must always appear, the kit's reference advises saving it into the conversation's files first.
- **Freezes are handled by guidance, not interception** (decided 2026-11-16). Only one task that runs too long can freeze a page, and an endless loop is rare. Heavy but finite work is the realistic case, and the guide plus the background helper move it off the main thread. Rewriting the page's code to interrupt loops is out of scope and can follow if freezes happen in practice.
- **Runtime errors after verification** are only indicated to the person (FR-021). Sending them to the agent automatically is out of scope.
- **Workspaces** are excluded for now. Page state per person in a shared space needs its own design.
- **The Vizier look stays monochrome** by default. Pages may use their own colours.
- **UI work follows the design-first rule.** The page frame, version label, error indicator and kit blocks are mocked before code. Because FigCraft may be unavailable, a simple HTML mockup is acceptable.
- **The change is agent-observable**, so it is checked end to end with the offline test provider, as the constitution requires.
