# Feature Specification: Triggers

**Feature Branch**: `071-triggers`

**Created**: 2026-11-27

**Status**: Draft

## Clarifications

### Session 2026-11-27

- Q: Which tools may a new webhook's runs use? → A: Reading memory (not writing it), which a person
  may widen. Sending messages was dropped from the default when Vizier took over delivering results
  (FR-046). A monitor a person creates starts with all the agent's tools except messaging and waking
  (FR-030).
- Q: Does a webhook an agent creates need approval? → A: Yes. It starts inactive (FR-036).
- Q: Who may manage triggers, and who may create public webhooks? → A: Each role says what it
  may do, through its own trigger permissions. By default only administrators hold them
  (FR-038).
- Q: Whose permissions apply to a request from Discord or Telegram? → A: Those of the user the
  sender's account is linked to (`/connect`, spec 069). An ID typed into a profile counts for
  nothing. (First decided as the typed ID; changed when linking arrived.) (FR-041).
- Q: How is it decided what reaches the person? → A: An optional triage by a model chosen per
  trigger, which may be a free or cheap one, judges each event. The hourly limit of 20 counts only
  runs for important events, and each run delivers at most once (FR-042 to FR-047).
- Q: Who receives a trigger's results? → A: The person who set the trigger up, not everyone who can
  see the agent (FR-046).
- Q: What happens when a trigger's queue is full? → A: Each trigger has its own queue and triage,
  and each kind decides: a webhook drops its oldest waiting event, a monitor refuses the new one so
  it reports it again later (FR-015).
- Q: How does a run say there is nothing worth sending? → A: It ends with "nothing to report" and a
  short note. Notes and unimportant events are sent as one summary every 5 hours (FR-050).
- Q: How long are event payloads kept? → A: 3 days (FR-051).
- Q: Are the run limit and messaging fixed? → A: Both are set per trigger, for webhooks and
  monitors alike. The limit defaults to 20 runs an hour and may be raised, lowered or switched off;
  messaging is off by default for every trigger (FR-016, FR-030).
- Q: What is the minimum monitor interval? → A: 5 seconds, as the issue proposes; the default is
  60 (FR-025).

