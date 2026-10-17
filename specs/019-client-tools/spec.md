# Feature Specification: Client Tools

**Feature Branch**: `019-client-tools`

**Created**: 2026-10-17

**Status**: Draft

**Input**: User description: "Client tools: let WebSocket clients declare their own tools on each request, and let agent-rendered UI start new turns. Foundation for third-party clients and WebUI extensions." (full decision list agreed in the brainstorm, summarised in Context below)

## Context

Today every tool an agent can use is defined by vizier itself: built-in tools, addons, MCP servers and channel tools. A client that talks to an agent (the WebUI, or a third-party client someone builds against the WebSocket API) can send messages and show replies, but cannot give the agent abilities that only the client has: drawing something on the user's screen, asking the user to pick a file, reading the browser's location, and so on.

This feature lets a WebSocket client **declare its own tools on each request**. The client owns their definitions (name, description, input schema) and executes them; vizier only checks their shape, offers them to the agent for that one turn, carries each call to the client and the client's answer back. It is meant as the foundation for people building their own clients and WebUI extensions, so the protocol is a documented, versioned public contract.

The first tool built on it is the WebUI's own **`render_interactive_ui`**: the agent writes a small HTML page (a chart, a table, a set of buttons, a form) and the WebUI shows it inline in the conversation. Pages can **send messages back**: if the agent renders a "say hello" button, clicking it puts "hello" into the conversation as the person's message and the agent answers it. The same "client event" mechanism is generic, so any extension can start a turn this way.

Client tools exist only for the turn a client asked for, over the socket that asked. Nothing remembers them: dreams, checkpoint handovers, scheduled tasks, background reports and agent-to-agent conversations never have client tools.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent shows data as an interactive page in the WebUI (Priority: P1)

A person asks an agent "show me last week's task runs as a chart". The agent writes an HTML page with the chart and calls `render_interactive_ui`. The chart appears inside the agent's reply bubble as soon as it is rendered, sized to its content and looking like part of the reply, and the agent's text reply follows in the same bubble. Reloading the topic, or opening it in a second tab, shows the same chart in the same place.

**Why this priority**: It is the visible payoff of the whole feature and exercises the full round trip (declaration, offer to the agent, call to the client, answer back, persistence).

**Independent Test**: In the WebUI, ask an agent on the offline `dummyplug` provider to call `render_interactive_ui` with a sample page; confirm the page appears, the agent's script receives a `rendered` result, and the page is still there after a reload.

**Acceptance Scenarios**:

1. **Given** a WebUI conversation, **When** the person sends a message, **Then** the request declares the WebUI's client tools and the agent can find `render_interactive_ui` by listing tool categories and functions, with the description and schema the WebUI supplied.
2. **Given** the agent calls `render_interactive_ui` with a page, **When** the WebUI receives the call, **Then** it shows the page inside the agent's reply bubble for that turn straight away, while the turn is still running, followed by the turn's reply text in the same bubble, and answers the call with an identifier for the page and status `rendered`, and the agent's turn continues with that result.
3. **Given** a rendered page, **When** the person reloads the topic or opens it in another tab, **Then** the page is shown again in the same reply bubble and position, without the agent being asked again.
4. **Given** a page rendered in browser 1, **When** the same person logs out, logs in on browser 2 (another browser or device) and opens the topic, **Then** browser 2 shows the same page in the same reply bubble, and it works there, even if browser 1 is closed.
5. **Given** the turn fails or is aborted after the page was rendered, **When** the person looks at the conversation, **Then** the page is still shown.
6. **Given** a rendered page, **When** the page's own script tries to read the person's login, cookies or the surrounding WebUI page, or to load anything from the network, **Then** it cannot.
7. **Given** a page taller than the default frame, **When** it renders, **Then** the frame grows to fit it (up to a maximum height, after which it scrolls).

---

### User Story 2 - Clicking inside a rendered page talks to the agent (Priority: P1)

