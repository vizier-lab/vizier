# Feature Specification: Task Run Results, Requester and Framing

**Feature Branch**: `011-task-completion-reports`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: "currently when an agent task is done, we delete those task. also, user have any report of the task being done. thus user didn't know there's any issue/blocker/etc."

## Problem

When a scheduled task fires, the agent does the work and answers — but nobody ever sees the answer.

Two things stand in the way. A one-time task is **deleted the moment it fires**, so there is nothing left to hang a result on. And although every run's conversation is already saved, nothing connects that conversation back to the task or puts it on screen.

The result is that a task is write-only. If the agent hit a problem, was blocked on a decision, or quietly did the wrong thing, it said so — into a void.

A third gap sits next to these. A task records who wanted it in a single free-form text field —
the web form defaults it to the literal string "user", the HTTP interface takes it from the
request body rather than from who is signed in, and an agent creating a task fills it with
whatever it decides. The field cannot say whether a person asked for the task or the agent set
it up on its own initiative, which is the first thing a person wants to know when they find a
task they do not recognise.

A fourth gap decides whether any of this is worth reading. A scheduled run reaches the agent
looking exactly like a message from a person: the request is attributed to a named sender, and
nothing anywhere says the run is unattended. The agent's operating instructions even tell it to
read the channel to understand the interaction, but a scheduled run supplies nothing to read, so
the only social cue available is that sender name. Models — cheaper ones especially — infer
correctly from the evidence they are given and reply as if someone is listening: they greet the
sender, offer to do things rather than doing them, and ask a clarifying question and stop,
waiting for an answer that can never arrive. The unattended reflection cycle already gets told
what it is doing and that its output is a report. Scheduled runs get nothing.

The fix is small: stop deleting the task, record who asked for it, tell the agent its run is
unattended, and show its last response for each run.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read what a task produced (Priority: P1)

A person opens a task they scheduled and reads the agent's last response from its most recent run — in the agent's own words, including any problem or blocker the agent mentioned. The task is still there to open, whether it was recurring or a one-time task that has already fired.

**Why this priority**: This is the whole ask. A person who can read the agent's answer can see for themselves that it worked, failed, or got stuck — no separate status vocabulary needed.

**Independent Test**: Schedule a one-time task and a recurring task against an offline test agent, let both fire, then open each in the WebUI: both still exist, and each shows the agent's final response from its run.

**Acceptance Scenarios**:

1. **Given** a one-time task scheduled for a moment that has now arrived, **When** it fires and the agent answers, **Then** the task still exists, is shown as no longer scheduled, and displays the agent's last response.
2. **Given** a recurring task that has run, **When** the person opens it, **Then** they see the agent's last response from the most recent run and when that run happened.
3. **Given** a task that has not run yet, **When** the person opens it, **Then** it reads as not yet run rather than showing an empty or stale result.
4. **Given** a run in which the agent reported it could not do the work, **When** the person opens the task, **Then** they read the agent's own account of the problem without having to look anywhere else.
5. **Given** a task whose run is still in progress, **When** the person opens it, **Then** it is shown as currently running rather than displaying the previous run's result as if it were current.
6. **Given** a task a person created themselves, **When** they open it, **Then** it shows them as its requester.
7. **Given** a task an agent created on its own initiative, **When** a person opens it, **Then** it shows the agent as its requester, distinguishably from a task a person asked for.

---

### User Story 2 - Look back at earlier runs (Priority: P2)

A recurring task has run many times. The person can see the list of its past runs and open any one of them to read what the agent said that time, not only the latest.

**Why this priority**: The latest response answers "is it working now?"; the list answers "when did it start going wrong?". Valuable, but a person served by Story 1 is no longer blind.

**Independent Test**: Let a recurring task fire three times, then open it and confirm three runs are listed newest-first and each expands to its own response. With a seeded history larger than one page, confirm paging to older runs neither duplicates nor skips a run.

**Acceptance Scenarios**:

