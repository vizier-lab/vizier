# Feature Specification: Persist In-Progress Turn Activity

**Feature Branch**: `023-persist-turn-activity`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "currently thinking/activity/toolcall list state is not persists, so when agent is still thinking, go to another page, and go back. the list will be reset"

## Context: what is already true

1. **A finished turn's trail already survives.** Spec 010 made reasoning and tool activity part of
   the transcript: once a turn ends, its trail is recorded with it and is shown folded above the
   answer, after navigation or a reload alike.
2. **A running turn's trail does not.** While the agent is still working, the trail exists only in
   the open chat page, built up from events as they stream in. Nothing about the turn so far is
   recorded until the turn ends.
3. **Returning mid-turn shows only what comes next.** A page reopened while the turn runs is told
   the agent is busy and receives the turn's events from then on (spec 012's session events), but
   everything that happened before it arrived is gone from view until the turn ends and the full
   trail appears.

So the gap is narrow: the trail of the turn that is *still running*.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Leaving and returning mid-turn keeps the trail (Priority: P1)

A person asks an agent something that takes a while: it thinks, runs a few tools, thinks again.
While it works, they open another page in the WebUI — an agent's settings, the memory view, a
different conversation — and then come back. Today the activity list is empty when they return,
or shows only the steps taken since, so they can't tell what the agent has already done or whether
it is stuck. They expect to find the list exactly as it would have been had they never left.

**Why this priority**: This is the reported problem. Long turns are exactly the ones a person is
most likely to step away from, and exactly the ones whose progress they most want to see.

**Independent Test**: Start a turn that makes several tool calls over at least ten seconds. After
the first two activity entries appear, navigate to another WebUI page, wait for at least one more
step to happen, and return. The activity list shows every step so far, in order, including the one
taken while away, and keeps updating live.

**Acceptance Scenarios**:

1. **Given** an agent is mid-turn with activity entries shown, **When** the person navigates to
   another page and back before the turn ends, **Then** every entry recorded so far is shown, in
   the order it happened, with nothing duplicated.
2. **Given** the person is away while the agent takes further steps, **When** they return, **Then**
   those steps are in the list too, not only steps taken after their return.
3. **Given** the person has returned mid-turn, **When** the agent takes further steps, **Then**
   they are appended live, exactly as for a person who never left.
4. **Given** the person returned mid-turn, **When** the turn ends, **Then** the trail folds above
   the answer just as it does for a person who never left, with the same entries.
5. **Given** the person moved to a different conversation and back, **When** they return, **Then**
   the running turn's trail is shown in its own conversation only, never in the one they visited.

---

### User Story 2 - Reloading or opening a second tab mid-turn shows the trail (Priority: P2)

A person reloads the page while the agent is working, or opens the same conversation in a second
tab or on another device. They should see the running turn's activity so far, the same as in the
first tab.

**Why this priority**: Same need as US1 from a different entry point; a reload is the most common
"fix" people reach for when a page looks stuck, and it currently makes things look worse.

**Independent Test**: Start a long turn in one tab. Mid-turn, reload it, and separately open the
conversation in a second tab. Both show the activity list so far, and both keep updating.

**Acceptance Scenarios**:

1. **Given** a turn is running, **When** the person reloads the conversation, **Then** the
   activity so far is shown and the "agent is working" indicator is shown with it.
2. **Given** a turn is running, **When** the conversation is opened in a second tab or on another
   device by someone allowed to view it, **Then** that view shows the same activity list as the
   first.

---

### Edge Cases

- **Turn ends while the person is away**: on return the finished trail is shown folded above the
  answer (already true today) — no stale "in progress" list lingers alongside it.
- **Turn is aborted while the person is away**: on return no in-progress list is shown, matching
  what a person who stayed would see after an abort.
- **Server restarts mid-turn**: the turn is lost (as today); on return no list claims it is still
  running. Whatever activity was recorded before the restart may be shown as an interrupted trail
  or not at all, but never as live.
- **Return in the instant a step lands**: a step arriving while the list is being restored appears
  exactly once.
- **Long-running turn with many steps**: the full list is restored, not only the latest few.
- **Queued messages behind the running turn**: restoring the trail does not disturb or reorder the
  queue shown below it.
- **Background report turns** (spec 012) and **client tool calls** (spec 019) that run while the
  person is away are restored the same way as a turn they started themselves.
- **Very long tool output in a step**: restored the same as it was shown live (no extra
  truncation or expansion).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The activity of a turn that is still running (reasoning, intermediate narration, tool
  calls and their results) MUST remain available for that conversation for as long as the turn
  runs, independent of whether any page is showing it.
- **FR-002**: When a person opens a conversation whose agent is mid-turn — by navigating back,
  reloading, or opening it elsewhere — the WebUI MUST show every activity entry of that turn
  recorded so far, in the order it happened.
- **FR-003**: After that initial view, new entries MUST continue to arrive live, and an entry MUST
  never appear twice or be missed because it landed while the view was being restored.
- **FR-004**: The restored list MUST look and behave the same as the list of a person who never
  left (same entries, same formatting, same expanded/live state).
- **FR-005**: When the turn ends, the trail shown to someone who returned mid-turn MUST be the same
  as the trail shown to someone who never left, and the same as the one shown after a later reload.
- **FR-006**: A running turn's activity MUST only ever be shown in its own conversation.
- **FR-007**: An aborted or interrupted turn MUST NOT be shown as still running after the person
  returns.
- **FR-008**: Viewing a running turn's activity MUST be subject to the same access rules as viewing
  the conversation itself.
- **FR-009**: Conversations, turns and trails recorded before this feature MUST display as they do
  today.

### Key Entities

- **Running turn**: the agent's current, unfinished response in one conversation and the ordered
  activity it has produced so far.
- **Activity entry**: one step in a turn's trail — a piece of reasoning, a line of narration, a
  tool call, or a tool result — with its position in the turn's order.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In 100% of trials, navigating away and back during a running turn shows every
  activity entry produced so far, with none missing and none duplicated.
- **SC-002**: The restored activity list appears within 1 second of the conversation opening.
- **SC-003**: The trail shown after the turn ends is identical whether the person stayed, left and
  returned, or reloaded.
- **SC-004**: Two views of the same running conversation show the same activity list at any moment,
  give or take entries still in flight.

## Assumptions

- Scope is the WebUI chat page. Discord and Telegram have no activity list and are unaffected.
- "Persist" means for the life of the running turn plus what spec 010 already keeps afterwards; it
  does not mean surviving a server restart mid-turn (a restart already ends the turn as
  interrupted).
- Whether each entry is expanded or folded while live follows existing behaviour; personal UI
  state such as which entries a person manually expanded need not survive leaving the page.
- The working indicator shows no elapsed time today, so none is added here; a turn's duration is
  shown once it ends, as before.
- Reasoning shown live today is the agent's `think` activity (spec 010); no new kind of activity is
  introduced.
- The existing "is the agent working" signal for a conversation stays the source of truth for
  whether a turn is running.
