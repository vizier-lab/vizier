# Research: Waking an agent in any conversation

Every decision below was settled against the code as it stands on `master` (spec 066 merged).
There are no open unknowns.

## D1 - A conversation is addressed by its channel slug, with the topic beside it

**Decision**: A conversation is named by **the slug of its channel** (`VizierChannelId::to_slug()`:
`discord__123`, `telegram__-9`, `http__ana__vizier-webui`, `project__ana__proj_abc`) plus, where
the channel has topics, **the topic id** beside it. The agent that owns it is a **separate**
argument, defaulting to the calling agent. `list_sessions` returns those three fields per row
(plus a title and whether a turn is running there); `wake_session` takes them back.

**Rationale**: The slug is already the system's address for a conversation: the conversation picker
lists it (`service/conversations.rs`), the WebUI's route carries it (`service/topics.rs`), and the
storage keys rows by it. Reusing it means the tool speaks the vocabulary the rest of the system
already speaks, and a reference from a listing is exactly what a wake takes (FR-004).

The agent id is kept out of the address because **an agent id is a user-supplied free-form string
with no character validation** (`service/agents.rs::create` takes `agent_id` verbatim and the FS
backend uses it as a file name), so it may contain `__`. A WebUI topic id is a `nanoid` of length 10
from the default `A-Za-z0-9_-` alphabet, so **it may also contain `__`**. Packing either into the
same `__`-separated string would make it ambiguous to parse, which is why the full session slug
(`{agent}__{channel}__{topic}`) is not the address here.

**Alternatives considered**: The full session slug (`VizierSession::to_slug()`). Rejected for the
ambiguity above — and the codebase already lives with it for storage keys, which is a different
problem from addressing a conversation to a model. A structured address (a channel kind plus a
numeric id). Rejected: it cannot express a WebUI channel id, and it makes the model choose a
variant instead of copying one string. A new opaque reference id stored per conversation.
Rejected: a new column, a new lifecycle, and a reference that has to survive a rename and a
delete, for an address the system already has.

## D2 - One parser, and it is the allowlist

**Decision**: The new module holds **one** private function, `wakeable_channel(slug) ->
Option<VizierChannelId>`, which parses `discord__{u64}`, `telegram__{i64}`,
`http__{user}__{channel}` and `project__{user}__{id}`, and returns `None` for everything else.
`wake_session` refuses a target it cannot parse, with the reason.

**Rationale**: FR-014 and FR-015 are one rule — *only conversation channels may be woken* — and the
slug prefix is exactly what distinguishes them. The machinery channels (`task__`, `SUBAGENT`,
`consultation__`, `DREAM__`, `SYSTEM`, `inter_agent__`, `workspace__`) are refused **by
construction**, not by a second list that could drift from the first. The username and channel id
are split with `split_once("__")` after the prefix is stripped, so a channel id containing `__`
survives; a username containing `__` would not, and usernames do not contain one.

**Alternatives considered**: A general `VizierChannelId::from_slug` on the schema type. Rejected:
it would have to parse the machinery kinds too, so it would be a parser with a second policy
layered on top of it, and every caller would have to remember the policy. Extending
`usage::slug_kind` (`usage/mod.rs:868`). Rejected: it is deliberately lossy (it drops the HTTP
username and channel id, the task timestamp and the inter-agent participants) and it is private to
the usage module, so it answers "what label does this slug get", not "what conversation is this".
Reusing `service::topics::readable_channel` (`service/topics.rs:60`). Rejected: it takes the
**caller's own** username and builds the caller's own channel, which is the opposite of addressing
somebody else's conversation — and its doc comment says the write routes deliberately do not use it.

## D3 - The woken turn is a new request content kind

**Decision**: `VizierRequestContent::Woken(WokenRequest)`, where
`WokenRequest { from_agent: AgentId, instruction: String }`. It joins the **history arm** of
`process.rs::handle_request` (the arm that lists `Chat`, `AudioChat`, `ClientEvent`,
`QuestionAnswer`, `SubagentTask`, `SubagentReply`, `BackgroundReport`, `DetachedDelivery`), so the
woken turn runs with the target conversation's history in view.

