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
script as a custom tool with a name, a one-line description, an input schema, a category and,
optionally, a sample input. The tool is checked before it is saved; a problem is reported with what to fix, and
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
5. **Given** a save naming a category the agent does not have yet, **When** it carries a category
   description, **Then** the category is created with that description in the same save; **when**
   it carries none, **Then** the save is refused asking for one.
6. **Given** a save with no category, **When** it is stored, **Then** the tool goes into the default
   `custom` category.

---

### User Story 2 - An agent discovers and calls its custom tools (Priority: P1)

In any later conversation the agent finds its custom tools through the same listing and description
tools it uses for every other tool, and calls one from a script by name with an input. The input is
checked against the schema, the tool runs, and the caller gets the returned value — or a structured
failure it can read and act on.

**Why this priority**: Reuse is the point of the feature; saving without calling delivers nothing.

There is no separate search or listing tool for custom tools: the agent learns one way to find a
tool, whoever made it.

**Independent Test**: Save a custom tool in category `reports` in one conversation; in a new
conversation confirm `reports` and its description are in the prompt's category list, list it with
the ordinary listing tool, describe the tool, call it with valid input and with invalid input, and
check the value and the failure shape.

**Acceptance Scenarios**:

1. **Given** an agent with saved, enabled custom tools, **When** a turn starts, **Then** each of its
   custom tool categories appears in the prompt's category list with the description the agent
   wrote, beside the built-in, addon and MCP categories.
2. **Given** a custom tool category, **When** the agent lists it with the existing listing tool,
   **Then** it gets each tool's name and description; listing every function (no category) includes
   custom tools, each marked as the agent's own.
3. **Given** a custom tool, **When** the agent describes it with the existing description tool,
   **Then** it gets the description, input schema and output description as for any tool, plus what
   only custom tools have: the current version, who made it (agent, dream or person), whether it
   calls a model-backed tool, and whether it needs attention.
4. **Given** a valid input, **When** a script calls the custom tool, **Then** the tool's script runs,
   its own tool calls go through the normal dispatch path, and the caller receives the returned
   value.
5. **Given** an input that does not match the schema, **When** a script calls the custom tool,
   **Then** the call fails before anything runs, with every mismatch named by its path.
6. **Given** a custom tool whose script fails partway, **When** it is called, **Then** the caller
   receives a failure that names the custom tool, the step that failed and the error, and the
   calls already made are listed.
7. **Given** an agent with no custom tools, **When** a turn starts, **Then** no custom tool category
   appears and the prompt is unchanged from today.
8. **Given** a disabled custom tool, **When** the agent lists or describes tools, **Then** it does not
   appear; it is seen only through the authoring tools.

---

### User Story 3 - A custom tool runs as part of the calling turn (Priority: P1)

A custom tool runs inside the turn that calls it. Its own tool calls go through the same path as
the caller's direct calls, so they see the same tools that turn has, and the call ends within the
calling script's time limit.

**Why this priority**: This is how a custom tool executes at all; reusing the existing call path is
what keeps the feature small.

**Independent Test**: Save a custom tool that calls an interactive-only tool; call it from an
unattended turn (a scheduled task) and confirm the inner call gets the same answer a direct call
would; call a slow custom tool and confirm it stops at the calling script's time limit.

**Acceptance Scenarios**:

1. **Given** a custom tool that calls a tool unavailable in the current turn, **When** it is called
   in that turn, **Then** that inner call gets the same answer a direct call gets.
2. **Given** a custom tool, **When** it is called, **Then** its run counts against the calling
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
   offered, and no category can be renamed or redescribed.
7. **Given** the Custom tools page, **When** it opens, **Then** tools are grouped by category, each
   group headed by its name and description, and each row shows version, author, last change,
   enabled state and any *needs attention* warning.
8. **Given** a person who may edit the agent, **When** they edit a category from its heading,
   **Then** they can rename it or change its description; **when** they pick another category (or
   type a new one, which asks for a description) in a tool's editor, **Then** the tool moves there
   as a new version authored by them.

---

### User Story 6 - Custom tools call other custom tools (Priority: P3)

A custom tool may call another of the agent's custom tools, so small tools combine into larger ones,
with no depth limit and with cycles refused.

**Why this priority**: Useful for building up a toolbox, but single-level tools deliver most of the
value.

