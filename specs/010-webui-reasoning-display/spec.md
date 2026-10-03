# Feature Specification: WebUI Reasoning & Tool Activity Display

**Feature Branch**: `010-webui-reasoning-display`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: "i want to adjust how webui display thinking and tool calling, currently we show them when a message is in progress and hide it after. i want it to be persisted but it could be bloated perhaps it should be collapsed after the actual agent response arrived. another thing we need to improve is python tool, i think we need additional parameter called "intent", so we mainly show the intent of the agent, while show a collapsible section for the code (and its output), also we don't need to show tools called by the script on the webui."

## Context: what is already true

Three facts shaped this spec and are worth stating up front, because they move most of the work out
of storage and into ordering and presentation.

1. **Reasoning is already recorded.** An agent thinks by calling a `think` tool, so each thought is
   already stored as a tool-call entry in session history. The live "thinking" frame the WebUI shows
   is an extra stream derived from that same call, not the only copy of it.
2. **Tool activity is already recorded**, and the replay path that rebuilds an agent's context from
   history already knows how to read intermediate assistant narration — nothing ever writes one.
3. **Nothing currently renders any of it.** The WebUI displays only requests, responses,
   checkpoints and commands, and holds the live trail in throwaway state that is wiped the moment an
   answer arrives.

So the gap is not "record the trail". It is "order the trail reliably, and show it".

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reasoning and tool activity survive the response (Priority: P1)

A person chatting with an agent in the WebUI watches it think and call tools while the answer is
composed. Today that whole trail is erased the moment the answer appears, so they can never go back
and ask "how did it get there?". They want the trail to stay attached to the answer — folded away,
so a long conversation still reads as a conversation and not as a wall of machine chatter.

**Why this priority**: This is the core complaint, and it is the foundation the other two stories
attach to — a python run's intent, code and output are themselves part of the trail that currently
disappears. It is also the single change that makes agent behaviour auditable after the fact.

**Independent Test**: Send a message that makes the agent think and call at least one tool. When the
answer arrives, confirm the reasoning and tool activity are still present above it, folded shut, and
that expanding them shows the same content that was visible while the answer was composed. Reload
and confirm it is still there. Delivers value with no python-specific work done.

**Acceptance Scenarios**:

1. **Given** an agent is composing an answer, **When** reasoning text and tool activity stream in,
   **Then** they are shown expanded and live, exactly as today.
2. **Given** reasoning and tool activity were shown live, **When** the final answer arrives,
   **Then** the trail remains in the transcript, attached to that answer, folded shut by default.
3. **Given** a folded trail, **When** the person expands it, **Then** they see the reasoning,
   the intermediate narration and the tool activity in the order the events occurred.
4. **Given** a folded trail, **When** the person collapses it again, **Then** the transcript returns
   to its compact form and the answer stays readable.
5. **Given** a turn in which the agent neither reasoned nor called a tool, **When** the answer
   arrives, **Then** no empty trail is shown.
6. **Given** a turn whose trail was shown, **When** the person reloads the conversation, **Then**
   the trail is still there, folded, in the same place, in the same order.
7. **Given** a conversation recorded before this feature, **When** it is opened, **Then** its
   already-recorded reasoning and tool activity appear as a folded trail, ordered as well as their
   timestamps allow — which may be imperfect within a single millisecond, and is accepted.

---

### User Story 2 - A python run reads as an intent, not as code (Priority: P2)

When the agent runs a python script, the person currently sees a raw script, its output, and a list
of the tool calls it made. What they want to know is *why* the agent ran it. The agent should state
that reason in its own words, and that sentence should be what the person reads; the script and its
output stay available but out of the way.

**Why this priority**: The largest single source of visual bloat in the trail, and the part the
person can least easily interpret. Depends on US1 only in that the trail must persist for the
collapsed form to matter.

**Independent Test**: Give an agent with the python tool enabled a task that requires computation.
Confirm the python entry leads with a plain-language sentence describing the purpose of the run, and
that the script and its output are reachable but not shown by default.

