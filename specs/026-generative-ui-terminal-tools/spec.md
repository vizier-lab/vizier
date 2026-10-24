# Feature Specification: Generative UI and Turn-Ending Tools

**Feature Branch**: `026-generative-ui-terminal-tools`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "currently interactive ui feature is slow, as agent has to build html from scratch, let's replace it with generative ui (a better name), by leveraging https://github.com/vercel-labs/json-render allowing agent(s) to build from prebuilt components while also allows for interactivity. while we are at it i would like to introduce 'terminal' flag (may need better name), when a tool (whether client, or builtin) have this flag it will ends the conversation turn and act as final answer, this flag also inject additional tool description to let the agent know that it is terminal. i want the new generative ui tool to be terminal"

## Context

Today an agent that wants to show something richer than text in the WebUI (a chart, a table, a form, a set of buttons) calls `render_interactive_ui` and hands it a complete HTML page it has written itself: markup, styles, scripts and all. That is slow (the agent emits hundreds of lines for a simple dashboard), costly in tokens, inconsistent in look from one reply to the next, and fragile (one script error leaves a blank frame). The agent also needs a long style guide (the `interactive-ui` skill) to get a decent result.

This feature replaces that tool with **generative UI**: the agent describes the interface as a small structured document that only names **prebuilt components** from a fixed catalogue (card, stat, table, chart, form, button, tabs, and so on) and fills in their content. The WebUI draws it with its own components, so it always matches the app's look and theme and needs no style guide. The agent learns the catalogue from the tool itself, on demand, with no skill to load. Interactivity is part of the description: components can read and write small pieces of page state (a selected tab, a typed value, a toggled filter), show or hide parts depending on that state, and trigger a limited set of actions, chiefly sending a message (with optional structured data) back into the conversation as the person's turn. The structured-document approach is the one used by the open-source *json-render* project, which is the reference for the document shape and the catalogue-as-guardrail idea.

Second, this feature introduces a **turn-ending flag** for tools (the request called it "terminal"; this spec uses *turn-ending* in prose because "terminal" already means the shell terminal in this product, and the final flag name is settled in planning). A tool carrying the flag, whether built into vizier, supplied by an integration, or declared by a client, ends the agent's turn once it has been called and its result is the agent's final answer: the agent does not get another round to comment on it. Because the agent has to know this before calling, the tool's description is automatically extended with a standard notice. Generative UI is the first turn-ending tool: showing the interface *is* the answer, and a trailing "Here is your dashboard!" message adds latency and noise.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent answers with a generated interface, quickly (Priority: P1)

A person asks an agent "show me last week's task runs by status". The agent calls the generative UI tool with a short description made of catalogue components (a heading, three stat cards, a table), and the WebUI draws it inside the agent's reply. It looks like the rest of the app, in light and dark theme, and does not depend on the agent knowing any CSS.

**Why this priority**: This is the point of the feature: the slow, hand-built-HTML path becomes a fast, consistent one.

**Independent Test**: On the offline `dummyplug` provider, have an agent call the generative UI tool with a sample description; confirm the interface appears in the reply bubble, matches the active theme, survives a reload and a second tab, and that the description the agent had to write is a fraction of the size of the equivalent hand-written page.

**Acceptance Scenarios**:

1. **Given** a WebUI conversation, **When** the person sends a message, **Then** the agent can discover the generative UI tool and see the catalogue of components it may use, each with its purpose and the content it accepts.
2. **Given** the agent calls the tool with a valid description, **When** the turn ends, **Then** the interface is shown inside the agent's reply bubble for that turn, drawn with the WebUI's own components and theme.
3. **Given** a drawn interface, **When** the person reloads the topic, opens it in a second tab, or opens it from another browser or device, **Then** the same interface is shown in the same place without asking the agent again.
4. **Given** the agent's description names a component that is not in the catalogue, or gives content a component cannot accept, **When** the tool is called, **Then** the call is refused with a message that names the problem and the valid options, so the agent can correct it in the same turn; nothing is drawn from the invalid parts.
5. **Given** the same information a person might ask for, **When** the agent shows it as generated UI instead of as hand-written HTML, **Then** the agent's output for it is substantially shorter (see SC-002).
6. **Given** a description that is well-formed, **When** the WebUI draws it, **Then** a single bad component (for example an unknown chart field value) shows an inline error placeholder for that component and the rest of the interface still renders.