**Input**: User description: [issue #220](https://github.com/vizier-lab/vizier/issues/220) — "A
**Trigger** starts a new unattended session for an agent when something happens, rather than at a
time." The first version ships two trigger types, **webhook** and **monitor**, behind an extension
point an addon can use to add its own.

## Context

Today an agent acts unprompted only on a schedule. Much real work is event-driven: "when CI fails",
"when this page changes", "when a form is submitted". Agents fake this with tight cron polling,
which spends a model turn on every check even when nothing has changed.

A trigger sits beside a scheduled task. A task fires on a time; a trigger fires on an event. Each
firing is an event. Events that pass the filter, and triage where it is on, lead to a new unattended
session, recorded as a run in the same shape as a task run, whose reply Vizier delivers.

The first version ships exactly two kinds, which cover the two ways an event can be learned of:

- **Webhook** (push): an external system sends a request to the trigger's own address.
- **Monitor** (pull): a short script runs on an interval without a model call, and a turn starts
  only when the script reports something.

Every other source (mail, feeds, file changes) can be written as a monitor today, or added later as
a kind of its own.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An external system starts a session (Priority: P1)

A person creates a webhook trigger on an agent, with an instruction. They are given an address and a
secret, once. They configure another system with them. When that system sends a request, the agent
starts a new session, reads the instruction and the request's content, and acts.

**Why this priority**: The smallest complete trigger. It establishes the trigger, the run, the
single firing path and the treatment of outside content that everything else uses.

**Independent Test**: With an offline test agent, create a webhook, send a request with the right
secret, and check that one run exists, its session holds the framed content, and the agent replied.

**Acceptance Scenarios**:

1. **Given** an active webhook, **When** a request arrives with the right secret, **Then** the
   sender is answered at once with an accepted status and an event id, without waiting for triage
   or the turn.
2. **Given** the accepted request, **When** the turn starts, **Then** it runs in a new session of its
   own, and the agent is told which trigger fired it, who asked for the trigger, and that the
   content is untrusted data.
3. **Given** a wrong secret, a missing secret, an unknown trigger or an inactive one, **When** a
   request arrives, **Then** all four get the same "not found" answer.
4. **Given** signature verification is configured, **When** the signature does not match the exact
   bytes received, **Then** the request gets the same "not found" answer.
5. **Given** a filter on a field of the body, such as a branch name, **When** a request does not
   match it, **Then** it is accepted, recorded as ignored, starts no turn and uses no limit.
6. **Given** a delivery already seen, **When** the sender redelivers it, **Then** no second turn
   starts and the sender gets the first event's id.

---

### User Story 2 - A noisy or looping source cannot run away (Priority: P1)

A source fires far more often than expected. The person's model budget and attention are protected:
runs of one trigger never overlap, runs are rate limited, important events wait rather than vanish,
and every event that started no turn is visible.

**Why this priority**: A webhook is an open entry point that spends money. It cannot ship without
its limits.

**Independent Test**: Send many requests while a run is in progress and beyond the rate limit, and
check the number of runs, the waiting events and the recorded drops.

**Acceptance Scenarios**:

1. **Given** a run in progress and the default overlap setting, **When** important events arrive,
   **Then** they wait, and the next run takes up to 20 of them together.
2. **Given** a run in progress and the drop setting, **When** an event arrives, **Then** it starts no
   turn and the drop is recorded.
3. **Given** the rate limit is spent, **When** important events arrive, **Then** they wait and are
   handled together when the limit allows.
4. **Given** a webhook's waiting events are full, **When** another arrives, **Then** the oldest is
   dropped, counted, and the setter is told.
5. **Given** a restart, **When** the system starts again, **Then** the run is resumed or closed as
   interrupted, and no waiting event is lost.
6. **Given** a run that fails or is interrupted, **When** it closes, **Then** its events wait again
   for one more run, and if that also fails they are marked failed and the setter is told.

---

### User Story 3 - A monitor watches for a change without spending model turns (Priority: P2)

A person writes a short script that checks something, and sets how often it runs. While nothing has
changed, nothing happens and no model is called. When the script reports something, the agent starts
a session with it.

**Why this priority**: Covers every source that cannot send a webhook, and replaces cron polling. It
depends on the firing path of Stories 1 and 2.

**Independent Test**: With an offline test agent, create a monitor over a value the test controls.
Check that the first check records a baseline silently, that an unchanged value starts nothing, and
that one change starts exactly one run.

**Acceptance Scenarios**:

1. **Given** an active monitor, **When** its script returns nothing, **Then** no turn starts and the
   saved state is unchanged.
2. **Given** a script that returns a new state and nothing to fire, **When** the check ends,
   **Then** the state is saved and no turn starts.
3. **Given** a script that returns something to fire, **When** the firing is accepted, **Then** one
   turn starts with it and the new state is saved.
4. **Given** the monitor's queue is full, **When** its script fires, **Then** the event is refused,
   the state is left unchanged, and the next check reports it again.
5. **Given** a script that fails, **When** the check ends, **Then** nothing fires, the error is
   visible on the trigger, and the next check waits longer.
6. **Given** repeated failures up to the limit, **When** the last one ends, **Then** the trigger is
   marked failed, stops checking, and its owner is notified.
7. **Given** a check still running when the next is due, **When** that time comes, **Then** the next
   check is skipped, not stacked.

---

### User Story 4 - An agent sets up its own trigger (Priority: P2)

A person says "tell me when this page changes". The agent writes the monitor, tries it, creates it,
and says what it made. It can likewise create a webhook and hand the person its address.

**Why this priority**: What makes triggers feel like an assistant acting for someone. It depends on
Stories 1 to 3.

**Independent Test**: With an offline test agent, call the trigger tools to test, create, list,
update and delete a monitor and a webhook.

**Acceptance Scenarios**:

1. **Given** a monitor script, **When** the agent tests it, **Then** it sees what the script
   returned, and nothing fires and no state is saved.
2. **Given** the agent creates a webhook or a monitor, **When** the call returns, **Then** it is
   inactive until a permitted person approves it. The agent never sees a webhook's secret: the
   person approving gets a fresh one, shown that once.
3. **Given** an agent creates a trigger, **When** it is saved, **Then** the agent's owner is
   notified, and the trigger records whether the agent acted on its own or for a named person.
4. **Given** a turn started by a trigger, a dream cycle, a worker or a consultation, **When** the
   agent looks for tools that create or change triggers, **Then** they are not offered.

---

### User Story 5 - A person manages triggers and reads what happened (Priority: P3)

A person sees an agent's triggers, edits or deactivates them, reads each run and its conversation,
and for a monitor sees when it last checked, what went wrong and when it checks next.

**Why this priority**: The API serves Stories 1 to 4; the page makes them usable day to day.

**Independent Test**: Create, edit, deactivate and delete triggers through the interface, and open a
run's conversation.

**Acceptance Scenarios**:

1. **Given** a trigger with runs, **When** the person lists them, **Then** each shows its time, its
   outcome and a link to its conversation, and each event shows its verdict, including unimportant,
   waiting, dropped and ignored ones.
2. **Given** a monitor, **When** the person opens it, **Then** they see the last check, the last
   error and the next check, so a quiet monitor is told apart from a broken one.
3. **Given** a person who may manage triggers but not public webhooks, **When** they try to create
   a webhook, **Then** it is refused, while monitors remain available to them.
4. **Given** a person who may only view triggers, **When** they try to create, change, test or
   delete one, **Then** it is refused, while they can read triggers and runs.

---

### User Story 6 - An addon adds a kind of trigger (Priority: P3)

An addon contributes its own kind, such as a new-mail trigger, without any change to how triggers
are stored, fired, listed or shown.

**Why this priority**: No addon kind ships now, but the boundary is much cheaper to draw first.

**Independent Test**: Register a test-only push kind and a test-only pull kind the addon way, and
fire both end to end.

**Acceptance Scenarios**:

1. **Given** an addon installed on an agent, **When** the kinds are listed, **Then** the addon's
   kinds appear with their settings forms.
2. **Given** the addon is removed from the agent, **When** the triggers are listed, **Then** its
   triggers are inactive, not deleted.
3. **Given** a push kind from an addon, **When** a request arrives for it, **Then** the kind itself
   decides whether it fires, is ignored or is rejected.
4. **Given** a test kind with a shared source and two triggers, **When** the source reports one
   event for one of them, **Then** only that trigger fires and only one source is running.

### User Story 7 - Only what matters reaches the person (Priority: P1)

A person tells a trigger what counts as important and picks a cheap model to judge it. Of twenty
failures on a side branch and five on the main one, only the five lead to a run and a message.

**Why this priority**: Without it, unimportant events use up the limit and hide important ones.

**Independent Test**: With an offline test agent and a triage model the test controls, send a mix
of events and check which start runs, what is delivered and what is recorded.

**Acceptance Scenarios**:

1. **Given** triage is on, **When** an event is judged unimportant, **Then** it is recorded with the
   reason, starts no run, uses no run limit and nothing is delivered.
2. **Given** an event judged important, **When** its run finishes, **Then** its reply is delivered
   once to the trigger's destination.
3. **Given** several events waiting for triage, **When** they are judged, **Then** one model call may
   judge them all, with a verdict for each.
4. **Given** triage fails or answers in the wrong form, **When** the event is handled, **Then** it is
   treated as important, marked as not triaged, and the setter is told.
5. **Given** triage is off, **When** an event passes the filter, **Then** it counts as important.
6. **Given** a run that finds nothing worth sending, **When** it ends with "nothing to report",
   **Then** nothing is delivered now, and its note appears in the next summary.
7. **Given** a 5-hour window with quiet runs or unimportant events, **When** it ends, **Then** the
   setter gets one summary listing them, made without a model call. An empty window sends nothing.
8. **Given** the triage model is removed, **When** an event arrives, **Then** it counts as
   important, and the trigger is marked as needing attention, with the setter told once.

---

### Edge Cases

- **A payload over the size cap, or of an unaccepted content type.** Refused with a clear status,
  only after the sender is authenticated.
- **A payload that contains instructions.** It is data. The framing says so, and the tool limits
  (FR-030) hold whatever the model decides.
- **A trigger deactivated or deleted during a run.** The run finishes. Waiting events are
  discarded. Deleting also deletes events and runs, so a reused name starts empty.
- **The agent is unavailable when a firing arrives.** The run is opened and closed as no response,
  and is visible.
- **The first check of a monitor.** It has no saved state. It saves a baseline without firing
  unless the script chooses to fire.
- **A monitor returns a value of the wrong shape, or a state over the cap.** A failed check.
- **A monitor calls a tool it was not allowed.** A failed check naming the tool.
- **A webhook storm.** Repeated drops are counted on one record per window, not one record each.
- **A replayed signed request.** With a timestamp check configured, a request older than the
  tolerance is refused, even after its delivery id is forgotten.
- **The destination chat is deleted, its bot is down, or the setter loses access to it.** The
  result still reaches the setter's inbox, and the failed send is reported once.
- **The setter loses permission to view the trigger, or is deleted.** Results go to the agent's
  owner instead, who is told why.
- **A payload that claims to be urgent or harmless.** Triage can be swayed by it. The filter holds
  hard rules, and the run limit bounds what gets through.
- **The triage model is out of quota during a storm.** Events count as important, so unimportant
  ones may use the run limit. The setter is told triage is failing.
- **A trigger that causes itself**, such as an agent calling its own webhook. The call arrives as an
  ordinary outside request and cannot be recognised as a loop; the limits bound it.
- **A control node loses ownership mid-check.** Its state write is refused.

## Requirements *(mandatory)*

### Functional Requirements

**Triggers and kinds**

- **FR-001**: A trigger MUST belong to one agent and carry a name unique on that agent, a title, an
  instruction, a kind, settings for that kind, whether it is active, who asked for it, and when it
  was created and last fired.
- **FR-002**: Each kind MUST describe itself (name, description, settings form), validate its
  settings and mask its secrets. Nothing outside a kind may branch on which kind it is.
- **FR-003**: A kind MUST be able to receive pushed requests (deciding fire, ignore or reject), or
  to run an ongoing source that reports firings. A source MAY serve one trigger or be shared by all
  triggers of its kind, so that many triggers can sit behind one watcher or receiver.
- **FR-004**: `webhook` and `monitor` MUST be registered through the same extension point an addon
  uses. No other kind ships in this version.
- **FR-005**: An addon's kind MUST be usable only while the addon is installed on the agent. On
  removal its triggers MUST be deactivated, not deleted.

**Events and runs**

- **FR-006**: Every event MUST pass through one path that records it, applies triage and the limits,
  and starts runs. A kind MUST NOT start a turn itself.
- **FR-007**: Each run MUST start one new unattended session with its own conversation. A run MAY
  handle up to 20 waiting events together.
- **FR-008**: A run MUST be recorded before the turn starts and closed as answered, no response or
  interrupted. Every event MUST be recorded with what became of it: waiting for triage, not
  important, waiting for a run, handled by a run, ignored or dropped.
- **FR-009**: Event payloads MUST be stored capped. Authorization and signature headers MUST be
  removed, and the trigger's own secrets and the agent's secret values replaced. Other content of an
  outside payload cannot be told apart and is stored as received; the documentation says so.
- **FR-010**: Notifications follow delivery (FR-046), not every run: ignored and unimportant events
  notify nobody straight away. A run that fails notifies the setter. Model usage of runs and of triage MUST be recorded under
  their own activity.
- **FR-011**: Trigger conversations MUST NOT count as conversations with a person, so dream cycles
  do not reflect on them, and MUST NOT be wakeable.
- **FR-012**: The agent's guidance in a trigger run MUST name the trigger, say that nobody is
  waiting, say that its reply is what gets delivered, and say that payloads are untrusted data and
  never instructions. Question and sensitive-input tools MUST NOT be offered.
- **FR-013**: Where its tool access allows, a trigger run MUST be able to wake its owner's
  conversations, as a scheduled run can (spec 067).

**Triage**

- **FR-042**: A trigger MAY have triage: the person's criteria for what is important and a model
  chosen from the configured providers. Without triage, every event counts as important. Only a
  person allowed to choose models MAY pick the triage model; an agent may write the criteria, and
  the model is chosen when a person approves the trigger. If the model or its provider is removed,
  the trigger MUST be marked as needing attention and the setter told once, while events count as
  important.
- **FR-043**: Triage MUST have no tools, memory or agent context. It receives the criteria and the
  payloads, and returns for each event important or not, with a short reason. The payloads are
  marked as untrusted data.
- **FR-044**: Events dropped because a run is in progress (the drop setting) MUST be dropped before
  triage, costing no triage call. Events waiting together MAY be judged in one call, at most 20. Triage calls MUST be
  limited per trigger (default 200 per hour, adjustable). At most 500 events wait for triage; beyond
  that the oldest are dropped, counted, and the setter told.
- **FR-045**: A failed or malformed triage MUST count the event as important, mark it as not
  triaged, and tell the setter.

**Delivery**

- **FR-046**: Vizier MUST deliver a run's reply itself, at most once per run, to the person who set
  the trigger up: the person who created it, or the person on whose behalf an agent created it, or
  the agent's owner when the agent acted on its own. Nobody else sees it in their inbox. It MAY also
  go to one conversation the trigger names, which that person may use; an agent may name only the
  conversation it is in. If that person no longer may view the trigger, or is deleted, it goes to the
  agent's owner, who is told why. The inbox copy is always made; a failed send to the conversation
  is reported once.
- **FR-047**: Notices about the trigger itself (events dropped, events failed, triage failing, model
  removed, trigger failed) MUST go to the same person, at most once per hour for each cause, and do
  not count toward the limit.
- **FR-050**: A run MAY end with "nothing to report" and a short note, which delivers nothing then.
  Every trigger MUST send its setter one summary per window (default 5 hours, adjustable or off)
  listing quiet runs with their notes and unimportant events with their reasons, made without a
  model call. A window with nothing in it sends nothing. Summaries do not count toward the limit.

**Limits**

- **FR-014**: Runs of one trigger MUST NOT overlap.
- **FR-015**: Each trigger MUST have its own queue and its own triage. An important event that
  cannot run yet, because a run is in progress or a limit is spent, MUST wait; at most 100 wait.
  When the queue is full, the kind decides: a webhook drops its oldest waiting event, a monitor
  refuses the new one and keeps its state, so the next check reports it again. Drops are counted and
  the setter told. A trigger MAY instead be set to drop events that arrive during a run.
- **FR-049**: When a run fails or is interrupted, its events MUST wait again for one more run. If
  that run also fails, they are marked failed and the setter told.
- **FR-016**: Each trigger, webhook or monitor, MUST have its own limit on runs (default 20 per
  hour), which a person who may manage it can raise, lower or switch off, and an optional cooldown
  between runs. Only important events lead to runs, so the limit bounds the results that reach the
  person, provided the run's tools cannot send messages or wake conversations. The interface MUST
  warn when the limit is switched off, and when tool access allows messaging or waking, that the
  bound no longer holds. An agent MAY lower a limit but not raise or switch it off.
- **FR-017**: A trigger run MUST start one hop deep, so work it hands on stays within the existing
  hop limit. As in a scheduled run, it MUST NOT launch detached scripts.

**Webhook**

- **FR-018**: Each webhook MUST have its own address, reachable without signing in, and its own
  high-entropy secret, accepted in an authorization header or in the address. The header is the
  recommended form, since addresses are often logged.
- **FR-019**: Signature verification over the exact received bytes MUST be optional, with a
  configurable header, algorithm and secret. A timestamp check MUST also be optional: the header
  holding the time, and a tolerance (default 5 minutes). A request outside it is refused like a bad
  signature.
- **FR-020**: Secrets MUST be encrypted at rest, shown once at creation, masked afterwards, and
  replaceable.
- **FR-021**: Authentication failures, unknown triggers and inactive triggers MUST be
  indistinguishable, and misses MUST be rate limited per caller address.
- **FR-022**: Bodies MUST be capped at 256 KiB. JSON, form-encoded and text bodies are accepted.
  Only the headers named in the trigger's settings reach the agent, and never an authorization or
  signature header.
- **FR-023**: An accepted request MUST be answered at once with the event id and its outcome
  (accepted, ignored, dropped or duplicate).
- **FR-024**: A webhook MAY name a filter on headers and on fields of a JSON body, applied without a
  model before triage and the limits, and a delivery id used to recognise redeliveries
  for at least 24 hours. The id MAY be read from a header or from a field of a JSON body.

**Monitor**

- **FR-025**: A monitor MUST have a script, an interval (default 60 seconds, minimum 5, both
  configurable by the operator), and the list of tools its script may call.
- **FR-026**: The script receives the saved state, empty on the first check, and returns either
  nothing, or a record with a new state and optionally something to fire. Saved state is capped at
  64 KiB.
- **FR-027**: State returned with a firing MUST be saved only once the event is recorded as
  accepted. A refused event leaves the state unchanged. State returned without one MUST be saved at
  once.
- **FR-028**: A check MUST make no model call. Model-backed tools, turn-ending tools, subagents and
  consultations MUST NOT be callable. A check's time limit is the shorter of the interval and 30
  seconds.
- **FR-029**: A failed check MUST NOT fire or change state. The wait before the next check is the
  longer of the interval and a delay that doubles from 2 seconds up to 5 minutes. After 10
  consecutive failures (operator setting) the trigger is marked failed and the setter told.
  Editing or reactivating it clears the failure.

**Tool access**

- **FR-030**: A trigger MUST limit which tool categories its runs may use, enforced by the system.
  A webhook, and any trigger an agent creates, starts with reading memory (never writing it) only.
  A monitor a person creates starts with all the agent's tools except sending messages and waking
  conversations. A person who may manage the trigger may change either, including allowing
  messaging. Reading memory MUST be grantable separately from
  writing it.
- **FR-031**: A monitor script MUST be able to call only the tools its trigger lists. For a monitor
  an agent creates, the list starts with reading memory only, and only a person may widen it.
- **FR-032**: Tools that create, change or delete triggers MUST NOT be offered in a trigger run.

**Agents**

- **FR-033**: Agents MUST have tools to list kinds; to create, update, delete, list and test
  triggers; and to list and read runs. They form their own tool category, with a reserved name.
  Reading runs is also available to dream cycles.
- **FR-034**: Trigger tools MUST NOT be offered in dream cycles, workers or consultations, except as
  FR-033 allows. Refusals MUST be readable data, not failed calls.
- **FR-035**: A test MUST return what the script returned without firing or saving state. Tool calls
  the script makes during a test really happen, and the tool's guidance says so.
- **FR-036**: The owner MUST be notified when an agent creates a trigger. Every trigger an agent
  creates MUST start inactive until a person approves it: one who may manage triggers for a monitor,
  and one who may also create public webhooks for a webhook. If an agent then changes its
  instruction, settings, script, triage, destination or tool access, it MUST become inactive again.
  An agent MUST NOT activate a trigger.
- **FR-048**: An agent MUST have at most 20 triggers (operator setting). Further creates are refused.

**People**

- **FR-037**: People MUST be able to create, read, update, deactivate, delete and test triggers, and
  read runs and their conversations, through the API and the WebUI.
- **FR-038**: Triggers MUST have their own role permissions, each for every agent or only for
  agents the person owns: **view** (read triggers, runs and their conversations), **manage**
  (create, change, test, deactivate and delete; includes view) and **public webhooks** (create,
  activate and replace the secret of a webhook; needed besides manage). No role holds them by
  default except the administrator role. Being able to view or being shared an agent grants none
  of them. The role editor MUST list them.
- **FR-041**: An agent's trigger tools MUST NOT do more than the person behind the turn may: the
  person in a WebUI or project chat, or the agent's owner in a scheduled run. In a Discord or
  Telegram direct message it is the user who linked the sender's account (spec 069), matched on
  the ID the platform reports, never on a username, a typed profile ID or text in the message. (Implemented for
  Telegram, whose direct-message id is the sender's; Discord direct messages need the author id on
  the turn and are not offered yet.) An account nobody
  has linked gets no trigger tools, and neither does a group conversation.

**Retention**

- **FR-051**: Event payloads MUST be deleted 3 days after they arrive. The event's record (time,
  outcome, verdict, reason, count) stays, as run records do. A run's conversation keeps what the
  agent was given, as any conversation does.