The agent renders a page with a "say hello" button. The person clicks it. "hello" appears in the conversation as the person's message and the agent replies, exactly as if they had typed it. A form inside a page can send both a short text ("Submitted filters") and structured data (the form values), and the agent receives both.

**Why this priority**: It is what makes the pages interactive rather than static, and it is the second half of what was asked for.

**Independent Test**: Render a page with a button that sends "hello" and a form that sends text plus data; click each, confirm the person's bubble shows the text, the agent's turn starts, and the agent can see the data and which page it came from.

**Acceptance Scenarios**:

1. **Given** a page with a button that sends "hello", **When** the person clicks it, **Then** "hello" is added to the conversation as the person's message and a new agent turn starts.
2. **Given** a page that sends text plus structured data, **When** it sends, **Then** the agent receives the text, the data, and a reference to the tool call that rendered the page, and knows the message came from the client rather than being typed.
3. **Given** a page whose script tries to send messages without the person interacting with it (on load, or on a timer), **When** it does, **Then** nothing is sent.
4. **Given** a page that sends many messages in quick succession after a click, **When** it does, **Then** the WebUI forwards at most one per second from that page and drops the rest, and vizier additionally refuses client events beyond a per-session limit with a clear error.
5. **Given** an agent turn already running in the conversation, **When** a page sends a message, **Then** it is queued behind the running turn like any message the person types.
6. **Given** a page rendered in an earlier session of the same topic (for example yesterday), **When** the person clicks it, **Then** the message is still delivered with its reference to the original call.

---

### User Story 3 - A developer adds their own client tool (Priority: P1)

A developer building a WebUI extension or their own client declares a tool of their own, for example `pick_color` in a category `palette`, with a description and an input schema. When their client sends a message, the agent sees the `palette` category, can look up `palette_pick_color`, and calls it. The call arrives on the developer's socket; they show a colour picker, and send the chosen colour back as the result. The documented protocol tells them everything needed to do this without reading vizier's source.

**Why this priority**: This is the stated purpose: the WebUI's own tool is only the first user of a general mechanism. Without it the feature is just one hard-coded widget.

**Independent Test**: With a minimal WebSocket test client (not the WebUI), declare one tool, send a message to a `dummyplug` agent that calls it, answer the call, and confirm the agent's script receives the answer. Also confirm the WebUI's built-in tool is registered through the same extension mechanism a third party would use.

**Acceptance Scenarios**:

1. **Given** a client that declares tools with optional categories and category descriptions, **When** it sends a request, **Then** vizier replies with the final name each tool was given, and the agent can list and describe them under those categories.
2. **Given** a declared tool with no category, **When** it is offered, **Then** it is placed in the category `client`.
3. **Given** a client category that clashes with a built-in category, an addon id, or one of the agent's MCP server names, **When** it is offered, **Then** it is renamed with a `_client` suffix and the reply to the client shows the new name; a client can never add tools to a built-in category or replace a built-in tool.
4. **Given** two declared tools whose final names would collide, **When** they are offered, **Then** the second gets a numbered suffix and the reply shows it.
5. **Given** a declaration that breaks the rules (bad name, too many tools, description or schema too large, schema not valid), **When** the client sends it, **Then** the request is refused before any turn starts, with an error naming the offending tool and rule.
6. **Given** the agent describes a client tool, **When** the description is shown, **Then** it is labelled as provided by the person's client, as is the category in the agent's list of categories.
7. **Given** the agent calls a client tool, **When** the call is sent, **Then** it goes only to the connection that sent the request; other tabs watching the same conversation may display it but are not asked to answer it.
8. **Given** a client answers a call with an error, **When** the agent's script receives it, **Then** it can catch it like any other tool failure and the turn continues.
9. **Given** the WebUI, **When** a developer registers a tool through the WebUI's extension mechanism (definition, how to handle a call, how to draw a saved call), **Then** it is declared on the WebUI's requests, handled live, and drawn on reload, the same way `render_interactive_ui` is.

---

