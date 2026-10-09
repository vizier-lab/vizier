# Feature Specification: Background Subagent Results

**Feature Branch**: `012-background-subagent-results`

**Created**: 2026-10-09

**Status**: Draft

**Input**: User description: "ok let's do some adjustment, paralel_subtasks should be non blocking. and for non-blocking one can we send back subagent response back to the original session message queue to "awake" the agent?"

## Context

An agent has three ways to hand work to someone else:

| Tool | Today | After this feature |
|---|---|---|
| `paralel_subtasks` | Blocking: the agent's turn waits until every subtask answers | **Non-blocking**: returns immediately; results come back later |
| `delegate_agent` | Non-blocking, fire-and-forget: the outcome never comes back | Non-blocking; **the outcome comes back** |
| `consult_agent` | Blocking: waits for the other agent's answer | Unchanged |

"Comes back" means the result is delivered into the conversation that started the work, as a new incoming message. If the agent is idle in that conversation, the message wakes it and it takes a new turn. If the agent is busy, the message waits its turn behind whatever is already queued there. The agent then decides what to do with the result: tell the person, carry on with the work, or stay quiet.

Running jobs can be listed and cancelled, by the agent through two new tools (`list_background_jobs`, `cancel_background_job`) and by the person from the WebUI. A cancelled job never reports back.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Parallel subtasks no longer freeze the conversation (Priority: P1)

A person asks an agent for something that splits naturally into several independent pieces, such as researching three topics. The agent fans the work out with `paralel_subtasks`. The agent's turn ends right away; it can tell the person "working on it", and the person can keep talking to it. When all the subtasks have finished, the agent is woken in that same conversation with every result, and it replies to the person there.

**Why this priority**: This is the change the user asked for directly. Today a long fan-out holds the agent's whole turn hostage: the person sees nothing, and any further messages pile up unanswered until the slowest subtask finishes.

**Independent Test**: On the offline `dummyplug` provider, call `paralel_subtasks` with two tasks. Check that the tool returns at once with an acknowledgement, and that the turn ends. Then check that a new turn starts in the same conversation carrying both results, without anyone sending another message.

**Acceptance Scenarios**:

1. **Given** an agent in a conversation, **When** it calls `paralel_subtasks` with N tasks, **Then** the tool returns within a second with an acknowledgement that names the batch and the number of tasks, and the agent's turn can end without waiting for any of them.
2. **Given** a batch has been launched and the agent is idle, **When** the last task in the batch finishes, **Then** the agent is woken in the originating conversation with one message containing every task's prompt and its result, in the order the tasks were given.
3. **Given** a batch is running, **When** the person sends a new message in the same conversation, **Then** the agent answers it normally without waiting for the batch.
4. **Given** the agent is mid-turn in the originating conversation when the batch finishes, **When** the result arrives, **Then** it waits behind the current turn and anything already queued, and is handled as the next turn. It never interrupts the running turn.

---

### User Story 2 - Delegated work reports back (Priority: P1)

An agent hands a job to another agent with `delegate_agent`. Today the result vanishes: the delegating agent is never told whether the job was done or what came of it. After this change, when the other agent finishes, its answer is delivered into the conversation the delegation came from, and the delegating agent is woken to act on it.

**Why this priority**: Without a report-back, delegation is unreliable for anything the person cares about, because the agent that promised the work cannot confirm it. It is the same mechanism as Story 1, so it costs little to cover both.

**Independent Test**: With two agents on `dummyplug`, have agent A call `delegate_agent` targeting agent B. Check that A's tool call returns immediately, and that once B answers, A gets a new turn in the original conversation containing B's answer and naming B.

**Acceptance Scenarios**:

1. **Given** agent A delegates a task to agent B, **When** B produces its answer, **Then** A is woken in the conversation the delegation was made from, with B's answer, B's identity, and the original task.
2. **Given** agent A delegates to an agent that does not exist or is not running, **When** the delegation is attempted, **Then** the tool call itself fails with a clear error and no wake is scheduled.

---

### User Story 3 - The person actually hears about the result in the WebUI (Priority: P1)

When work was started from a WebUI topic, the woken agent's reply appears in that topic, live if it is open, and like any other message from the agent.

This round, the outbound route is WebUI-only. Work started from Discord or Telegram still wakes the agent, and the agent's reply is recorded in that conversation's history, but nothing is posted to Discord or Telegram: neither the report nor the reply. Rendering on those channels is deferred.

