# Feature Specification: Prune old tool outputs, recall them on demand

**Feature Branch**: `060-prune-tool-outputs`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "The vizier loop can be token hungry, especially in project mode. Prune old tool outputs, but with our own approach, because we rely on Python tool calling: when an output is pruned, instead of a bare stub the model sees an id, something like `[output pruned - use recall_history(<id>) to recall]`, and we provide a `recall_history` tool so the agent can recall a given tool call. The agent decides by itself which outputs to recall, and can recall several in the same Python script. Don't cap or limit what `recall_history` returns for now; we wait and see how it works for people first. `recall_history` gets a new tool category."

## Context

Every result a tool produces in a turn stays in what the model is sent, round after round, and
in every later turn of the conversation, until the conversation is compacted near the end of the
model's context window. In a project chat a single file read or search can be tens of kilobytes,
so a turn of twenty rounds pays for its early reads twenty times, and the next turn pays for them
again. The record kept for people (storage, the WebUI) needs all of it; the model, most of the
time, does not.

Other agent harnesses solve this by clearing old tool outputs and letting the model re-run the
tool if it needs them again. Vizier's model does all of its work through Python scripts, which
changes the trade: a script can bring an old output back *into the script*, filter it there, and
return only the part that matters, so recalling is cheap and the model can decide what is worth
recalling. This feature prunes old outputs from the model's view, leaves each one an addressable
placeholder, and gives the agent one tool to read an output back.

Nothing about what is stored or shown to people changes. Only the model's view is narrowed.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Long turns stop re-sending old outputs (Priority: P1)

A person asks an agent in a project chat to investigate and fix something. The agent reads a
dozen files and runs several searches over many rounds. Once the outputs it has accumulated pass
a budget, the oldest ones are replaced, in what the model is sent, by a one-line placeholder that
names the output's id and size. The agent keeps working, and the rounds after the prune cost far
fewer input tokens than they would have.

**Why this priority**: This is the saving the feature exists for. Without it nothing else matters.

**Independent Test**: Run a turn on an offline test agent whose scripted tool calls produce
outputs that pass the budget, and compare the input the model receives in each round with and
without pruning: the old outputs are placeholders, the newest are whole, and the stored history
and the WebUI trail still hold every output in full.

**Acceptance Scenarios**:

1. **Given** a turn whose tool outputs stay under the budget, **When** the turn runs, **Then**
   nothing is pruned and the model's view is exactly what it is today.
2. **Given** a turn whose tool outputs pass the budget, **When** the next request is sent to the
   model, **Then** the oldest eligible outputs are replaced by placeholders until what remains is
   at or under the lower mark, and the newest outputs are whole.
3. **Given** a pruned turn, **When** a person opens the conversation in the WebUI, **Then** every
   tool output is shown in full, as before.
4. **Given** a pruned conversation, **When** the person sends the next message, **Then** the
   earlier turns are replayed to the model with the same outputs pruned, in the same way, as the
   live turn showed them.

---

### User Story 2 - The agent recalls what it needs (Priority: P1)

Later in the same turn, or in a later turn, the agent needs the exact contents of a file it read
before the prune. It sees the placeholder, calls `recall_history` with the id from it inside a
script, and gets the full original output back as data in the script. It can recall several
outputs in one script, extract what it needs, and return only that.

**Why this priority**: Pruning without a way back would make the agent re-run tools (re-read files
that may have changed, repeat searches, repeat side effects) or lose information. Recall is what
makes pruning safe to turn on.

**Independent Test**: In a pruned conversation on the offline test agent, run a script that
recalls two pruned ids and one unknown id, and check the script receives both full outputs,
identical to what is stored, and a clear "not found" result for the third.

**Acceptance Scenarios**:

1. **Given** a placeholder with an id, **When** a script calls `recall_history` with that id,
   **Then** the script receives the original output exactly as it was stored, with no size limit.
2. **Given** several placeholders, **When** one script recalls each of them, **Then** every recall
   succeeds and the script can combine them.