### User Story 4 - Turns without a client behave sensibly (Priority: P2)

A person in the WebUI asks the agent to research something in the background. When the background work finishes, the agent's report turn runs in the same topic, but no client asked for that turn, so it has no client tools. The agent does not try to render a page and fail; it answers in text and can offer to show it visually when the person next writes. Dreams, checkpoint handovers, scheduled tasks and conversations between agents never see client tools either.

**Why this priority**: Without it the agent would carry over expectations from earlier turns and fail confusingly, but the feature is usable without the extra framing.

**Independent Test**: Start a background job from a WebUI turn that declared client tools; when the report turn runs, confirm no client category is offered, that calling a client tool by name fails as an unknown tool, and that the report framing mentions client tools are unavailable.

**Acceptance Scenarios**:

1. **Given** a turn started by anything other than a client's request over a live connection (dream, handover, scheduled task, background report, background piece, consult or delegate session), **When** the agent lists categories, **Then** no client category appears.
2. **Given** such a turn, **When** the agent's script calls a client tool by a name used in an earlier turn, **Then** it gets the normal unknown-tool result with suggestions, and the turn continues.
3. **Given** a background report turn in a conversation where earlier turns had client tools, **When** the turn starts, **Then** its framing tells the agent client tools are unavailable in this turn.
4. **Given** the next message from the person over the WebUI, **When** it arrives, **Then** that turn has the client tools its request declares, regardless of what earlier turns had.

---

### User Story 5 - Connection problems never hang a turn (Priority: P2)

The person closes the tab while the agent is waiting on a client tool call. The agent's script gets a "client disconnected" failure, the turn finishes with whatever the agent says next, and the reply is still saved to the conversation. A client that never answers is timed out.

**Why this priority**: A hung turn blocks the whole conversation queue, so this matters for reliability, but it is an edge path.

**Independent Test**: With the test client, accept a call and close the socket without answering; confirm the script receives `client_disconnected` promptly and the turn completes. Repeat by never answering and confirm the timeout fires.

**Acceptance Scenarios**:

1. **Given** a pending client tool call, **When** the connection that should answer it closes, **Then** the call fails with `client_disconnected` within a few seconds and the turn continues.
2. **Given** a pending client tool call, **When** no answer arrives within the tool's time limit (the client's declared limit, or the default), **Then** the call fails with a timeout and the turn continues.
3. **Given** an answer for a call that already timed out, was aborted, or belongs to another connection, **When** it arrives, **Then** it is ignored and the client is told the call is no longer pending.
4. **Given** a turn waiting on a client tool call, **When** the person aborts the turn, **Then** the pending call is cancelled and the client is told so.

---

### User Story 6 - Pages remember their own state (Priority: P2)

The agent renders a to-do checklist. The person ticks two items and types a note into the page. Later they reload the topic, or log in on another browser, and the page shows the two items ticked and the note filled in, rather than the blank page the agent first drew. If the topic is also open in another tab, that tab's copy of the page can update to match.

**Why this priority**: Without it every reload resets interactive pages, which makes anything longer-lived than a single click (checklists, filters, forms filled over time) frustrating. The pages are still useful without it, so it follows the core stories.

**Independent Test**: Render a page that saves a counter on each click; click three times, reload the topic in another browser, and confirm the page starts at three. With the test client, save state for a call and confirm it comes back with the history and is broadcast to a second connection.

**Acceptance Scenarios**:

1. **Given** a rendered page, **When** its script saves a state value, **Then** the value is stored on the server against the call that rendered the page, replacing any earlier value.
2. **Given** a page with saved state, **When** any client the same person uses loads the topic, **Then** the page is drawn from its original HTML and given its latest saved state when it starts.
3. **Given** a page that has never saved state, **When** it is drawn, **Then** it starts with no state, exactly as the agent rendered it.
4. **Given** the same topic open in two tabs, **When** the page in one tab saves state, **Then** the page in the other tab is told the new state and may update itself; a page that does not listen simply keeps showing what it has.
5. **Given** a page saving state, **When** it saves, **Then** no agent turn starts and the agent is not told; state the agent should know about must be sent as a client event.
6. **Given** a state value larger than the size limit, or saves faster than the rate limit, **When** the page saves, **Then** the save is refused and the page is told why; the last accepted state is kept.
7. **Given** a third-party client that drew a call of its own tool, **When** it saves state for that call, **Then** it is stored and returned the same way; saved state is not specific to `render_interactive_ui`.