**Why this priority**: Waking the agent is pointless if what it says next goes nowhere. Today a reply only reaches the person while their own message is still being answered. A turn nobody "asked for" has no route out, so a route out has to be part of this feature. The WebUI is where the progress tray lives, so it is the first route.

**Independent Test**: Start a conversation in the WebUI, trigger a `paralel_subtasks` batch, and check that the agent's follow-up reply appears in that same topic after the batch completes, without the person sending anything.

**Acceptance Scenarios**:

1. **Given** a batch was started from a WebUI topic that is open, **When** the agent is woken and replies, **Then** the reply streams into that topic like any other agent reply.
2. **Given** a batch was started from a WebUI topic and the browser tab has since been closed, **When** the agent replies, **Then** the reply is stored in the topic's history and is visible the next time the topic is opened.
3. **Given** a batch was started from a Discord channel or Telegram chat, **When** the batch finishes, **Then** the agent is woken in that conversation and its reply is recorded in history, and nothing is posted to Discord or Telegram.
4. **Given** the work was started from a conversation with no person in it (a scheduled task run, a dream cycle, or an inter-agent conversation), **When** the agent is woken, **Then** it handles the result in that conversation, its reply is recorded in that conversation's history, and nothing is posted to any person-facing channel.

---

### User Story 4 - Failures and stuck work are reported, not lost (Priority: P2)

A subtask errors out, or a delegated agent never answers. The agent is still woken, told plainly which piece failed or timed out and why, and given whatever partial results did succeed.

**Why this priority**: Once work runs in the background, nobody is waiting on it, so a silent failure would go unnoticed. Today a failed subtask is silently dropped from the result list, which also shifts every later result out of line with its task.

**Independent Test**: Launch a batch where one task is made to fail, or make the target unreachable after the batch starts. Check that the wake message lists the successful results next to an explicit failure entry for the bad task, in the original task order.

**Acceptance Scenarios**:

1. **Given** a batch of three tasks where the second fails, **When** the batch is reported, **Then** the report contains three entries in task order: results for tasks 1 and 3, and a failure entry with a reason for task 2.
2. **Given** a background piece of work produces no answer within the time limit, **When** the limit passes, **Then** the agent is woken with that piece marked as timed out, and any answer that arrives later is discarded rather than delivered as a second wake.

---

### User Story 5 - See running background work in the WebUI (Priority: P2)

A person chatting in the WebUI can see, without scrolling, what the agent has running in the background. A compact tray docked directly above the message input lists each running job on one line, showing progress, elapsed time and who is doing the work. The tray moves with the input. Expanding a job shows its pieces and their states. Opening a piece shows that piece's own conversation (its reasoning and tool calls) in a read-only side panel. When a job finishes, it shows a brief "done" state in the tray and then leaves it. From then on the conversation holds the record: the report, marked as a background result, followed by the agent's reply.

**Why this priority**: Once work runs in the background, the person needs to know it exists. Otherwise the agent looks idle or forgetful while it is actually busy. P2 because Stories 1–3 deliver the behaviour; this makes it visible and inspectable.

**Independent Test**: In the WebUI, trigger a `paralel_subtasks` batch on `dummyplug` and check that the tray appears above the input with the job and its pieces. Open a piece and see its conversation. Reload the page mid-run and check the tray comes back in the same state. Let the batch finish and check the tray clears, the report and the agent's reply appear in the conversation, and both survive a reload.

**Acceptance Scenarios**:

1. **Given** the agent has launched background work from the topic being viewed, **When** the person looks at the chat, **Then** a tray directly above the message input shows one collapsed line per running job: its kind (batch or delegation, with the target agent named), pieces done out of total, and elapsed time. The tray stays above the input wherever the conversation is scrolled.
2. **Given** a job in the tray, **When** the person expands it, **Then** each piece is listed with its prompt (shortened), its state (running, answered, failed or timed out) and elapsed time. The list updates live as pieces finish, without a refresh.
3. **Given** an expanded job, **When** the person opens a piece, **Then** a read-only side panel shows that piece's conversation, rendered the same way as an ordinary turn's activity trail, and updating live while the piece is running.
4. **Given** a job in the tray, **When** the person chooses to jump to it, **Then** the conversation scrolls to the tool call that launched the job.
5. **Given** a job finishes, **When** its report is delivered, **Then** the job shows a brief done state with its outcome counts and then leaves the tray. The report appears in the conversation as a collapsed entry styled as a background result, not as a message from the person, and the agent's reply follows it as a normal message.
6. **Given** a job is running, **When** the person reloads the page or reopens the topic, **Then** the tray shows the job in its current state. **Given** a job has finished, **When** the person reloads, **Then** its report and the agent's reply are in the conversation history and the job is not in the tray.
7. **Given** a narrow (phone-width) screen, **When** jobs are running, **Then** the tray collapses to a single pill showing the number of running jobs, which opens the full list on tap.
8. **Given** the agent has background work running in a topic other than the one being viewed, **When** the person looks at the topic list, **Then** that topic shows a running-jobs badge with the count.