3. **Given** an id that does not belong to this conversation, or does not exist, **When** a script
   recalls it, **Then** the script receives a "not found" result it can handle, and the script
   does not fail.
4. **Given** an output recalled into a script and returned whole, **When** that new output later
   passes the budget, **Then** it can itself be pruned, and its placeholder is recallable too.

---

### User Story 3 - Operators can see whether it helps (Priority: P2)

The maintainers want to judge the feature on real use before adding limits to recall. For each
turn they can see how many outputs were pruned, roughly how many tokens that removed from what
the model was sent, and how often the agent recalled something.

**Why this priority**: The decision to leave recall unlimited is explicitly "wait and see"; this
is what there is to see. It does not change behaviour, so it comes after the two stories above.

**Independent Test**: Run a turn that prunes and recalls on the offline test agent with telemetry
export on, and check the turn reports the pruned count, the estimated tokens removed and the
recall count.

**Acceptance Scenarios**:

1. **Given** a turn that pruned outputs, **When** its telemetry is exported, **Then** it carries
   the number of outputs pruned and the estimated tokens removed.
2. **Given** a turn in which the agent recalled outputs, **When** its telemetry is exported,
   **Then** each recall is visible as a tool call like any other, with the id it asked for.

---

### Edge Cases

- **Small outputs** (under about 2 KB) are never pruned: the placeholder would save almost
  nothing, and a small output (a status, an error) often explains the next step.