---

### Edge Cases

- A client declares the same tool name twice in one request: the request is refused.
- A client declares a category description for a category no tool uses: it is ignored.
- A client changes its declared tools between requests: each turn uses only what its own request declared; the docs warn that a changing set reduces prompt caching.
- A request with an empty or absent `client_tools` list: behaves exactly as today.
- A Discord or Telegram turn: no client tools are ever declared there.
- The answer to a call is larger than the size limit: it is refused and the call fails with a clear error.
- The page HTML for `render_interactive_ui` is larger than its limit: the WebUI answers the call with an error so the agent can shorten it.
- A saved call whose tool the current client does not recognise (made by another client, or a removed extension): shown as a collapsed "tool call" item with its name, not as an error.
- A client event that references a call id that does not exist in this conversation: delivered without the reference, not refused.
- State saved for a call id that does not exist in the conversation, is not a client tool call, or belongs to another person's topic: refused.
- Two tabs save state for the same page at nearly the same time: the last save the server receives wins, and both tabs are told the result.
- A topic is deleted: the saved state of its pages is deleted with it.
- Several client tool calls in one script: each is sent and answered independently; answers may arrive in any order.
- The connection drops and the person reconnects before the call times out: v1 does not let the new connection take over; the call fails with `client_disconnected`.

## Requirements *(mandatory)*

### Functional Requirements

**Declaring tools**

- **FR-001**: A request sent over the WebSocket MUST be able to carry a list of client tool definitions, each with a name, a description, an input schema, and optionally an output schema, a category and a time limit, plus an optional list of category descriptions.
- **FR-002**: vizier MUST check each declaration's shape before starting the turn: name of lowercase letters, digits and underscores starting with a letter; no duplicate names; at most a fixed number of tools; description, category description and schema within size limits; schemas that are valid JSON Schema. A failing request MUST be refused with an error naming the tool and the rule, and no turn starts.
- **FR-003**: vizier MUST NOT treat a declaration as more trusted than the person who sent it: declarations are accepted only on an authenticated connection and grant the agent nothing beyond calling back to that client.
- **FR-004**: A tool's category MUST be the one it declares, or `client` when it declares none. Built-in categories, addon ids and the agent's MCP server names MUST be reserved first; a client category that clashes MUST be renamed with a `_client` suffix.
- **FR-005**: A client tool's final name MUST be `<category>_<tool>`, assigned by the same naming rules as every other tool (built-in names reserved first, numbered suffix on collision), so a client can never replace or shadow another tool.
- **FR-006**: vizier MUST tell the client the final name of each declared tool, before any call to it can arrive.

**Offering tools to the agent**

- **FR-007**: Client tools MUST be offered only for the turn started by the request that declared them, and only while that request's connection is open. They MUST NOT be stored as part of the agent, the session or the topic, or reused in any later turn.
- **FR-008**: Turns not started by a client request over a live connection (dream cycle, checkpoint handover, scheduled task runs, background reports, background pieces, consult and delegate sessions) and every Discord or Telegram turn MUST have no client tools.
- **FR-009**: The agent MUST be able to discover and call client tools exactly as it does other tools in code mode: the client categories appear in its list of categories, and listing and describing them returns the client's descriptions and schemas.
- **FR-010**: Client categories in the agent's category list, and client tools in tool descriptions, MUST be labelled as provided by the person's client.
- **FR-011**: The agent's standing instructions MUST list categories only, not individual client tools, so that a client sending the same declaration on every request produces the same instructions on every turn.
- **FR-012**: A background report turn's framing MUST tell the agent that client tools are unavailable in that turn.

