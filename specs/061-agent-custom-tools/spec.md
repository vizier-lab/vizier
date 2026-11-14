# Feature Specification: Agent custom tools

**Feature Branch**: `061-agent-custom-tools`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "let's do this idea: https://github.com/vizier-lab/vizier/issues/120" — *Add functions: agents compose tools into reusable custom tools.* Because every agent already works through a Python sandbox, an agent can compose existing tools into a saved, named tool with a declared purpose and inputs. Once saved it is callable like any other tool, runs the same way every time (fixed control flow, no model judgment inside), and is reused across conversations. It sits between skills (guidance the model may follow loosely) and built-in tools (fixed code).

## Context

Today an agent that repeats a multi-step workflow ("gather X, transform it, save it, tell me")
re-derives the steps in a new script every time, guided at best by a skill written in prose. The
result varies from run to run, costs a model round to write each time, and is hard for a person to
review. There is no way to keep a script that worked and call it again by name.

This feature lets an agent save such a script as a **custom tool**: a name, a description, an input
schema and the script. A custom tool is found and called through the same discovery and dispatch an
agent already uses for every other tool, and runs with exactly the rights of the turn that calls it —
it is a saved script, never a way around an agent's toolset or policy.

**Naming.** The issue calls these "functions", but inside the sandbox every tool is already reached
as a function. This spec uses **custom tool** throughout; "function" keeps its sandbox meaning.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent saves a workflow as a custom tool (Priority: P1)

An agent has just written a script that does something its person will ask for again (for example,
"collect today's unread notes, summarise their titles, save the list to memory"). It saves the
script as a custom tool with a name, a one-line description, an input schema and, optionally, a
sample input. The tool is checked before it is saved; a problem is reported with what to fix, and
nothing is saved.

**Why this priority**: Without saving there is nothing to reuse. This is the minimum viable slice.

**Independent Test**: With an offline test agent, run the create tool with a valid definition and
confirm it is listed with its version 1; run it again with a broken script, an invalid schema or a
name that is taken and confirm each is refused with a specific reason and nothing is stored.

**Acceptance Scenarios**:

1. **Given** an agent with no custom tools, **When** it creates one with a valid name, description,
   input schema and script, **Then** the tool is stored as version 1, with the agent as author and
   the time it was made.
2. **Given** a definition whose script does not parse, whose input schema is not a valid object
   schema, or whose sample input does not match the schema, **When** the agent tries to save it,
   **Then** the save is refused with every problem found, each naming what is wrong.
3. **Given** a definition whose script calls a tool the agent does not have, **When** the agent
   tries to save it, **Then** the save is refused naming the unknown tool, with the nearest known
   names.
4. **Given** a definition with a sample input, **When** the agent saves it, **Then** the tool is run
   once with that input, the save happens only if that run succeeds, and the result says the run
   was real (any tool calls it made took effect).
5. **Given** a script containing a value from the agent's secret store, **When** the agent tries to
   save it, **Then** the save is refused and the value never appears in storage, the result or logs.

---

### User Story 2 - An agent discovers and calls its custom tools (Priority: P1)

In any later conversation the agent finds its custom tools through the same listing and description
tools it uses for every other tool, and calls one from a script by name with an input. The input is
checked against the schema, the tool runs, and the caller gets the returned value — or a structured
failure it can read and act on.

**Why this priority**: Reuse is the point of the feature; saving without calling delivers nothing.

**Independent Test**: Save a custom tool in one conversation; in a new conversation list the custom
tool category, describe the tool, call it with valid input and with invalid input, and check the
value and the failure shape.

**Acceptance Scenarios**:

1. **Given** an agent with saved, enabled custom tools, **When** it lists its tool categories,
   **Then** a custom tools category is present, and listing it shows each tool's name and
   description.
2. **Given** a custom tool, **When** the agent asks for its description, **Then** it gets the
   description, the input schema, the output description and whether the tool depends on any
   model-backed tool.