**Operation**

- **FR-039**: Monitors MUST run only on the node that owns control, with their writes refused once
  ownership is lost, and MUST stop and resume with it.
- **FR-040**: Documentation MUST cover both kinds and the security of public addresses and untrusted
  payloads. Any compatibility or migration code MUST be recorded on issue #82.

### Key Entities

- **Trigger**: an agent's standing rule "when this happens, do this": instruction, kind, settings,
  limits, tool access, who asked for it, and whether it is active or failed.
- **Trigger kind**: a source of events. Describes and validates its settings, and either receives
  pushed requests or runs a source. Built in or contributed by an addon.
- **Trigger event**: one thing that happened: capped payload, what became of it, triage verdict
  and reason.
- **Trigger run**: one turn over one or more events: time, outcome, conversation, delivery.
- **Monitor state**: what a monitor remembers between checks, with its last check, last error,
  failure count and next check.
- **Delivery record**: a recently seen delivery id, used to refuse a repeat.
- **Summary**: one message per trigger per window listing quiet runs and unimportant events.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can go from nothing to a webhook that starts a session in under 5 minutes.
- **SC-002**: A webhook sender gets its answer within 1 second, however long the turn takes.
- **SC-003**: A monitor that finds nothing causes 0 model calls over any period.
- **SC-004**: 0 triggers ever have two runs in progress at once.
- **SC-005**: No trigger starts more runs, or delivers more results, in an hour than its limit, in
  100% of storm tests with the default tool access.
