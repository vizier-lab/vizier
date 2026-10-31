# Feature Specification: Asking the Person — Structured Questions and Sensitive Input

**Feature Branch**: `042-agent-asking-question`

**Created**: 2026-11-01

**Status**: Draft

**Input**: User description, captured from the conversation: "i want to tackle this, basically it allow agent to ask question in structured manner, be it a text prompt, multiple choice, a selection. also allow to ask multiple quetions at once. this will be a universal turn-ending tool. the webui handling would be obviuous by rendering a custom ui. but i am still not sure about other channel such as discord and telegram" — and, later: "ok let's add another tool `ask_secret` (may need better name, suggest me) what it does is allow agent to ask for sensitive data, this tool not necessarily save it to secret that's why we need better name, this tool also use the same scrub mechanism as get secret, can we?" — settled as `ask_sensitive_input`, not turn-ending, shown as a WebUI dialog with a reject action, where closing the dialog also declines.

## Context

An agent that needs something from a person has two needs and, today, only prose to meet either. It can ask a question in its reply and end its turn, leaving the person to answer in free text the agent must interpret — easy to misread, impossible to validate, and offering no choices. Or it needs a credential, and asks for one in prose, at which point the person's answer becomes an ordinary message: stored in the conversation, sent to whatever provider the agent uses, shown to anyone who can read the topic, and replayed into every later prompt. `set_secret` does not help, because the person still has to paste the value into chat for the agent to pass on.

This feature adds one family of two tools, one for each need:

- **`ask_question`** asks one or more structured questions — free text, choose one, choose several, or confirm — and **ends the turn**: the questions are the agent's answer, and the person's answers arrive in the agent's next turn as structured data tied to the question that asked for them. It is universal: the WebUI draws a card, and Discord and Telegram present the same questions with their own controls.
- **`ask_sensitive_input`** asks for one sensitive value and **does not end the turn**: the turn stays open, the value goes to the script that is already running, and it never enters the conversation, the model's context, the logs or the telemetry. It is WebUI-only, and in the WebUI it is a dialog with a reject action, where dismissing the dialog also declines.

The two differ on turn-ending because of where the answer has to go, not for want of consistency. A structured answer is content: it is safe to keep, so it becomes the person's next message and takes the ordinary path through history and the queue. A sensitive value must never be content anywhere, so it has to be consumed by the turn that is already running — the only turn whose report, history entry, model copy, log line and telemetry are scrubbed by the secret redaction that `get_secret` already relies on. A turn-ending ask would hand the value to the *next* turn, whose context is precisely the copy that must stay clean.

Channels follow the same logic. A question is safe to keep, so it is asked everywhere a person can answer. A sensitive value is not: a value sent through Discord or Telegram is stored on that platform's own side, readable by its administrators, and beyond vizier's reach — so there the tool is absent and the agent asks in prose as it does today.

This builds on the turn-ending tool mechanism introduced with generative UI (spec 026), the secret redaction introduced with agent secrets (spec 027), and the conversation event path used by interactive interfaces (spec 019). `ui_render` remains the tool for dashboards, forms, games and other rich interfaces; these tools are only for asking. Approval and permission prompts for tool execution are **not** part of this specification; they belong to the guardrails work in issue #119.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One structured question, one structured answer (Priority: P1)

A person asks an agent to "set up the database" without saying which one. The agent asks one question with three choices (PostgreSQL, SQLite, MySQL), the turn ends, and the WebUI shows a question card as the reply. The person picks SQLite and submits. The card becomes a read-only record of the answer, and the agent's next turn receives "SQLite" tied to the question it asked, so it can continue without guessing.

**Why this priority**: This is the smallest complete slice of the question tool: the agent asks, the person answers, the agent continues. Everything else builds on it.

**Independent Test**: On the WebUI, ask an agent to make an ambiguous choice; confirm the agent calls the question tool, the turn ends immediately, the card appears, a choice can be submitted, the card becomes read-only, and the next turn carries the answer.

**Acceptance Scenarios**:

1. **Given** an interactive conversation, **When** the agent asks one question, **Then** the turn ends without another model round, and the question card is shown as the agent's reply.
2. **Given** a question card with choices, **When** the person selects one and submits, **Then** the answer is recorded as the person's next turn and the agent receives the question's id and the selected value.
3. **Given** a free-text question, **When** the person types an answer, **Then** the text is delivered as answer data, not interpreted as a message, command, or markup.
4. **Given** a required question with no answer, **When** the person submits, **Then** the submission is refused, the card says which question is missing, and nothing is sent to the agent.
5. **Given** an optional question, **When** the person submits without answering it, **Then** the submission is accepted and the agent sees that question as skipped.
6. **Given** a question card, **When** the person dismisses it, **Then** the agent receives a declined outcome for that set and can continue.
7. **Given** a question card, **When** the person reloads, opens a second tab, or opens the conversation on another device, **Then** the same card and its state are shown.
8. **Given** an invalid question set (duplicate ids, a choice with no options, too many questions), **When** the agent asks it, **Then** the call is refused with every problem named, the turn continues, and nothing is shown to the person.

---

### User Story 2 - Several questions in one card (Priority: P1)

A person asks the agent to scaffold a project. The agent asks three questions at once: a project name (free text), a database (choose one), and the features to include (choose several). The WebUI shows one card with all three in order; the person fills them in and submits once. The agent receives all three answers in one turn.

**Why this priority**: Asking one question per turn would make the person wait through several model rounds for a single setup decision; the request explicitly asks for multiple questions at once.

**Independent Test**: Ask an agent for a task that needs three answers; confirm one card holds all three, one submission sends all three, and a required question left blank blocks the whole submission with an inline message.

**Acceptance Scenarios**:

1. **Given** a question set with several questions, **When** it is shown, **Then** all questions appear together in the order the agent gave them, each with its prompt, help text, options and required/optional state.
2. **Given** several questions, **When** the person submits, **Then** all answers are sent in one turn, not one turn per question.
3. **Given** a question set with optional questions, **When** some are left blank, **Then** the agent receives the answered ones and an explicit skipped state for the others.
4. **Given** a submission with one invalid answer (a choice not offered, a text over the limit, too few or too many selections), **When** the person submits, **Then** the whole submission is refused, the problem is shown on that question, the other answers are kept, and nothing is sent.
5. **Given** more questions than the documented limit, **When** the agent asks them, **Then** the call is refused with the limit and the turn continues.
6. **Given** a question set with several questions, **When** the person answers it, **Then** the card in history shows every answer and every skipped question.

---

### User Story 3 - Questions work on every channel, with that channel's own controls (Priority: P1)

A person talks to the same agent from the WebUI, Discord and Telegram. The agent asks which database to use and which features to include. In the WebUI the question card appears; on Discord the question is a message with a button per option and a multi-select for the feature list; on Telegram it is the same with an inline keyboard. The person presses what they want instead of typing it, and the answer arrives as their next turn. The agent receives the same answer shape everywhere and never needs to know which channel was used.

**Why this priority**: The request calls the question tool "universal", and the chosen channel approach is native controls. If answering meant typing a syntax on Discord or Telegram, the tool would be a worse version of prose questions there.

**Independent Test**: Ask the same two-question set from each supported channel and answer it with the native controls; confirm the answers reach the agent with the same question ids and values, the question message turns into a record of the answer, and pressing a control again is refused. Restart the server with the set still pending and confirm the controls still work.

**Acceptance Scenarios**:

1. **Given** the WebUI, **When** the agent asks a question, **Then** the custom card is drawn (see FR-021).
2. **Given** a Discord conversation, **When** the agent asks a choose-one question, **Then** the options appear as buttons on the question message, and pressing one answers that question.
3. **Given** a Discord conversation, **When** the agent asks a question with more options than one row of buttons holds, **Then** the platform's select control is used, and every option is still reachable.
4. **Given** a Telegram conversation, **When** the agent asks a choose-one or confirm question, **Then** the options appear as an inline keyboard, and pressing one answers that question.
5. **Given** a free-text question on Discord or Telegram, **When** the person answers it, **Then** they do so through the platform's own text entry, opened from the question, without typing a command or a syntax.
6. **Given** a press on a control, **When** the platform asks vizier to acknowledge the interaction, **Then** it is acknowledged in time and the person sees the answer registered rather than a failed interaction.
7. **Given** a question set answered or declined, **When** the question message is shown again, **Then** it shows the answers (or the decline) and its controls no longer accept a press.
8. **Given** a stale control (already answered, or another person's answer won), **When** it is pressed, **Then** the person gets a short notice, no turn starts, and nothing is recorded.
9. **Given** a pending question set, **When** the bot reconnects or the server restarts, **Then** its controls still answer the set.
10. **Given** a set that exceeds the platform's native limits, **When** it is presented, **Then** it falls back to the documented text form (FR-027) with every question, its order and its id intact, and one submission still answers the whole set.
11. **Given** an answer from any supported channel, **When** it reaches the agent, **Then** it carries the same question set id, question ids, values, skipped/declined state, and answerer as a WebUI answer.
12. **Given** a channel that cannot present or collect answers, **When** a turn runs there, **Then** the question tool is not offered, and the agent answers in text as it does today.
13. **Given** a question that cannot be delivered (the channel send fails, the conversation is gone, or the turn is aborted first), **When** the agent asks it, **Then** the call fails, the turn does not end, and the agent gets an error it can act on.

---

### User Story 4 - The agent gets a credential without ever seeing it (Priority: P1)

A person asks an agent to file an issue in their tracker. The agent needs an API token. It asks for one, naming what it is for and why it is needed. The turn keeps running; a dialog opens with a masked field, the person pastes the token and confirms. The agent's script receives the value and uses it to make the call. The agent's own reasoning, the transcript, the logs and the telemetry show `[redacted]` where the value was.

**Why this priority**: This is the whole point of the sensitive-input tool. Without it there is no safe path from "the agent needs a credential" to "the agent has it".

**Independent Test**: Have an agent ask for a token, supply one, and use it in a request to a local echo endpoint. Confirm the request carried the real value, and that the value appears nowhere in the conversation history, the model's context, the logs or the telemetry.

**Acceptance Scenarios**:

1. **Given** an agent running a script that needs a credential, **When** it asks for one, **Then** a dialog opens with a masked field naming what is requested and why, the turn stays open while it is up, and it does not end when the ask is answered.
2. **Given** a value supplied through that input, **When** the script receives it, **Then** the script holds the real value and everything downstream of the sandbox — report, history entry, model copy, log, telemetry — shows `[redacted]` in its place.
3. **Given** a script that passes the value to another tool (an HTTP request's header, a secret-setting call), **When** that call is recorded, **Then** its arguments are scrubbed the same way.
4. **Given** a script that prints the value or puts it in its result, **When** the report is produced, **Then** the printed and returned copies are scrubbed and the script still completed normally.
5. **Given** the ask succeeded, **When** the turn ends and the conversation is reopened later, **Then** the transcript shows that a value was requested and provided, with `[redacted]` in place of the value, and never the value itself.
6. **Given** the ask succeeded, **When** the person looks at the conversation afterwards, **Then** the dialog is closed and the value is not shown back to them.

---

### User Story 5 - A supplied value stays out of every copy the system keeps (Priority: P1)

A security-minded operator reviews what Vizier retains. They check the conversation history, the running-step records, the log output, the telemetry spans, the notification inbox and the model's own context. A value supplied through the sensitive-input tool appears in none of them; only the marker does.

**Why this priority**: A secret that leaks into one of these copies is as good as published, and the transcript is replayed to the model on every later turn.

**Independent Test**: Supply a distinctive value, then grep the conversation history, the step records, the log output and the exported telemetry for it. Confirm zero occurrences, and confirm the marker is present where the value was used.

**Acceptance Scenarios**:

1. **Given** a supplied value, **When** the conversation history is read through the normal listing and through the running-step trail, **Then** the value does not appear.
2. **Given** a supplied value, **When** logs and telemetry are exported with content capture on, **Then** the value does not appear.
3. **Given** a supplied value, **When** the next turn's context is built, **Then** it contains the marker, not the value.
4. **Given** a supplied value, **When** notifications and the reply's statistics are produced, **Then** neither carries it.
5. **Given** a supplied value, **When** the person reloads the topic or opens it in another tab, **Then** the closed ask shows no value.

---

### User Story 6 - The agent knows when and how to ask a question (Priority: P2)

An agent that is unsure what the person wants asks a structured question instead of guessing; an agent doing unattended work does not try to ask at all. The question tool's own description tells the agent what it is for, which question types exist, how many questions it can ask, that the call ends the turn, what the answer will look like, and where it is unavailable.

**Why this priority**: A tool the agent misuses is worse than no tool; the description is the whole interface the agent has.

**Independent Test**: On an agent with no skill installed, give it a prompt with an obvious ambiguity; confirm it discovers the tool, asks a valid question, and that the answer arrives with the expected shape. Run a scheduled task or dream and confirm the tool is absent.

**Acceptance Scenarios**:

1. **Given** an interactive turn, **When** the agent lists its tools, **Then** the question tool is discoverable through the normal listing and description path, with a description that covers its purpose, types, limits, turn-ending behavior, and answer shape.
2. **Given** the agent reads the tool's description, **Then** the standard turn-ending notice is present exactly once and the author's description limit is not reduced by it.
3. **Given** a dream, scheduled task, background piece, inter-agent turn, or side chat, **When** the agent lists tools, **Then** the question tool is not offered; a direct call there fails with a clear message and does not end the turn.
4. **Given** the agent writes explanatory text before asking, **When** the turn ends, **Then** that text is kept and shown with the question card.
5. **Given** an answered question set, **When** the agent's next turn starts, **Then** it sees the question set id, each question, the answer or skipped/declined state, the option labels and values, and who answered.
6. **Given** a later turn, **When** the agent needs another decision, **Then** it can ask a new question set; the earlier set is not reused.
7. **Given** the agent has already asked one question set in this turn, **When** it asks another, **Then** the second call is refused and the first set remains the turn's answer.

---

### User Story 7 - The agent knows when it may ask for a sensitive value (Priority: P2)

An agent considers asking for a token, and can tell from the tool's own description what it is for, that the value will never be visible to it, that the person may reject the dialog or let it time out, that the call does not end the turn, what the length limits are, and that it is only available in the WebUI. A scheduled task or a Discord conversation does not offer the tool at all, so the agent there falls back to explaining what it needs.

**Why this priority**: An agent that misuses the tool either leaks a value or wastes a person's attention; the description is the whole contract it has.

**Independent Test**: With no skill installed, give the agent a task needing a credential; confirm it discovers the tool, asks with a clear purpose, and uses the returned value through another tool. Run the same task from Discord and from a scheduled run; confirm the tool is absent.

**Acceptance Scenarios**:

1. **Given** a WebUI conversation, **When** the agent lists its tools, **Then** the sensitive-input tool is discoverable through the normal listing and description path, in the same category as the other secret tools.
2. **Given** the tool's description, **Then** it states that the value goes to the script only, is never shown to the agent or the transcript, that the person may reject the dialog or let it time out, that the call does not end the turn, the length limits, that redaction is exact-string only, and where the tool is unavailable.
3. **Given** a conversation on a channel without a secure input, **When** the agent lists tools, **Then** the sensitive-input tool is not offered.
4. **Given** a dream, scheduled task, background piece, inter-agent turn or side chat, **When** the agent lists tools, **Then** the sensitive-input tool is not offered; a direct call there fails with a clear message.
5. **Given** an ask that succeeded, **When** the agent needs another value later in the same turn, **Then** it may ask again.

---

### User Story 8 - The person can decline a sensitive-input ask, and the agent copes (Priority: P2)

The agent asks for a password it does not actually need. The person rejects the dialog — or simply closes it. The script's call returns a declined result rather than hanging or failing the turn, so the agent can explain what it could not do and finish.

**Why this priority**: A blocking ask is only acceptable if refusing it is a normal outcome, not a broken turn.

**Independent Test**: Decline an ask; confirm the script receives a declined result, the turn finishes normally, and the transcript records the refusal.

**Acceptance Scenarios**:

1. **Given** an open dialog, **When** the person presses reject or dismisses it, **Then** the script receives a declined result it can branch on and the turn continues.
2. **Given** an open ask, **When** the person does not answer before the wait limit, **Then** the call ends with a timeout result the script can branch on, and the turn continues.
3. **Given** a declined or timed-out ask, **When** the agent asks again in the same turn, **Then** a new ask is shown; the earlier one stays closed and cannot be answered.
4. **Given** an ask the person answers after it already timed out, **When** they submit, **Then** the submission is refused with a short notice and nothing is delivered to the agent.
5. **Given** a declined ask, **When** the turn ends, **Then** the transcript shows the refusal and no value.

---

### User Story 9 - Questions are safe, accessible, and part of history (Priority: P2)

A person using only a keyboard can answer a question; a screen reader names every question and control; the card works in light and dark themes and on a phone. Answers are treated as untrusted text: a choice answer must be one of the offered options, free text is never rendered as markup or run as a command, and a question is answered once. Pending questions survive a reload or a server restart, and answered or declined cards stay in the conversation as a record.

**Why this priority**: Questions collect input from people; validation, accessibility, and history are not optional polish.

**Independent Test**: Complete a three-question set with the keyboard only and with a screen reader; try an answer not in the options and an over-limit text; reload and restart the server while a question is pending; confirm the card is still answerable and the history shows the final state.

**Acceptance Scenarios**:

1. **Given** a choice question, **When** an answer is not one of the offered options, **Then** the submission is refused and the card stays open.
2. **Given** a free-text question, **When** the answer exceeds the limit, **Then** the submission is refused with the limit and the card stays open.
3. **Given** an answer containing markup, script, or a command-looking string, **When** it is shown or delivered, **Then** it is treated as text and nothing is executed.
4. **Given** a question card, **When** the person uses only the keyboard, **Then** every question and control can be reached, operated and submitted; focus moves to the first problem on a refused submission.
5. **Given** a screen reader, **When** the card is read, **Then** each question's prompt, help text, required state, options and errors are announced with their controls.
6. **Given** a question set, **When** the person answers it, **Then** the card becomes read-only; a second submission is refused and does not start a turn.
7. **Given** a pending question, **When** the server restarts, **Then** the question is still shown and still answerable.
8. **Given** an answered or declined question, **When** the conversation is reopened, **Then** the card shows the answers or the declined state.
9. **Given** a conversation or agent is deleted, **When** it is removed, **Then** its question sets and answers are removed with it.
10. **Given** a person who cannot access the conversation, **When** they try to answer, **Then** the answer is refused.

---

### User Story 10 - The sensitive-input boundaries are stated, not implied (Priority: P2)

A person relies on the tool for a four-digit PIN and for a token they intend to re-encode. The system tells them, and the agent, what redaction can and cannot do: values shorter than the redaction minimum cannot be scrubbed, and a value the script transforms or writes elsewhere is beyond the guarantee.

**Why this priority**: The feature's value is the honesty of its guarantee. A promise that silently fails for short values is worse than no tool.

**Independent Test**: Ask for a value shorter than the minimum and confirm the ask states that it cannot be protected; supply a long value and have the script write it into a file and confirm the system does not claim it was protected.

**Acceptance Scenarios**:

1. **Given** an ask, **When** the person supplies a value shorter than the redaction minimum, **Then** the ask states plainly that a value this short cannot be scrubbed from the report and lets them decide.
2. **Given** the agent's description, **Then** it says that a transformed form of the value (encoded, hashed, split) is outside the guarantee.
3. **Given** a script that writes the value to a file, memory, a scratch file or an artifact, **When** the turn ends, **Then** the system does not claim the value was protected there.
4. **Given** the agent's description, **Then** it tells the agent to pass the value to another tool rather than print, log or store it.

---

### Edge Cases

**Questions**

- The agent asks a question while another turn is running: the question is part of the running turn's reply; the person's answer queues behind that turn like a typed message.
- The agent asks a question and the person sends an unrelated typed message instead: the typed message is a normal turn; the question card stays pending and can still be answered later.
- The person answers from two tabs or two devices at once: the first valid submission wins; the other is refused as already answered.
- The person answers a question after the agent has been updated or restarted: the answer starts a new turn on the current agent instance and carries the original question context.
- The person answers after the channel integration was removed: a WebUI card still works; a channel answer is refused because the conversation can no longer be reached.
- A question set has duplicate ids, empty prompts, duplicate option values, a default outside the options, or inconsistent selection limits: the call is refused before anything is shown.
- A question set has no questions or more than the documented maximum: the call is refused with the limit.
- A multiple-choice question has a minimum greater than its maximum: the call is refused.
- A required question has a default: the default counts as an answer, so the person may submit without touching it.
- A free-text answer is only whitespace: it counts as unanswered for a required question and as skipped for an optional one.
- A question set is asked by a script that then fails: the question is withdrawn, not left pending, and the turn continues with the script error.
- A question set is asked twice in one turn: the second call is refused; only the first set is shown.
- A question set is pending when the conversation is deleted: it is removed with the conversation; no answer turn is created.
- A question's option labels contain emoji, right-to-left text, or very long words: they are displayed without breaking the card.
- The person dismisses a question set: the agent receives a declined outcome and may ask a different question or proceed.
- The question is asked in a shared channel: any participant who can reply may answer; the first valid answer wins and the answer records who submitted it.
- The question is asked in a WebUI topic: only a person with access to that topic can answer.
- The agent asks a question in a turn that is later aborted before the question is delivered: nothing is shown and no answer is possible.
- A person presses a control on a set that another person, or another channel, already answered: the press is refused as already answered and no turn starts.
- A person answers a question by typing a message instead of pressing the control: the typed message is an ordinary turn, and the set stays pending until it is answered through the controls or dismissed.
- A control is pressed while the agent is stopped or restarting: the press is answered with a short notice (or held and applied when the reader is back) and is never silently dropped.
- A question set needs more controls than the platform allows in one message: it is split across messages, each part answering its own questions, and the set submits once every required question is answered.
- Two controls of the same set are pressed in quick succession: both answers are recorded and the set submits only when it is complete.
- A control identifier is too long for the platform: the identifier is kept in storage and the control carries only a short reference to it.

**Sensitive input**

- The person answers by typing the value into the ordinary message box instead of the dialog: it is an ordinary message and is stored like any other. The system cannot prevent this, and the tool's description tells the agent to ask through the tool, not in prose.
- The person is not present: the ask waits up to its limit, then returns a timeout result; the turn is not failed.
- The turn is aborted while the dialog is open: the ask closes, the script ends, and a late submission is refused.
- The agent is restarted while a dialog is open: the turn is resumed or sealed as it is today, and the ask closes rather than staying answerable against a dead turn.
- Two asks in one turn: the second waits for the first to close, so a person never sees two open dialogs for the same conversation at once.
- Two tabs are open on the conversation: the dialog appears in both, the first submission or dismissal wins, and the other tab's dialog closes without sending anything.
- The person dismisses the dialog by accident (Escape, or a press outside it): the ask is declined, not left open. The result is branchable, so the agent can ask again or explain what it could not do; it must not retry on its own in a loop.
- The value contains leading or trailing whitespace, or line breaks: it is delivered exactly as supplied, with no trimming.
- The value is empty or only whitespace: it counts as unanswered, so the ask is refused with that reason rather than delivering an empty value.
- The value exceeds the maximum: it is refused with the limit stated and the dialog stays open.
- The script that asked then fails: the turn continues with the script's error, and nothing about the value is recorded.
- The conversation is deleted while a dialog is open: the ask is removed with the conversation.
- The person declines, then the agent asks again in the same turn: a new dialog opens; the decline is not reused.
- The value is supplied while the agent's provider request is in flight: the script receives it when the call completes, as with any tool call.

**Both tools**

- The agent asks for a credential with the question tool: the value would enter the transcript. The question tool's description forbids it and points at the sensitive-input tool.
- The agent uses the sensitive-input tool to ask for something that is not sensitive (a project name): the person sees a masked dialog for ordinary text. The tool's description tells the agent to use a question instead, and the system does not prevent it.

## Requirements *(mandatory)*

### Functional Requirements

**The question tool**

- **FR-001**: vizier MUST offer agents a built-in question tool that asks a person one or more structured questions and, on a successful call, ends the agent's turn. It MUST be available in every interactive conversation where a person can answer, independent of the client or channel; it MUST NOT be a tool that only one client can provide.
- **FR-002**: The tool MUST accept a question set: an optional title and description, an ordered list of one or more questions, an optional submit label, and an optional dismiss label. Each question MUST have a stable id unique within the set, a prompt, a type, and type-specific content; it MAY be marked optional and MAY carry a default answer and a short help text.
- **FR-003**: The supported question types MUST be: free text (single-line or multi-line, with optional placeholder and maximum length), choose one from a list of options, choose several from a list of options (with optional minimum and maximum selections), and confirm/decline (a yes/no choice). Each option MUST have a stable value and a display label and MAY have a short description.
- **FR-004**: The tool MUST allow several questions in one set, up to a documented limit, presented together and answered in one submission. It MUST NOT require one turn per question.
- **FR-005**: The tool MUST reject an invalid question set before anything is shown, naming every problem at once: no questions, too many questions, duplicate question ids, empty prompts, a choice with no options, duplicate option values, a default outside the options, inconsistent selection limits, or a payload over the limit. A rejected call MUST NOT end the turn; the agent MUST get the errors and another round to correct them.
- **FR-006**: The tool MUST be turn-ending: a successful call ends the turn without another model round, the question card is the turn's final answer, and any text the agent wrote in the same step is kept and shown with it.
- **FR-007**: The tool MUST be offered only in conversations where a person can answer: interactive WebUI topics, and Discord and Telegram conversations that support it. It MUST NOT be offered in dreams, scheduled task runs, background pieces, inter-agent turns, side chats, or any unattended turn; a direct call in those contexts MUST fail with a clear message and MUST NOT end the turn.
- **FR-008**: At most one question set MAY be presented per turn. A second successful call in the same turn MUST be refused with a clear message, and the first set remains the turn's answer.
- **FR-009**: The tool's description, wherever the agent reads it, MUST explain its purpose, the supported question types, the maximum number of questions, that a successful call ends the turn, the shape of the answer the agent will receive, the limits, and that it is unavailable in unattended turns. The standard turn-ending notice MUST be appended exactly once and MUST NOT count against the author's description size limit.
- **FR-010**: A question set MUST be presented only if the script that asked it completes successfully. If the script fails after the call, the question MUST be withdrawn, MUST NOT be left pending or answerable, and the turn MUST continue with the script error. If delivery fails (the channel send fails, the conversation is gone, or the turn is aborted before delivery), the call MUST fail, the turn MUST NOT end, and the agent MUST get an actionable error.

**Questions: presentation and answering**

- **FR-011**: The WebUI MUST draw the question set as a custom card in the agent's reply, in the active theme, with all questions in order, their prompts, help text, options, defaults and required/optional state. The card MUST be keyboard-operable and accessible.
- **FR-012**: The card MUST validate on submit: every required question answered, every choice answer among the offered options, multiple-choice minimum and maximum respected, and text within its limit. On failure it MUST name the question and the problem, keep the answers already given, and submit nothing.
- **FR-013**: A valid submission MUST be delivered as the person's next turn in the same conversation, carrying the question set id, each question id, the answer value(s), a human-readable summary, and who answered. The card MUST become read-only and show the answers. The agent MUST receive the answers as structured data, not as an opaque text blob.
- **FR-014**: The person MUST be able to dismiss or decline a question set. The agent MUST receive a declined outcome with the same question ids and no answers, and MUST be able to continue.
- **FR-015**: The answer turn MUST behave like a typed message: same history, same queueing behind a running turn, same permissions and attribution. It MUST NOT bypass the queue or start a turn outside the normal conversation path.
- **FR-016**: The question set and its state (pending, answered, declined) MUST be saved with the conversation, so a reload, a second tab, another device and a server restart show the same card and state. A pending question MUST remain answerable after a restart.
- **FR-017**: A question set MUST be answerable once. After a valid submission or dismissal, later submissions MUST be refused with a clear "already answered" or "already declined" message and MUST NOT start a turn.
- **FR-018**: In a shared channel, any participant who can reply in the conversation MAY answer; the first valid submission wins and the answer records who submitted it. In a WebUI topic, only a person with access to that topic MAY answer. A person who cannot access the conversation MUST NOT be able to answer.
- **FR-019**: A question set that is never answered MUST remain pending until answered, dismissed, or the conversation or agent is deleted; it MUST NOT expire on its own. A new question set in the same conversation MUST NOT invalidate an earlier pending set; every answer MUST name its set.
- **FR-020**: The card in history MUST show its state: pending, answered with the chosen answers, or declined. A closed card MUST NOT be re-submittable.

**Questions: channels**

- **FR-021**: On the WebUI, the question set MUST be drawn as the custom card described in FR-011.
- **FR-022**: On Discord and Telegram, the question set MUST be presented with that channel's own interactive controls: a choose-one question as a row of buttons (a select control when there are more options than one row holds), a choose-several question as the platform's multi-select control, a confirm/decline question as a two-button row, and a free-text question through the platform's own text entry (a form opened from the question, where the platform requires one). Native controls MUST be used wherever the platform provides them; a plain text rendering is only the fallback of FR-027.
- **FR-023**: Pressing a control MUST answer only the question it belongs to, and the interaction MUST be acknowledged within the platform's own deadline so the person never sees a failed or expired interaction. Where a channel has controls for a question, answering it MUST NOT require the person to type a message.
- **FR-024**: Once the set is answered or declined, its controls MUST be disabled, or its message edited into a record of the answers. A press on a stale control MUST be refused with a short notice and MUST NOT start a turn.
- **FR-025**: A pending set's controls MUST keep working across a bot reconnect and a server restart: a control on a question asked before the restart MUST still be answerable, and MUST NOT depend on anything held only in memory.
- **FR-026**: An answer given through a control MUST be validated and delivered with the same structure as a WebUI answer (FR-013), and MUST record the answerer as the platform identifies them.
- **FR-027**: Where a question set cannot be expressed with native controls within the platform's limits — more options than one control holds, more questions than one message carries, or free text where the platform has no form — it MUST degrade to a documented text form answered by replying to the question message, without losing a question, its order, its id, or the one-submission rule.
- **FR-028**: A channel that cannot present or collect answers MUST NOT offer the tool; the agent MUST NOT be able to ask a question there, and its text replies MUST remain unchanged.
- **FR-029**: If a channel cannot show every question in one message, the presentation MUST be split in a way that preserves the set's order and question ids and still collects one answer submission; it MUST NOT silently drop or merge questions.
- **FR-030**: The same question set MUST produce the same answer structure (set id, question ids, values, skipped/declined state, answerer) on every supported channel, so the agent does not need channel-specific handling.
- **FR-031**: Presenting questions natively MUST NOT require additional bot permissions, privileged intents, or platform features a bot does not have. An integration that lacks them MUST fall back to FR-027 rather than fail the turn.

**The sensitive-input tool**

- **FR-032**: vizier MUST offer agents a tool, `ask_sensitive_input`, that asks the person for a sensitive value and delivers it to the calling script, in the same category as the existing stored-secret tools.
- **FR-033**: The tool MUST NOT end the turn. A successful, declined or timed-out ask MUST leave the turn running so the script continues with the result, and the turn ends only as it otherwise would.
- **FR-034**: The ask MUST state what value is wanted and why, in the agent's own words, so a person can judge whether to supply it; the reason is shown to the person and is not secret.
- **FR-035**: In the WebUI the ask MUST be presented as a dialog: a masked field for the value, the agent's stated purpose, a submit action and a reject action. The field MUST be masked by default, MUST offer an explicit reveal, MUST NOT autocomplete or autofill, MUST NOT be offered to the browser's password manager, and MUST NOT echo the value after submission.
- **FR-036**: Dismissing the dialog — the reject action, Escape, a press outside it, or its close control — MUST decline the ask (FR-050). There MUST be no way to close the dialog that leaves the ask open or the script waiting.
- **FR-037**: The value MUST be delivered exactly as supplied — no trimming, normalisation or newline conversion — subject only to the length limits of FR-056.
- **FR-038**: An empty or whitespace-only submission MUST be treated as unanswered and refused with that reason, leaving the ask open.
- **FR-039**: The ask MUST be answerable only by a person who can access that conversation, MUST record who supplied the value and when, and MUST NOT be answerable from another conversation.
- **FR-040**: The tool's description, wherever the agent reads it, MUST state: that the value goes to the script only; that the agent will not see it; that the call does not end the turn; that the person may reject or dismiss the dialog, or let it time out; the length limits; that redaction is exact-string only, so a transformed form is outside the guarantee; and where the tool is unavailable.

**Handling a sensitive value**

- **FR-041**: The value MUST go to the running script only. It MUST NOT be returned to the model, written into the conversation history, or included in any copy of the turn the model later reads.
- **FR-042**: The value MUST be armed into the turn's existing secret-redaction mechanism at the moment it is received, so that the execution report, its history entry, the model's copy, the log line and the tool telemetry are scrubbed exactly as they are for a value read from the stored secrets.
- **FR-043**: A value this mechanism cannot arm — because it is shorter than the redaction minimum — MUST NOT be silently delivered as if protected; the ask MUST state that a value this short cannot be scrubbed, and the person MUST be able to decline on that basis.
- **FR-044**: The ask MUST NOT store the value anywhere. There MUST be no retention of it beyond the turn that received it, and no way to read it back later — neither by the agent, nor by a person, nor by a later turn.
- **FR-045**: The tool MUST NOT be presented as a way to store a credential. A script that wants a supplied value stored MUST pass it to the existing secret-setting tool, and the tool's description MUST say so.

**Sensitive input: availability, declining and waiting**

- **FR-046**: The tool MUST be offered only in a conversation where a secure input can be shown to the person. It MUST NOT be offered on a channel where the value would be stored by the platform, or in any unattended turn (dream, scheduled task, background piece, inter-agent turn, side chat).
- **FR-047**: A call in a conversation where the tool is not offered MUST fail with a message naming why, and MUST NOT fail the turn.
- **FR-048**: A person MUST be able to answer an ask from any view of that conversation, including a second tab or another device, and a valid submission or a dismissal from one view MUST close the ask everywhere.
- **FR-049**: An ask MUST NOT be answerable after it has been answered, declined, timed out, or after its turn ended or was aborted; a late submission MUST be refused with a short notice and MUST NOT reach the agent.
- **FR-050**: The person MUST be able to decline an ask, by the dialog's reject action or by dismissing it (FR-036). The script MUST receive a declined result it can branch on, and the turn MUST continue; declining MUST NOT end the turn.
- **FR-051**: An ask MUST have a bounded wait. On expiry the script MUST receive a timeout result it can branch on, the turn MUST continue, and the ask MUST close.
- **FR-052**: The agent MUST be able to ask again in the same turn after a decline, a timeout or a successful ask, each ask being independent and each closing on its own outcome.
- **FR-053**: At most one ask MAY be open per conversation at a time; a second request MUST wait for the first to close rather than showing two dialogs at once.
- **FR-054**: A declined or timed-out ask MUST be recorded in the conversation as such — so the person and the agent can see what happened — without any value.
- **FR-055**: The tool MUST NOT be offered where the value would be visible to someone who is not the person answering: it MUST NOT be presented in a way that broadcasts a supplied value to other participants.

**Limits and honest boundaries**

- **FR-056**: The value MUST be subject to a minimum and a maximum length, stated in the refusal and in the tool's description. The minimum MUST be the redaction mechanism's own minimum, so a value is either protectable or explicitly flagged as not.
- **FR-057**: The system MUST NOT claim protection for a value the script transforms (encoded, hashed, split, concatenated) or writes elsewhere (a file, memory, a scratch file, an artifact, an outbound request body it stores). The tool's description MUST say this plainly.
- **FR-058**: The system MUST NOT claim to prevent a person from typing the value into an ordinary message instead of the dialog; the tool's description MUST instruct the agent to ask through the tool and never in prose.

**Shared: safety, accessibility and observability**

- **FR-059**: Answers MUST be treated as untrusted person input. Choice answers MUST be validated against the offered options; free text MUST be length-limited and rendered as text, never as markup, script or a command; no answer may cause the system to execute anything.
- **FR-060**: A question set MUST be size-limited: a maximum number of questions, options per question, prompt/label/help lengths, and total payload. An answer MUST be size-limited per text answer and in total. Refusals MUST name the limit; an invalid tool input MUST continue the turn, and an invalid answer MUST keep the card open.
- **FR-061**: The question set, its state, the answer, who answered and when MUST be stored with the conversation and handled like other conversation content for visibility, retention and deletion. They MUST NOT be written to logs as content unless the existing content-capture setting is on.
- **FR-062**: The agent MUST see, in its next turn, the question set id, each question, the answer or skipped/declined state, the option labels and values, and who answered. It MUST be able to tell a question answer from an ordinary typed message.
- **FR-063**: The system MUST prevent a question from being asked in an unattended conversation, and MUST prevent an answer from being accepted from someone who cannot access the conversation.
- **FR-064**: Both surfaces MUST be accessible: keyboard-only operation, labelled controls, errors associated with their field or question, focus moved to the first problem, screen-reader readable, usable on mobile, and readable in light and dark themes.
- **FR-065**: The feature MUST NOT change the behavior of `ui_render` or any other turn-ending tool. A conversation MAY contain a question card and a generated interface, and each MUST keep its own behavior.
- **FR-066**: The question tool MUST NOT be used for secrets. Its description MUST tell the agent to use the sensitive-input tool for credentials and other sensitive values, and that tool's description MUST tell it not to use a masked dialog for ordinary text.
- **FR-067**: Neither tool MUST be used as an approval or permission gate for tool execution in this release; approval flows remain separate (issue #119).

### Key Entities

- **Question set**: The unit the agent asks. It has an id, an optional title and description, an ordered list of questions, optional submit and dismiss labels, a state (pending, answered, declined), the conversation it belongs to, the agent that asked it, and timestamps.
- **Question**: One item in a set. It has an id unique in the set, a prompt, a type, optional options, a required/optional flag, an optional default answer, optional help text, and type-specific limits.
- **Option**: A choice offered by a choice question. It has a stable value, a display label, and an optional description.
- **Answer**: The person's response to a question set. It has the set id, a per-question answer (value(s), text, or skipped), the answerer, and a timestamp; a declined set has no answers.
- **Answer turn**: The person's next turn carrying the answers. It has a readable summary shown in the conversation and structured data the agent reads.
- **Question card**: The rendered representation of a question set in a conversation, with its state and, once answered, the recorded answers.
- **Ask**: One request for a sensitive value. It has an id, the agent's stated purpose, the conversation and turn it belongs to, a state (open, answered, declined, timed out, abandoned), a wait limit, and the requester's identity.
- **Supplied value**: The text the person submitted to an ask. It exists only for the turn that received it, is never stored, and is visible only to that turn's script.
- **Outcome**: What a script receives from an ask — the value, a decline, or a timeout — as a result it can branch on rather than an exception.

## Success Criteria *(mandatory)*

### Measurable Outcomes

**Structured questions**

- **SC-001**: A person can answer a single question in under 10 seconds from seeing it (median), and a three-question set in under 30 seconds, on the WebUI.
- **SC-002**: At least 95% of valid submissions are accepted on the first attempt, and at least 99% within two attempts, excluding deliberately invalid input.
- **SC-003**: 100% of successful question calls end the turn with no further model round.
- **SC-004**: 100% of answers reach the agent with the correct question set id and question id, including after a reload, a second tab and a server restart.
- **SC-005**: On every supported channel, 100% of question sets in a fixed sample are presented with that channel's native controls and are answerable through them; on unsupported channels, the tool is never offered.
- **SC-006**: A keyboard-only person can complete a question set without a mouse, and a screen reader names every question and control.
- **SC-007**: The question card renders in light and dark themes with no per-question styling by the agent.
- **SC-008**: An agent with no skill installed asks a valid question set on the first or second attempt in at least 95% of 20 sample prompts, using only the tool listing and description.
- **SC-009**: Submitting an answer starts the next agent turn within one second on the same machine, excluding model time.
- **SC-010**: No answer is accepted from a person who cannot access the conversation, and no question set is answered twice, in 100% of tests.
- **SC-011**: A pending question remains answerable after a server restart in 100% of cases.
- **SC-012**: A question set with three questions and five options each is presented in one reply, not one turn per question.
- **SC-013**: In a sample of 50 presses on channel controls, 100% are acknowledged within the platform's deadline and 0 show a failed or expired interaction to the person.

**Sensitive input**

- **SC-014**: In 100% of supplied values, the value does not appear in the conversation history, the running-step trail, the model's next context, the log output or the exported telemetry.
- **SC-015**: In 100% of cases, the script receives the exact value supplied, byte for byte.
- **SC-016**: A reject, a dismissal, a timeout and an abort each end the call with a result the script can branch on, and the turn finishes normally in 100% of cases.
- **SC-017**: The tool is offered in 0 unsupported conversations (other channels, dreams, scheduled runs, background pieces, inter-agent turns, side chats) and 100% of supported ones.
- **SC-018**: A person can supply a value in under 20 seconds from seeing the dialog (median).
- **SC-019**: In 100% of asks for a value shorter than the redaction minimum, the person is told the value cannot be scrubbed before they submit.
- **SC-020**: An agent with no skill installed uses the tool correctly — a stated purpose, the value passed to another tool rather than printed — in at least 95% of 20 sample prompts.
- **SC-021**: A late submission after an ask closed is refused in 100% of cases and never reaches the agent.
- **SC-022**: A supplied value is unrecoverable after its turn ends: no surface in the product returns it.
- **SC-023**: In 100% of asks, the turn is still running when the ask closes, and the script receives a branchable result for every outcome (value, reject, dismissal, timeout).
- **SC-024**: Every way of closing the dialog — reject, Escape, an outside press, the close control — leaves no ask open and no script waiting, in 100% of cases.

**Both tools**

- **SC-025**: An agent given an ambiguous request chooses the question tool and an agent given a task needing a credential chooses the sensitive-input tool, in at least 95% of 20 sample prompts, using only the tool listings and descriptions.

## Assumptions

**One family, two tools, and why they differ**

- The two tools are one feature because they answer the same need — the agent needs something from the person — and share the same surfaces, access rules, limits and validation discipline. They are two tools rather than one because the answer has to go to a different place.
- **The question tool is turn-ending; the sensitive-input tool is not, and that is forced by the guarantee.** A structured answer is content, so it becomes the person's next turn and takes the ordinary path through history and the queue. A sensitive value must never be content, so it has to be consumed by the turn that is already running — the only turn whose report, history entry, model copy, logs and telemetry are scrubbed. Handing the value to the next turn would put it in exactly the copy that must stay clean.
- **Names**: the family is verb-first and consistent — `ask_question` and `ask_sensitive_input`. Both were checked against the tree: no existing or retired tool name starts with `ask`, so neither collides. `ask_secret` was rejected because `secret` already means a *stored* secret in this product (`set_secret`/`get_secret`) and this tool stores nothing; `request_sensitive_input` was rejected in favour of the shared `ask_` prefix so the pair reads as one family in a tool listing. Tool names are dispatch keys, so renaming later is a breaking change — this is the moment to settle them.
- **Categories**: the question tool is listed under its own `questions` category, offered only in conversations where a person can answer (FR-007). It is not grouped under a channel-specific category (`webui`, `discord`, `telegram`) or under `agents`, because a category naming one surface would tell the agent the tool is limited to that surface, and a tool the agent cannot find in a normal listing cannot be used at all. The sensitive-input tool sits in the existing `secrets` category beside the stored-secret tools. Both are per-turn offers, never part of the always-on toolset, so neither appears in an unattended turn's prompt.

**Structured questions**

- The question set is the unit of answer: all questions are answered in one submission. There is no per-question turn.
- The question types are free text, choose one, choose several, and confirm/decline. "Selection" means choosing from a list of options the agent supplies; selecting arbitrary entities (files, agents, memories) is out of scope.
- Each question is required by default; the agent may mark one optional. A default answer counts as an answer.
- One question set per turn; the agent may ask again in a later turn.
- A pending question does not expire and is not invalidated by later messages or later question sets; every answer names its set.
- The answer is attributed to the person who submits it; in a shared channel the first valid answer wins.
- Discord and Telegram present questions with their native controls — buttons, select menus, inline keyboards and text-entry forms — with a reply-to text form only as the fallback for sets that exceed the platform's limits (FR-027). This is more work than a text rendering, because each channel needs an interaction path of its own (acknowledging a press, editing the message once it is answered), and that cost is accepted deliberately: typed answers would make the tool worse than a prose question on those channels.
- A pending question's controls carry nothing but an identifier resolved against stored data, so a set asked before a restart is still answerable afterwards; no part of a pending set lives only in the channel process.
- Platform limits — how many buttons or options a control holds, how many fields a form takes, how long a control identifier may be — are respected by choosing the fitting control and falling back to text. vizier does not request extra bot permissions or privileged intents to work around them.
- The question card is part of the agent's reply and is saved with the conversation; it does not create a new notification category, because the reply itself notifies as usual.

**Sensitive input**

- **WebUI only.** A secure input is the one thing a chat platform cannot provide: a value sent through Discord or Telegram is stored by that platform, readable by its administrators, and beyond vizier's reach. So the tool is absent there, and the agent asks in prose as it does today. This is a security decision, not a missing feature, and it is the one place this tool deliberately differs from the question tool.
- **The WebUI presents it as a dialog, and closing the dialog declines.** A dialog is the one surface where a value can be entered without entering the conversation, and it is already the app's pattern for a blocking decision. Dismissing it (reject, Escape, an outside press, its close control) declines rather than leaving the ask open, so nothing can be left waiting on a dialog nobody can see. The trade-off is accepted: an accidental dismissal costs the agent one declined result, and because the decline is a branchable result the agent can ask again — it just must not retry on its own in a loop.
- **The tool does not store.** Its purpose is to obtain a value, not to keep one. A script that wants it kept passes it to the existing secret-setting tool in the same turn, which is already covered by the same guarantee.
- **Redaction is inherited, not strengthened.** It is exact-string replacement over the values a turn handled. A transformed or split form escapes it, values below the minimum cannot be armed at all, and it protects what the *system* writes — not what a script deliberately writes into a file, memory or an outbound payload. The feature states these limits instead of implying more.
- **The wait is bounded.** A person who is away must not hold a conversation open indefinitely, so the ask has a limit and the script gets a branchable timeout rather than an exception. A retry is the agent's decision, not an automatic loop.
- **One ask at a time per conversation.** Two simultaneous masked dialogs would be ambiguous and would let a value be submitted to the wrong request.

**Shared**

- Existing limits and mechanisms for messages, history, turn-ending tools, channel delivery and secret redaction are reused; this feature adds the two question shapes and their surfaces, not a new transport, a new store or a new dependency.
- `ui_render` remains the tool for arbitrary interfaces; these tools are only for asking. Approval and permission prompts are out of scope (issue #119).
- Voice or audio answers, editing an answer after submission, file attachments in answers, and questions that span conversations are out of scope.
- Access to a conversation is the trust boundary, as it already is for the stored secrets: anyone who can read a topic can ask the agent to use its credentials. The sensitive-input tool does not change that; it only removes the value from the conversation's copies.
- The feature fits the existing single-binary, embedded-storage model and needs no new external dependency.