- **Failed scripts and errors** are never pruned, for the same reason.
- **The newest round's outputs** are never pruned, so the model always sees what it just did.
- **An output that ended the turn** (a turn-ending tool's result) is never pruned.
- **Images** loaded into the conversation by a tool are not pruned by this feature; only text
  outputs are.
- **The script's own code** (the tool call the model made) is never pruned; it is what tells the
  agent what a pruned output contained, alongside the placeholder's size.
- **Compaction**: when a conversation is compacted, outputs from before the compaction are no
  longer sent at all; their ids stay recallable if the agent still knows them.
- **Resumed turns** (after a restart) are pruned the same way as if they had never stopped.
- **Prompt caching**: pruning changes what the model is sent from the first newly pruned output
  onward, which costs one cache miss from that point. Pruning therefore happens rarely and in
  large steps (see FR-004), never one output per round.
- **Subagent workers and other conversations** (scheduled tasks, Workspaces, channels) are pruned
  by the same rule, since they run the same loop; each recalls only from its own conversation.
- **Dream cycles** do not run this loop and are not affected.
- **A recalled output that is very large** is returned whole (no cap in this version); if a
  script returns it whole, it re-enters the model's view and is subject to pruning again.

## Requirements *(mandatory)*

### Functional Requirements

**Pruning**

- **FR-001**: The system MUST narrow only the model's view of a conversation. The stored history,
  the WebUI, the activity trail, notifications and usage MUST see every tool output in full.
- **FR-002**: A pruned output MUST be replaced in the model's view by a single-line placeholder
  carrying the output's id, its original size, and how to recall it, in the form
  `[output pruned (<size>) - use recall_history("<id>") to recall]`.
- **FR-003**: The id in a placeholder MUST identify exactly one stored tool output, MUST be unique
  within the whole installation, and MUST stay the same for the life of the conversation.
- **FR-004**: Pruning MUST be triggered by a high mark and run down to a low mark: nothing is
  pruned until the unpruned eligible output in the model's view passes the high mark, and a prune
  then removes the oldest eligible outputs until what remains is at or under the low mark. The
  defaults are about 60,000 tokens (high) and 30,000 tokens (low), estimated from text size.
- **FR-005**: Pruning MUST be one-way: once an output is pruned in a conversation's model view, it
  stays pruned in every later request of that conversation.
- **FR-006**: The live turn and the replay of earlier turns MUST make the same pruning decisions
  for the same history, so a later turn's request begins exactly as the previous turn's last
  request did (and keeps its prompt cache).
- **FR-007**: The system MUST NOT prune: outputs under the small-output threshold (about 2 KB),
  outputs of failed scripts or tool errors, outputs of the newest round, an output that ended the
  turn, images, or the tool calls themselves.
- **FR-008**: Pruning MUST apply to every conversation that runs the agent turn loop, including
  project chats, ordinary chats, channels, Workspaces, scheduled tasks and subagent workers. It
  is on for all of them from the start, with no per-agent switch.

**Recall**

- **FR-009**: The system MUST provide a `recall_history` tool, reachable from scripts like every
  other tool, that takes one output id and returns the original output exactly as stored.
- **FR-010**: `recall_history` MUST NOT cap, truncate or summarise what it returns in this version.
- **FR-011**: `recall_history` MUST return only outputs of the conversation it is called from. An
  id from another conversation, another agent, or one that does not exist MUST return the same
  "not found" result, a status the script can read, never a script failure.
- **FR-012**: `recall_history` MUST be able to recall any output of its conversation, including
  outputs from earlier turns and from before a compaction.
- **FR-013**: A script MUST be able to call `recall_history` any number of times.
- **FR-014**: `recall_history` MUST be in a new tool category, `history`, listed with the other
  categories in the agent's tool guide. `history` MUST become a reserved category name, so an MCP
  server or client category with that name is renamed the way other reserved names are.
- **FR-015**: `recall_history` MUST be available in every conversation where pruning can happen
  and MUST NOT be offered to the dream cycle.
- **FR-016**: The agent's guidance MUST say, in one sentence, that a pruned output can be recalled
  into a script with `recall_history` and that the script should return only what it needs from it.

**Measurement**

- **FR-017**: Each turn MUST report, in its telemetry, the number of outputs pruned in that turn's
  requests and the estimated tokens removed.
- **FR-018**: Recalls MUST be visible as ordinary tool calls in the trail, history and telemetry.

### Key Entities

- **Tool output**: the text result of one tool call (in practice one script run), stored in the
  conversation's history with a stable id. Unchanged by this feature.
- **Placeholder**: what the model sees in place of a pruned output: id, original size, how to
  recall. Never stored; derived each time the model's view is built.
- **Pruning marks**: the high and low token marks and the small-output threshold that decide what
  is pruned.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a project-chat turn of 20 or more rounds that reads files and searches, the total
  input tokens sent to the model are at least 40% lower than the same turn without pruning.
- **SC-002**: In a conversation whose outputs never pass the high mark, the model's view is
  byte-identical to today's.
- **SC-003**: 100% of recalls of a pruned id from its own conversation return the output exactly
  as stored; 100% of recalls of a foreign or unknown id return "not found" without failing the
  script.
- **SC-004**: Across two consecutive turns of a pruned conversation, the second turn's first
  request starts with the same content as the first turn's last request (no prompt-cache loss
  caused by pruning between turns).
- **SC-005**: Prunes happen at most once per roughly 30,000 tokens of new tool output, never on
  every round.
- **SC-006**: Every turn's telemetry shows its pruned count and estimated tokens removed, so the
  decision about limiting recall can be made from real use.

## Assumptions

- Token counts for the marks are estimated from text size (about four characters per token); an
  exact count per provider is not needed for the marks to work.
- The marks and the small-output threshold are fixed defaults in this version, not per-agent
  settings; they can be made configurable later if real use shows a need.
- Each stored tool output already has a unique history id, and outputs are stored the moment they
  exist (including mid-turn), so an output is recallable as soon as it can be pruned.
- The tool category list in the agent's guide gains one line, which changes every agent's system
  prompt once after the upgrade (one cache miss per conversation, once).
- Limits on recall (size caps, slicing) are deliberately out of scope until real use is observed.
- Other token savings discussed alongside this one (output size caps, compaction thresholds,
  subagent exploration, repository maps) are out of scope for this feature.