**Acceptance Scenarios**:

1. **Given** the agent decides to run a script, **When** it issues the run, **Then** it supplies a
   short plain-language statement of what the script is for.
2. **Given** a run issued without that statement, **When** the tool receives it, **Then** the run is
   rejected back to the agent with an error naming the missing input, and the agent may retry.
3. **Given** a python run with a stated intent, **When** the entry is rendered, **Then** the intent
   is the prominent text and the script source is not shown by default.
4. **Given** a python run entry, **When** the person expands its detail section, **Then** they see
   the script source, anything it printed, its returned value, and any error with its traceback.
5. **Given** a python run that failed, **When** the entry is rendered, **Then** the failure is
   evident from the collapsed form without expanding it.
6. **Given** a python run recorded before this feature existed, **When** the entry is rendered,
   **Then** it still renders correctly using a neutral fallback label in place of the missing intent.

---

### User Story 3 - The script's own tool calls stay out of the transcript (Priority: P3)

A script running in code mode may call a dozen of the agent's tools in a loop. Listing each one in
the transcript tells the person nothing they want and crowds out what they do want. Those inner
calls should not appear in the WebUI at all, while remaining fully available to the agent itself.

**Why this priority**: Smallest of the three and purely subtractive, but it is the difference between
a readable python entry and an unreadable one for loop-heavy scripts.

**Independent Test**: Run a script that calls a tool several times in a loop. Confirm no per-call
list appears anywhere in the WebUI transcript, while the agent's own behaviour and the data it
receives back are unchanged.

**Acceptance Scenarios**:

1. **Given** a script that called several tools, **When** its entry is rendered in the WebUI,
   **Then** no list of those individual calls is shown, collapsed or expanded.
2. **Given** a script that called several tools, **When** the agent receives the run's outcome,
   **Then** the information the agent gets about those calls is unchanged from today.
3. **Given** a tool call made by a script fails and the script does not catch it, **When** the entry
   is rendered, **Then** the resulting run failure is still visible to the person.

---

### Edge Cases

- **Several entries are recorded within the same millisecond.** This is the normal case, not an
  edge case: a whole turn's entries are flushed in one tight loop when the turn ends. They must come
  back in occurrence order every time, not in an order that varies with the query plan.
- **A page boundary falls inside a group of entries sharing a timestamp.** For entries recorded
  after this change, paging must neither skip nor duplicate an entry across the boundary.
- **A conversation mixes entries from before and after this change.** It must render without error.
  The newer entries order exactly; the older ones keep the ordering their timestamps give them. No
  attempt is made to interleave the two groups more cleverly than by timestamp.
- **Intermediate narration sits between a tool call and its result.** Replaying such history must
  still produce a valid conversation for providers that require a tool result to immediately follow
  its tool call, or the agent's next turn fails outright.
- **A turn is aborted mid-flight.** The trail accumulated so far is discarded with the turn, matching
  today's behaviour, so nothing is left dangling without an answer to attach to.
- **A turn ends in an error instead of an answer.** The trail attaches to the error entry and folds
  like any other turn, so the person can inspect what led to the failure.
- **The page is reloaded while a turn is in progress.** Whatever is already recorded reappears; the
  still-streaming remainder resumes under the live indicator.
- **Reasoning arrives as many small fragments.** The person sees one continuous block of reasoning
  per turn, not one entry per fragment.
- **A turn contains several python runs.** Each gets its own intent-led entry, in order.
- **The reasoning for one turn is very long.** The collapsed form stays a fixed, small height
  regardless of what is inside it, and expanding scrolls rather than pushing the conversation off
  screen.
- **An intent is uselessly long.** The entry still renders; the collapsed form truncates it and the
  full text is reachable when expanded.
- **Other channels.** Discord and Telegram presentation is unchanged by this feature.

## Requirements *(mandatory)*

### Functional Requirements

#### Ordering: making the recorded trail trustworthy

