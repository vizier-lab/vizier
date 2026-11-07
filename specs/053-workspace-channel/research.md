# Research: Workspace Channel

Decisions for [spec.md](spec.md), based on reading the code on `develop` at d0a19f5f. Nothing here has been run yet. The technical context had no open unknowns; these are the choices the design turns on.

## R1. One shared timeline, with one session per agent

**Decision**: Each member agent keeps its own session, `VizierSession(agent, VizierChannelId::Workspace(workspace_id), Some(topic_id))`. A new table, `workspace_entry`, holds the one shared order of a topic. Topic and Workspace ids are UUIDs, so a rename never changes a session key. The slug is `workspace__{id}`.

**Rationale**: History, the turn queue, durable turns, aborts, tools, and usage are all keyed by session and work per agent. Keeping that means nothing in the agent loop has to learn about a shared room.

**Alternatives**: A single session shared by all agents. Rejected because the queue, the journal, history resume and tool-call sealing all assume one agent per session.

## R2. Silent reading is a lazy catch-up, not a request per message

**Decision**: Posting writes one `workspace_entry` and sends nothing to agents that were not mentioned. When an agent is activated (and before a dream cycle), `workspaces::catch_up` copies the entries it has not read into its session history as `SilentRead` requests, up to the entry that activated it, and moves its `workspace_read` position, all in one transaction. At most the latest 500 are copied. If more were skipped, a note in `## Workspace` says how many and points to `read_workspace_messages`.

**Rationale**: The Discord/Telegram path is unsuitable for a room of agents:
- Each silent read goes through `agent_process`, takes a queue slot behind a running turn, and writes a `session_turn` journal row (`is_durable` does not exclude it).
- `refresh_session_detail` runs for it and can spend a titling model call on a session's first message (`process.rs` ~604).
- `initiative_factor` draws from an inclusive `0..=1` range and returns only when `factor > chance`, so even at chance 0 a draw of exactly 0.0 falls through to a model call.

Together those would break SC-001 and scale as messages × agents.

**Alternatives**:
- *Reuse `SilentRead` requests with a Workspace bypass.* Still costs queue and journal per message.
- *Insert rows into each agent's history when a message is posted.* Rejected as a correctness risk: a row inserted between a running turn's `ToolCall` and `ToolResult` splits a call from its result in `history_entries_to_messages`, and providers reject that.

**Consequence**: an agent's session history holds what it read only as of its last activation. That is invisible from outside, because people read the timeline, not agent sessions.

## R3. Each agent keeps its own record; the timeline holds only messages

**Decision**: As on Discord, each agent has its own record of the Workspace, one session history per topic:

| In agent A's record | Entered as |
|---|---|
| People's messages | `Request(SilentRead)`, or `Request(Chat)` for the one that activated it |
| Other agents' mid-turn and final messages | `Request(SilentRead)`, with `sender` set to that agent |
| A's own thoughts, tool calls, results and messages | Its normal turn entries |
| Other agents' thoughts, tool calls and results | **Never** |
| Other agents' checkpoints, commands | **Never** |

`workspace_entry` is what people and agents see as messages. `SqlStorage::save_session_history` adds a row in its existing transaction, the same way it already writes `artifact_reference`, only for text (`AssistantMessage`, a `Response` with a message), copying the saved text into `body` after redaction.

Trails are not copied anywhere. A timeline read pages `workspace_entry`, then runs one query for the channel's non-`Request` rows of `session_history` in that page's time span, and merges them by time. Live steps already arrive as tagged `history_entry` frames. This needs an index on `session_history(channel, topic, timestamp)`.

**Rationale**: `save_session_history` is the single write path, so no call site can forget. An agent's checkpoint, handover, resume and dream all then work on its own record unchanged. A trail stays where it was produced, in the agent's own record.

**Alternatives**:
- *A `workspace_entry` row per step.* I drafted this first. It stored pointers to data that already has a time, for no gain.
- *Writing from `record_step` plus the reply path.* There are at least three save paths (`record_step`, `save_history_or_log`, the turn flush), so one would eventually be missed.
- *Copying messages into every agent's record at once, as Discord does.* The final record is identical, but see R2 for why the copy waits until activation.

## R4. Mentions are parsed against the member list, and dispatched through an outbox

**Decision**: `workspaces::mentions::parse(text, handles)` is a pure function. It skips code spans, fenced blocks and quoted lines, then matches `@` plus the longest member handle, which must not be followed by a word character. So it needs no rule for what an id may contain. Resolved mentions are written to `workspace_mention` with state `pending` in the same transaction as the entry. Dispatch then sends the requests and moves them to `woken` or `unavailable`. At startup, `pending` rows are dispatched again.