1. **Given** a recurring task that has run three times, **When** the person opens it, **Then** three runs are listed newest-first, each showing when it ran.
2. **Given** a listed run, **When** the person expands it, **Then** they can read the full exchange that run produced, and it contains only that run's exchange.
3. **Given** a task with hundreds of runs, **When** the person opens it, **Then** only the first page of recent runs loads, and the page appears as quickly as for a task with three runs.
4. **Given** the first page of runs is shown, **When** the person asks for more, **Then** the next older page is appended, with no run duplicated across pages and none skipped between them.
5. **Given** several runs that finished within the same instant, **When** they fall on a page boundary, **Then** none of them is lost or repeated.
6. **Given** the oldest run has been reached, **When** the person asks for more, **Then** they are told there are no further runs rather than being offered an empty page.

---

### User Story 3 - See each task's last run in the list (Priority: P3)

In the task list, each row shows when it last ran and what state that run reached, so a person scanning the list can tell which tasks are running, which have never run, and which finished without the agent answering.

**Why this priority**: Convenience on top of Stories 1 and 2 — the information is already reachable, this just saves opening each task to find out whether it ran.

**Independent Test**: With tasks in each state present, open the task list and confirm every row shows its last run time and state, and that never-run, running, and finished-without-a-response are each distinguishable.

**Acceptance Scenarios**:

1. **Given** several tasks that have run, **When** the person opens the task list, **Then** each row shows when it last ran and the state that run reached.
2. **Given** a task that has never run, **When** it is listed, **Then** it reads as not yet run rather than as blank or an error.
3. **Given** a task whose run finished without the agent answering, **When** it is listed, **Then** it is distinguishable from one whose run produced a response.

### User Story 4 - An agent can read its own task reports (Priority: P2)

An agent can ask for the reports of a task's past runs — when each ran and what it answered — and nothing more. Not the reasoning, not the tool activity. Enough to see that a task has been failing, to avoid repeating work the last run already did, or to continue from where it left off.

**Why this priority**: Separating each run into its own conversation means a task no longer carries its own past runs as history. Without a way to ask, a recurring task starts every run with no idea what the previous one did or whether it worked — which is a regression against today's behaviour, accidental though that behaviour is.

**Independent Test**: Let a recurring task run twice against an offline test agent, then have the agent list its own task's runs and fetch one by address; confirm the list is newest-first and carries no response text, the fetch returns that run's full response, and neither carries reasoning or tool activity.

**Acceptance Scenarios**:

1. **Given** a task that has run three times, **When** the agent lists its runs, **Then** it receives the three runs newest first, each with its address, when it ran, and the state it reached, and no responses.
2. **Given** a run it has seen in that list, **When** the agent asks for that run by its address, **Then** it receives that run's full response.
3. **Given** either request, **When** the agent receives the result, **Then** it contains responses only — none of the run's reasoning or tool activity.
4. **Given** a task with hundreds of runs, **When** the agent lists them, **Then** it receives a bounded number of recent ones and can ask for older ones in a further request.
5. **Given** a run that produced no response, **When** the agent lists the runs, **Then** that run is present and marked as having produced none, so a failing task is visible rather than absent.
6. **Given** an agent asking about a task that is not its own, **When** it asks, **Then** it is refused.

---

### User Story 5 - A scheduled run answers with a report, not a reply (Priority: P1)

When a task fires, the agent is told plainly that the run is unattended: nobody is reading as it
works, no question it asks will be answered, and its final message is a report someone will read
later. Attribution says the run came from the scheduler on behalf of whoever requested the task —
or on the agent's own initiative — rather than presenting it as a message a person just sent. So
the agent decides with what it has, acts with its tools, and closes by saying what it did, what it
concluded, and what needs attention.

**Why this priority**: Co-equal with Story 1 rather than subordinate to it. Story 1 puts the agent's
last response on screen; this story is what makes that response worth the screen space. Shipping
the display alone would surface conversational filler — a greeting, an offer, a question nobody
can answer — and a person reading that still cannot tell whether their task worked.

**Independent Test**: Run a task on an offline test agent and inspect what the agent was given:
it states the run is unattended and does not name a person as the sender. Then run the same
instruction on a small, cheap model and confirm its answer reads as a report — no question
addressed to a reader, no offer to act later.

**Acceptance Scenarios**:

1. **Given** a task that fires, **When** the agent receives the run, **Then** it is told the run is unattended and that no one is reading in real time.
2. **Given** a task that fires, **When** the agent receives the run, **Then** the run is not presented as a message sent by a person; attribution names the scheduler as the origin and the requester as who it is for.
3. **Given** an instruction the agent could interpret as needing clarification, **When** it runs unattended, **Then** it proceeds on a stated assumption or reports the blocker, rather than asking a question and stopping.
4. **Given** a completed run, **When** its final message is read, **Then** it states what the agent did, what it concluded, and anything needing attention.
5. **Given** the unattended reflection cycle, **When** it runs, **Then** it keeps its own existing framing and does not also receive the scheduled-run framing.
6. **Given** a person chatting with the agent interactively, **When** they send a message, **Then** no scheduled-run framing is applied and the exchange is unaffected.

---

### Edge Cases

- **Task deleted by the person**: when a person deletes a task themselves, its run results go with it — deletion stays a deliberate act, and nothing is orphaned behind.
- **Run produced no response**: the agent errored before answering, or was unreachable when the task fired. The run must still be listed, marked as having produced no response, rather than being absent and indistinguishable from a run that never happened.
- **Server restarted mid-run**: a run that was in progress when the process stopped must not be shown as perpetually running.
- **Overlapping fires**: a recurring task's next moment arrives while the previous run is still going. The displayed result must be unambiguous about which run it came from.
- **One-time task refires**: once a one-time task has fired it must not be picked up again by the scheduler, even though its record now survives.
- **Very long response**: an agent answers at length. The task view must stay readable without hiding the response — it is the thing the person came to read.
- **Slug reused**: a person creates a new task reusing the slug of a completed one. The new task must not inherit the old one's results.
- **Reflection cycle shares the content kind**: the unattended reflection cycle issues its work as the same kind of request a scheduled task uses, while carrying its own framing. Framing keyed on the content kind rather than on the run being a scheduled task would double up on it, and must not.
- **Instruction that asks for input**: a task's instruction is itself a question for a person ("ask them what they want"). The agent must reach that person through a capability that delivers, or report that it could not — not ask into a run nobody reads.
- **No way to reach anyone**: the agent is asked to tell someone something but has no capability that reaches them. This must end as a reported outcome, not a message addressed into the void.
- **Identity document favours conversation**: an agent whose own identity document tells it to be chatty and personable still answers a scheduled run with a report. The framing changes the situation, not the character — the report may be warm, but it is still a report.
- **Requester with no account**: a person reachable only on a chat channel asks the agent to schedule something. The task must record them as that channel identifies them, not be refused for lacking an account.
- **Requester no longer resolvable**: the person a task is attributed to leaves, or their channel identity changes, while the task lives on. The task must keep running and keep showing the attribution it recorded, rather than going blank or appearing to be the agent's own initiative.
- **Agent misattributing**: an agent declares a person as requester for what was really its own initiative. This is not detectable and is accepted — the distinction exists so the agent can be honest about it, not to police it.
- **Large history**: a minutely recurring task accumulates thousands of runs. Opening the task must load only a page of recent runs, never the whole history.
- **New run during paging**: a run completes while the person is paging back through older runs. Paging must not therefore duplicate or skip a run.

## Requirements *(mandatory)*

### Functional Requirements

**Keeping and showing what a run produced**