**Independent Test**: Save tool A that calls tool B, call A, and see B's result used; save B so that
it calls A and confirm the save is refused as a cycle; build a long chain and confirm the calling
script's time limit, not a depth rule, is what stops it.

**Acceptance Scenarios**:

1. **Given** tool A calling tool B, **When** A is called, **Then** B runs inside A's call and its
   value is returned to A.
2. **Given** an update that would make a tool reach itself through other custom tools, **When** it
   is saved, **Then** it is refused naming the cycle.
3. **Given** a long chain of custom tools calling each other, **When** it runs past the calling
   script's time limit, **Then** the call ends with a timeout failure naming the custom tool that
   was running, and the outer tools receive it as a failure.

---

### User Story 7 - A dream turns a repeated workflow into a custom tool or a skill (Priority: P2)

During its dream cycle the agent notices that it ran the same sequence of steps in several
conversations (or that its person asked for the same thing again). It decides whether that workflow
should become a custom tool (fixed steps, varying inputs), a skill (steps that need judgment), or
nothing, checks what it already has, and creates or updates the right one. The dream's report says
what it made.

**Why this priority**: This is how the toolbox grows without anyone asking: the dream cycle already
turns conversations into memories and skills, and custom tools are the third durable output.

**Independent Test**: With an offline test agent, seed two conversations that run the same tool
sequence with different inputs, run a dream cycle, and confirm the extraction lists the workflow,
consolidation creates one custom tool authored "during a dream cycle", and the report counts it.
Seed a workflow that needs judgment and confirm it becomes a skill instead. Run the cycle again and
confirm it updates rather than duplicates.

**Acceptance Scenarios**:

1. **Given** conversations that repeat one fixed sequence of tool calls with different inputs,
   **When** the dream cycle runs, **Then** its extraction report lists the workflow with its steps
   and varying inputs, and consolidation saves it as a custom tool.
2. **Given** a repeated workflow whose steps depend on judgment, **When** the dream cycle runs,
   **Then** it creates or updates a skill, not a custom tool.
3. **Given** a custom tool or skill that already covers the workflow, **When** the dream cycle runs,
   **Then** it updates the existing one (a new version for a custom tool) or leaves it, and creates
   no duplicate.
4. **Given** a custom tool the dream saved, **When** a person opens Custom tools, **Then** the
   version shows it was made by the agent during a dream cycle, which cycle, and why.
5. **Given** a dream cycle, **When** it tries to delete, disable or call a custom tool, **Then** it
   cannot: those tools are not in its toolset.
6. **Given** a dream save that fails validation, **When** it is attempted, **Then** nothing is
   saved and the dream's report names the workflow and the reason.
7. **Given** categories holding one or two closely related tools each, **When** the dream cycle
   consolidates, **Then** it may move tools between categories and rewrite category descriptions,
   and its report says what it reorganised.

---

### User Story 8 - The agent organises its custom tools into its own categories (Priority: P2)

The agent groups the tools it makes into categories it names and describes, so the prompt's
category list tells it, in a line each, what its own tools are for. A category comes into being
with the first tool saved in it, can be renamed or redescribed, and goes away with its last tool.

**Why this priority**: With one generic category the agent must list every custom tool to find the
one it wants; named and described categories are what make a growing toolbox findable, the same
way an MCP server's or addon's category does.

**Independent Test**: Save two tools into `reports` (giving a description the first time) and one
into `billing`; confirm the next turn's category list shows both lines; rename `reports` and
confirm both tools moved and their names did not change; delete `billing`'s only tool and confirm
the category is gone.

**Acceptance Scenarios**:

1. **Given** a new category name with a description, **When** the first tool is saved into it,
   **Then** the category exists with that description; there is no separate step to create one.
2. **Given** an existing category, **When** the agent renames it or changes its description,
   **Then** every tool in it moves with it, no tool is renamed, and callers keep working.
3. **Given** a tool, **When** it is updated with another category, **Then** it moves there, and the
   move is a version in its history.
4. **Given** a category name that is taken by a built-in, addon or MCP category, **When** it is
   used, **Then** it is stored under that name with a `_custom` suffix and the save result names the
   resulting category.
5. **Given** a category whose last tool is deleted, **When** that happens, **Then** the category is
   removed; **given** a category whose tools are all disabled, **Then** it leaves the agent's prompt
   but stays on the WebUI page.
