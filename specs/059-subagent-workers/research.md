# Research: Subagents as temporary workers

**Feature**: `059-subagent-workers` | **Spec**: [spec.md](spec.md)

Every decision below was taken against the code on `master` at `1b780fad` (spec 058 merged).
"Today" means that code.

## What exists today

- `parallel_subagents` (`agents/tools/subagents.rs`, category `agents`) calls
  `Subagents::run_and_wait` (`agents/subagents.rs`). Each piece is a `VizierRequest` with
  `content: Prompt(text)`, `user: <origin agent>`, `transient: true`, sent to the **same agent** on
  `VizierSession(origin, VizierChannelId::Subagent, Some(uuid))`. The tool holds every reply
  stream, waits under a deadline, aborts late pieces (`abort_piece` → `Command("abort")`), and a
  `WaitGuard` aborts unfinished pieces when the call is dropped.
- A piece's turn runs the ordinary `handle_request` → `VizierAgent::chat` path and gets
  `prepare_system_prompts` unchanged: boot, sandbox, configured `system_prompt`, owner profile,
  `identity_md` (`core/soul`), project, documents. Its frontmatter is the default branch,
  `sender: <its own agent id>`.
- `transient: true` makes `is_durable` false, so a piece has no `session_turn` row and dies with
  any restart. Its history *is* saved (the topic stays inspectable).
- Spec 058 built exactly-once delivery for detached scripts: `claim_deliverable_scripts` (one
  transaction setting `delivered_at`), claimed at three sites — the round boundary in
  `prompt_with` (`agents/agent/mod.rs` ~1555), an idle poke in `agent_process`
  (`agents/process.rs` ~187), and a queued poke in `handle_request` (~1500) — and rendered by
  `detached::render_delivery` into `VizierRequestContent::DetachedDelivery(text)`.
- `recovery::recover_agent` re-sends a claimed agent's `session_turn` rows (`resumed` for
  `running`) with the session's broadcast sender as reply channel, then runs
  `DetachedScripts::recover_agent` (interrupt, poke).
- Per-turn availability is `VizierTool::available(ctx)`; turn-ending tools are
  `VizierTool::ends_turn()` and surface as `VizierResponseStats.ended_by_tool`.

## Decisions

### D1. A worker stays the same agent on a `Subagent` session

**Decision**: keep addressing a worker as a turn of the main agent on
`VizierSession(agent, Subagent, Some(worker_topic))`. Its tools, memory, permissions and
attribution are therefore the main agent's by construction (FR-002), with nothing copied.

**Rationale**: "on behalf of the main agent, inheriting everything" is what the current
addressing already gives; a separate agent identity would need copying config and would break
attribution.

**Alternatives**: a synthetic agent id per worker (rejected: no stored config is allowed, and
every storage scope keys on the agent id).

### D2. The worker framing is chosen by the session, in `prepare_system_prompts`

**Decision**: `prepare_system_prompts` branches once on `session.1 == Subagent` (the way
`is_scheduled_task` is a session question, never a content one):