3. **Given** a valid input, **When** a script calls the custom tool, **Then** the tool's script runs,
   its own tool calls go through the normal dispatch path, and the caller receives the returned
   value.
4. **Given** an input that does not match the schema, **When** a script calls the custom tool,
   **Then** the call fails before anything runs, with every mismatch named by its path.
5. **Given** a custom tool whose script fails partway, **When** it is called, **Then** the caller
   receives a failure that names the custom tool, the step that failed and the error, and the
   calls already made are listed.
6. **Given** an agent with no custom tools, **When** it lists categories, **Then** no custom tools
   category appears and the prompt is unchanged from today.

---

### User Story 3 - A custom tool never widens what the caller may do (Priority: P1)

A custom tool runs with the rights of the turn that calls it, not of the turn that wrote it. Each
tool call inside it is checked exactly as if the caller had made it directly: a tool not offered in
this turn is refused, secrets are redacted the same way, and timeouts still apply.

**Why this priority**: A custom tool is generated code with tool access; it must not become a way
around an agent's toolset, a conversation's availability rules or secret handling. This is a
safety requirement for the whole feature.

**Independent Test**: Save a custom tool that calls an interactive-only tool; call it from an
unattended turn (a scheduled task) and confirm the inner call is refused exactly as a direct call
would be; call one that reads a secret and confirm the value is redacted from the report.

**Acceptance Scenarios**:

1. **Given** a custom tool that calls a tool unavailable in the current turn, **When** it is called
   in that turn, **Then** that inner call is refused with the same answer a direct call gets.
2. **Given** a custom tool that reads a secret, **When** it is called, **Then** the secret value is
   scrubbed from its result, its recorded calls, history and telemetry as for any script.
3. **Given** a custom tool whose script tries to call a turn-ending tool or to ask a person for
   input, **When** it is saved, **Then** the save is refused, because a custom tool returns a value
   to its caller and cannot end the turn or wait on a person.
4. **Given** a custom tool, **When** it is called, **Then** its run counts against the calling
   script's time limit; it cannot outlive the call that started it.

---

### User Story 4 - An agent updates a tool and can roll it back (Priority: P2)

The agent improves a custom tool. Each update makes a new version; the agent can list versions, see
what changed between two of them, and roll back to an earlier one. Callers always get the current
version.

**Why this priority**: Tools will need fixes as the agent learns; without history a bad update loses
a working tool.

**Independent Test**: Create a tool, update it twice, view the diff between versions 1 and 3, roll
back to version 1, and confirm calls now run version 1's script, and the rollback itself is
recorded as version 4.

**Acceptance Scenarios**:

1. **Given** a custom tool at version N, **When** the agent updates any of its parts, **Then**
   version N+1 is stored with who made it, when, and why (if given), and version N is kept.
2. **Given** two versions, **When** the agent asks what changed, **Then** it receives a readable
   difference of the description, schema and script.
3. **Given** an earlier version, **When** the agent rolls back to it, **Then** a new version is
   created with that version's content, and the update is validated like any other.
4. **Given** an update that fails validation, **When** it is attempted, **Then** the current version
   is untouched and keeps working.

---

### User Story 5 - A person sees, edits, disables and deletes an agent's custom tools (Priority: P2)

A person who manages the agent opens its **Custom tools** page in the WebUI and sees every custom
tool: name, description, version, author, last change, when it was last called, and whether it is
enabled. They can read its script and its history, edit it, disable or enable it, roll it back and
delete it.

**Why this priority**: People must be able to see and stop what an agent has built for itself; the
issue sets a read-only gallery with enable/disable and delete as the minimum.

**Independent Test**: With a person who may edit the agent, open the page, disable a tool and
confirm the agent can no longer list or call it; re-enable it; delete it and confirm it is gone.
With a person who may only view the agent, confirm the page is read-only.

**Acceptance Scenarios**:

1. **Given** a person who may view the agent, **When** they open Custom tools, **Then** they see
   every tool, its current script and its version history, and no controls that change anything.
2. **Given** a person who may edit the agent, **When** they disable a tool, **Then** it disappears
   from the agent's listings and a call to it is refused as disabled, starting with the agent's next
   tool lookup — no restart, no running turn aborted.
3. **Given** a person who may edit the agent, **When** they delete a tool, **Then** it and its
   history are removed after they confirm, and a later call is refused as unknown.
4. **Given** a disabled tool, **When** the agent tries to update or call it, **Then** it is told the
   tool was disabled by a person and cannot re-enable it itself.
5. **Given** a person who may edit the agent, **When** they edit a tool's description, schema or
   script and save, **Then** it is checked like an agent's save, refused with every problem found
   or stored as a new version authored by that person, and the agent's next call runs it.
6. **Given** a person who may only view the agent, **When** they open a tool, **Then** no editor is
   offered.

---

### User Story 6 - Custom tools call other custom tools (Priority: P3)

A custom tool may call another of the agent's custom tools, so small tools combine into larger ones,
within a fixed depth and with cycles refused.

**Why this priority**: Useful for building up a toolbox, but single-level tools deliver most of the
value.

**Independent Test**: Save tool A that calls tool B, call A, and see B's result used; save B so that
it calls A and confirm the save is refused as a cycle; build a chain deeper than the limit and
confirm the call beyond it is refused.

**Acceptance Scenarios**:

1. **Given** tool A calling tool B, **When** A is called, **Then** B runs inside A's call and its
   value is returned to A.
2. **Given** an update that would make a tool reach itself through other custom tools, **When** it
   is saved, **Then** it is refused naming the cycle.
3. **Given** a chain of custom tools calling each other, **When** a call would go deeper than the
   limit, **Then** that call is refused with the limit named, and the outer tools receive it as a
   failure.

---

### Edge Cases

- **A dependency disappears**: a tool a custom tool calls is removed (an MCP server detached, an
  integration deleted, a grant switched off). The custom tool stays saved; its description marks it
  *needs attention*, naming the missing tool, and a call fails at that step with the same answer a
  direct call would get. It is never silently deleted.
- **Name collisions**: custom tools are always reached under their own namespace, so a later
  built-in, addon or MCP tool can never take a custom tool's name and a custom tool can never shadow
  one. Two custom tools of one agent cannot share a name; renaming to a taken name is refused.
- **Rename**: renaming a custom tool that another custom tool calls is refused, naming the callers,
  until they are updated.
- **Deleting a tool other tools call**: the deletion is refused naming the callers, unless a person
  deletes it from the WebUI, in which case the callers become *needs attention*.
- **Model-backed calls**: a custom tool may call tools whose result comes from a model (a subagent,
  another agent, image generation). Its control flow is still fixed; its description says it
  depends on a model-backed tool, so the caller knows its result can vary.
- **Detached calls**: a custom tool called from a detached script runs inside that script under the
  script's own limits.
- **Limits**: an agent may hold at most 100 custom tools; a script is at most 64 KiB; a description
  at most 2 KB; a version history keeps every version (no automatic pruning).
- **Agent deleted**: all its custom tools and their history are deleted with it.
- **A person edits while the agent updates**: same rule as concurrent updates; the person's editor
  shows the tool changed underneath them and offers to reload.
- **Concurrent updates**: two updates of the same tool at once — one wins as the next version, the
  other is refused as out of date and can be retried.
- **Mid-turn change**: a tool updated, disabled or deleted while a turn is running affects the next
  lookup or call in that turn; a call already running finishes on the version it started with.

## Requirements *(mandatory)*

### Functional Requirements

**Definition and storage**

- **FR-001**: The system MUST let an agent store custom tools, each with a name, a description, an
  input schema, an optional output description, a script, and metadata: author, created and updated
  times, version number, enabled state and last-called time.