- **FR-001**: The system MUST NOT delete a task as a consequence of running it. A one-time task that has fired MUST remain retrievable and MUST be shown as no longer scheduled.
- **FR-002**: A one-time task that has already fired MUST NOT be scheduled again.
- **FR-003**: The system MUST associate each run of a task with the conversation that run produced, so the run's result can be found from the task.
- **FR-004**: For a task's most recent completed run, the system MUST expose the agent's last response, and when that run happened.
- **FR-005**: The system MUST distinguish a task that has never run, a task whose run is currently in progress, and a task whose run completed without producing a response — none of these may be presented as a result.
- **FR-006**: The system MUST list a task's past runs, newest first, each identified by when it ran.
- **FR-007**: The run list MUST be paginated — a request returns at most one page of recent runs, and the response MUST carry what is needed to request the next older page.
- **FR-008**: Paging through runs MUST neither return the same run on two pages nor skip a run between pages, including when several runs share the same instant and when a new run completes mid-paging.
- **FR-009**: The system MUST indicate when the oldest run has been reached, so the person is not offered a further page that does not exist.
- **FR-010**: A person MUST be able to expand any listed run and read the full exchange it produced, within the task's own view.
- **FR-011**: The system MUST retain every run of a task; runs MUST NOT be discarded to bound history, since pagination already keeps the view cheap.
- **FR-012**: Deleting a task MUST also remove its run results.
- **FR-013**: A new task reusing a deleted task's slug MUST NOT show the previous task's run results.
- **FR-014**: The task list MUST show, per task, when it last ran and the state that run reached.
- **FR-015**: The task view MUST render the agent's response as readable formatted text.
- **FR-016**: Run results MUST be readable by exactly the people already permitted to view the task, and MUST NOT be reachable through a route scoped to a different person's conversations.
- **FR-017**: A failure to record or associate a run's result MUST NOT prevent the task from running or the agent from completing its work.
- **FR-018**: A run left in progress by a stopped process MUST NOT remain shown as in progress indefinitely.

**An agent reading its own reports**

- **FR-019**: An agent MUST be able to list its own task's past runs, receiving for each one an address it can use to ask for that run, when it ran, and the state it reached — the response itself MUST NOT be included in a listing.
- **FR-020**: A listing MUST be bounded and ordered newest first, and MUST let the agent ask for older runs in a further request, so a task with a long history cannot flood its context in one call.
- **FR-021**: An agent MUST be able to ask for one run by the address a listing gave it and receive that run's full response.
- **FR-022**: Neither the listing nor an individual run MUST include a run's reasoning or tool activity — responses only.
- **FR-023**: An individual run's response MUST be truncated if long enough to crowd out the agent's own work, and MUST say that it was truncated.
- **FR-024**: A run that produced no response MUST still appear in a listing, marked as such, so a failing task is visible rather than silently absent.
- **FR-025**: An agent MUST only be able to list or read runs for its own tasks.
- **FR-026**: Listing runs and reading one MUST both be available to an agent during its unattended reflection cycle, so that what its scheduled work produced can inform what it remembers.

**How a scheduled run is framed**

- **FR-027**: A scheduled run MUST tell the agent that the run is unattended and that nobody is reading it as it works.
- **FR-028**: A scheduled run MUST tell the agent that a question it asks will not be answered, and that it should proceed on a stated assumption or report the blocker instead.
- **FR-029**: A scheduled run MUST tell the agent that its final message is a report read later, and what that report should cover — what it did, what it concluded, and anything needing attention.
- **FR-030**: A scheduled run MUST NOT be attributed as though a person had just sent it. Attribution MUST name the scheduler as the origin, and the requester as who the run is for, or the agent itself where the task was its own initiative.
- **FR-031**: The framing MUST be selected by the run being a scheduled task, and MUST NOT be selected by the kind of request content — the unattended reflection cycle issues the same content kind with its own framing and MUST NOT receive this one as well.
- **FR-032**: Interactive exchanges MUST NOT receive this framing, and MUST behave exactly as they do today.
- **FR-033**: The framing MUST be identical for every run, adding no per-run variation, so that it does not defeat reuse of an unchanged instruction prefix between runs.
- **FR-034**: Where a task's instruction calls for telling or asking a person something, the agent MUST be directed to reach that person through a capability that actually delivers to them, or to report that it could not — never to address them in the run itself.
- **FR-035**: The framing MUST describe the situation of the run and MUST NOT alter the agent's own character or voice as its identity document defines it.

**Who asked for the task**