6. **Given** a category created, renamed or removed during a turn, **When** that turn continues,
   **Then** its category list is unchanged; the change appears from the next turn. Adding a tool to
   an existing category never changes the prompt.

---

### Edge Cases

- **A dependency disappears**: a tool a custom tool calls is removed (an MCP server detached, an
  integration deleted, a grant switched off). The custom tool stays saved; its description marks it
  *needs attention*, naming the missing tool, and a call fails at that step with the same answer a
  direct call would get. It is never silently deleted.
- **Name collisions**: custom tools are always reached under their own namespace, so a later
  built-in, addon or MCP tool can never take a custom tool's name and a custom tool can never shadow
  one. Two custom tools of one agent cannot share a name; renaming to a taken name is refused.
- **Many small categories**: every category is a line in every prompt, and there is no cap in this
  version; the dream prompt nudges toward merging categories that hold one or two tools.
- **Renaming a category onto an existing one**: refused naming the existing category; moving its
  tools there one by one is the way to merge.
- **Rename**: renaming a custom tool that another custom tool calls is refused, naming the callers,
  until they are updated.
- **Deleting a tool other tools call**: the deletion is refused naming the callers, unless a person
  deletes it from the WebUI, in which case the callers become *needs attention*.
- **Model-backed calls**: a custom tool may call tools whose result comes from a model (a subagent,
  another agent, image generation). Its control flow is still fixed; its description says it
  depends on a model-backed tool, so the caller knows its result can vary.
- **Detached calls**: a custom tool called from a detached script runs inside that script under the
  script's own limits.
- **Limits**: none in this version on count, script or description size, nesting depth or history
  length; every version is kept. Only cycles are refused, and the calling script's time limit ends
  a call that runs too long.
- **A dream and a person change the same tool**: the same out-of-date rule as concurrent updates;
  the dream's report names the tool it could not update.
- **A dream writes a tool that calls a tool the dream cannot use**: allowed, because the check is
  against the agent's full toolset; the tool simply is not called in the dream.
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

- **FR-001**: The system MUST let an agent store custom tools, each with a name, a description, a
  category, an input schema, an optional output description, a script, and metadata: author, created and updated
  times, version number, enabled state and last-called time.
- **FR-002**: Custom tools MUST belong to exactly one agent and be visible and callable only by that
  agent. Copying a tool to another agent, export and import are out of scope for this version.
- **FR-003**: Custom tool scripts MUST be Python run in the existing sandbox; no declarative flow
  format in this version.

**Authoring**

- **FR-004**: The agent MUST have tools to create, update, list, describe, view the history of, diff
  versions of, roll back and delete its custom tools, and one tool to rename a custom tool category
  or change its description, all in an authoring category of their own. The authoring list shows
  every custom tool, disabled and *needs attention* ones included; it is for managing tools, not for
  finding one to call (FR-010).
- **FR-005**: Before a custom tool is saved (created, updated or rolled back), the system MUST check:
  the name follows the naming rule and is free; the input schema is a valid object schema; the
  script parses and declares the entry it is called through; every tool the script calls exists for
  the agent; it creates no cycle with other custom
  tools; the sample input, if given, matches the schema. Every problem MUST be
  reported at once, as data the agent can act on.
- **FR-006**: When a sample input is supplied, the system MUST run the tool once with it before
  saving, save only on success, and say in the result that the run took effect.
- **FR-007**: *(Removed: refusing scripts that hold secret values or call turn-ending / person-waiting
  tools is deferred to a later security pass; see Assumptions.)*
- **FR-008**: A custom tool or version that passes validation MUST be callable at once, with no
  approval step; oversight is after the fact (FR-021–FR-025), which is sound because a custom tool
  runs with the caller's own rights (FR-013) and can do nothing a script in the same turn could not.