| Part | Main turn | Worker turn |
|---|---|---|
| Boot | `boot_md` | `worker_md` (new `system_prompt/worker.rs`): who it works for, the reader is the main agent, the report shape, the ask tool |
| Sandbox | yes | yes |
| Configured `system_prompt` | yes | **no** (it is the agent's persona) |
| Owner profile | `owner_md` | the same text under `## The person <name> serves` |
| Identity | `identity_md` | the same block under `## The agent you work for` |
| Project block | when in a project | when the spawn came from a project chat (existing `project_scope`) |
| Documents | yes | yes |

**Rationale**: FR-003–FR-005. Same content, different relationship. The prompt cache is
explicitly not a concern for this feature (brainstorm decision).

**Alternatives**: appending a framing block after an unchanged prompt (rejected: the identity
would still read as "you" before the correction arrives); dropping the soul (rejected in
brainstorming: a worker needs the agent's values to serve it).

### D3. Two new request kinds: `SubagentTask` and `SubagentReply`

**Decision**: a worker's first request is `VizierRequestContent::SubagentTask(SubagentBrief {
subagent_id, position, group_size, text })`; a reply is `VizierRequestContent::SubagentReply {
subagent_id, text }`. `generate_frontmatter` gets one branch for both:
`sender: <agent id>`, `via: subagent` (or `subagent_reply`), `subagent: <id>`,
`task: "2 of 4"`. In `handle_request` both join the `Chat` arm (same history, hooks, steps,
resume) — not the `Prompt` arm, which is the dream/unattended path.

**Rationale**: FR-006. A content kind is how every other machine-sent request (scheduled run,
background delivery, question answer) is told apart in frontmatter; metadata on `Prompt` would
also be shown to the model as raw metadata.

**Alternatives**: `Prompt` + metadata (rejected: invisible to `kind()`, mixed with the dream path).

### D4. One `subagent` row per worker, fenced on the agent's scope

**Decision**: a `subagent` table behind `SubagentStorage`, a new member of
`VizierStorageProvider` implemented once on `SqlStorage` (SQLite init + PostgreSQL migration
`0009`). Every write that advances a worker calls `check_fence` on `agent:{id}`, each state change
is one guarded update, and `fence_tests.rs` gets one row per method. Details in
[data-model.md](data-model.md).

**Rationale**: FR-019 and the lifetime decisions need state that outlives the tool call and the
process; spec 057 requires fencing for in-flight work.

### D5. Outcomes are recorded agent-side, when the worker's turn ends

**Decision**: the engine — not a waiting tool — records a worker's outcome:

- `ask_main_agent` (turn-ending) writes `running → asking` with the question **in its own call**,
  then ends the turn.
- After `VizierAgent::chat` returns in a `Subagent` session, `handle_request` calls
  `Subagents::settle(session, Answered(text))`; a row already in `asking` is left alone (guarded
  `running → answered`).
- A worker turn that errors or panics is settled `failed` in the `join_next` arm of
  `agent_process` (`outcome != Done`, or `handle_request` returned `Err`), again guarded from
  `running`, so a late duplicate is a no-op.

**Rationale**: a resumed turn's replies go to the session's broadcast sender, not to whoever
spawned it (`recover_agent`), so a listener-based design loses outcomes on restart. Agent-side
recording makes FR-017a and SC-006 hold for free.

**Alternatives**: a per-worker watcher task holding the reply stream (rejected: lost on restart,
duplicated across nodes); subscribing to `session.>` frames (rejected: indirect and lossy under lag).

### D6. Worker turns become durable

**Decision**: worker requests are sent with `transient: false`, so `send_request` journals a
`session_turn` and `recover_agent` resumes it like any turn. `parallel_subagents`' reply channels
go: spawn sends with no reply sender.

**Rationale**: FR-017a. Durable turns already resume from the last step and seal open calls.

### D7. Waiting reads rows and wakes on a session frame

**Decision**: every worker change publishes `SessionFrame::Subagent(SubagentSnapshot)` on the
**hiring** session (the frame the peek spec will draw). `wait_subagents` subscribes to session
events, re-reads the rows on each matching frame (and once at start), and returns when the mode is
met or `ctx.deadline` is reached. A wait never changes a worker.

**Rationale**: FR-010. The hiring turn and the worker turn are the same agent, so the same owner
node; the frame is published through the existing pump, so it also works across processes. Rows
are the truth; the frame is only a wake-up, so a lagged subscriber just re-reads.

**Alternatives**: polling (rejected: latency or load); an in-process `Notify` map (rejected: the
peek spec needs the frame anyway).

### D8. "Seen" and "delivered" are sequence numbers, not flags

**Decision**: a row carries `outcome_seq` (incremented on every settle and every ask),
`seen_seq` (set by `wait_subagents`/`get_subagent` when they return that outcome) and
`delivered_seq` (set by the delivery claim). An outcome is deliverable iff the state is
`answered | failed | timed_out | asking` and `max(seen_seq, delivered_seq) < outcome_seq`.
`stopped` is never deliverable.

**Rationale**: FR-024–FR-026. A worker can ask several times; a flag would not tell a second
question from the first. The claim is one guarded update per row (`delivered_seq = outcome_seq`
where still deliverable), so a wait and a claim racing on the same outcome hand it out once.

### D9. One delivery path with two sources

**Decision**: introduce `agents/delivery.rs` with a `DeliverySource` trait
(`claim(session) -> Vec<String>` rendered sections, `sessions_with_deliveries(agent)`) and a
`Deliveries` aggregator on `VizierDependencies` holding `[detached, subagents]`. The three claim
sites call `deps.deliveries.claim(&session)` instead of `deps.detached.claim`, and the poke
(`DetachedDelivery(String::new())`) is sent by either source through one `Deliveries::poke`.
`recover_agent` asks the aggregator for sessions to poke. The request kind stays
`VizierRequestContent::DetachedDelivery` (its serde name is stored in history and read by the
WebUI) with its doc comment widened; the subagent section heading is `[Subagent update]`.

**Rationale**: Constitution II: a second source is the moment to make the seam, not to copy the
three claim sites. Keeping the stored kind avoids a history migration.

**Alternatives**: a `SubagentDelivery` kind with its own three claim sites (rejected: duplicate
paths); renaming the stored kind (rejected: migration and a #82 alias for no user value).

### D10. Time limits are enforced by the service, not a waiter

**Decision**: a row stores `deadline_at` while `running`. `Subagents` keeps one timer task per
running worker (re-armed by `recover_agent` for rows it now owns). On expiry: guarded
`running → timed_out`, then `Command("abort")` to the worker session. Entering `asking` stores
`remaining_ms` and clears the deadline; a reply sets `deadline_at = now + remaining_ms`.

**Rationale**: FR-009, FR-017b. With no waiter, nothing else would end a stuck worker.

### D11. Stopping and the parent's `/abort`

**Decision**: `stop_subagent` is a guarded `running | asking → stopped(by_agent)` followed by an
abort of the worker session. The `WaitGuard` and every abort-on-drop are removed: a parent's
`/abort`, a cancelled detached script, a dropped wait — none touch workers (FR-017).

### D12. Deleting conversations and agents

**Decision**: `delete_session` of a hiring conversation stops its unfinished workers
(`stopped(conversation_deleted)`), aborts their sessions, then deletes the rows and each worker
topic's history through the existing session-deletion path. `delete_agent` deletes every row.
Deleting a worker topic alone is not offered.

### D13. The depth limit is its own counter and a seed-config setting

**Decision**: `VizierRequest.subagent_depth: u8` (serde default 0, carried on the wire like
`background_depth`) and `ToolContext.subagent_depth`. A worker's requests carry the hiring
context's depth + 1; a detached script inherits its turn's value. The management tools'
`available(ctx)` is `ctx.subagent_depth < config.subagents.max_depth`; `ask_main_agent`'s is
`ctx.session.1 == Subagent`. `VizierConfig.subagents: SubagentsConfig { max_depth: u8 }`,
default `1`, beside `lsp`. Spawning still increments `background_depth` as today, so the existing
overall nesting bound is unchanged.

**Rationale**: FR-018. A hidden tool (not a refused one) keeps a worker's catalogue honest.

### D14. Category, retirement and doctrine

**Decision**: `ToolCategory::Subagents` (`"subagents"`, the 20th) holds the seven tools;
`agents` keeps the detached-script tools and its description becomes "See and stop your
background work". `RETIRED_TOOL_NAMES` gets `parallel_subagents`; the `paralel_subtasks` and
`BACKGROUND_REPLACED` hints are rewritten to name `spawn_subagents`/`wait_subagents`. Boot's
"Large tasks" section is rewritten per FR-022. None of the tools enter `DREAM_TOOL_NAMES`.

### D15. No HTTP or WebUI in this spec

**Decision**: the only new surface is the tools and the session frame. The routes, the strip item
and the FigCraft pass are the peek spec. No FigCraft pass here: no UI.

### D16. Pre-1.0 tracking (#82)

**Decision**: add #82 entries for the new `RETIRED_TOOL_NAMES` row and the rewritten hints. Old
`Subagent` topics written before 059 have no `subagent` row and are left as they are (no
migration).