- **FR-001**: Every session history entry MUST carry an explicit ordering position, assigned when
  the entry is recorded, that increases monotonically within a conversation and does not depend on
  wall-clock timestamp resolution.
- **FR-002**: Entries that share a timestamp MUST be returned in the order they occurred, on every
  retrieval, independent of query plan.
- **FR-003**: The order in which entries are returned MUST equal the order the events occurred
  within the turn.
- **FR-004**: Every ordered retrieval path over session history MUST apply the ordering position,
  not timestamp alone — including the paths that read history up to a checkpoint and the paths that
  rebuild an agent's own context.
- **FR-005**: Paged retrieval MUST use a cursor that includes the ordering position, so a page
  boundary falling inside a group of entries sharing a timestamp neither skips nor duplicates an
  entry. For entries that have no ordering position, paging MAY degrade to today's behaviour.
- **FR-006**: Entries recorded before this feature MUST NOT be assigned ordering positions. No
  backfill is performed; this is an accepted breaking change.
- **FR-007**: An entry with no ordering position MUST still load and render, ordered by timestamp
  alone — that is, with today's ordering behaviour, neither improved nor made worse.
- **FR-008**: Ordering guarantees FR-002, FR-003 and FR-005 apply only to entries recorded after this
  change. A conversation that mixes positioned and unpositioned entries MUST render without error,
  keeping the positioned ones in occurrence order.
- **FR-009**: The absence of a backfill MUST be recorded as a breaking change in the project
  changelog, naming what degrades: pre-existing history keeps its existing imperfect ordering within
  a single timestamp.

#### Recording the full trail

- **FR-010**: The intermediate narration an agent produces alongside its tool calls MUST be recorded
  rather than discarded, and MUST be retained when history is read back for display.
- **FR-011**: A piece of intermediate narration MUST be recorded at a position before the tool calls
  it accompanies, so that replaying the history keeps each tool call adjacent to its result.
- **FR-012**: Replaying history that contains intermediate narration MUST produce a valid
  conversation for providers that require a tool result to immediately follow its tool call.
- **FR-013**: What the agent receives when its context is rebuilt from history MUST remain coherent
  after this change; in particular, no turn may become unreplayable because of a newly recorded
  entry.

#### Displaying and folding the trail

- **FR-014**: The WebUI MUST render the reasoning, intermediate narration and tool activity recorded
  for a turn, drawn from stored history rather than only from the live stream.
- **FR-015**: The retained trail MUST be presented folded shut by default once the turn's answer has
  arrived, and MUST remain expanded and live while the turn is still in progress.
- **FR-016**: A person MUST be able to expand and re-collapse any turn's trail independently of
  every other turn's.
- **FR-017**: A turn with no reasoning, no narration and no tool activity MUST NOT display a trail.
- **FR-018**: The collapsed trail MUST occupy a bounded amount of vertical space that does not grow
  with the amount of content inside it.
- **FR-019**: A trail MUST be attached to the turn it belongs to, so the association between an
  answer and the activity that produced it is unambiguous.
- **FR-020**: Consecutive fragments of reasoning within one turn MUST be presented as a single
  continuous block rather than as separate entries.
- **FR-021**: A trail MUST be shown for turns that end in an error, attached to that error.
- **FR-022**: When a turn is aborted, its accumulated trail MUST be discarded rather than left in the
  transcript.
- **FR-023**: A conversation whose history contains entry kinds the WebUI does not recognise MUST
  still render, skipping only what it cannot interpret.

#### Python runs led by intent

- **FR-024**: The python execution tool MUST accept an additional `intent` input: a short
  plain-language statement, written by the agent, of what the script is for.
- **FR-025**: `intent` MUST be required. A run that omits it MUST be rejected back to the agent with
  an error that names the missing input, so every recorded run has a human-readable headline.
- **FR-026**: The tool's description MUST tell the agent what `intent` is for and that it is
  addressed to the person watching, not to the agent itself.
- **FR-027**: A python run entry in the WebUI MUST present the stated intent as its prominent,
  always-visible text.