- **FR-002**: Custom tools MUST belong to exactly one agent and be visible and callable only by that
  agent. Copying a tool to another agent, export and import are out of scope for this version.
- **FR-003**: Custom tool scripts MUST be Python run in the existing sandbox; no declarative flow
  format in this version.

**Authoring**

- **FR-004**: The agent MUST have tools to create, update, list, describe, view the history of, diff
  versions of, roll back and delete its custom tools, in a category of their own.
- **FR-005**: Before a custom tool is saved (created, updated or rolled back), the system MUST check:
  the name follows the naming rule and is free; the input schema is a valid object schema; the
  script parses and declares the entry it is called through; every tool the script calls exists for
  the agent; it calls no turn-ending or person-waiting tool; it creates no cycle with other custom
  tools; the sample input, if given, matches the schema; size limits hold. Every problem MUST be
  reported at once, as data the agent can act on.
- **FR-006**: When a sample input is supplied, the system MUST run the tool once with it before
  saving, save only on success, and say in the result that the run took effect.
- **FR-007**: The system MUST refuse to save a script that contains any value currently in the
  agent's secret store, without storing, echoing or logging that value.
- **FR-008**: A custom tool or version that passes validation MUST be callable at once, with no
  approval step; oversight is after the fact (FR-021–FR-025), which is sound because a custom tool
  runs with the caller's own rights (FR-013) and can do nothing a script in the same turn could not.
- **FR-009**: Custom tools MUST NOT be created, updated or deleted from a dream cycle; the dream
  cycle may not call them either in this version.

**Discovery and calling**

- **FR-010**: Enabled custom tools MUST appear in the agent's tool discovery (category listing,
  per-category listing, description) like any other tool, under a reserved custom tools category
  that exists only when the agent has at least one enabled custom tool.
- **FR-011**: Custom tools MUST be named under a fixed namespace that no built-in, addon, MCP or
  client tool can take, and the custom tools category name MUST be reserved.
- **FR-012**: A call MUST validate its input against the schema before running and fail with every
  mismatch, by path, if it does not match.
- **FR-013**: A custom tool MUST run inside the calling script's call, with the calling turn's tool
  availability, permissions, secret redaction and per-tool timeouts applied to each of its inner
  calls exactly as for a direct call; its run MUST count against the calling script's time limit.
- **FR-014**: A custom tool MUST return a value to its caller; a failure MUST be returned as data
  naming the custom tool, the failing step, the error and the calls already made — never as a crash
  of the calling script.
- **FR-015**: A custom tool MAY call other custom tools of the same agent, up to a nesting depth of
  3; deeper calls MUST be refused, and cycles MUST be refused at save time and at call time.
- **FR-016**: A custom tool's description MUST state whether it calls model-backed tools, so its
  result is known to be variable.
- **FR-017**: Custom tools MUST be callable from any turn in which the agent can run scripts —
  conversations, scheduled tasks, detached scripts and subagent workers — subject to FR-013.

**Versioning**

- **FR-018**: Every successful create, update or rollback MUST record a new version holding the full
  definition, who made it (the agent or a person), when, and an optional reason; earlier versions
  MUST be kept.
- **FR-019**: The agent and people who may edit the agent MUST be able to see the difference between
  any two versions and roll back to any earlier version; a rollback MUST be validated as an update.
- **FR-020**: Concurrent updates of one tool MUST NOT lose a version: the second is refused as out of
  date.

**People and the WebUI**

- **FR-021**: The WebUI MUST offer a Custom tools page per agent listing each tool with its name,
  description, version, author, last change, last call, enabled state and *needs attention* state,
  with its current script and version history readable.
- **FR-022**: People who may view the agent MUST be able to read the page; people who may edit the
  agent MUST be able to enable, disable, roll back and delete (with confirmation), and edit an
  existing tool's description, input schema, output description and script.