- **SC-006**: 100% of events are accounted for: handled, not important, waiting, ignored, dropped or
  duplicate.
- **SC-011**: 0 tool calls are ever made by triage.
- **SC-012**: 0 delivered results or trigger notices appear in the inbox of anyone but the person
  they go to under FR-046.
- **SC-013**: 0 event payloads are readable 3 days after they arrived.
- **SC-007**: 0 secrets appear in any response after creation, in run records or in logs written by
  Vizier.
- **SC-008**: 0 tool calls from a trigger run or a monitor fall outside its allowed tools.
- **SC-009**: A new kind can be added with no change to firing, storage, API or page code.
- **SC-010**: An offline test agent shows a webhook firing and a monitor firing each starting a turn
  that sees the framing and the payload.

## Assumptions

- **Changes from the issue.** This spec differs from issue #220 and these need the issue author's
  agreement: monitors always return a state record (so a baseline can be saved silently); state is
  saved only after a firing is accepted; events are recorded apart from runs, and the sender gets an
  event id rather than a run id; optional triage; Vizier delivers results to the setter, with a
  summary of quiet runs; every agent-made trigger needs approval; a source may be shared by a kind's triggers; triggers have their own permissions; tool access is
  limited per trigger; redeliveries are recognised, by header or body field; push kinds decide their own requests; and the
  depth guard is not relied on to stop webhook loops.
- A result reaches the person through delivery (FR-046). A run never happens *in* an existing
  conversation.
- A free or local triage model may keep or limit what it is sent; checking its terms is the
  operator's choice, and the documentation says so.
- Run records are kept and removed as task runs are.
- The API shape follows the issue's proposal and is settled in the plan.
- The webhook is reachable only if the operator's server is reachable by the sender. No relay or
  tunnel is provided, and the desktop app, which opens no port by default, cannot receive webhooks
  until it is configured to.
- The WebUI page is mocked up and reviewed before any UI code is written.
- **Delivery order.** (1) events, runs, limits, tool access, delivery, triage and webhooks, through the API;
  (2) monitors; (3) agent tools; (4) the WebUI.
- **Platform accounts are linked** (spec 069): only a proven link names the person behind a
  Telegram direct message.
- **Out of scope**: mail or any addon kind; firing into an existing conversation; sources with no
  interval (long poll, streaming); a script that filters webhook payloads; replaying a stored
  payload; presets for particular services; turning scheduled tasks into a trigger kind.