A handle clash, where a person's username equals an agent id in one Workspace, is refused when the member is added (`409 handle_in_use`).

**Rationale**: A saved mid-turn message is complete, so it must wake its target even if the author crashes straight after. Subscribing to the session-event broadcast would lose mentions on lag, and that broadcast is explicitly lossy.

**Who dispatches**: `save_session_history` cannot send requests, so `record_step` and the reply save call `workspaces::after_save(uid)`, which reads the entry's mentions. The startup sweep is what catches a missed call.

## R5. An activated turn is an ordinary `Chat` request

**Decision**: Dispatch sends `VizierRequestContent::Chat(text)` through `deps.agents.request` with `user` set to the author, leading mentions removed for that request (as Discord's `command_text` does), `metadata.workspace_entry = seq`, and the skill rule in R19. A response sender republishes the frames on the session-event broadcast, exactly as a background report's woken turn does (`agents/background/mod.rs`).

**Rationale**: Durable turns, resume, skills, queuing and abort all work unchanged. It goes through `AgentsClient`, the one seam the gateway side may use (#163); the existing chat socket calling `transport.send_request` directly is not a model to copy.

## R6. `mention_agent` writes a timeline entry, not a history row

**Decision**: `mention_agent` is called from inside a script, so it runs while the `execute_python` call has no result yet. It therefore inserts `workspace_entry` directly (kind `agent`, no `history_uid`) and never touches session history. `catch_up` skips entries whose `source_session` is the agent's own session, so it does not read its own message back. The agent still knows what it sent, from its own tool call.

`send_message_to_workspace` uses the same insert with `origin` set. Since the sender's source session differs, the sender does read it back on its next activation there, which is what it needs because it wakes without its earlier context.

**Rationale**: A row between a call and its result is the same correctness risk as in R2.

## R7. Stop all uses a generation guard

**Decision**: Stop all sets `workspace.stopped_at_seq` to the current head, marks all `pending` mentions `dropped`, and calls `AgentsClient::abort` for each member agent's session in each topic. A mention by an agent is dropped at dispatch when the turn that wrote it was activated by an entry at or before `stopped_at_seq` (`activated_by_seq`, carried on the request metadata).

**Rationale**: Abort covers one session and clears its queue, but a turn finishing in that instant could still save a message with a mention and restart the chain. With no chain limit (FR-016), Stop all is the only brake, so it must not race (SC-005). Messages from people and from outside the Workspace are never blocked.

## R8. Workspace turns are excluded from person-scoped views

**Decision**:
- `is_non_user_channel` stays false for the slug, so dreams reflect on it (FR-026).
- `kind_of` and `readable_channel` must not expose these sessions on an agent's conversation pages. Access to the content runs only through membership.
- `refresh_session_detail` skips titling for `Workspace`, since the topic name is the title.
- `usage_activity` gains `UsageActivity::Workspace`, labelled **Workspaces**, and `usage_place` is `workspace:{id}:{topic}`.
- `is_interactive_turn` admits `Workspace` for `Chat` requests, so `ask_question` works. `ask_sensitive_input::available` refuses it. A socket client binding is never set, so client tools and generative UI are absent.
- Background jobs in a Workspace session are excluded from `job_ended` notifications, like platform channels.

**Rationale**: FR-005 lets a member use an agent they cannot view, which must not widen in the other direction: someone who can view an agent gets no Workspace content they are not a member of.

**Known risk**: the slug is checked by string prefix in about eight places, with no shared parser (`storage/history.rs:12`, `access.rs:51`, `service/conversations.rs:38`, `service/topics.rs:61`, `usage/mod.rs:671`, `storage/session.rs:30`, `storage/sqlite/session.rs:125,140`, `projects/migrate.rs:25`). Each is reviewed, and tests pin the access ones.

## R9. Files are one blob with one reference per agent session

**Decision**: An upload is stored once under `uploads/<file_id>/`, as today. The entry records the file ids, and `catch_up` adds a `session_file` record per agent session pointing to the same blob. The existing reference count (`session_file_reference_count`) already keeps a blob until the last record goes. `SessionFiles` refusing while a turn runs applies per session.

**Rationale**: Reuses the file tier and its deletion rules with no new blob store.

**To confirm during implementation**: that `save_session_file` accepts an existing `file_id`.

## R10. A Workspace socket watches a set of sessions

**Decision**: A new `workspace_topic::WorkspaceConnection` beside `TopicConnection`, with the same `open` / `handle_inbound` / `outbound().run()` / `close` shape, so the WebSocket handler and the desktop IPC both drive it. It filters session events by channel and topic rather than by one session, and tags each frame with its `agent_id`. New `workspace_entry` and `workspace_mentions` frames are sent from a fourth arm of the event pump, `workspace.{id}.events` on the bus. Membership is re-checked per delivery in `service/workspaces.rs`, as `service/inbox.rs` does, so removal takes effect on an open socket.

**Alternatives**: Generalising `TopicOutbound`'s filter. Rejected because it is tied up with `ClientConnection`, asks and deferred calls, none of which a Workspace has.

## R11. Mid-turn messages are a WebUI change only

**Decision**: `groupHistory` stops merging `AssistantMessage` into the trail. A `Turn` gains `blocks: (TrailBlock | MessageBlock)[]`, a new trail starting after each message. A trail's time is the span of its entries' timestamps. `trailSummary` loses its `note` count. A python report still pairs by `call_id` across blocks. No backend change: `entries_for` already emits `AssistantMessage`, and `record_step` already publishes it as a `HistoryEntry` for any content.

Fix the stale comment on `liveEvent` (`trail.ts:333`), which still says intermediate text is not streamed.

**Rationale**: Stored and live share one model, so it costs little and saved conversations pick it up with no migration (FR-037).

## R12. Removing the old tools

**Decision**: Delete `ConsultAgent`, `DelegateAgent` and `other_agents` (used nowhere else). Add both names to `RETIRED_TOOL_NAMES`, which already gives scripts a catchable hint. Keep for saved data, and list on #82: `VizierChannelId::InterAgent`, `JobKind::Delegation`, `delegated_to`, `delegate_agent` in `JOB_LAUNCHING_TOOLS`, and `MessageList`'s delegation label.

**Rationale**: Startup already sweeps running jobs to `interrupted`, so an upgrade mid-delegation needs nothing new.

## R13. New agent tools

**Decision**: A new built-in category `workspaces`, reserved like the others (an MCP server keyed `workspaces` becomes `workspaces_mcp`). `mention_agent` has `available` true only in a `Workspace` session; the other three are true except in `Dream` and `Subagent` sessions. All are on `default_toolset`, absent from `DREAM_TOOL_NAMES`. Each refusal is a `status` result, never `Err`, as for scratchpad tools.

The category stays listed for agents in no Workspace, so `SANDBOX.md` stays byte-identical across turns.

## R14. Convention goes in `context_md`

**Decision**: A `## Workspace` section in the per-message `context_md` carries the name, topic, member handles, who mentioned the agent, and the rule that an agent wakes only when mentioned. It joins `context_md_with`, like `## Project state`, not the system prompt.

**Rationale**: Membership changes must not invalidate the prompt cache for the history (the same reason as spec 041).

## R15. People search needs its own endpoint

**Decision**: `GET /workspaces/{id}/candidates?q=&limit=`, restricted to people who may manage the Workspace, returns capped people and agents, listing addable ones first (FR-040). `GET /auth/users` is unusable because it requires `users:manage`. Agents are filtered by `can_view_agent`, with `addable` from `can_edit_agent`. Ranking: starts-with before contains.

**Accepted for now**: any Workspace manager can search all usernames on the installation. This is fine for one team and worth revisiting for shared installs. It belongs with the separate tightening spec.

## R16. Dummyplug gains an additive `say` key

**Decision**: `{ "say": "text", "tool": "...", "arguments": {} }` makes dummyplug emit the text in the same model message as the tool call. It is additive, since other keys are already ignored, so existing scripts keep working. The protocol contract is updated in the same change.

**Rationale**: Mid-turn messages and mid-turn mentions are agent-observable and must be checked with dummyplug (constitution). The real dummyplug is the Node server in the `dummyplug` submodule (the Rust `model/dummyplug.rs` is not compiled). It has no other way to write text before a tool call. Final-text mentions need nothing new, because the tool-result echo already carries the result text.

## R17. Not behind a feature flag

**Decision**: No `channel-*` flag.

**Rationale**: The constitution's flag rule covers *optional* capabilities. Workspace has no external dependency, and with `consult_agent`/`delegate_agent` gone it is the only way agents coordinate, so a build without it would have a hole.

## R19. Skills and commands use `/<command> @<agent-id>`

**Decision**: The format is `/<command> @<agent-id> [input]`. `workspaces::commands::parse` reads a message's start: `/word` (or `/skill word`), then one or more mentions, then the input. It is pure and server-side, so the API and every client agree.

| Message | Effect | Other agents read it? | Who |
|---|---|---|---|
| `/name @a input`, `/skill name @a input` | a receives `/name input` as `Chat`, which runs through the existing `skill::invocation` | Yes, as typed | Any member, or an agent |
| `/skills @a`, or an unknown name | Answered directly with no model call, as today, and shown as a's reply | Yes | Any member, or an agent |
| `/name @a @b input` | Each resolves against its own skills | Yes | Any member, or an agent |
| `/abort @a` | Stops a's turn in this topic. A `note` entry says who | No | Any member |
| `/abort` alone | The same as Stop all | No | Any member |
| `/checkpoint @a` | Runs on a's own record of this topic. A `note` entry says who | No | Any member |
| `/lobotomy @a` | Same, without a handover | No | Owner, or someone who may edit a |
| `/btw ...`, `/clear-files` | Refused, not posted: 'Side chats and clearing files are not available in a Workspace.' Files are shared, and a side chat implies a privacy a shared room can't give | No | Nobody |
| Any other `/word` with no mention right after it | Plain message. It is not refused, since it may be plain text such as a path. Mentions later in it still wake those agents, as plain mentions | Yes | Anyone |
| `@a /foo` | A plain mention. a wakes and reads `/foo` as text, and no skill runs | Yes | Anyone |

**Plain mentions must not run a skill.** The Chat arm of `agents/process.rs` parses any text starting with `/`, and R5 removes leading mentions, so `@a /foo` would otherwise reach it as `/foo`. Dispatch therefore sets `metadata.workspace_plain = true` on every request that isn't the command form, and the Chat arm skips `invocation::parse` when it is set.

**Commands come from people only.** An agent writing `/abort @b` or `/lobotomy @b` does not run the command. `b` wakes with that text as a skill request, the skill parser finds no such skill, and it answers directly with no model call. Otherwise an agent could reset another agent's context, which no one asked for.

**Rationale**: Each agent having its own record is what makes `/checkpoint` and `/lobotomy` well defined: they affect one agent and one topic, and nobody else's context. Commands stay out of other agents' records, since they're instructions to the system and not conversation.

**Discovery**: The command comes first, so the composer's `/` menu lists everything at once. `GET /workspaces/{id}/commands` returns the commands plus each member agent's skill names and descriptions, each with the agents that have it. The `@` menu after a chosen command offers only those agents. A member may lack view access to an agent, and adding it already shared its use (FR-005), so this exposes names only and not content.

## R20. One notification row per message, seen by current members

**Decision**: Every `person` and `agent` entry creates one `notification` row, category `workspace_message`, with `event_key = workspace:{seq}`, `workspace_id` and `author` set, and `audience_user_id` null. It's created by the post path, `after_save` and the tools. The startup sweep that re-dispatches `pending` mentions also inserts any missing row, and `event_key`'s uniqueness makes that safe. Notes create none.

The visibility clause on list, counts, per-id read and mark-all gains: `workspace_id IN (the person's Workspaces) AND author != me`.

Opening a topic calls `POST /workspaces/{id}/topics/{tid}/read`, which inserts read rows for its unread notifications. A topic on screen does this for each arrival, using the existing `isViewing` path. The rail's counts come from the same clause, grouped by Workspace and topic.

**Rationale**: "Unread is the absence of a read row" already means no per-person fan-out, so a message costs one insert whatever the membership. Evaluating membership at read time means removal takes effect at once with nothing to delete, and a person added later sees earlier notifications only within 30-day retention: acceptable, as they now have access to those messages anyway. The inbox stream's `judge` uses the same rule per delivery.

**Alternatives**: One row per recipient. Rejected, since it scales with members × messages and needs deleting on removal.

**Accepted**: a topic with long unread history writes many read rows at once when opened. Retention bounds this at 30 days.

## R18. Permissions

**Decision**: Two new permissions in `AVAILABLE_PERMISSIONS`: `workspaces:create` and `all_workspaces:manage` (read, post and manage any). Decisions live in `workspaces/access.rs`, on the pattern of `projects/access.rs`. Membership is `owner` or `member`. An owner manages; several owners are allowed, and the last owner cannot be removed or demoted except by `all_workspaces:manage`.

An `all_workspaces:manage` holder is treated as a member with management rights but does not appear in the member list or the mention menu until added.

## R21. A typed name is trimmed and lower-cased before it is checked

**Decision**: `workspaces::names::normalize` trims and lower-cases what a person typed, then `validate` applies the rule. So `OPS` is `ops` and collides with it (`409 name_in_use`), while `Not Allowed!` is still refused (`422 invalid_name`).

**Rationale**: FR-001 says unique ignoring case, the quickstart expects `409` for `OPS`, and the mockup's New workspace dialog already lower-cases input. A strict reject would have answered `422` and never reached the uniqueness check. Found by the service test `names_are_unique_ignoring_case_and_follow_the_rule`.