- **FR-022a**: A person's edit MUST pass the same checks as an agent's save (FR-005, FR-007) except
  the sample run, which is not performed from the WebUI; it MUST be saved as a new version authored
  by that person, and the agent MUST see that authorship in the tool's history and description.
- **FR-023**: A tool disabled by a person MUST be hidden from the agent's listings and refused on
  call, and the agent MUST NOT be able to re-enable it.
- **FR-024**: Changes to custom tools MUST take effect on the agent's next tool lookup or call
  without restarting the agent or aborting a running turn.
- **FR-025**: Disabling, deleting or rolling back by a person MUST be recorded (who, what, when) and
  visible in the tool's history.

**Lifecycle and observability**

- **FR-026**: When a tool a custom tool calls stops existing for the agent, the custom tool MUST be
  marked *needs attention* naming the missing tool, without being deleted or disabled.
- **FR-027**: Deleting an agent MUST delete its custom tools and their history.
- **FR-028**: A custom tool call MUST appear in traces as one span containing its inner tool calls,
  and in the calling script's report as one call with its own nested calls, without repeating the
  inner calls at the outer level.
- **FR-029**: The documentation MUST explain when to use a skill (guidance the model interprets) and
  when to use a custom tool (a workflow that runs the same way every time), and the agent's tool
  description for creating custom tools MUST say the same in one sentence.

### Key Entities

- **Custom tool**: an agent-owned, named, callable workflow — name, description, input schema,
  output description, current version, enabled state, *needs attention* state with the missing
  tools, last-called time, the tools it calls (derived from its script).
- **Custom tool version**: one immutable snapshot of a custom tool's definition — version number,
  description, schemas, script, author (agent or person), time, reason, and whether it came from a
  rollback (and of which version).
- **Custom tool event**: a person's or the agent's lifecycle action that is not a version — enable,
  disable, delete — with who and when.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can save a workflow it just ran as a custom tool and call it from a different
  conversation in two tool calls (one to save, one to call), with no model round spent rewriting the
  workflow.
- **SC-002**: Calling the same custom tool with the same input runs the same sequence of tool calls
  every time, in 100% of runs whose inner tools are not model-backed.
- **SC-003**: No custom tool call ever reaches a tool that a direct call in the same turn could not
  reach: 0 bypasses across the availability, permission and secret-redaction checks pinned by tests.
- **SC-004**: 100% of invalid definitions (bad schema, bad script, unknown tool, cycle, secret in
  source, turn-ending call) are refused with a reason naming the problem, and none are stored.
- **SC-005**: A person can find, read and disable any of an agent's custom tools in under 30 seconds
  from the agent's page, and the agent stops seeing it on its next lookup.
- **SC-006**: After any update, the previous working version can be restored in one action by the
  agent or a person.
- **SC-007**: An agent without custom tools sees a prompt byte-identical to today's.

## Assumptions

- A custom tool is per agent, like memory and skills, and lives in its own store, not in the memory
  bundle; it is not an artifact.
- The sandbox, the tool dispatch path, secret redaction, tool categories and the discovery tools
  already exist and are reused as they are; a custom tool is a new kind of entry in the same
  catalogue.
- The custom tools category is appended at the end of the existing category list.
- "May view / may edit the agent" uses the existing agent permissions; no new permission is
  introduced unless planning shows one is needed.
- The new WebUI page is drawn in FigCraft (or a mockup) before it is built, as the project requires.
- Workers, detached scripts and scheduled tasks get custom tools because they are the agent; dreams
  do not in this version.
- Decided with the requester (2026-11-15): tools are callable at once with no approval step; they
  are per agent with no sharing, copying or export; people get an editor in the WebUI. Creating a
  brand-new tool from the WebUI and running a tool from the WebUI are out of scope — the agent
  creates tools, people inspect, edit and govern them.
- The default limits (100 tools per agent, 64 KiB script, 2 KB description, nesting depth 3) are
  provisional and may be tuned in planning.
