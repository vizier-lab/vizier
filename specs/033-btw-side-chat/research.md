# Research: /btw Ephemeral Side-Chat

**Feature**: `033-btw-side-chat` | **Date**: 2026-10-28

The design agreed in [vizier-lab/vizier#108](https://github.com/vizier-lab/vizier/issues/108) is the starting point. Each decision below either confirms a point of the issue against the code as it stands, settles one of the issue's six open questions, or records a place where the code forced a change to the issue's sketch. Deviations from the issue are marked **(deviates)**.

## R0. What a side chat is: one aside with follow-ups, discarded at the end

Asked in review on PR #109: is `/btw` a throwaway one-shot question, or does it keep history?

**Decision**: neither extreme. A side chat is **one aside with follow-ups**. `/btw` starts it, and the same person's follow-ups continue it, up to 9 questions. Each answer sees the conversation plus the aside's own earlier turns. When the aside ends (FR-014), **all of it is discarded**: it never enters the conversation's history, memory or any store, so nothing carries over to the next aside or the next conversation turn. "Throwaway" in #108 means *thrown away at the end*, not *one question long*. #108 says the same: "A few follow-up turns are fine inside one exchange … the whole exchange (however many turns) is thrown away together on close/expiry."

**How the turns live without being stored**: the agent side keeps nothing between questions. Every ask carries the aside's earlier turns in the request (`SideChat.turns`), held in between only by whoever shows the aside: the WebUI panel's state in the browser, or the Discord/Telegram reader's process memory (R11). A server restart, a closed panel or the channel's idle timeout ends it.

**One rule for all channels**: follow-ups continue the open aside; invoking `/btw` again always starts a new, empty one. Each channel continues an aside with its native gesture: the panel's composer on the WebUI, the **Follow up** button on Discord (R12), a reply to an answer on Telegram (R13).

**Alternatives considered**:
- **One-shot** (each `/btw` is one question and one answer). It would remove the follow-up machinery (`turns`, the exchange store, the Discord button and form, Telegram reply handling), but it loses the main use case in #108, asking the agent to explain or adjust its own side answer. Rejected.
- **Kept history** (asides saved and visible later). This contradicts FR-006 and the point of the feature. Rejected.

## R1. Where the side chat is dispatched

**Decision**: `VizierRequestContent::Btw(SideChat)` is intercepted in `start_session_job` (`src/agents/process.rs`) next to `checkpoint`/`lobotomy` and routed to `run_btw`, which never calls `VizierAgent::chat`, `handle_request` or `save_history_or_log`. `handle_request` gets a defensive `Btw(_)` arm that returns an error, so a future refactor that bypasses the dispatch fails loudly instead of saving.

**Rationale**: as the issue says, `chat_turn` saves the request (`agent/mod.rs` "Always save request to history") and the reply at four separate sites. Adding a skip flag to `chat()` would have to be remembered at every one of them; the checkpoint/lobotomy precedent keeps the side chat out of that function altogether.

**Alternatives considered**: a `persist: bool` on `chat()` (rejected as above); a separate per-agent channel outside `agent_process` (rejected: new transport, and it would bypass the agent's session queue, see R4).

## R2. Things the agent loop does to *every* request, which a side chat must skip

The issue traced history saving and journaling. Reading `agent_process` found three more writes that happen before or around dispatch and would leave a trace:

| Code | What it writes | Side chat |
|------|----------------|-----------|
| `refresh_session_detail` (spawned for every incoming request, `process.rs:161`) | Creates the session's detail row if there is none, and if the session is still titled by its slug, **generates a title from the request text** with a model call and saves it | **Skipped.** Otherwise a `/btw` asked in a new or untitled conversation would become that conversation's title, and invoking it in a brand-new conversation would create the conversation. |
| `update_thinking_state(true/false)` in `start_session_job` | The session's `is_thinking` flag, shown by the WebUI as the conversation working | **Skipped.** A side chat must not make the main conversation look busy with a turn that never appears. |
| `ThinkingStart` frame | Only sent for `Chat`/`AudioChat`/`BackgroundReport`/`ClientEvent` | Not sent (no change needed: `Btw` is not in that list). |
| `retrieve_auto_context`, `record_context_followup`, `recommend_skills`, `notifications.chat_replied` | Memory retrieval, a follow-up record, skill lookup, a WebUI notification | Not reached: they live in `handle_request`'s `Chat` arm. |

**Decision**: one predicate, `VizierRequestContent::is_side_chat()`, gates the detail refresh and the thinking-state writes. No other branch on `Btw` exists in the loop.

## R3. Durability, restart, timeout (confirms the issue)

`transient: true` on the request makes `is_durable` false, so `VizierTransport::send_request` journals nothing and `resend_pending_turns` never sees it. Verified in `src/transport.rs:242-272` and `src/schema/request.rs:254`.

- Agent restart in the same process: a queued `/btw` is handed on by `hand_on_queued` like any queued request. A *running* `/btw` has no `turn.id`, so `stop_all` treats it as not resumable and `notify_turn_stopped` sends the sender an `AgentUnavailable` error — the person sees a failure and may ask again (FR-012).
- Process restart: the response channel is gone, the caller sees a closed channel. Nothing to recover, by design.
- Timeout: the caller races the reply against the agent's `prompt_timeout`, as `agent_chat` does; the agent side passes the same deadline to `complete_until`.

## R4. Sharing the host conversation's session queue (confirms the issue, with a cost)

**Decision**: the side chat is sent on the host conversation's `VizierSession`, so it queues behind a running turn and messages queue behind it.

**Rationale**: the issue decided this and verified that `session_queues`/`session_is_busy` and scratch-on-idle are in-memory and durability-blind. It also means the side chat always sees the conversation's latest finished reply, and `/abort` stops it with everything else.

**Cost, recorded so it is a decision and not a surprise**: a `/btw` asked while the agent is in the middle of a long tool-using turn waits for that turn, possibly past `prompt_timeout` (408). SC-001's "under 10 seconds" holds only when the conversation is idle, and the spec now says so. Running side chats outside the queue (their own `JoinSet` in `agent_process`, aborted when the instance stops) would remove the wait and is a contained change if this becomes a complaint; it is not done in v1 because the issue settled it and a reviewer agreed.

## R5. Calling the model with no tools (settles "skip tool definitions")

**Decision**: one completion, built with the existing `completion_request(history, message, tools, answer_only: true)`: the turn's tool definitions are **still sent**, with `tool_choice: none`; any tool call in the reply is dropped with `answer_without_tool_calls`-style filtering; if nothing textual is left, the reply is a fixed notice that tools are unavailable in a side chat.

**Rationale**: `completion_request`'s own comment records why — "providers reject a history holding tool calls when no tools are". The host history replayed into the side chat almost always holds `execute_python` calls, so sending no tool definitions would make the side chat fail on Anthropic-dialect providers for most real conversations. Keeping the definitions also keeps the request prefix byte-identical to the host conversation's turns (system prompts, tool list, replayed history), so providers with prompt caching serve most of the side chat from cache — cheaper and faster, which helps SC-001. A model that ignores `tool_choice: none` (dummyplug does, by contract) cannot act anyway: there is no tool loop, and the call is discarded. This is also what makes SC-005 hold with zero exceptions: the reply to a tool-requiring ask is either the model's own explanation or the fixed notice, never an action.

**Alternatives considered**: no tools field (rejected: provider errors as above); flattening the host history's tool calls to text (rejected: breaks the cache prefix and duplicates `history_entries_to_messages`).

## R6. What the side chat sees (settles open question 3)

**Decision**: exactly what the host conversation's next turn would see — `list_session_history_until_checkpoint(session, Some(request.timestamp))` plus the checkpoint handover — then the side exchange's earlier turns, then the question.

- System prompts: `prepare_system_prompts(&snapshot.categories, &session)`, unchanged (same identity, CORE.md, owner, documents).
- Handover: the same system message `chat_turn` builds, extracted into one helper used by both (`checkpoint_context(handover) -> Message`) so the two cannot drift.
- The question is wrapped with `with_context(..., context_md(...))` like a chat turn's message, plus a `## Side Chat` section: this is an off-the-record aside, nothing is saved, the messages after the conversation's last reply belong to it, and no tools can be used — say so if asked to do something that needs one.
- No automatic memory passages, no skill recommendations, no reactions digest beyond what `context_md` is given (`&[]`, `&[]`, `None`, `None`).

**Rationale**: the host conversation is already bounded by automatic checkpointing (the conversation is compacted before it outgrows the context window), so a second, side-chat-specific window would only break the cache prefix (R5) without bounding anything new. The side exchange itself is bounded by R8.

## R7. Usage accounting (settles open question 2)

**Decision**: `UsageScope::turn(Some(host_session), &TurnId::new(), 1)` — no new `UsagePurpose` or `UsageActivity`.

**Rationale**: `complete_until` already writes one `usage_event` per answered call, so SC-006 holds with no new code. The activity comes from the host channel (`chat` for WebUI/Discord/Telegram), and the cost lands in the host conversation's breakdown, which is where the person spent it. The row holds token counts, provider, model, channel and topic — no content — and nothing the agent can read. No `TurnGuard`: a single completion that errors records nothing, which is the existing rule for unanswered calls.

**Alternatives considered**: a `side_chat` activity (rejected for v1: a new enum value in `UsageActivity::ALL`, the usage API schema and the WebUI usage page, for a distinction nobody has asked to see; it is additive later).

## R8. Exchange limits (settles "a small number of follow-ups")

| Limit | Value | Where enforced |
|-------|-------|----------------|
| Earlier turns carried in one exchange | at most **8** (so 9 questions) | `side_chat::ask` (all channels); a 10th question is refused with "start a new side chat" |
| Question length | at most **4,000** characters | `side_chat::ask` |
| Earlier answer length (client-supplied, WebUI) | at most **16,000** characters each | `side_chat::ask` |
| Attachments | none | the request carries none |

Constants in `src/channels/side_chat.rs`, not config (Principle I; the issue recommends constants for v1).

## R9. Rate limiting (settles where the limiter lives) — **(deviates)**

**Decision**: one `RateLimiter` (the existing sliding-window type, reused in place from `channels/http/rate_limit.rs`), **5 asks per 60 s**, held on `VizierDependencies` (as `side_chat_limits`), checked inside `side_chat::ask` before anything is sent. The module's doc comment, which says the limiter lives on `HTTPState`, is updated to say it is also shared with the Discord and Telegram readers.

**Why not `HTTPState`** (the issue's suggestion): the Discord and Telegram readers are given `VizierDependencies`, not `HTTPState`, so a limiter there could not cap them. `HTTPState` already carries `deps`, so the HTTP handler reaches the same instance.

**Key**: the person as each platform identifies them — `http:{username}`, `discord:{user id}`, `telegram:{user id}`. **(Narrows the original spec assumption, which the spec now states this way.)** The first draft of the spec assumed one limit per person across platforms. Vizier has no verified link between a WebUI account and a Discord/Telegram user: `UserProfile.discord_id`/`telegram_id` are self-declared and there is no lookup by them. Keying on a self-declared link would let anyone drain another person's budget by claiming their id. The limit therefore holds per identity: across all of a WebUI user's tabs and API keys, and per Discord or Telegram account. A cross-platform key can be added once identity links are verified.

**Over the limit**: `429` with `retry_after_secs` on HTTP (shown inline in the WebUI panel); an ephemeral reply on Discord; a self-deleting reply on Telegram.

## R10. Entry point for the WebUI/API (settles open question 1)

**Decision**: a sibling route, `POST /api/v1/agents/{agent_id}/btw`, with its own request type (`BtwRequest`: `channel_id`, `topic_id`, `question`, `turns`). The send-and-await loop from `agent_chat` (send on the transport, wait for the first `Message`/`Error`/`Abort` under `prompt_timeout`, map to 200/500/408) moves into `side_chat::ask`, and `agent_chat` keeps its own copy only if extracting it would change its behaviour; the target is one implementation.

**Rationale**: as the issue argues, a `kind` field on `ChatRequest` would make one endpoint "sometimes saved, sometimes never", and `ChatRequest` carries attachments a side chat must refuse. Same auth (`require_auth`, `user_can_view_agent`), same session construction (`HTTP(username, channel_id)`, topic), so a person can only ask beside their own conversations.

## R11. Who keeps the exchange's earlier turns — **(extends the issue)**

The issue's `Btw(String)` carries one question; follow-ups need the earlier turns, and the server must not keep them anywhere durable.

**Decision**: the request carries the whole exchange (`SideChat { question, turns: Vec<SideChatTurn> }`), so the agent side is stateless. Who holds the exchange between asks depends on whether the channel has a client:

- **WebUI**: the side panel's component state (a plain array, discarded on close/unmount, as the issue specifies). It is sent with every ask.
- **Discord, Telegram**: there is no client to hold it, so the reader holds it **in process memory** in one shared `SideChats` store (`src/channels/side_chat.rs`), keyed by an exchange id, swept on a timer (R12/R13). Nothing in it is written to storage; a restart loses it.
- **The sweep is shared too (D1)**: `SideChats::spawn_sweeper(every, on_end)` owns the 15 s interval, the `sweep(now)` call and the per-exchange loop. Each platform supplies only `on_end`, its delete call (Discord: the interaction-token deletes; Telegram: `deleteMessage` plus the ledger clear), so the loop exists once (Principle II). The returned handle is aborted when the reader stops.

The WebUI's transcript is client-supplied, so a person could put words in the agent's mouth in their own aside. That is accepted: the side chat has no tools and saves nothing, so a forged earlier turn can only change the answer the forger sees.

## R12. Discord: entry and follow-ups (settles open question 5) — **(deviates)**

**Decision**:

1. `/btw` is a registered built-in slash command with one **required** `message` option (`builtin_commands()`), answered with a deferred response flagged `EPHEMERAL`, then edited to the answer.
2. The answer carries a **Follow up** button (`custom_id` `btw:{exchange id}`). Clicking it opens a Discord modal with one text input; submitting it is answered ephemerally like step 1, appended to the same exchange.
3. A new `/btw` always starts a new exchange.
4. Lifetime: an exchange ends after **5 minutes** without activity or **14 minutes** after it started, whichever is first. On end, the reader deletes every ephemeral message of the exchange through its interaction token (`delete_response`), then forgets the exchange. A **Follow up** click on an ended exchange gets an ephemeral "This side chat has ended — start a new one with /btw."

**Why not follow-ups as "normal replies within the interaction window"** (the issue's recommendation): an ephemeral message cannot be replied to, and a normal channel message is public, which breaks FR-008. The button + modal keeps every step ephemeral using only interaction responses.

**Why not "run `/btw` again to continue"**: then `/btw` would mean "new aside" on the WebUI and Telegram but sometimes "continue" on Discord, and a person couldn't start a fresh aside within the idle window. The button makes continuing explicit and keeps `/btw` = new aside on every channel (R0).

**Why 14 minutes**: an interaction token is valid for 15 minutes. Every message in the exchange was created after the exchange started, so with a 14-minute cap every token is still valid when the exchange ends and its messages can be deleted — the "no longer accessible to anyone" scenario (US2-3) holds, and expiry is visible rather than a silent failure.

Answers over Discord's 2,000-character limit are split, the rest sent as ephemeral follow-ups (`create_followup` with `EPHEMERAL`); each chunk's message id is kept for deletion.

## R13. Telegram: entry, follow-ups, removal (settles open question 4) — **(deviates)**

**Decision**:

1. `/btw <question>` (also `/btw@bot`) is recognised with `is_command` before `/skill` and chat/`SilentRead` parsing, so it never reaches the agent as `Chat` or `SilentRead` (both of which are saved). It is added to the bot's built-in command menu.
2. The answer is sent as a **reply to the person's message**, prefixed with a one-line notice: this is a side chat, it isn't saved, and it is deleted after **2 minutes** without activity.
3. A follow-up is a Telegram **reply to one of the exchange's answers**. Replies are checked against the live exchanges before any other parsing; they are delivered to bots in privacy mode. A reply is classified like this, first match wins, and **no branch ever forwards it as `Chat` or `SilentRead`**:
   - It replies to a message of a **live** exchange, and comes from that exchange's person → a follow-up.
   - It replies to a message of a live exchange, but comes from **someone else** (a group) → not answered, not forwarded; its id joins the exchange's deletion list. It belongs to a thread that is about to disappear, and keeping it as an ordinary message would leave a dangling, possibly quoting, trace.
   - It replies to a message on the **deletion ledger** but no live exchange owns it. This happens in the window between an exchange ending and the 15 s sweep, and after a restart, when the in-memory store is empty but the messages are still visible. → reply "This side chat has ended — start a new one with /btw.", and put both the reply and the person's message on the ledger with the ended exchange's deadline (or now + 2 min if it has passed).
   - Otherwise it isn't a side-chat reply, and normal parsing continues.

   The ledger check reads one state key, and only for messages that reply to one of the bot's own messages, so ordinary traffic costs nothing. Without the ledger branch, a person in a private chat who replies a few seconds late, or after a restart, would have their follow-up handled as an ordinary message and **saved to the conversation's history**, which breaks FR-006.
4. Lifetime: **2 minutes** idle (the issue's proposed default, as a constant). A sweep every 15 s deletes the person's messages and the bot's replies with `deleteMessage`, so removal lands within 2 min 15 s of the last activity — the stated window in the notice (SC-004).

**Deletion is best-effort where Telegram forbids it**: a bot may always delete its own messages and incoming messages in a private chat; in a group it may delete other people's messages only as an admin with *delete messages*. Without that right, the person's own `/btw` message stays. Rather than query the bot's rights, the notice in a group says so plainly ("…your messages too, if I'm allowed to delete them here"); a failed delete is logged at `debug` and dropped from the ledger.

**Restart — (deviates from "no DB row, ever")**: a timer in memory dies with the process, which would leave an exchange in the chat indefinitely and break SC-004. The reader therefore keeps a **deletion ledger** in the existing key-value state storage (`StateStorage`). It is **one key per agent**, `side_chat_pending__{agent_id}`, holding items of chat id, message ids and a delete-after time — **no text, no user, no question or answer**. It is written when a message is posted, cleared as soon as the deletion has been attempted, and processed when the reader starts. This is the minimum needed to keep the promise made in the notice; the turn itself is still never stored. The spec's FR-006 names it as the one kept note besides cost. The alternative (no ledger, leftovers after a crash) was rejected because it makes SC-004 false after any unclean restart.

**Ledger hygiene (I3)**: `StateStorage` has only `save_state`/`get_state`, so it gains one method, `delete_state(key)` (a single `DELETE`, implemented once in `SqliteStorage` and forwarded by `VizierStorage`). With it:

- The key is deleted when its item list becomes empty.
- Deleting an agent deletes its key, in the same path that deletes the agent's other rows.
- At reader start, items older than **48 hours** are dropped without trying, because Telegram refuses to delete messages that old.
- If an agent's Telegram connection is removed and never replaced, its key stays (content-free) until the agent is deleted. A later connection with a different bot token will try the deletions, get refused, and drop the items.

One key per agent, rather than one per chat, is what makes deletion by agent possible without a prefix listing.

## R14. WebUI presentation

**Decision**: typing `/btw` (optionally followed by a question) in the composer opens a **side panel** (`SlideOver` from `@vizier/design`, the same panel the notifications use) titled "Side chat", with the agent's model shown read-only, a note that nothing in it is saved, the exchange's turns (design `AgentReply` for answers), and a compact composer for follow-ups. The question after `/btw`, if any, is asked immediately. Closing the panel discards the exchange with no confirmation. `/btw` is added to the composer's slash menu (`BUILTIN` in `Composer.tsx`) and intercepted in `send()` alongside `checkpoint|lobotomy|abort`, before anything is sent on the socket.

**Rationale**: the spec's US1 says "a side panel", and a panel leaves the main conversation visible to ask about. No new design component is expected; if one turns out to be needed, it lands in `design/` first (constitution's commit order design → webui → vizier).

## R15. Telemetry and logs

**Decision**: the side chat opens the usual `invoke_agent` span (via `invoke_agent_span`, `vizier.request.kind = "btw"`) and the model call its usual `chat {model}` span. Message content is attached to spans only when the operator has opted in with `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true`, exactly as for every other turn; the side chat does not override that operator decision. No `tracing` line logs a question or answer. `docs/` states both facts so "nothing is saved" is not read as covering an operator's own trace export.

## R16. Scope check against "no tools, ever" (issue open question 6)

Not a v1 question for this plan: no tools is the spec's FR-004. Nothing in this design (stateless agent side, no `ToolContext`, no tool loop) makes a later opt-in harder, but none is planned.
