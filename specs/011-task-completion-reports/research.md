# Research: Task Run Results, Requester and Framing

**Feature**: `011-task-completion-reports` · **Date**: 2026-10-04

Every decision below was taken against the code as it stands on `master` at `041eab9`.
Line references are to that revision.

---

## Decision 1 — A run is a stored record, not something derived from sessions

**Decision**: Add one derived-but-authoritative table, `task_run`, holding one row per firing:
agent, task slug, `ran_at`, the session key the run wrote into, and a state. The response text
is **not** copied into it.

**Rationale**: Three of the spec's requirements cannot be answered by sessions at all. A session
has no notion of having *started* (FR-005 `running`), of having *died with the process*
(FR-018 `interrupted`), or of a firing that produced no conversation because the agent was
unreachable (FR-024). All three are facts about an attempt, and only an attempt record can hold
them.

Two further things fall out for free once the row exists: the run list becomes an ordered,
indexable query (see Decision 4), and the row doubles as the overlap lock (Decision 5).

**Why the response text stays out**: the spec's own reason for deleting previews applies here —
a second copy of the response in a place people read as authoritative is a liability, and it can
drift from the conversation it was copied from. The row points at the session; the response is
sliced from `session_history` when asked for. This mirrors `memory_passage`, which stores a
passage's coordinates and never its text (`specs/009-memory-semantic-chunking/`).

**Alternatives rejected**:
- *Derive runs from session keys by prefix-matching `task__{slug}__%`*. Treats a formatted string
  as an identity, breaks on a task slug containing `__`, and still cannot express `running`,
  `interrupted`, or a firing with no session. `get_session_list` also matches `channel = ?`
  exactly (`storage/sqlite/session.rs:110`) with no ordering and no pagination, so the query
  would have to be written regardless.
- *Store the response in the row*. Two copies, drift, and contradicts the spec's rationale for
  removing previews.

---

## Decision 2 — Fix the session key so a run gets its own conversation

**Decision**: `VizierChannelId::Task` renders its timestamp in full in `to_slug()`
(`schema/session.rs:62`). Legacy rows keep their old key and are presented as one pre-existing
run; nothing is split retroactively.

**Rationale**: The current slug is `task__{id}__{datetime.timestamp_subsec_nanos()}`, and the
scheduler hands it a time truncated to the whole second (`scheduler/mod.rs:166`,
`Utc.timestamp_opt(now.timestamp(), 0)`). Sub-second nanos of a second-truncated time is always
`0`, so **every run of a task collides into `task__{slug}__0`**. There is no per-run separation
today; what looks like one is one conversation appended to forever.

That collision has two live consequences. A recurring task replays every previous run as history
on each fire, so its context grows without bound — a minutely task is a slow token leak. And the
same accident is the only continuity those runs have, which is why US4 ships alongside this fix
rather than after it.

**Retroactive split rejected**: inferring run boundaries from timestamps inside one merged
conversation is guesswork, and the payoff is tidier history for runs that have already happened.
One legacy run per task, labelled as pre-dating the split, costs nothing and lies about nothing.

---

## Decision 3 — Close a run from the response channel, copying the dream scheduler

**Decision**: The scheduler passes a `flume::Sender<VizierResponse>` to `send_request` and awaits
the final response in a spawned task, exactly as `DreamScheduler` already does
(`scheduler/dream/mod.rs:178-220`). The run closes `answered` when a final response carrying
message content arrives, and `no response` when the channel closes without one or the request
could not be dispatched.

**Rationale**: `send_request` already takes `Option<flume::Sender<VizierResponse>>`
(`transport.rs:124`); the task scheduler passes `None` and drops the result on the floor, while
the dream scheduler passes a channel and awaits it. The pattern, the plumbing and the
"is this the final response" predicate all exist — this is using an abstraction already in the
codebase for its second real case, not inventing one (Principle II).