---

### User Story 2 - Generated interfaces are interactive (Priority: P1)

The agent shows a task list with filter tabs ("All / Open / Done"), a text box and a "Add task" button. Switching tabs changes what is visible without any round trip to the agent. Typing in the box and pressing the button sends "Add task: buy milk" into the conversation as the person's message, with the box's value attached as structured data, and the agent answers it like any typed message.

**Why this priority**: The request is explicit that generated UI must "allow for interactivity"; static pictures would be a regression from today's interactive pages.

**Independent Test**: Render an interface with tabs, an input and a send-message button; switch tabs and confirm the visible content changes locally with no new turn; click the button and confirm a turn starts with the expected text and data.

**Acceptance Scenarios**:

1. **Given** an interface with tabs or toggles that change what is shown, **When** the person uses them, **Then** the interface updates immediately without starting an agent turn.
2. **Given** an input component bound to a piece of page state, **When** the person types, **Then** the value is held in that state and other components that show or depend on it update as they type.
3. **Given** a button that sends a message, **When** the person clicks it, **Then** the text appears in the conversation as the person's message, any structured data the interface collected reaches the agent, and the agent's turn starts, queueing behind a running turn like a typed message.
4. **Given** the person has not yet interacted with the interface, **When** it is drawn, **Then** nothing is sent to the agent on its own, and message-sending is limited to a reasonable rate per interface.
5. **Given** an interface whose state has been changed by the person, **When** they reload or open another tab, **Then** the interface comes back in its last state (selected tab, entered values) as it does today for saved page state.
6. **Given** the description asks for an action that is not in the allowed set, **When** the tool is called, **Then** it is refused like any other catalogue violation.

---

### User Story 3 - A turn-ending tool ends the turn as the final answer (Priority: P1)

When the agent calls the generative UI tool, the turn ends right there: the interface is the answer, the agent is not asked to write a closing message, and the person sees the interface (plus any text the agent already wrote in the same step) without waiting for another model round.

**Why this priority**: The user asked for this behaviour specifically for generative UI, and it makes the feature feel fast.

**Independent Test**: Have an agent on `dummyplug` call the generative UI tool; confirm the turn is marked complete immediately after the tool result, that no further model call is made for that turn, and that the conversation history shows the interface as the turn's final entry.

**Acceptance Scenarios**:

1. **Given** the agent calls a turn-ending tool successfully, **When** the tool returns, **Then** the turn ends without another model round, and the tool's output (or what it displayed) is recorded as the turn's final answer.
2. **Given** the agent wrote explanatory text in the same step before calling the tool, **When** the turn ends, **Then** that text is kept and shown with the interface.
3. **Given** the tool call fails (refused description, client gone, timeout), **When** the failure is returned, **Then** the turn does **not** end; the agent gets the error and another round to correct it.
4. **Given** the agent runs a script that calls several tools including a turn-ending one, **When** the script finishes, **Then** all of its calls have completed and the turn ends after the script, not in the middle of it; if the script itself fails, the turn does not end.
5. **Given** a turn that was ended by a turn-ending tool, **When** the person sends their next message (or interacts with the interface), **Then** a new turn starts normally with the earlier turn, including the interface, in its history.
6. **Given** a background, scheduled, dream or inter-agent turn where the tool is not available, **When** those turns run, **Then** their behaviour is unchanged.

---

### User Story 4 - The agent is told a tool is turn-ending (Priority: P2)