- **FR-028**: A python run entry MUST NOT show the script source by default; the source MUST be
  reachable through a collapsible section on that entry.
- **FR-029**: That collapsible section MUST contain the script source, anything the script printed,
  the value it returned, and, on failure, the error and its traceback.
- **FR-030**: The collapsed form MUST make a failed run distinguishable from a successful one without
  expanding it.
- **FR-031**: The collapsed form MUST keep an over-long intent from breaking the layout, truncating
  it as needed while leaving the full text reachable when expanded.
- **FR-032**: A python run recorded without an intent — that is, one recorded before this feature —
  MUST render with a neutral fallback label in its place.
- **FR-033**: The stated intent MUST be recorded alongside the run so it is present when the
  conversation is reloaded.

#### Hiding script-internal tool calls

- **FR-034**: The WebUI MUST NOT display the individual tool calls a python script made, in either
  the collapsed or the expanded form of the run's entry.
- **FR-035**: The information the agent itself receives about a script's tool calls MUST be
  unchanged by this feature.
- **FR-036**: A run that failed because of an uncaught error from a tool call MUST still surface that
  failure to the person as part of the run's error.

#### Scope boundaries

- **FR-037**: Presentation of reasoning and tool activity in channels other than the WebUI MUST be
  unchanged by this feature.
- **FR-038**: Which tools the agent has, when it calls them, and what it receives back MUST be
  unchanged, apart from the addition of the required `intent` input.

### Key Entities

- **Activity trail**: The ordered record of what an agent did while composing one answer — its
  reasoning, its intermediate narration, and its tool activity. Belongs to exactly one turn, and is
  displayed folded once that turn's answer exists.
- **Ordering position**: An explicit, monotonically increasing position recorded with every history
  entry. It is what makes occurrence order recoverable when several entries share a timestamp, and
  it is what paging cursors key on.
- **Reasoning block**: The agent's thinking for one turn, assembled from however many thoughts it
  recorded. Already stored today as `think` tool calls.
- **Intermediate narration**: Assistant text produced alongside tool calls, mid-turn. Newly
  recorded, and positioned before the tool calls it accompanies.
- **Tool activity entry**: One tool the agent chose to call during a turn, with its arguments.
  Already recorded today; newly displayed after the fact.
- **Python run entry**: A single execution of the python tool, carrying the agent's stated
  **intent**, the script source, what it printed, what it returned, and any error. Its inner tool
  calls are part of the record but are not shown in the WebUI.
- **Intent**: A short statement, written by the agent and addressed to the person watching, of what
  a python script is for. A display headline, not an instruction to the sandbox.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After an answer arrives, 100% of the reasoning and tool activity that was visible
  while it was being composed is still reachable in the transcript.
- **SC-002**: A turn containing at least 10 interleaved thoughts and tool calls returns them in
  occurrence order on 100% of loads, across at least 20 repeated loads of the same conversation.
- **SC-003**: Paging through a conversation of 200 entries in pages of 20 yields exactly 200
  distinct entries, with no duplicate and no omission.
- **SC-004**: Conversations recorded before this feature load with no errors and no missing entries.
  Their ordering is not required to improve; it is required not to regress below today's behaviour.
- **SC-005**: Agents continue to complete turns against a provider that enforces tool-call/result
  adjacency, with a 0% rate of replay-rejected turns across at least 20 multi-tool turns.
- **SC-006**: A turn's trail adds no more than two lines of height to the transcript while folded,
  irrespective of how much activity it contains.
- **SC-007**: A person can go from reading an answer to seeing the reasoning behind it in a single
  interaction.
- **SC-008**: Reading a conversation containing python runs requires no scrolling past script source:
  the default view of a python run is one line of intent.
- **SC-009**: 100% of python runs recorded after this change carry an intent, by construction.
- **SC-010**: Reading a sample of 20 recorded intents, each describes its run in plain language that
  a person who has not seen the script can understand.