`send_request` returns `Err` when the agent is not registered (`transport.rs:130`), which is the
"agent unreachable" case: the run opens and closes `no response` in the same tick, so the firing
is visible rather than silently absent.

**Alternatives rejected**:
- *Infer completion by polling session history for a trailing `Response`*. Cannot distinguish
  "still working" from "stopped without answering", and spends queries to learn what the channel
  would have told it.
- *Have the agent loop write the run record itself*. Puts scheduler bookkeeping inside the agent,
  and the agent does not know a request came from the scheduler except by inspecting its channel —
  the same coupling this feature is trying to remove.

---

## Decision 4 — Cursor pagination on `(ran_at, id)`, mirroring session history

**Decision**: Runs are ordered `ran_at DESC, id DESC` and paged with `before` + `before_id` +
`limit`, the same cursor shape `HistoryQuery` already uses (`channel.rs:53-60`, where
`before_seq` exists so entries sharing a millisecond cannot straddle a page boundary).

**Rationale**: FR-008 forbids duplicating or skipping a run across pages including when runs
share an instant and when a new run lands mid-paging. An offset cursor fails both: a new row at
the head shifts every offset. A key-set cursor on a strictly-ordered pair fails neither, and the
codebase already carries one for exactly this reason — reusing its shape means one mental model
for both surfaces rather than two.

**Alternatives rejected**: `LIMIT/OFFSET` (shifts under insert), cursor on `ran_at` alone (two
runs within a millisecond collide).

---

## Decision 5 — The run row is the overlap lock, replacing the in-memory set

**Decision**: "Is a run of this task already in flight?" is answered by a `running` row in
`task_run`, not by the scheduler's local `HashSet`.

**Rationale**: The existing `running: HashSet<(String, String)>` is inserted and then removed
within the same loop iteration (`scheduler/mod.rs:135,183`) because the dispatch is
fire-and-forget — it never actually spans a run, so it cannot prevent overlap today. Once runs
are awaited in spawned tasks, in-memory state would also have to be shared back across those
tasks, and would still be lost on restart. A row survives both, and it is the same row FR-018's
startup sweep already has to read.

---

## Decision 6 — Resolve orphaned runs with a startup sweep

**Decision**: On startup, any `task_run` still `running` is set to `interrupted`. This runs with
the other one-time migrations in `dependencies.rs`.

**Rationale**: A process that stops mid-run leaves a row claiming to be in flight, which would
both display as perpetually running (FR-018) and permanently block the task via Decision 5. A
sweep at startup is correct because nothing can legitimately be running before the scheduler
starts. `dependencies.rs` is where the existing one-time migrations live, so it is the existing
home for this rather than a new mechanism.

---

## Decision 7 — Requester is a tagged enum replacing `Task.user`

**Decision**: `Task.user: String` becomes `Task.requester: Requester`, an enum of
`User(String)` and `Agent(AgentId)`. Migration maps every existing `user` value to
`User(value)`.

**Rationale**: `user` is free-form and set by whoever creates the task — the web form defaults it
to the literal `"user"` (`webui/app/routes/tasks.tsx:49`), `create_task` reads it from the body
while ignoring the `AuthenticatedUser` it already holds, and the agent tool takes whatever the
model supplies (`tools/scheduler/mod.rs:28`). A tagged enum makes the one distinction that
matters — own initiative versus someone asked — unforgeable in the type, which is the whole point.

`User`'s payload stays an unvalidated string because it is not an account: Discord and Telegram
already synthesise `"@name (DiscordId: …)"` (`channels/discord/mod.rs:526`,
`channels/telegram/mod.rs:178`), and accounts exist for web sign-in only. Requiring one would
refuse the commonest case there is.

**Migration imprecision, accepted**: nothing recorded today distinguishes an agent-initiated task,
so every pre-existing task migrates to `User(...)`. Guessing from the string would be worse than
being plainly approximate.

**Note for implementation**: `VizierRequest.user` is a different field and is not being renamed.
The scheduler currently passes `task.user` into it; it will pass a rendering of the requester
instead (Decision 9).

