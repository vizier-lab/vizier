# Research: Consulting other agents (and removing Workspaces)

Every decision below was settled against the code as it stands on `master` (spec 063 merged).
There are no open unknowns.

## D1 - One worker engine, two kinds of row

**Decision**: A consultation is a `subagent` row with a **consultee**. The table gains
`consultee_id` (nullable: null = a temporary worker, set = a real agent) and `principal_user_id`
(the person whose access admitted the consultation, re-checked on every reply). `Subagents`
(`agents/subagents.rs`) becomes the one service for both: `spawn` takes a `Target`
(`Worker { task }` | `Agent { agent_id, request }`), and everything else (`wait`, `reply`, `stop`,
`get`, `list`, `turn_ended`, the timers, `Watch`, delivery, recovery and deletion) is shared.
Consultation ids are `c_` + 8 base36 (workers keep `s_`), and groups stay `g_`.

**Rationale**: Spec 064 FR-005 and FR-021 say "as for subagent workers". Constitution II forbids a
second copy of a state machine, its fencing, its seen/delivered sequences and its delivery claim.
The only differences are which agent runs the turn, who may be targeted, and the follow-up
transition. Those are a column, a check and one guarded update.

**Alternatives considered**: A separate `consultation` table and service. This would mean
copying around 1,500 lines (`storage/sqlite/subagent.rs`, the timers, the watch), so it was
rejected. A generic "delegation" trait with two implementations was also rejected: there is
one storage implementation and one engine, so a trait would have a single implementor (I).

## D2 - The consultation conversation is the consultee's, on a new channel kind