Whoever defines a tool (vizier developers for built-in tools, integration authors, client authors over the WebSocket protocol) can mark it turn-ending. The agent always sees, in that tool's description, a standard notice that calling it ends the turn and counts as the final answer, so it does the rest of its work first and does not expect a follow-up round.

**Why this priority**: It makes the flag a general mechanism rather than a one-off, but only the generative UI tool depends on it for this release.

**Independent Test**: Declare a client tool with the flag; list and describe it as an agent; confirm the notice is present exactly once, appended to the author's own description, and that calling it ends the turn. Repeat without the flag and confirm nothing changes.

**Acceptance Scenarios**:

1. **Given** a built-in tool marked turn-ending, **When** the agent lists or describes it (in either the model-facing tool list or the in-script description), **Then** its description ends with the standard notice.
2. **Given** a client-declared tool with the flag set, **When** the request is accepted, **Then** the same notice is added and the same turn-ending behaviour applies, over the same public contract.
3. **Given** a tool without the flag, **When** anything about it is shown or called, **Then** behaviour is exactly as before this feature.
4. **Given** a client sends the flag on a tool it declares, **When** the description is already close to the size limit, **Then** the limit check applies to the author's text, not the added notice (an accepted declaration is never refused because of the notice).
5. **Given** the request declares the same tools on every request, **When** the standing instructions are built, **Then** they stay identical from turn to turn (the notice appears in tool descriptions only).

---

### User Story 5 - Rich apps and simulations, not only dashboards (Priority: P2)

A person asks for "an interactive chess board I can play against you" or "a low-poly 3D terrain generator with sliders for seed and roughness". The agent builds each one from the same catalogue: a grid of cells with pieces and highlights for chess, a 3D scene with a heightfield mesh and orbit controls for the terrain. The parts a fixed catalogue cannot express (which chess moves are legal, how heights are computed from a seed) are small pure functions the agent writes once and the interface runs locally, so moving a piece or dragging a slider responds instantly with no agent turn and no network.

**Why this priority**: The catalogue alone covers dashboards and forms; the stated goal is that the system is flexible enough for apps like these too. Without it people would fall back to hand-written pages and the speed gain is lost for exactly the most impressive uses.

**Independent Test**: With a live model (or a recorded interface description for the offline provider), generate a playable chess board and a terrain generator; confirm that selecting a piece highlights its legal moves, a move updates the board and passes the turn, a slider change regenerates the terrain, the 3D view can be orbited, and none of these interactions starts an agent turn.

**Acceptance Scenarios**:

1. **Given** an interface with a board of cells built from a list in its state, **When** the person presses a cell, **Then** the interface can compute and show the consequence (highlighted moves, a moved piece) locally, using functions the agent supplied with the interface.
2. **Given** an interface with a 3D scene (shapes, lights, a height-field surface drawn flat-shaded, a camera the person can orbit), **When** a slider bound to the scene's inputs changes, **Then** the scene updates immediately.
3. **Given** the agent's functions loop forever, run too long, return too much data, or throw, **When** they are called, **Then** the call is stopped, the person sees a small inline error on that interface, and the rest of the WebUI is unaffected.
4. **Given** the agent's functions try to reach the network, the page, the person's login or storage, **When** they run, **Then** they cannot: they receive only the values passed in and return only data.
5. **Given** the interface needs motion (a simulation step, a clock), **When** it is drawn, **Then** it can run a timed action at a bounded rate that stops when the interface is no longer on screen.
6. **Given** the person's next typed message asks the agent to change the game or terrain, **When** the agent answers, **Then** it can replace the interface with a new one, and the old one stays in history.

---

### User Story 6 - Existing interactive pages keep working, the old tool is retired (Priority: P3)

Conversations that already contain pages drawn with the old tool still show them. New turns no longer offer the old tool; agents use generative UI instead. The old authoring guide is replaced by a short one for the new tool.

**Why this priority**: Protects saved history while removing the slow path.

**Independent Test**: Open a topic that has a previously saved page and confirm it still draws and keeps its saved state; start a new turn and confirm only the new tool is offered.