- **SC-011**: No individual script-internal tool call appears anywhere in the WebUI transcript.
- **SC-012**: Scrolling a conversation of 100 turns, each with a folded trail, stays as smooth as the
  same conversation does today.
- **SC-013**: The agent's choice of tools and the content it receives from them is unchanged from
  today for the same inputs, except for the added intent.

## Decisions

These were open questions, resolved before planning.

- **The trail is sourced from stored history, ordered by an explicit position recorded per entry.**
  Rather than leaning on the implicit row identifier for tie-breaking, the position is explicit, so
  ordering survives any future table rebuild that would renumber implicit identifiers. Rejected:
  page-lifetime-only retention (loses the trail on reload and shows nothing for old conversations);
  implicit-identifier tie-break (free, but fragile).
- **No backfill. This ships as a breaking change.** Pre-existing entries get no ordering position and
  keep the ordering their timestamps give them, which within a single millisecond is the arbitrary
  order they already have today. This buys away the entire migration — no backfill pass, no
  resumability, no interrupted-upgrade states to reason about — at the cost of leaving old history
  exactly as imperfect as it is now. Nothing gets worse; the fix simply does not reach backwards.
  Rejected: backfilling from insertion order, which would have ordered old history correctly but
  reintroduced a migration over a table that grows without bound.
- **Intermediate narration is recorded and shown inside the folded trail.** It is often the most
  readable account of what the agent is doing, and folding means it costs nothing in the transcript.
  This is the change that carries the adjacency risk in FR-011 through FR-013, which is why those
  requirements are hard ones: narration recorded in the wrong position would make turns
  unreplayable.
- **`intent` is strictly required.** A run that omits it fails and the agent retries. This trades a
  possible extra round trip on a non-compliant provider for the guarantee that no recorded run is
  ever headline-less. Rejected: optional-with-fallback, which would have left a meaningful share of
  entries reading as a generic label and undercut the whole point of the change.

## Breaking Changes

Two, both deliberate, both to be flagged in the changelog.

1. **Ordering of pre-existing history is not fixed.** Entries recorded before this change carry no
   ordering position and are ordered by timestamp alone, so entries sharing a millisecond keep the
   arbitrary order they have today. New history orders exactly. No data is lost or moved; the
   improvement simply does not apply retroactively.
2. **`intent` becomes a required input on the python execution tool.** Any caller that issues a run
   without it — an external API client, a stored script, a prompt that hand-writes the call — starts
   failing until it supplies one. Agents adapt on their own, since the tool definition they are given
   declares the input.

## Assumptions

- "Persisted" means the trail stays attached to its turn and survives a reload. The live, expanded
  display during a turn in progress is already what the person wants and is kept as is; only what
  happens *after* the answer arrives changes.
- Today's abort behaviour — discard the trail — is correct and is kept, because an aborted turn has
  no answer for a trail to belong to.
- Occurrence order equals the order entries are recorded, because a turn's entries are written by
  walking the turn's message sequence in order. The ordering position therefore only needs to be
  monotonic, not derived from anything richer.
- The intent is for human eyes. It does not alter how the script runs, is not fed back to the agent
  as guidance, and has no effect on sandbox limits or tool availability.
- Only the python tool gains an intent. Other tools continue to be displayed from their name and
  arguments, as today.
- Hiding script-internal tool calls is a presentation decision. The calls stay in the run's record,
  so a future surface — a debug view, another channel — could still show them.
- Existing records are strictly read-only: nothing is rewritten, no ordering position is assigned to
  a past entry, no intent is invented for a past run, and no reasoning is reconstructed that was
  never captured.
- Imperfect ordering of pre-existing history is acceptable to the people reading it. The trail is
  most valuable on recent conversations, and old ones remain exactly as legible as they are today.
- Collapsed-by-default applies to completed turns only. There is no per-user preference for
  default-open trails; that can be added later if asked for.
- Changes are verified by running the binary against an agent on the offline test provider, per the
  project constitution, in addition to unit tests over the ordering logic.