---

## Decision 8 — Framing is a system message selected on the session's channel

**Decision**: A `scheduled_run_md()` section joins the other `system_prompt/` modules and is
appended by `prepare_system_prompts` only when the session's channel is
`VizierChannelId::Task`. `prepare_system_prompts` gains the run kind as a parameter.

**Rationale**: `prepare_system_prompts` (`agents/agent/mod.rs:282`) today takes no session and
builds a fixed vector — boot, sandbox, configured prompt, owner, CORE, documents. Making one
section conditional means threading the run kind in; that signature change is the structural edit
this story needs.

**The trap, and why the obvious implementation is wrong**: the dream cycle sends its work as
`VizierRequestContent::Task` (`scheduler/dream/mod.rs:195` and `:343`) while carrying its own
framing in `EXTRACTION_PROMPT`. Selecting on the request *content kind* would therefore double up
on the dream cycle — which FR-031 forbids and scenario 5 of US5 tests. Selection must be on the
channel, where a scheduled task (`Task`) and a dream (`Dream`) are distinct.

**Placement**: appended last, after CORE and documents. The content, not the ordering, is what
keeps the framing from touching the agent's character (FR-035) — but last is where a reader
expects situational context, and it keeps the cacheable prefix of boot/CORE identical to an
interactive turn's.

**Per-run variation**: none. The section is a constant string, so FR-033 holds and the prefix
stays reusable between runs of a session. Nothing per-run goes in it — the task, the requester and
the time are already in the request frontmatter.

---

## Decision 9 — Frontmatter attributes the run to the scheduler

**Decision**: For a scheduled run, the request frontmatter carries `sender: scheduler`, the task
slug, and `requested_by` — the requester, rendered as the person's identity or `self` for an
agent's own initiative.

**Rationale**: `generate_frontmatter` (`schema/request.rs:239`) emits `sender: <user>`, so a
scheduled run presents itself as a message a named person just sent. Combined with BOOT.md
directive 4 — *"Check channel metadata (discord, websocket, etc.) to understand the interaction"*
(`system_prompt/boot.rs:16`) — a scheduled run offers nothing to check and the sender name becomes
the only social cue available. Models infer from it correctly and reply conversationally. This is
the same class of defect as an unverified requester: a field asserting provenance it does not have.

**Alternatives rejected**: *dropping `sender` entirely for a task*. The agent genuinely benefits
from knowing who the work is for — a report for a specific person can be written for them. The fix
is to stop implying they sent it, not to withhold who they are.

---

## Decision 10 — Two agent tools, list then fetch, both available while dreaming

**Decision**: `list_task_runs` returns addresses, times and states; `get_task_run_detail` returns
one run's response. Both join the existing scheduler toolset and both go into
`VizierTools::DREAM_TOOL_NAMES`.

**Rationale**: The split is the idiom the memory tools already establish — `memory_search` returns
addressed passages, `memory_read` returns one whole document by address (`CLAUDE.md`) — and it
exists for the same reason here: one call must not be able to spend the whole context. Naming
follows the task family's own convention, `list_task` / `get_task_detail`
(`tools/scheduler/mod.rs:183,253`).

Including both in the dream set is what lets a task's outcome reach memory at all.
`is_non_user_channel` excludes `task__` from `list_user_sessions_in_window`
(`storage/sqlite/history.rs:63`), so the dream cycle never reflects on task conversations — these
tools are the only route. Four of the five existing scheduler tools are already in that set
(`tools/mod.rs:388-391`), so this is consistent rather than novel.

**Known limitation, out of scope**: the dream cycle early-returns when no conversations occurred in
the window (`scheduler/dream/mod.rs:37`), so an agent that only runs tasks still never dreams and
would never use these tools. Pre-existing gating; widening it is a separate question about what
dreaming is for.

---

## Decision 11 — No retention, no new background work

**Decision**: Every run is kept. Nothing trims `task_run` or `session_history`.