- **FR-036**: Every task MUST record a requester, which is either a named person or an agent itself.
- **FR-037**: A task created through the HTTP interface MUST take its requester from the person making the request; a requester named in the request body MUST be ignored rather than honoured.
- **FR-038**: An agent creating a task MUST be able to declare the requester as either itself, when the task is its own initiative, or a person, when it is acting on that person's behalf.
- **FR-039**: A requester naming a person MUST record that person as the originating channel identifies them, and MUST NOT require them to hold an account on this system — a person reachable only through a chat channel is a valid requester.
- **FR-040**: Tasks that predate this feature MUST be migrated to carry a requester derived from their existing attribution, losing nothing that was recorded.
- **FR-041**: The task view and the task list MUST show the requester, distinguishing a person from an agent's own initiative.
- **FR-042**: A recorded requester MUST remain displayable as recorded even when that person can no longer be resolved, and MUST NOT be presented as blank or as the agent's own initiative.

### Key Entities *(include if data involved)*

- **Task**: an instruction scheduled against an agent, recurring or at a single moment. It is no longer destroyed by being run; a fired one-time task persists, marked as no longer scheduled. It records a **requester**.
- **Requester**: who wanted the task — either a named person, or an agent acting on its own initiative. Replaces today's unvalidated free-text attribution. Which of the two it is, and which person, is decided by the system for tasks created by a person and declared by the agent for tasks it creates itself.
- **Task Run**: one firing of a task — when it ran, and the conversation it produced. Belongs to exactly one task. The agent's last response in that conversation is the run's result. Its **state** is one of:
  - **running** — started and not yet finished.
  - **answered** — finished, and the agent produced a response. This includes a run whose response reports that the agent could not do the work; that news is in the response, not the state.
  - **no response** — finished without the agent answering at all, because it errored or could not be reached.
  - **interrupted** — left unfinished by a process that stopped, and resolved on restart.

  A task with no runs is *not yet run*, which is the absence of runs rather than a state of one.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero tasks are removed from the system as a side effect of running.
- **SC-002**: 100% of task runs are reachable from their task afterwards, including runs that produced no response.
- **SC-003**: A person can read what a task most recently produced in under 10 seconds from opening the task list.
- **SC-004**: A person reading a failed or blocked run's result can state what went wrong from the agent's own response, without reading logs, in the large majority of cases.
- **SC-005**: Recording and association add no observable delay to the agent starting work on a task.
- **SC-006**: Opening a task that has run thousands of times loads its first page of runs as quickly as one that has run twice.
- **SC-007**: Paging through the entire run history of a task yields each run exactly once.
- **SC-008**: Every task in the system names a requester that is either an agent or an identified person — zero tasks carrying a placeholder attribution such as the literal string "user".
- **SC-009**: A person finding a task they do not recognise can tell whether a person asked for it or an agent set it up, without leaving the task view.
- **SC-010**: An agent can determine whether its own recurring task has been failing, and for how long, in a single request.
- **SC-011**: Reading the reports of a task with a thousand runs returns a bounded amount of text — the request cost does not grow with the length of the history.
- **SC-012**: An agent reflecting on its recent work can reach what its scheduled tasks produced, where today that is the one category of its own work it cannot review.
- **SC-013**: Zero scheduled runs present themselves to the agent as a message sent by a person.
- **SC-014**: Across a set of ordinary task instructions run on a small, inexpensive model, the large majority of responses read as reports — no question addressed to a reader, and no offer to act later instead of acting.
- **SC-015**: Interactive conversations and the reflection cycle are unchanged by this feature — their responses are indistinguishable from before it.

## Assumptions