**Calling client tools**

- **FR-013**: When the agent calls a client tool, vizier MUST send the call (a call id, the tool's final name, the arguments) to the connection that sent the request, and to no other connection for answering. Other connections watching the same conversation MAY receive it for display.
- **FR-014**: The client MUST be able to answer a call with either a result or an error. The agent's script MUST receive a result as the tool's return value and an error as a catchable tool failure.
- **FR-015**: A call MUST fail with `client_disconnected` when the answering connection closes, with a timeout error when no answer arrives within the tool's limit (declared or default, capped at a maximum), and be cancelled (with the client told) when the turn is aborted. None of these may hang or fail the turn itself.
- **FR-016**: Answers for unknown, finished or foreign calls MUST be ignored and the client told the call is not pending. Answers above a size limit MUST be refused.
- **FR-017**: Each client tool call and its outcome (result, error, disconnect, timeout, cancellation) MUST be saved in the conversation history with its arguments, so any client can draw it again on reload.
- **FR-018**: Saved client tool calls MUST be returned with the rest of the conversation history to clients loading a topic, in conversation order, together with each call's latest saved state (FR-018a).

**Saved state**

- **FR-018a**: A client MUST be able to save a state value (any JSON value) for a client tool call in a conversation the person owns. Only the latest value is kept per call; there is no history of earlier values.
- **FR-018b**: Saving state MUST NOT start a turn, appear as a message, or be shown to the agent.
- **FR-018c**: After a save, vizier MUST tell every connection watching that conversation the call's new state, including the one that saved it (as confirmation).
- **FR-018d**: State values MUST be limited in size and saves rate-limited per call; a refused save MUST leave the last accepted state unchanged and tell the client why.
- **FR-018e**: A call's saved state MUST be deleted with its conversation, and with the agent.

**Client events**

- **FR-019**: A client MUST be able to send a client event into a conversation: a text, optional structured data, and optionally the id of the client tool call it came from.
- **FR-020**: A client event MUST start an agent turn like a message the person typed (same history, same queueing behind a running turn, same identity and permissions as the sending person), and its text MUST appear in the conversation as the person's message.
- **FR-021**: The agent MUST be able to tell a client event from typed text, and see its data and source call id.
- **FR-022**: vizier MUST limit client events per conversation and refuse ones beyond the limit with a clear error.
- **FR-023**: A client event's request MAY itself declare client tools, which then apply to the turn it starts.

**WebUI**

- **FR-024**: The WebUI MUST provide an extension mechanism to register a client tool with its definition, a handler for live calls, and a renderer for saved calls. Registered tools MUST be declared on every message the WebUI sends.
- **FR-025**: The WebUI MUST register `render_interactive_ui` through that same mechanism. It takes a title and a self-contained HTML page and answers each call immediately with a page id and status `rendered`.
- **FR-026**: Pages MUST be security-sandboxed from the WebUI: they can run their own scripts and styles but cannot read the person's credentials, cookies or the surrounding page, and cannot load anything from the network (inline resources and data images only). The sandbox is about access, not appearance: visually the page is part of the reply (no separate panel or frame chrome).
- **FR-026a**: A page MUST appear inside the agent's reply bubble for the turn that rendered it, in order with that turn's reply text, as soon as the agent calls the tool (while the turn is still running), not held back until the turn ends. A page stays visible even if the turn later fails or is aborted.
- **FR-027**: Pages MUST be given a small built-in interface to send a client event (`text`, optional `data`) tied to the call that rendered them, to report their content height so the frame fits it (up to a maximum), to read the state they start with, to save their state (FR-018a), and to be told when their state changes elsewhere (FR-018c).
- **FR-027a**: Saving state from a page MUST NOT require the person to have interacted with it (unlike sending a client event), since it never starts a turn; it is subject only to the limits in FR-018d.
- **FR-028**: The WebUI MUST forward a page's client events only after the person has interacted with that page, and at most one per second per page.
- **FR-029**: The WebUI MUST draw saved calls of tools it recognises with their renderer, and show any other saved client tool call as a collapsed item naming the tool.
- **FR-030**: The WebUI MUST answer a `render_interactive_ui` call with an error when the page exceeds its size limit.

**Protocol and scope**

- **FR-031**: The client tools protocol (declaring tools, the final-names reply, calls, answers, client events, the related errors and limits) MUST be documented as a versioned public contract in vizier's documentation, including the advice to keep a client's declared set stable.
- **FR-032**: Client tools are available over the WebSocket only. REST requests MUST NOT accept client tool declarations in this version.
- **FR-033**: Requests without client tool declarations MUST behave exactly as before this feature.

### Key Entities

- **Client tool definition**: declared by a client per request; name, description, input schema, optional output schema, optional category, optional time limit. Never stored outside the turn it was declared for.
- **Client category**: a name (client-chosen or `client`) and optional description; renamed on clashes with reserved names.
- **Client tool call**: one call by the agent to a client tool during a turn; call id, final tool name, arguments, the connection it was sent to, and its outcome (result, error, disconnected, timed out, cancelled). Saved in the conversation history.
- **Client event**: a message from a client into a conversation; text, optional data, optional source call id. Starts a turn as the sending person.
- **Interactive page**: the HTML a `render_interactive_ui` call carries, with its page id and title; drawn by the WebUI from the saved call.
- **Call state**: the latest state value a client saved for one client tool call; belongs to that call's conversation, overwritten on each save, deleted with the conversation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can declare a tool, have an agent call it, and answer it using only the published protocol documentation and a minimal WebSocket client, with no change to vizier.
- **SC-002**: A rendered page appears in the conversation within 1 second of the agent calling the tool (excluding model time), and reappears identically after reloading the topic.
- **SC-003**: Clicking a "say hello" button in a rendered page results in "hello" in the conversation and an agent reply, with the person taking no other step.
- **SC-004**: A page's script cannot read the person's credentials or reach the network in any of a set of attempted escapes (reading storage and cookies, accessing the parent page, network requests, sending without a click).
- **SC-005**: When the answering connection closes, the waiting call fails within 5 seconds and the turn completes; no turn is ever left waiting longer than the tool's time limit.
- **SC-006**: Conversations that declare no client tools, and every Discord, Telegram, scheduled, dream and background turn, behave identically to before the feature.
- **SC-007**: For a client sending the same declaration on every message, the agent's standing instructions are identical from turn to turn.
- **SC-008**: A page's saved state is restored on any browser the person logs in from, and reaches another open tab of the same topic within 1 second of being saved.

## Assumptions

- Limits (adjustable during planning): at most 32 client tools and 16 categories per request; tool description up to 2 KB, category description up to 512 bytes, each schema up to 16 KB; default call time limit 60 s, maximum 600 s; answers up to 1 MB; `render_interactive_ui` pages up to 256 KB; client events limited to 10 per minute per conversation; saved state up to 64 KB per call and at most 2 saves per second per call (the WebUI coalesces faster saves from a page into the latest one); page frames grow to at most about 80% of the viewport height.
- The person's existing WebSocket authentication is reused; no new permission is introduced. A person may only declare tools for, and send events into, conversations they could already message.
- Only the person's own WebUI topics are involved; there is no shared viewing of another person's topic.
- Client events count as the person's own messages for usage accounting, history and reactions.
- Taking over a pending call from a reconnected connection, REST/SSE support, and letting non-WebSocket channels (Discord components, Telegram keyboards) implement client tools are out of scope for this version and may follow later.
- The offline `dummyplug` provider, plus a minimal WebSocket test client that declares a tool, answers its call and sends a client event, is how this is verified end to end, as the constitution requires for agent-observable changes.
- "Interaction" for FR-028 means the person has clicked or focused inside the page frame; it is checked by the WebUI, not by the page.