---

### User Story 6 - Stop background work that is no longer wanted (Priority: P2)

A person changes their mind ("never mind, stop that research"), or the agent realises a job is pointless. The agent can list its running background jobs and cancel one. The person can also cancel a job directly from the WebUI tray. Cancelling stops the job's running pieces, along with anything those pieces launched in turn, and the job never sends a report.

**Why this priority**: Background work spends tokens and time with nobody waiting on it. Without a way to stop it, a mistaken or abandoned fan-out runs to its time limit and then wakes the agent for nothing.

**Independent Test**: On `dummyplug`, launch a batch with one slow piece and one fast one. Wait for the fast piece to answer, then call `cancel_background_job`. Check that the tool returns the fast piece's answer and marks the slow piece cancelled, that the slow piece's turn stops, and that no report ever arrives. Repeat with the tray's ✕.

**Acceptance Scenarios**:

1. **Given** the agent has running jobs launched from any of its conversations, **When** it calls `list_background_jobs`, **Then** it gets each job's id, kind, originating conversation, pieces done out of total, elapsed time, and a short prompt for each piece.
2. **Given** a running job the agent launched, **When** the agent cancels it, **Then** every still-running piece is stopped and marked cancelled, the job is marked cancelled, and the tool returns the results of the pieces that had already answered. No report is delivered later.
3. **Given** a job whose pieces have themselves launched background jobs, **When** the job is cancelled, **Then** those nested jobs are cancelled too.
4. **Given** a job that has already finished, **When** the agent tries to cancel it, **Then** the tool says it already finished and changes nothing.
5. **Given** a job that was launched by a different agent, **When** the agent tries to cancel it, **Then** the tool refuses, in the same way as for an unknown job.
6. **Given** a running job in the WebUI tray, **When** the person clicks its ✕ and confirms, **Then** the job is cancelled the same way, it leaves the tray marked cancelled, and the conversation records that the person cancelled it. The agent is not woken.

---

### Edge Cases

- **Server restarted while work is outstanding**: the work is lost and no wake is delivered. The agent is never woken with a false "completed". The job is recorded as interrupted, so the WebUI can show it as lost. (Resuming work across a server restart is out of scope; see Assumptions.)
- **Agent respawned (its settings updated) while work is outstanding**: pieces cut off by the respawn are reported as failed with the reason "the agent was restarted". The report is delivered to the respawned agent once it is back up. (Amended during planning: research Decision 9.)
- **Originating conversation aborted or cleared** before the result arrives: the result is still delivered to that conversation and starts a new turn there. The abort stopped the turn, not the work the turn had started.
- **Several batches outstanding in one conversation**: each batch reports independently, as its own wake, in whatever order the batches finish. Each report names its batch, so the agent can tell them apart.
- **Background work that itself starts background work**: a subtask may launch its own subtasks or delegate. Its results report back to the subtask's conversation, not to the top-level one. Nesting is limited (FR-012), so a chain of agents waking each other cannot run forever.
- **Two agents delegating to each other in a loop**: A delegates to B, and B's turn delegates back to A. Every hop counts toward the same limit (FR-012), so the chain stops.
- **Empty batch** (`paralel_subtasks` with zero tasks): rejected immediately with an error. No wake is scheduled.
- **Very large results**: the wake message caps each result's length. A result over the cap is cut off with a visible marker, so one runaway subtask cannot swamp the agent's context.
- **Many jobs at once in the WebUI**: the expanded tray has a maximum height (about 40% of the viewport) and scrolls within itself beyond that, so it never pushes the input or the conversation off screen.
- **WebUI tab closed or offline while a job finishes**: nothing is lost. The report and the agent's reply are in history the next time the topic is opened, and the tray no longer shows the job.
- **Job lost to a restart while the WebUI is open**: the tray must not show it as running forever. Once the server no longer knows the job, it leaves the tray, marked as lost rather than done.
- **Cancel racing with completion**: if the last piece finishes at the same moment a cancel arrives, exactly one wins. Either the job is reported and the cancel says "already finished", or the job is cancelled and no report is sent. It is never both.
- **Cancelling from another conversation**: the agent may cancel a job launched from any of its own conversations (for example, a job started on Discord can be cancelled when the person asks on the WebUI). The originating conversation is not woken.