- Every run's conversation is already persisted by the existing execution path; this feature connects it to the task and puts it on screen rather than capturing anything new.
- Showing the agent's own last response is the report. No separate success/failure/blocked status is introduced — if the agent hit a problem it says so in its answer, and a derived status would only be a guess layered on top of text the person can read directly.
- A fired one-time task is kept and marked as no longer scheduled rather than deleted. Automatic deletion is what caused the reported loss, so nothing here reintroduces it; the person may still delete it explicitly.
- The WebUI is where results are read. Nothing is pushed to the person — no notification, email, or chat message. If that is wanted it is a later, separate feature built on the records this one creates.
- Separating runs into their own conversations removes continuity a recurring task currently gets by accident, because today every run shares one conversation and so replays all previous runs. That accident is also an unbounded cost — a frequently recurring task's context grows forever. Reading past reports deliberately replaces it, which is why it belongs in this feature rather than a later one.
- An agent reads responses, not trails. Its own previous reasoning and tool calls are the bulk of the text and the least use to it; what it needs is what happened and what came of it.
- An agent lists runs and then asks for one, rather than receiving every response at once. The same two-step shape the memory tools already use — a search that returns addressed results, and a read that returns one whole thing by address — for the same reason: one call must not be able to spend the whole context.
- A listing carries no response text at all, in either surface. A preview would be leading characters of prose, which is a poor signal — a run that went wrong can open cheerfully — and it would put a second, lossy copy of the response in a place people and agents would read as authoritative. The state says whether a run is worth opening; the response is read by opening it.
- Reading past reports IS available during the dream cycle, both the listing and the individual read. The cycle does not consider task conversations at all — they are filtered out of the sessions it reflects on — so without these, a task's outcomes are the one kind of work an agent does that it can never learn anything from. Giving it the same two tools it has while awake is a smaller change than widening what the cycle reflects on, and it puts task outcomes on the same footing as everything else the agent remembers.
- Every run is kept, and the run list is paginated rather than trimmed. Session history already grows unbounded for every other channel, so trimming task runs alone would be inconsistent as well as surprising — a person looking for the run where something first broke should still find it.
- A run's full exchange is read inside the task's own view. The existing chat surface addresses one person's web conversations only and cannot name a task run, so reusing it would mean widening a person-scoped route to reach sessions that are not theirs.
- Existing scheduling behaviour is unchanged — how schedules are expressed, when tasks fire, and who may create them all stay as they are.
- The framing belongs with the agent's standing operating instructions for a scheduled run, not appended to the task's instruction text. Keeping it out of the per-run body is what lets it stay identical between runs, and the instruction a person wrote stays the instruction the agent reads.
- The framing is selected by the kind of run, not by the kind of request content, because the reflection cycle issues the same content kind and already frames itself. This is the one place where the obvious implementation is the wrong one.
- Framing tells the agent about its situation, not about itself. An agent's character comes from its identity document and is untouched; a report from a warm agent is still a report.
- Attribution depends on the requester this same feature introduces, which is why the two ship together: there is no honest thing to attribute a run to until a task records whether a person asked for it.
- A single requester field is enough; how the task entered the system is not stored separately. The system already knows the call path at creation time, so recording it as well would be storage for something derivable.
- The agent decides *whether* a task is its own initiative or on someone's behalf, because only it knows. The system sets the person when the task is created over HTTP, where it already knows who is signed in.
- A requester who is a person is recorded as whatever identity the originating channel supplies, and is not checked against accounts on this system. Accounts exist for web sign-in only, while most people reach an agent through a chat channel and have none; requiring one would refuse the most ordinary case there is. The identity is therefore descriptive rather than verified, which is accepted for now.
- An agent declaring a person as requester is taken at its word. There is no way to check it, and the enum exists so the agent can distinguish its own initiative from someone's request — not to prevent it misattributing.
- Forcing the requester to the authenticated person is a **breaking change** to the task-creation interface: a caller that today passes an arbitrary name will have it ignored. The field is currently unvalidated and commonly holds the literal string "user", so little of value is lost.
- Migrating existing tasks attributes each to the person named in its current field. Nothing recorded today can distinguish a task an agent set up on its own initiative, so those become person-attributed too; the imprecision is accepted rather than guessed at.
- Whether the reflection cycle runs at all still depends on there having been conversations to reflect on — an agent that only runs scheduled tasks and talks to nobody does not currently reflect, so these tools would go unused by it. That gating is pre-existing and is not changed here.
- Out of scope: changing how the reflection cycle frames its own work, framing for agent-to-agent or sub-agent runs, automatic retries, resuming a run, task chaining, pushing results anywhere outside the WebUI, any derived status or health scoring of tasks, reworking the existing chat view to open non-web sessions, and notifying the requester when a run needs attention. A requester is recorded and displayed here; **delivering** to one would need a structured, channel-qualified identity rather than today's descriptive string, and that structure should be designed by the feature that needs it rather than guessed at now.
