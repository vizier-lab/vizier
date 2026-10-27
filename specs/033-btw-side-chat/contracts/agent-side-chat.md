# Contract: `VizierRequestContent::Btw` in the agent loop

**Feature**: `033-btw-side-chat`

The internal contract every channel relies on. All three entry points call `channels::side_chat::ask`, which is the only constructor of a `Btw` request.

## Sending

`side_chat::ask(deps, session, person, side_chat, timeout)`:

1. Validates `side_chat` (data-model limits) → `Invalid`.
2. `deps.side_chat_limits.check(person)` → `RateLimited`.
3. Builds `VizierRequest { content: Btw(side_chat), transient: true, user, timestamp: now, attachments: [], ..Default::default() }` and sends it with `deps.transport.send_request(session, request, Some(tx))`. Because `transient` is set, no `session_turn` row is written.
4. Waits under `timeout` (the agent's `prompt_timeout`) for the first `Message` (→ answer), `Error` (→ `Agent`), `Abort` (→ `Agent`), or a closed channel (→ `Agent { AgentUnavailable }`). Other frames are skipped.

## Receiving (`src/agents/process.rs`)

| Step of `agent_process` | For `Btw` |
|-------------------------|-----------|
| `refresh_session_detail` spawn | **Not spawned** (`is_side_chat()`) |
| `abort` / `dream` handling | n/a |
| Queue when the session is busy | **Same as any request** |
| `start_session_job` thinking frame | None (not a chat kind) |
| `update_thinking_state(true/false)` | **Skipped** |
| Dispatch | `Btw(_) => run_btw(...)`, beside `checkpoint`/`lobotomy`; never `handle_request` |
| Turn end (`join_next`) | As any turn: no `finish_turn` (no id), next queued request starts, scratch cleared when idle |
| Instance stop while running | Not resumed (no turn id); sender gets `AgentUnavailable` |
| Instance stop while queued | Handed on to the next instance, or `AgentUnavailable` if deleted |
| `/abort` on the conversation | Aborts it with the rest; sender gets `Abort` |

## `run_btw` → `VizierAgent::side_chat`

```text
history, handover = storage.list_session_history_until_checkpoint(session, Some(req.timestamp))
snapshot          = agent.tools.load_full()
messages          = prepare_system_prompts(&snapshot.categories, &session)
                  + [checkpoint_context(handover)]?
                  + history_entries_to_messages(&history)
                  + for turn in exchange: [User(turn.question), Assistant(turn.answer)]
message           = with_context(User(req.to_prompt()), context_md(&[], &[], None, None) + SIDE_CHAT_MD)
request           = completion_request(&messages, &message, &snapshot.tools(), answer_only = true)
response          = model.complete_until(request, Some(now + prompt_timeout),
                                         &UsageScope::turn(Some(session), &TurnId::new(), 1))
text              = text parts of response (tool calls dropped); SIDE_CHAT_NO_TOOLS if empty
send Message { content: text, stats } on response_tx
```

- Runs inside `invoke_agent_span(req, session, false)` (`vizier.request.kind = "btw"`).
- No `ToolContext` with a deadline, no `ToolRouter` call, no `StepRecorder`, no hooks, no `publish_session_event`, no `save_history_or_log`, no `store_session_file`, no auto-context retrieval, no skill recommendation, no notification, no checkpoint (`maybe_checkpoint` is not called even when the context is large).
- A completion error is sent as `Error { kind: ErrorKind::classify(..), message }`.

`SIDE_CHAT_MD` (in `system_prompt/`, rendered as a section of the context block, never a system message):

```markdown
## Side Chat
This is an off-the-record side chat beside the conversation above. Nothing in it is saved:
you won't remember it, and it won't appear in the conversation. Any messages after the
conversation's last reply belong to this side chat. You can't use tools here — if asked to
do something that needs one (searching memory, running code, reading files, contacting
anything), say you can't in a side chat and suggest asking in the conversation instead.
```

`SIDE_CHAT_NO_TOOLS` (the reply when the model offered only a tool call):

```text
I can't use tools in a side chat. Ask me in the conversation if you'd like me to do that.
```

## Invariants (checked by tests)

- After any number of `Btw` requests, `list_session_history` for the host session is unchanged, `get_session_detail_by_topic` is unchanged (or still absent), and `pending_turns` for the agent is unchanged.
- A `Btw` reply never has a `history_uid`.
- `is_durable` is false for every request `side_chat::ask` builds.