**Rationale**: FR-011 states this positively so a later reader does not helpfully reintroduce a
trim. Session history is already unbounded for every other channel, so trimming task runs alone
would be inconsistent as well as surprising, and pagination is what keeps the cost of *reading*
bounded. A cleanup job would also be new background machinery on the default path, which
Principle I asks us not to add without a current need.

Note that Decision 2 *reduces* the growth this feature might be suspected of causing: per-run
sessions stop a recurring task's context from compounding.

---

## Decision 12 — Rename `VizierRequestContent::Task` to `Unattended`, with no serde alias

**Decision**: Rename the variant to `Unattended`. Add `VizierSession::is_scheduled_task()` and
select the framing through it. **No `#[serde(alias = "task")]`** — the stored tag becomes
`"unattended"` and old rows carrying `"task"` no longer parse.

**Rationale**: The variant is misnamed, and the name is the bait for Decision 8's trap. It is
constructed three times — once by the task scheduler (`scheduler/mod.rs:175`) and **twice by the
dream cycle** (`scheduler/dream/mod.rs:195`, `:343`) — so two thirds of its uses are not tasks. It
is never matched on its own: the single match that mentions it groups it with `Prompt` and
`AudioPrompt` and then branches on the session channel to decide anything
(`process.rs:1063-1066`), and its `Display` impl is byte-identical to `Prompt`'s
(`request.rs:85`). The variant carries no behaviour; it is a label, and the label is wrong.

`Unattended` is what its three construction sites actually share: a machine wrote the prompt and
no person is waiting on the answer. It is also already this spec's word for the condition.

`Scheduled` was considered and rejected: the dream cycle would still be wearing it, which
reintroduces exactly the confusion the rename is meant to remove.

**Deleting the variant** in favour of `Prompt` was considered — it distinguishes nothing today.
Rejected because it does record something true that nothing else records, and removing it buys no
behaviour.

**No alias, and what that costs** (user decision, this ships in a breaking release):
`VizierRequestContent` is externally tagged, so the stored key is `"task"`, and `VizierRequest` is
persisted inside `session_history`. Without an alias, those rows fail to deserialize, and
`parse_history_row` drops a failed row **silently** — it is `serde_json::from_str(…).ok()?`
(`storage/sqlite/history.rs:32`), with no log line.

The loss is bounded: only `SessionHistoryContent::Request` entries whose content was `Task` are
affected — the opening request row of each legacy task run and each legacy dream request.
Responses in those same sessions parse unchanged. Combined with Decision 2, a legacy task
conversation therefore surfaces as one legacy run whose agent responses are intact and whose
original request rows are gone.

**Worth noting for implementation**: the silence is pre-existing behaviour, not something this
change introduces, but it is what makes the loss invisible. A `tracing::warn!` on a dropped row
would be a cheap, separate improvement and is not required by this feature.

**The rename is not the real guard.** It removes the temptation; `is_scheduled_task()` removes the
opportunity, by giving the question one obvious home that cannot be answered from the content kind.

---

## Resolved unknowns

| Unknown from the spec | Resolution |
|---|---|
| How a run learns it finished | Response channel, dream scheduler's pattern (D3) |
| Where `running` / `interrupted` live, given sessions cannot express them | `task_run` state (D1), startup sweep (D6) |
| How "no response" is distinguished from "never ran" | A row exists for a firing; absence of rows is never-ran (D1) |
| Pagination that cannot duplicate or skip | Key-set cursor on `(ran_at, id)` (D4) |
| How overlap is actually prevented, given today's set cannot | The `running` row (D5) |
| How framing avoids hitting the dream cycle | Select on channel, never content kind (D8) |
| Whether a requester must be an account | No — channel identities are valid (D7) |
| Whether run history needs trimming | No (D11) |
| Whether the misleading `Task` content variant should be renamed | Yes → `Unattended`, no alias, plus `is_scheduled_task()` (D12) |

No `NEEDS CLARIFICATION` items remain.