## Requirements *(mandatory)*

### Functional Requirements

**Non-blocking dispatch**

- **FR-001**: `paralel_subtasks` MUST return as soon as its tasks have been handed off, without waiting for any of them to finish. Its return value MUST be an acknowledgement naming the batch and the number of tasks.
- **FR-002**: `delegate_agent` MUST stay non-blocking. Its acknowledgement MUST name the delegation, so the later report can be matched to it.
- **FR-003**: `consult_agent` MUST stay blocking and unchanged.
- **FR-004**: The descriptions of `paralel_subtasks` and `delegate_agent` that the model sees MUST say that the call returns immediately and that results arrive later as a new message in the same conversation. That way the model knows not to wait, poll, or claim the work is already done.

**Report-back ("wake")**

- **FR-005**: When background work finishes, the system MUST deliver its outcome into the conversation where the work was started (the same agent, channel and topic), as a new incoming message.
- **FR-006**: An outcome delivered while the agent is idle in that conversation MUST start a new turn. One delivered while the agent is busy MUST be queued behind the current turn and any earlier queued messages, in arrival order. It MUST NOT interrupt or be merged into a running turn.
- **FR-007**: A `paralel_subtasks` batch MUST report exactly once, after every one of its tasks has reached a final state (answered, failed or timed out). The report MUST list one entry per task in the original task order: the task's prompt, then either its result or its failure reason.
- **FR-008**: A `delegate_agent` call MUST report exactly once, carrying the target agent's identity, the delegated task, and either the target's answer or the failure or time-out reason.
- **FR-009**: The report message MUST be clearly marked as a background result, not something a person said. The agent and anyone reading the conversation history MUST be able to tell it apart from a human message.
- **FR-010**: For work started from a WebUI topic, the agent's reply to a report MUST be delivered to that topic like a reply to a human message, streamed live to any open tab. In every conversation, the reply MUST be recorded in the conversation history even if no person is connected at that moment. For work started from Discord or Telegram, nothing MUST be posted to those channels in this version: neither the report nor the reply.
- **FR-011**: In a conversation with no person in it (scheduled task run, dream cycle, inter-agent conversation), the report MUST still wake the agent there. The resulting reply MUST be recorded in that conversation's history and MUST NOT be posted to any person-facing channel.

**Safety and robustness**

- **FR-012**: The system MUST limit nesting of background work. Work started from a turn that was itself woken by a background result counts as one level deeper. Once the limit is reached, further `paralel_subtasks` and `delegate_agent` calls MUST be refused with an error that explains why. The default limit is 3 levels.
- **FR-013**: Each piece of background work MUST have a time limit. A piece that has not answered in time MUST be reported as timed out. An answer that arrives after that MUST be discarded, not delivered as a second report. The default time limit is 10 minutes.
- **FR-014**: A failure to hand off one task in a batch MUST be recorded as that task's failure entry. It MUST NOT abort the other tasks or silently drop the entry.
- **FR-015**: A result longer than the per-result cap MUST be cut off in the report, with a marker saying it was cut off.

**Visibility (WebUI)**

- **FR-016**: The system MUST let an authorized viewer of a conversation see that conversation's running background jobs, with each job's pieces, their states and elapsed times. This MUST be available both as live updates while the conversation is open and as a current-state read after a reload.
- **FR-017**: The system MUST push to an open WebUI conversation, without any action from the person: job progress changes, job completion, and the agent's reply to a report (FR-010).
- **FR-018**: The system MUST let an authorized viewer read a job piece's own conversation, live while it runs and afterwards. For a delegation, that is the target agent's side of the conversation.
- **FR-019**: The WebUI MUST show running jobs for the open conversation in a tray docked directly above the message input. It MUST be collapsed to one line per job by default, expandable to per-piece rows, and reduced to a single count pill at phone width.
- **FR-020**: The tray MUST show only jobs in flight. A finished job MUST leave the tray after a brief done state, and from then on the conversation history MUST be the only record of it. A job the server no longer knows about MUST leave the tray marked as lost.
- **FR-021**: The WebUI MUST render a background report as a distinct, collapsible entry that cannot be mistaken for a message from the person, and MUST let the person open each piece's conversation from both the tray and the report.
- **FR-022**: The WebUI topic list MUST show a running-jobs count on any topic other than the open one that has background work in flight.
- **FR-023**: Only someone allowed to view the originating conversation MUST be able to see its jobs and their pieces' conversations, and to cancel them.