**Acceptance Scenarios**:

1. **Given** a topic with a page saved by the old tool, **When** it is opened, **Then** the page is drawn as before.
2. **Given** a new turn from the WebUI, **When** the agent lists tool functions, **Then** the old tool is not among them and the generative UI tool is.
3. **Given** the agent attempts to call the old tool's name, **When** the call is made, **Then** it fails with a message pointing to the generative UI tool.

---

### Edge Cases

- The description is too large, nested too deeply, or has too many components: refused with a size/shape message; nothing is drawn.
- The description refers to a component or state path that does not exist (a child id with no definition, a binding to an undefined path): refused where it can be checked up front; otherwise that component shows an error placeholder.
- The agent calls the generative UI tool more than once in a turn: the turn ends at the first successful call (it is turn-ending), so only one interface exists per turn.
- The person's client has no generative UI support (older WebUI, third-party client, Discord/Telegram): the tool is not offered there. Those channels get ordinary text replies, and a turn-ending flag on a tool that is offered still ends the turn.
- The WebUI socket that declared the tool is gone when the agent calls it: behaves like other client tool calls (deferred if the tool is deferrable, otherwise failed), and a failure does not end the turn.
- A turn-ending tool is called while the agent has queued background jobs: the turn ends; the jobs' later reports wake the conversation as they do today.
- A turn is aborted or interrupted before the tool's call has returned: no final answer is recorded and the turn is treated as stopped, as today.
- A resumed turn (after an instance restart) that had already made a successful turn-ending call: it is not asked to continue; the turn ends with the recorded answer.
- Interface state (typed text, selections) grows beyond the saved-state limit: the latest state is refused with the same message as other saved states, and the interface keeps working locally.
- Theme switches (light/dark) while an interface is displayed: it re-themes immediately with no page reload.
- Accessibility: components are keyboard-operable and labelled; the interface can be read by a screen reader as normal app content, not a sealed frame.

## Requirements *(mandatory)*

### Functional Requirements

**Generative UI tool**

- **FR-001**: The WebUI MUST offer the agent a generative UI tool whose input is a structured description of an interface made only of components from a fixed catalogue, plus an optional initial page state and a title.
- **FR-002**: The catalogue MUST cover, at minimum, layout (stack, row, grid, card, tabs), text (heading, paragraph, label, badge), data display (stat, table, list, progress, chart for bar/line/pie), inputs (text input, select, checkbox/toggle, slider), and a button; and MUST be extensible without changing the tool's contract for agents beyond a catalogue update.
- **FR-003**: The agent MUST be able to discover the catalogue (component names, purposes, accepted content and examples) through the tool categories it already sees and the normal tool-listing and tool-describing path, without a skill having to be loaded and without the catalogue being added to the standing instructions. The reference MUST be available on demand, whole or narrowed to chosen components, and MUST include complete worked examples.
- **FR-003a**: The catalogue reference the agent reads MUST be generated from the same definitions that validate and draw the interface, so it cannot drift from what is accepted.
- **FR-004**: vizier MUST validate a description against the catalogue before accepting the call and MUST refuse it, with a message naming each problem and the valid alternatives, if it uses a component or action not in the catalogue, gives content of the wrong shape, references missing elements, or exceeds the size, depth or component-count limits.
- **FR-005**: A refused call MUST NOT end the turn and MUST give the agent the chance to correct it (see FR-014).
- **FR-006**: The WebUI MUST draw the description using its own components, in the active theme, inside the agent's reply for the turn, and MUST keep drawing the rest of an interface if one component cannot be drawn.
- **FR-007**: The accepted description and the interface's latest state MUST be saved with the conversation so reload, a second tab, and another device show the same interface in the same place.
- **FR-008**: An interface MUST NOT load external resources or reach the person's credentials, cookies, storage, network or other parts of the WebUI. The only things an interface can do are draw the catalogue's components, change its own page state, call its own functions (FR-029) and take the allowed actions. The only agent-written code that ever runs is those functions, and only inside the isolation FR-029 describes.