**Rationale**: FR-008 wants the target conversation's earlier turns in view. **`Unattended` cannot
do that**: it is not in the history arm, and its branch calls `agent.chat(…, vec![], …)` with an
empty history (this is why spec 012 rejected `Unattended` for a background report). `Chat` does
have the history, but `Chat` means "a person spoke": it is classified as an interactive turn, so
the question tools are offered (FR-023 forbids them), and its frontmatter attributes the message
to a person.

The codebase has already made this exact decision twice — spec 012 D3 for `BackgroundReport` and
spec 064 for `SubagentTask` — and both times for the same reason: *a distinct content kind is what
keeps the history, the frontmatter and the WebUI from having to sniff a flag*. Spec 012's exact
words: "`Chat(text)` with a metadata flag. Rejected: history and the WebUI would have to sniff
metadata, and a person's message and a report would look alike in every other consumer."

**Alternatives considered**: `Unattended(instruction)` plus a `woken_by` field on the request.
Rejected for spec 012's reason. `Chat(text)` plus a flag. Rejected: it offers the question tools
and credits a person. A new **prompt section** (like `scheduled_run.md`). Rejected because the
prompt sections are selected on the **session's channel**, and the target channel of a wake is an
ordinary WebUI or Discord channel that also carries the person's own messages — a channel-selected
section could not tell the two apart. The content kind carries the framing instead, exactly as the
dream cycle carries its own framing in `EXTRACTION_PROMPT` rather than in a section.

## D4 - The woken turn's frontmatter names the waking agent

**Decision**: A new arm in `VizierRequest::generate_frontmatter` (`schema/request.rs:525`):

```yaml
sender: <the waking agent's id>
via: wake
sent_at: <when>
metadata: <the request's metadata>
```

**Rationale**: The frontmatter is the only place the model learns who sent the request. The
scheduler's arm (`sender: scheduler`, `requested_by: …`) and the consultation's arm
(`sender: <consulting agent>`) are the precedent, and `via` is how the consultation arm tells the
reader it is not a person's message. FR-009 asks for exactly this.

**Alternatives considered**: Putting the waking agent in the instruction text. Rejected: the text
is the instruction, and a prefix on it would be re-rendered on every replay of the request, and
would be the only place naming the sender — where the frontmatter already is.

## D5 - A woken turn is not interactive

**Decision**: `is_interactive_turn` (`agents/tools/mod.rs:382`) returns `false` for `Woken`, so the
question tools and the sensitive-input tool are not offered in it.

**Rationale**: FR-023. Nobody is there to answer a question: the request comes from an agent, not
from a person who is watching. The match on the content kind is **deliberately exhaustive**, per
its own doc comment: "a request kind nobody has classified yet is unattended until someone says
otherwise… adding a request kind fails the build here rather than silently offering a question
tool to a turn with no person on the other end." The new kind therefore forces the decision at
compile time, in every seam that classifies a turn.

## D6 - A woken turn notifies the person's inbox

**Decision**: `notifies_chat` (`agents/process.rs:1708`) includes `Woken`, so the reply raises one
inbox notification for the person whose conversation it is.