**Cancellation**

- **FR-024**: The agent MUST have a tool, `list_background_jobs`, that lists its own running jobs across all of its conversations, with the details in User Story 6, scenario 1.
- **FR-025**: The agent MUST have a tool, `cancel_background_job`, that cancels a running job it launched, from any of its conversations. Jobs launched by another agent, and unknown ids, MUST be refused alike.
- **FR-026**: Cancelling MUST stop every still-running piece and mark it cancelled, MUST cancel any jobs launched from inside those pieces, and MUST mark the job cancelled. A cancelled job MUST NOT deliver a report or wake any conversation.
- **FR-027**: `cancel_background_job` MUST return the results of pieces that had already answered, in task order, together with the cancelled entries. Cancelling a job that has already finished MUST change nothing and say so.
- **FR-028**: The WebUI tray MUST offer a cancel control per running job, with a confirmation step. A cancel from the tray MUST behave as FR-026 and MUST record in the conversation that the person cancelled the job. It MUST NOT wake the agent.
- **FR-029**: A job MUST end in exactly one of reported or cancelled, even when a cancel and the last piece's completion coincide.

### Key Entities

- **Background job**: one hand-off of work that will report back later. It is either a whole `paralel_subtasks` batch or a single `delegate_agent` call. Attributes: an identifier quoted in the tool's acknowledgement; the conversation it reports back to; its nesting depth; its start time and time limit; and its pieces.
- **Job piece**: one unit of work within a job (a subtask, or the single delegation). Attributes: the prompt, who runs it (this agent or a named agent), its final state (answered, failed, timed out or cancelled), and its result text or failure reason.
- **Background report**: the message delivered into the originating conversation when a job finishes. It holds the job identifier and one entry per piece in original order, and is marked as a background result, not a human message.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A `paralel_subtasks` call returns to the agent within 1 second, regardless of how long its tasks take.
- **SC-002**: While a batch is running, a person's new message in the same conversation is answered without waiting for the batch, in the same time it would take with no batch running.
- **SC-003**: Every background job that finishes without being cancelled (100% of batches and delegations) produces exactly one report in its originating conversation: none lost, none duplicated. A cancelled job produces none.
- **SC-004**: In 100% of reports, every task in a batch appears in its original position, including tasks that failed or timed out.
- **SC-005**: Where a person started the work in the WebUI, the agent's follow-up reply reaches them in that same topic, with no further message from them.
- **SC-006**: A chain of agents delegating to each other stops at the nesting limit, rather than running without end.
- **SC-007**: In the WebUI, a newly launched job appears in the tray, and a piece's state change shows there, within 2 seconds, with no refresh.
- **SC-008**: The tray shows the same jobs and states after a reload as before it, and a finished job is never shown as running.
- **SC-009**: A person can get from a running or finished job to any piece's full conversation in at most two clicks.
- **SC-010**: After a cancel, whether by the agent or from the tray, no piece of the job is still running within 5 seconds, and no report for it ever arrives.

## Assumptions

- **One report per batch, not one per task.** A batch wakes the agent once, when it is complete. This keeps one fan-out from costing N extra turns and matches how the tool's results were consumed when it was blocking. Per-task streaming is out of scope.
- **`consult_agent` stays blocking.** The request covers only the non-blocking tools, and `consult_agent` exists to get an answer within the current turn.
- **Outstanding background work does not survive a server restart.** Work cut off by a server restart is lost and recorded as interrupted. An agent respawn is handled as described under Edge Cases. A later feature could resume outstanding jobs.
- **Cancel, not retry.** The agent and the person can cancel a job, but neither can retry it from the tray. Retrying means launching a new job. Cancelling is per job, not per piece.
- **A person's cancel is not reported to the agent.** A cancel from the tray is recorded in the conversation for the person to see, but it does not wake the agent, and it is not part of what the agent sees on later turns. The person cancelled it, so they already know.
- **Discord and Telegram show nothing of this feature in this version.** They show no progress, no report, and no woken reply. The agent is still woken and its reply is kept in history. Rendering on those channels is deferred to a later feature.
- **Who can be delegated to is unchanged.** Any agent the delegating agent can already see may be delegated to. This feature changes only whether the outcome comes back.
- **Default limits** (nesting depth 3, time limit 10 minutes per piece, a per-result size cap) are starting values. They should become per-agent configuration only if real use shows they need tuning.
- **Validation** follows the project's end-to-end rule for agent-observable changes: tool calls run through the real agent loop on the offline `dummyplug` provider.