**Flexibility (apps, games, simulations, 3D)**

- **FR-027**: The catalogue MUST let an interface build a list-driven layout (one template repeated per item of a list in its state, with access to the item and its position), so boards, galleries and tables of arbitrary size need no hand-repeated components.
- **FR-028**: The catalogue MUST include a 2D drawing surface and a 3D scene (basic shapes, lights, a camera the person can orbit, a flat-shaded height-field surface and a free-form mesh), both able to report presses on what was pressed.
- **FR-029**: An interface MAY carry small named functions that take and return plain data only. They run locally on the person's device, off the main thread of the page, with no access to the network, the page, storage or the person's login, under a hard time limit and a size limit on what they return. An action can call one with values from the interface's state and write its result back to state; state changes and a timer can trigger such actions.
- **FR-030**: Values an interface derives (a height map, a board after a move) MUST be allowed to live in state that is not saved with the conversation and is recomputed when the interface is drawn, so large derived data never counts against the saved-state limit.
- **FR-031**: A function that fails, times out or returns too much MUST produce a visible inline error on the interface, leave the previous state intact, and never block the rest of the WebUI.
- **FR-032**: Timed actions MUST have a minimum interval and MUST stop when the interface is not on screen.

**Interactivity**

- **FR-009**: Components MUST be able to read and write named pieces of page state; other components MUST be able to display values derived from that state and to be shown or hidden depending on it, all locally without starting a turn.
- **FR-010**: The allowed set of actions MUST include: set a state value; send a message (text, plus optional structured data that may include current state values) into the conversation as the person's turn; and nothing that reaches outside the conversation.
- **FR-011**: Sending a message from an interface MUST behave like a message the person typed (same history, same queueing, same permissions), MUST only be possible after the person has interacted with that interface, and MUST be rate-limited per interface.
- **FR-012**: The agent MUST be able to tell that a message came from an interface, see its structured data, and identify which interface it came from.
- **FR-013**: An interface's page state MUST be saved as it changes (without starting a turn or being shown to the agent), subject to a size limit and rate limit, restored on reload, and kept in step across tabs.

**Turn-ending tools**

- **FR-014**: A tool definition MUST be able to carry a turn-ending flag, whether the tool is built in, contributed by an integration, or declared by a client over the WebSocket protocol.
- **FR-015**: When a call to a turn-ending tool returns successfully, the agent's turn MUST end without a further model round, and the call's outcome MUST be recorded in history as the turn's final answer. A call that fails MUST NOT end the turn.
- **FR-016**: Text the agent produced in the same step as the turn-ending call MUST be preserved and shown with the result.
- **FR-017**: When the call is made from within a script that makes other calls, the script MUST be allowed to finish first; the turn ends only if the script completed without error and at least one turn-ending call in it succeeded.
- **FR-018**: The description the agent sees for a turn-ending tool MUST end with a standard notice that calling it ends the turn and is the final answer, added by vizier in every place the description is shown; it MUST NOT count against any author-facing description size limit and MUST NOT appear for tools without the flag.
- **FR-019**: Standing instructions for the agent MUST remain identical across turns regardless of the flag (the notice lives in tool descriptions only).
- **FR-020**: The client tools public contract MUST document the flag, the notice, and the turn-ending semantics, versioned with the rest of the contract; declarations that omit the flag behave exactly as before.
- **FR-021**: The generative UI tool MUST be marked turn-ending.
- **FR-022**: Turns in which a tool is not available (dream, scheduled, background, inter-agent) MUST be unaffected; a resumed turn that already contains a successful turn-ending call MUST end with it rather than continue.

**Replacing the old tool**