**Decision**: Add `VizierChannelId::Consultation(String)`. It carries the consulting agent's id,
with the consultation's topic UUID as `session.2`. The worker session of a consultation is
`(consultee, Consultation(consulting agent), topic)`. Its slug is
`consultation__{consulting agent}`, and `usage_activity()` is a new `UsageActivity::Consultation`
(label **Consultations**, place `consultation:{consulting}:{topic}`). `InterAgent(Vec<String>)`
is not reused: it stays as the read-only legacy kind from before spec 053 (#82).

**Rationale**: The prompt framing, interactivity, conversation listing and usage are all selected
on the session's channel (the rule CLAUDE.md states for scheduled runs and workers). Keeping the
consulting agent in the channel gives B's conversation list its "Consultation from A" label with
no extra read. `subagent_by_worker(topic)` already finds the row by topic, whichever agent the
session belongs to.

**Alternatives considered**: Reusing `Subagent` with B as the session agent. That would give B
the worker framing ("you work for…"), which FR-002 forbids. Reusing `InterAgent` would mix legacy
rows with new ones.

## D3 - Fencing: each side writes under its own agent's fence

**Decision**: A row has two sides. Transitions the **consulting** side causes (open, reply,
follow-up, stop by the agent, seen, delivery claim, time-out, recovery re-own, stop on
conversation or consulting-agent delete) check `agent:{A}`. Transitions the **consulted** side
causes (settle on turn end, `running → asking`, stop on consultee delete) check `agent:{B}`. The
storage methods take the scope from the transition rather than from `row.agent_id`. Workers are
unchanged, because both sides are the same agent. `owner_node`/`owner_token` keep recording the
consulting side, which is what recovery re-owns.

**Rationale**: `check_fence` flips `ownership_lost(scope)` on its first refusal (spec 057). If
B's host settled a row under `agent:{A}`, then in a Fleet where A lives elsewhere it would release
A from a node that never held it. Each host fencing only the scope it owns keeps spec 057's
contract intact. In one node, nothing changes.

**Alternatives considered**: Routing B's outcome to A's host as a bus message, with A writing it.
That adds a message kind and a failure mode (A unreachable means the outcome is lost until
recovery) for no gain, since the row is already the shared truth.

## D4 - The follow-up is one more guarded transition

**Decision**: `answered → running` (`follow_up_subagent`), allowed only when `consultee_id` is set.
It leaves `outcome_seq` alone (the next settle increments it, so the next answer is a new
outcome), keeps the old answer in B's conversation, sets a fresh deadline from the row's `timeout_secs`,
and clears `ended_at`. `reply_consultation` picks the transition from the current state: `asking`
gives a reply, `answered` gives a follow-up, and anything else is refused with `not_waiting`. The
request sent is `SubagentReply`, framed `via: consultation_followup` (D6).

**Rationale**: FR-008. One tool for "answer B's question" and "ask B something more" matches how
a person continues a conversation, and saves the model a choice between two tools that do the
same thing.

**Alternatives considered**: A separate `follow_up_consultation` tool (more surface, same
effect), and follow-ups for workers too (out of scope: a worker is one task).

## D5 - Access: the person's view right, resolved in core

**Decision**: `consultations::access::principal_for_turn(deps, ctx)` decides whose access
counts. For an `HTTP`/`Project` session it is `web_user()`. For every other kind (Discord,
Telegram, Task, System, a detached script's conversation of those kinds) it is A's `owner_id`.
An agent with no owner gets `{status: "no_owner"}`. The person is resolved to a `Principal`
through the same path `Services::principal_for` uses, with method `System`. B is consultable when
`access::can_view_agent(principal, B)`, B ≠ A, and B is not in the chain (D8). An unknown agent
and a not-allowed one both answer `{status: "unknown_agent"}` with the same message. The check
runs on `consult_agent`, `reply_consultation` (both transitions) and `list_consultable_agents`.
Dreams are refused through `may_hire`'s existing dream test, applied to consultations as well.

**Rationale**: Clarification Q1 (view rights), FR-010, FR-011 and FR-013. No code links Discord
or Telegram identities to people (`service/mod.rs` has only `principal_for(user_id)`), so the
owner rule is the only sound fallback, and the spec now says so. Authorization lives in core, per
CLAUDE.md ("new authorization goes in core").

**Alternatives considered**: Using the scheduled task's `requester` as the person. This is more
precise, but the spec chose the owner and a requester can be an agent. Keeping it simple.

## D6 - Framing: the consultee stays itself, with one appended section

**Decision**: `system_prompt_layout` keeps the consultee's own layout (BOOT, sandbox, its
configured system prompt, owner, identity, project, documents) and, for a `Consultation`
session, appends `consultation_md(consulting agent's name)` last, as `scheduled_run_md` is
appended. The section is fixed text plus the name: "you are being consulted by `<A>`; you speak
for yourself; your answer is read by `<A>`, not by a person; answer compactly; you may ask `<A>`
with `ask_consulting_agent`; you cannot ask the person." Requests reuse the worker content kinds
`SubagentTask` (the first request) and `SubagentReply` (a reply or follow-up), framed by
`generate_frontmatter` from the channel: `sender: <A>`, `via: consultation |
consultation_reply | consultation_followup`, `on_behalf_of: <person's display name> | none`,
`consultation: N of M`. A consultation turn is never interactive: the conversation test in
`is_interactive_turn` excludes `Consultation`, which removes `ask_question` and
`ask_sensitive_input` (FR-017). A consultation turn is **durable**, exactly as a worker turn is.

**Rationale**: FR-002, FR-003 and FR-027. Appending at the end keeps the consultee's cacheable
prefix identical to its other turns (the reason given for `scheduled_run_md`). Framing by
channel, not by content kind, is the rule CLAUDE.md pins for the scheduler.

**Alternatives considered**: New `ConsultationTask`/`ConsultationReply` content kinds. They would
be two more serde variants whose only difference is the frontmatter, which the channel already
decides.

## D7 - Tools: a `consultations` category beside `subagents`

**Decision**: There are eight tools in a new category `consultations` (the 24th, appended), in
`agents/tools/consultations.rs`, all thin over `Subagents`:

| Tool | Offered | Notes |
|---|---|---|
| `consult_agent` | consultation depth < max, not a dream | `consultations: [{agent, request}]` (1..=8), `timeout_secs?` |
| `wait_consultations` | same | `ids`, `until: any \| all` |
| `reply_consultation` | same | asking → reply, answered → follow-up |
| `stop_consultation` | same | |
| `list_consultations` | same | |
| `get_consultation` | same | marks seen |
| `list_consultable_agents` | same | `{id, name, description, available}`, never self |
| `ask_consulting_agent` | only in a `Consultation` session | turn-ending, like `ask_main_agent` |

The management tools refuse a worker's `s_` handle as `unknown_consultation`, and the subagent
tools refuse a `c_` handle as `unknown_subagent`, so each family manages its own rows. The two
`ask_*` tools share one implementation, with `TurnEnd::Asked` recognising either name. None of
them is in `DREAM_TOOL_NAMES`, and `consultations` becomes a reserved category name.

**Rationale**: The user's `consult_agent` keeps its natural meaning, now live again. A separate
family keeps the model from mixing "a copy of me" with "another agent" (FR-026). A list argument
mirrors `spawn_subagents` (FR-004), with the cap of 8 bounding one call's fan-out on other agents'
budgets.

**Alternatives considered**: Letting `wait_subagents` and the others accept both kinds. This
would mean fewer tools, but the names would be wrong for real agents, and a model reading
"subagents" would assume the target shares its memory.

## D8 - Depth and chains

**Decision**: There is a new seed setting `consultations.max_depth` (default `1`) and a new
`ToolContext.consultation_depth` / `VizierRequest.consultation_depth`, carried exactly like
`subagent_depth`. A consultation request is sent with `consultation_depth + 1`. A worker's request
copies its parent's `consultation_depth` and adds nothing, but a worker is offered no
consultation tools while its `subagent_depth > 0` (FR-016). `VizierRequest.consultation_chain:
Vec<AgentId>` (serde-skipped on the wire, journaled with the turn like `project_scope`) lists the
agents above this one, and `consult_agent` refuses a target in the chain or equal to the caller
(`{status: "in_chain"}` / `"self"`).

**Rationale**: FR-015. The default makes A → B → A impossible without a counter. The chain makes
it impossible at any depth.

## D9 - People's views

**Decision**: On the consulting side, `service::subagents` gains a `kind` filter, and the HTTP
routes mirror spec 059's: `GET …/topic/{topic}/consultations[/{id}[/history]]`, scoped to the
caller's own topic, with history read from the consultee's conversation. `SessionFrame::Subagent`
keeps its name and gains `consultee: Option<{id, name}>`, and the WebUI splits it by that field.
On the consulted side, `conversation_rows`/`kind_of` list `Consultation` sessions as kind
`consultation` with the consulting agent's name, shown only to `can_edit_agent(B)` callers, and
the conversation is read-only (the topic socket refuses a `chat` frame on it with
`read_only_conversation`).

**Rationale**: FR-014, FR-028 to FR-030. Owner/editor access, because there is no "channel
conversations" permission to reuse. The platform rule is per platform (`show_discord_chats`), and
adding `show_consultation_chats` would be new permission surface for one list (I).

## D10 - Deletion

**Decision**: Deleting A's conversation or A uses the existing path (`Subagents::delete_session` /
`delete_agent`), which now also deletes each consultation's worker session. These sessions belong
to B, so `delete_worker_conversations` takes the row's `worker_session()`, whose agent is B. That
covers history, session files, scratch and `session_turn`, and B's running turn is aborted first.
Deleting B runs the new `stop_consultations_of(B, ConsulteeDeleted)` (fenced on `agent:{B}`, D3)
and pokes each consulting conversation, so A gets the stop as a delivery. Stop reasons gain
`ConsultingAgentDeleted` and `ConsulteeDeleted`, and `ByAgent` is rendered as "stopped by the
consulting agent" for a consultation.