**Rationale**: The whole point of the feature is that the work lands where the person lives. A
scheduled run's work appearing in their thread with no badge would be missed. The contrast is the
background report, which is left out precisely because it has its own job notification
(`process.rs:1706`: "A woken background report already has its own job notification, so it is left
out"). A woken turn has no such notification, so it takes the chat one.

**Alternatives considered**: Leaving it out, like a background report. Rejected: the wake has no
job notification of its own, so nothing would tell the person it happened.

## D7 - The hop limit is the background-work limit

**Decision**: The woken request carries `background_depth: ctx.background_depth + 1`, and a wake is
refused when `ctx.background_depth >= MAX_BACKGROUND_DEPTH` (3), using the same `check_depth`
helper the detached-script and background tools use.

**Rationale**: FR-021 asks for a hop limit that is independent of the subagent and consultation
limits. `MAX_BACKGROUND_DEPTH` is exactly that — a background-work limit, a different number from
`subagents.max_depth` and from `MAX_CONSULTATION_DEPTH` — and it already counts "hops of background
work", which a wake is. **No new setting** (constitution I): a fourth depth number would be a
second answer to a question the codebase already answers.

**Alternatives considered**: A new `sessions.max_depth` setting. Rejected: constitution I — a
config flag for a second call site of an existing rule. The `subagents.max_depth` setting.
Rejected: it counts *subagent* hops, and a wake is not a subagent.

## D8 - The chain is the existing chain, widened

**Decision**: The woken request appends the waking agent to `VizierRequest::consultation_chain`
(`schema/request.rs:362`), and a wake to an agent already in the chain is refused.

**Rationale**: FR-022. The field is already "the agents above this turn", and its length is
already the depth that offers the consultation tools only below `MAX_CONSULTATION_DEPTH`. A wake is
another way for one agent to be above another, so it belongs in the same chain rather than in a
second one. The field's **doc comment** is widened to say so (consultations and wakes); its name
and type stay, so nothing in the stored rows changes shape.

**Alternatives considered**: A separate `wake_chain`. Rejected: a second chain field is a second
answer to "which agents are above this turn", and the two would have to be kept in step. Not
appending at all, and relying on the depth alone. Rejected: the depth stops a runaway, it does not
stop A → B → A.

## D9 - Access is the consultation access, unchanged

**Decision**: The wake uses `consultations::access::consulter(storage, ctx)` for "whose access
decides in this turn" and `consultations::access::consultable(storage, ctx, &consulter, agent_id)`
for "may this turn reach that agent". A wake to an agent that may not be consulted is refused
**exactly** as an agent that does not exist.

**Rationale**: FR-016 and FR-017 say what the consult rules already say: an agent may reach
another agent only if the person it is working for may use that agent, and a refusal must not
leak whether the agent exists. One rule for "who may reach whom" is the point; two would drift.

## D10 - The tools are offered only where a person's access can be decided

**Decision**: The two tools are offered in a turn only where `consultations::access::may_consult`
would say yes: not inside a dream, not inside a subagent worker, and **not in a Discord or Telegram
conversation**, nor in work started from one.

**Rationale**: From a Discord or Telegram conversation the agent does not know which person it is
working for — those platform identities are self-declared until
[vizier-lab/vizier#116](https://github.com/vizier-lab/vizier/issues/116). FR-018 needs the
person's identity to check that a WebUI conversation is the caller's own, and FR-016 needs it to
decide whether another agent may be reached. Offering the tools where there is no such person
would mean either refusing every call or letting an agent act for nobody. This mirrors the
consultation rule exactly (spec 064 FR-011a) and unblocks with it.

**Alternatives considered**: Offering them everywhere and refusing per call. Rejected: the model
would be offered a tool that cannot work, and would have to learn that from an error.

## D11 - A wake is a call, not a record

**Decision**: No table, no handle, no states, no panel. The tool sends the request and — when asked
— waits for the answer. The result lives in the target conversation, which is where the person
wants it.

**Rationale**: The spec's Assumptions say a wake is transient, and constitution I agrees: there is
no second use case for a handle yet. A consultation needed one because it has to survive the turn,
be stopped, be answered back and be peeked at; a wake needs none of those, because the answer goes
into the conversation it was aimed at. There is no delivery-back mechanism either, for the same
reason.

**Alternatives considered**: A `wake` table with a handle and a state machine, modelled on the
consultation row. Rejected: it is the whole consultation machinery for a case that does not need
it, and it would need the fencing, the seen/delivered sequences and the panel to be honest about a
lifecycle that does not exist.

## D12 - The wait follows the scheduler's pattern

**Decision**: `transport.reply_channel()` gives `(tx, rx)`; `transport.send_request(session,
request, Some(tx))` dispatches; the caller reads `rx` until a frame that ends the turn
(`VizierResponse::ends_turn()`).

**Rationale**: The pattern already exists, in `scheduler::fire` (`scheduler/mod.rs:334-397`): open a
reply stream, send with the stream attached, then read frames until `is_final_response`. FR-012
wants the call to return at once when not waiting (send with no stream) and FR-013 wants the
answer in the same call when waiting.

**Alternatives considered**: `transport.await_session_outcome(&session, ran_at)`. Rejected: it is
keyed by a run's time, which a wake does not have. A detached script. Rejected: it is for work the
caller does not wait on, and it outlives the turn in a way a wake should not.

## D13 - The tools' home and category

**Decision**: A new module `crates/vizier-core/src/agents/tools/sessions/`, and a new
`ToolCategory::Sessions` **appended** to the enum and to `ToolCategory::ALL` (which grows from 24
to 25). Its name is `sessions`.

**Rationale**: The tools address **every** channel, so no existing category is honest: putting
them under `webui` would hide a tool that works on Discord, and the model's own browse
(`list_tool_functions`) would lie. `Subagents` and `Consultations` are the precedent — a
cross-cutting family gets its own bucket — and the appended-category test
(`a_new_category_is_appended_to_the_end_of_the_list`, `tools/mod.rs:2383`) is exactly the rule
that keeps a new category from being slotted in among the ones older prompts already saw.

**Alternatives considered**: Reusing `Workspaces`, the empty slot spec 064 left in the enum.
Rejected: its label is "Workspaces (removed)" and it exists only to keep the display order.
Reusing `Agents`, which holds the detached-*script* family. Rejected: a detached script is
background work, not a conversation.

## D14 - The tool surface

**Decision**: Two tools.

`list_sessions(agent_id?, channel?, query?)` — the conversations a turn may reach. Each row:
`agent_id`, `channel` (the slug), `topic_id`, `title`, `is_thinking`. The calling turn's own
conversation is always in the list, under the channel it is on.

`wake_session(target, agent_id?, topic_id?, instruction, wait?)` — start a turn in the named
conversation. `target` is `"current"` or a channel slug; `agent_id` defaults to the calling
agent; `topic_id` is needed for the channels that have topics; `instruction` is the work; `wait`
asks for the answer in the same call.

**Rationale**: FR-001 to FR-007. The listing needs the channel filter and the title search
(FR-002), and the wake needs one target, one instruction and the choice to wait (FR-007). Two
tools rather than one, because they are two different questions: "where may I work" and "work
there". The discovery tool is not optional: references cannot be invented, so without it the wake
tool has no usable input.

**Alternatives considered**: One tool with a mode argument. Rejected: the model would have to
choose a mode to get a listing it usually wants first. A `wake_session` that takes the whole
session slug. Rejected in D1. Listing the conversations of an agent by title only. Rejected: the
title is not unique and may be empty.

## D15 - Where the listing comes from

**Decision**: `storage.get_session_list(agent_id, None)` — every conversation of that agent, across
every channel. The channel filter is applied in the tool, on the parsed channel, and the title
search on the title.

**Rationale**: The method already exists and already returns the right shape
(`Vec<VizierSessionDetail>`: agent, **typed** channel, topic, title, thinking state), and `None`
means every channel. No new storage method, no new query, no new table. The rows are filtered
down to the wakeable channels and to the agents this turn may reach, so the listing cannot be
wider than the set of things a wake would accept.

**Alternatives considered**: `storage.conversation_rows(agent_id, q)` — the conversation picker's
source. Rejected: it returns one row per **channel** (with a topic count and the newest topic),
not one row per conversation, so it cannot address a specific topic. A new storage method with the
filters pushed down. Rejected: constitution I — the filters are two predicates over rows the
engine already loads.

## D16 - Verification

**Decision**: The behaviour is verified end to end with the offline `dummyplug` provider, scripted
in `quickstart.md` as the constitution's e2e gate requires: a message, the expected reply, the
stored state and the history entry. No live model is needed — every step is about which tool runs
and what lands in the conversation.

**Rationale**: The constitution requires an agent-observable change to be exercised end to end
against a running binary with a dummyplug agent, and the dummyplug protocol
(`tools` → tool name → JSON request → echoed result) is deterministic, which is what makes the
"which tool ran, and where did the reply land" question answerable offline.