- **FR-023**: The previous interactive HTML tool MUST no longer be offered for new turns, and calling its name MUST fail with a message pointing to the generative UI tool.
- **FR-024**: Pages already saved by the previous tool MUST keep being drawn, with their saved state, in existing conversations.
- **FR-025**: The authoring guide for hand-written pages MUST be retired and replaced by guidance for the new tool where guidance is still useful (when to use it, composing from the catalogue).
- **FR-026**: Clients that cannot draw generative UI (other channels, third-party clients that do not declare the tool) MUST NOT be offered it.

### Key Entities

- **Interface description**: The structured document the agent submits: a root component, a set of named components each with a type, content and children, an optional initial page state, and a title.
- **Component catalogue**: The fixed list of components and allowed actions, each with a purpose, accepted content and an example. It is the sole vocabulary an interface can use, and is what the agent reads and what validation enforces.
- **Page state**: The small piece of structured data an interface keeps (selected tab, typed values). Saved per interface, never shown to the agent unless attached to a sent message.
- **Interface event**: A message sent from an interface (text, optional data, source interface), starting a turn.
- **Turn-ending flag**: A property of a tool definition marking its successful call as the turn's final answer.
- **Final answer**: The recorded outcome of a turn that ended through a turn-ending tool.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A typical dashboard-style answer (title, three stats, one table, one chart) appears in the conversation in at most half the time it takes today with a hand-written page, measured on the same request and model.
- **SC-002**: For the same content, the agent's output for the interface is at least 70% smaller than the equivalent hand-written page.
- **SC-003**: At least 95% of interface descriptions written by agents on a fixed set of 20 sample requests are accepted on the first attempt, and at least 99% within two attempts.
- **SC-004**: 100% of drawn interfaces match the active theme in light and dark with no per-interface styling by the agent.
- **SC-005**: Switching tabs, toggling and typing in a drawn interface respond with no perceptible delay (under 100 ms) and no agent turn.
- **SC-006**: After a successful call to a turn-ending tool, no further model request is made for that turn in 100% of cases, and the closing message round that exists today is eliminated for generative UI answers.
- **SC-007**: A tool author can mark a tool turn-ending with a single flag and nothing else, and 100% of such tools show the standard notice to the agent; tools without the flag are byte-for-byte unchanged in what the agent sees.
- **SC-009**: From a single request each, an agent can produce a playable chess board and a low-poly terrain generator that respond to presses and slider changes in under 100 ms without starting an agent turn, and at least 8 of 10 attempts on a fixed prompt run without a function error on first draw.
- **SC-010**: An agent with no skill installed uses the generative UI tool correctly (valid on first or second attempt) in at least 95% of 20 sample requests, reading only the tool categories, the tool listings and the component reference.
- **SC-008**: Every conversation that contained a page from the old tool before the change still shows that page, with its saved state, after it.

## Assumptions

- The generative UI tool is a WebUI client tool (declared per request over the WebSocket, as `render_interactive_ui` is today), not a server-side tool, so it exists only where a WebUI is connected to draw it.
- The structured description follows the json-render model: a root component and a flat set of named components with type, content and children, plus state bindings, conditional visibility and actions, validated against a catalogue. The project is used as the reference for shape and behaviour; whether vizier depends on the library itself or implements the same format is a planning decision.
- "Turn-ending" is the working name for the flag in this spec because "terminal" already refers to the shell tool; the exact flag name in code and in the public protocol is settled in planning.
- A successful turn-ending call ends the turn by default; there is no per-call override. If the agent needs to show an interface and then keep working, it does the work first.
- The first catalogue is a deliberately small, general-purpose set (FR-002); domain-specific components are added later.
- The functions of FR-029 are written in the one language every browser runs; a separate rules engine or library for any particular game is not shipped.
- Existing saved pages (old tool) keep their current renderer and sandbox; no data migration converts them to the new format.
- Existing limits and mechanisms for client tools (declaration checks, call timeouts, deferral, saved state, rate limits, interface events) are reused; this feature changes what the tool carries, not how client tools work in general.
- Mobile layouts, Discord and Telegram rendering of generative UI are out of scope; those channels receive the agent's text only.