## D11 - Removing Workspaces

**Decision**: Delete the feature outright:

- **Core**: `workspaces/` (all seven files), `service/workspaces.rs`, `schema/workspace.rs`,
  `storage/workspace.rs` and `storage/sqlite/workspace.rs` (and `WorkspaceStorage` in the
  supertrait), `agents/tools/workspaces.rs`, `Hub`, the Workspace arms in `process.rs`,
  `steer.rs`, `steps.rs`, `agent/mod.rs`, `system_prompt/{context,mod}.rs`, `scheduler/dream`'s
  catch-up, `notifications` (`workspace_message`), `service/{inbox,notifications,conversations}`,
  `usage` and `storage/user.rs` (the permissions).
- **HTTP**: `api/v1/workspaces.rs`, `workspace_topic.rs`, their router lines, and
  `tests/conformance/groups/workspaces.rs`. Their routes leave `tests/conformance/pending.txt`
  too (the list may only shrink).
- **Data**: SQLite init (`init_workspace_cleanup`) and PostgreSQL migration
  `0012_drop_workspaces.sql` delete, in one transaction: every session whose channel is
  `Workspace` (history, `session_detail`, `session_turn`, `scratch_file`, `session_file` and their
  blobs by reference count, `message_reaction`, `platform_message_link`), the `notification` rows
  of category `workspace_message` (their reads cascade), the two permissions from every role, then
  `DROP TABLE` for every `workspace*` table. This runs once, under marker
  `migration.drop_workspaces_v1`.