- **FR-009**: The dream cycle MUST be able to list, describe, view the history of, diff, create and
  update custom tools, and rename or redescribe their categories, so a custom tool, like a skill, is one of a dream's outputs. A dream MUST NOT
  delete, disable or enable a custom tool, and MUST NOT call one (a custom tool acts through the
  agent's full toolset, which an unattended dream does not have).
- **FR-009a**: A save made in a dream MUST pass every check of FR-005, judged against the
  agent's full toolset (where the tool will run), not the dream's; the sample run of FR-006 is not
  performed in a dream. The version MUST be recorded as authored by the agent during a dream cycle,
  naming the cycle, and the cycle's report MUST list the custom tools it created or updated.

**The dream prompt**

- **FR-009b**: The dream extraction step MUST look for **reusable workflows** in the conversations it
  reflects on: a sequence of tool calls the agent ran more than once, or one the person asked to
  repeat, noting the steps, the inputs that varied and whether any step needed judgment.
- **FR-009c**: The dream consolidation step MUST decide, for each reusable workflow, between three
  outputs, and the prompt MUST state the rule: **a custom tool** when the steps are fixed and only
  the inputs vary; **a skill** when the steps need the model's judgment; **nothing** when it is a
  one-off. It MUST check the existing skills and custom tools first and prefer updating one over
  creating a near-duplicate, place a new custom tool in a fitting existing category before making a
  new one, and suggest merging categories that hold only one or two related tools.
- **FR-009d**: The consolidation audit MUST count the skills and custom tools created or updated,
  beside the counts it already reports, and name any reusable workflow intentionally left alone.

**Discovery and calling**

- **FR-010**: Enabled custom tools MUST be found through the existing discovery tools (the per-
  category listing, the full listing and the description tool, and their in-script equivalents),
  with no separate search or listing tool for them. The full listing MUST mark custom tools as the
  agent's own; the description MUST add version, author, model-backed dependency and *needs
  attention* to the usual fields.
- **FR-010a**: Custom tools MUST be grouped into categories that belong to the agent. Each category
  has a name and a one-line description written by whoever created it, and every category with at
  least one enabled tool MUST appear in the prompt's category list beside the other categories.
- **FR-010b**: A category MUST be created by the first save that names it, and that save MUST carry
  the category's description (refused otherwise); there is no separate create-category step. A save
  with no category MUST go into a default `custom` category, which needs no description from the
  agent.
- **FR-010c**: A category MUST be removed when its last tool is deleted. A category whose tools are
  all disabled MUST leave the prompt but remain visible on the WebUI page.
- **FR-010d**: Custom tools MUST NOT be placed in built-in, addon or MCP categories. A category name
  that clashes with one MUST be stored with a `_custom` suffix, and the save result MUST give the
  resulting name. Renaming a category onto another custom tool category MUST be refused.
- **FR-010e**: Renaming a category or changing its description MUST move every tool with it and
  rename none; moving a tool to another category MUST be an update that records a version.
- **FR-010f**: The prompt's category list MUST be fixed for the whole turn: categories created,
  renamed, redescribed or removed during a turn appear from the next turn, and adding, changing or
  removing a tool in an existing category MUST NOT change the prompt at all.
- **FR-011**: Custom tools MUST be named under a fixed namespace that no built-in, addon, MCP or
  client tool can take, independent of their category, so a category change never renames a tool.
- **FR-012**: A call MUST validate its input against the schema before running and fail with every
  mismatch, by path, if it does not match.
- **FR-013**: A custom tool MUST run inside the calling script's call, making its inner tool calls
  through the same dispatch path as the caller's direct calls; its run MUST count against the
  calling script's time limit.
- **FR-014**: A custom tool MUST return a value to its caller; a failure MUST be returned as data
  naming the custom tool, the failing step, the error and the calls already made — never as a crash
  of the calling script.
- **FR-015**: A custom tool MAY call other custom tools of the same agent with no nesting limit;
  cycles MUST be refused at save time and at call time. The calling script's time limit (FR-013)
  is what bounds a long chain.
- **FR-015a**: This version sets no limit on the number of custom tools per agent, the size of a
  script or description, or the number of versions kept.
- **FR-016**: A custom tool's description MUST state whether it calls model-backed tools, so its
  result is known to be variable.
- **FR-017**: Custom tools MUST be callable from any turn in which the agent can run scripts —
  conversations, scheduled tasks, detached scripts and subagent workers — subject to FR-013.

**Versioning**

- **FR-018**: Every successful create, update or rollback MUST record a new version holding the full
  definition, who made it (the agent in a turn, the agent during a dream cycle, or a person), when,
  and an optional reason; earlier versions
  MUST be kept.
- **FR-019**: The agent and people who may edit the agent MUST be able to see the difference between
  any two versions and roll back to any earlier version; a rollback MUST be validated as an update.
- **FR-020**: Concurrent updates of one tool MUST NOT lose a version: the second is refused as out of
  date.

**People and the WebUI**

- **FR-021**: The WebUI MUST offer a Custom tools page per agent with tools grouped by category, each
  group headed by its name and description, listing each tool with its name, description, version,
  author, last change, last call, enabled state and *needs attention* state, with its current
  script, schema, version history and diffs readable.
- **FR-022**: People who may view the agent MUST be able to read the page; people who may edit the
  agent MUST be able to enable, disable, roll back and delete (with confirmation), edit an existing
  tool's description, category, input schema, output description and script, and rename or
  redescribe a category from its heading. Choosing a new category in a tool's editor MUST ask for
  its description.
- **FR-022a**: A person's edit MUST pass the same checks as an agent's save (FR-005) except
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
- **Custom tool category**: an agent-owned group of custom tools — name, one-line description, who
  last changed it; exists while it holds at least one tool; shown in the prompt while it holds an
  enabled one.
- **Custom tool version**: one immutable snapshot of a custom tool's definition — version number,
  description, category, schemas, script, author (agent in a turn, agent in a dream cycle with the cycle
  named, or person), time, reason, and whether it came from a
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
- **SC-003**: A custom tool's inner calls get the same answers as the same calls made directly in
  the calling turn.
- **SC-004**: 100% of invalid definitions (bad schema, bad script, unknown tool, cycle) are refused
  with a reason naming the problem, and none are stored.
- **SC-005**: A person can find, read and disable any of an agent's custom tools in under 30 seconds
  from the agent's page, and the agent stops seeing it on its next lookup.
- **SC-006**: After any update, the previous working version can be restored in one action by the
  agent or a person.
- **SC-007**: An agent without custom tools sees a prompt byte-identical to today's.
- **SC-008**: Given conversations that repeat a fixed workflow at least twice, one dream cycle turns
  it into exactly one custom tool (or one new version of an existing one), and a second cycle over
  the same conversations creates no duplicate.
- **SC-009**: An agent finds the custom tool it needs by opening one category (one listing call) in
  most cases, because the prompt's category lines describe what each group of its tools does.
- **SC-010**: Within a turn the prompt never changes because of a custom tool change, and across
  turns it changes only when a category is added, renamed, redescribed or removed.

## Assumptions

- A custom tool is per agent, like memory and skills, and lives in its own store, not in the memory
  bundle; it is not an artifact.
- The sandbox, the tool dispatch path, secret redaction, tool categories and the discovery tools
  already exist and are reused as they are; a custom tool is a new kind of entry in the same
  catalogue.
- The authoring tools get one built-in category, appended at the end of the existing list; the
  agent's own custom tool categories follow the built-in, addon and MCP categories in the prompt.
- Decided with the requester (2026-11-15, fourth pass): one discovery path for every tool; the agent
  names and describes its own categories, created implicitly by the first save; the WebUI groups by
  category and lets editors rename, redescribe and move.
- "May view / may edit the agent" uses the existing agent permissions; no new permission is
  introduced unless planning shows one is needed.
- The new WebUI page is drawn in FigCraft (or a mockup) before it is built, as the project requires.
- Workers, detached scripts and scheduled tasks can call custom tools because they are the agent.
  Dreams author custom tools but do not call them.
- The dream cycle already creates skills and sees each conversation's tool calls and scripts in its
  history, so finding repeated workflows needs no new source of data, only the prompt changes of
  FR-009b–FR-009d.
- Decided with the requester (2026-11-15): tools are callable at once with no approval step; they
  are per agent with no sharing, copying or export; people get an editor in the WebUI. Creating a
  brand-new tool from the WebUI and running a tool from the WebUI are out of scope — the agent
  creates tools, people inspect, edit and govern them.
- **Security is deferred** (requester, 2026-11-15, third pass): make the feature work first. This
  version adds no security checks of its own: it does not refuse scripts that contain secret
  values, nor scripts that call turn-ending or person-waiting tools; what such a call does inside a
  custom tool is whatever the existing call path does. A later pass will look at secrets in saved
  source, turn-ending and asking tools, and policy around generated code.
- Decided with the requester (2026-11-15, second pass): dreams write custom tools as one of their
  outputs, the dream prompts change to look for them, and there are no limits for now.