- **Kept, legacy (#82)**: the `VizierChannelId::Workspace(String)` variant, never constructed, so
  `usage_event` rows (kept per FR-033) still parse and are labelled "Workspaces (removed)". Also
  kept are the guidance entries in `RETIRED_TOOL_NAMES` and the `SilentRead` kind (still used by
  Discord and Telegram).
- **WebUI**: `routes/workspace.tsx`, `screens/workspaces/`, `stores/workspaces.ts`,
  `lib/workspace*.ts` (+ tests), `api/endpoints/workspaces.ts`, the rail's Workspace group, the
  `CreateChooser` (the rail's create control goes straight to New agent), the member search, and
  the mobile switcher's Workspaces list and Home's Workspaces list. Mid-turn message rendering
  stays (FR-034).
- **design**: `CreateChooser`, and the Workspace-only `MentionMenu`/`MentionChip`/`OriginBadge`
  if no other screen uses them (checked at implementation: `@` file mentions in project chats use
  `MentionMenu`, so it stays), plus the rail's Workspace entry variant.
- **Docs**: `docs/src/configuration/workspaces.md` is removed, a `consultations.md` is added,
  `SUMMARY.md` is updated, and `scripts/workspace-e2e.mjs` is replaced by
  `scripts/consultations-e2e.mjs`.

**Rationale**: Clarification Q2 (delete with the Workspace), FR-031 to FR-035. Keeping the
channel variant costs one enum arm, while removing it would break the usage report for every
install that used Workspaces.

**Alternatives considered**: Keeping the tables until #82 and only hiding the feature. Rejected:
the spec says the data is removed.

## D12 - Retired names

**Decision**: `RETIRED_TOOL_NAMES` drops `consult_agent` (live again) and gives `delegate_agent`,
`mention_agent`, `send_message_to_workspace`, `list_workspaces` and `read_workspace_messages` one
shared hint: "Workspaces were removed: consult another agent with `consult_agent`, then
`wait_consultations` for its answer." A test asserts that no hint names a retired tool (FR-032,
SC-006).

## D13 - Verification

**Decision**: There are three layers of checks:

- **Unit tests**: the row transitions (follow-up, two-sided fencing) in
  `storage/sqlite/subagent.rs` tests and `fence_tests.rs`, plus the access rule, the chain, the
  framing (`system_prompt_layout` with a `Consultation` session) and the retired hints.
- **dummyplug e2e**: `tests/dummyplug_e2e.rs`.
- **Script**: `scripts/consultations-e2e.mjs` against two dummyplug agents, as `quickstart.md`
  scripts it.

No dummyplug protocol change is needed: consulting is a tool call like any other, and B's reply
is scripted with the existing echo commands.
