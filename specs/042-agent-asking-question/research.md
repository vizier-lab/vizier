# Research — Asking the Person (042)

**Feature**: `042-agent-asking-question` | **Date**: 2026-11-01

Every decision below was checked against the tree on branch `042-agent-asking-question`, not
against the docs. Anchors are `path:line` at this commit. The two tools are implemented as one
module (`crates/vizier-core/src/questions/`) because they share the conversation, the access
rule, the redaction boundary and the limits; they differ only in where the answer goes.

## Verified anchors

| Fact | Where |
|------|-------|
| 17 tool categories, `ALL`, `name()`, `description()` | `crates/vizier-core/src/agents/tools/mod.rs:91-188` |
| `VizierTool::ends_turn()`, `CatalogueEntry.ends_turn`, `ENDS_TURN_NOTICE` | `tools/mod.rs:489-520`, `:604-619` |
| `turn_catalogue` / `turn_categories` (the per-turn offer) | `tools/mod.rs:678-706` |
| `ToolRouter::catalogue` / `dispatch` | `tools/mod.rs:810-954` |
| `ToolContext` fields (session, client, redactor, deadline, project) | `tools/mod.rs:213-247` |
| `RouterBridge` dispatches script calls through the router, stamps `ends_turn`, applies the tool timeout | `tools/python/bridge.rs:69-152`, `:180-245` |
| A tool's output can already be a whole `VizierResponse` | `tools/mod.rs:637-650` |
| `execute_python` scrubs its report *before* it leaves the tool | `tools/python/mod.rs:107-110` |
| `ExecutionReport::ended_turn()` / `ending_call()` | `sandbox/report.rs:107-128` |
| The loop ends the turn and answers with the step's own text | `agents/agent/mod.rs:1356-1360`, `:1418-1426` |
| `VizierResponseStats.ended_by_tool` | `schema/response.rs:30-32` |
| `SecretRedactor::arm` (silently ignores < `MIN_VALUE_BYTES`), `MIN_VALUE_BYTES = 8`, `MAX_VALUE_BYTES = 65_536` | `secrets/redact.rs:32-41`, `secrets/mod.rs:17-22` |
| Client-tool round trip with a oneshot answer and a saved call row | `agents/client/mod.rs:241-330`, `agents/client/connection.rs:53-71` |
| `SessionFrame` variants and the one-way session broadcast (capacity 1024, lag drops frames) | `transport.rs:17`, `:35-68` |
| WebSocket inbound frame dispatch (`client_tool_result`, `client_call_state`, reaction, request) | `vizier-http/src/topic.rs:144-291` |
| WebSocket outbound frame mapping | `vizier-http/src/topic.rs:294-364` |
| `client_error` frames and rate limits | `vizier-http/src/api/v1/agents/channel.rs:398-474` |
| `seal_open_calls` closes what a dead turn left open | `agents/steps.rs:101-143` |
| Startup sweeps (interrupted runs, jobs, client calls, scratch) | `dependencies.rs:187-190` |
| History read-time merge of `client_tool_call` | `storage/sqlite/history.rs:115-168`, `:180-192` |
| `channel` / `topic` are part of every row's key | `storage/sqlite/history.rs:869-882` |
| Explicit deletes per conversation, because cascades need `PRAGMA foreign_keys` | `storage/sqlite/session.rs:336-400` |
| `VizierChannelId::platform()` and `web_user()` | `schema/session.rs:83-115` |
| Discord handles `InteractionCreate` already; `twilight_util::builder` is available | `channels/discord/mod.rs:105-111`, `:172-174` |
| Telegram polls only Message / EditedMessage / MessageReaction today | `channels/telegram/mod.rs:166-178`, `:212-230` |
| `is_non_user_channel` (SYSTEM, SUBAGENT, DREAM__, task__, inter_agent__) | `storage/history.rs:11-17` |
| Dummyplug request shape `{"tool": …, "arguments": {…}}` | `agents/agent/model/dummyplug.rs:277-307` |

The `webui/` and `design/` submodules are **not checked out in this worktree**
(`git submodule status` shows all four uninitialised). Everything below about those two
repos is written against the shipped contracts in `specs/019-client-tools/contracts/`,
`specs/026-generative-ui-terminal-tools/contracts/` and `CLAUDE.md`, and the implementation
step must start with `just checkout 042-agent-asking-question` to read the real code.

---

## R1 — Where the tools live, and how they disappear

**Decision**: two native tools in a new `questions` module, registered in `BaseTools` like
every other tool, with `ToolCategory::Questions` added as the 18th category (declaration order
after `Secrets`). `ask_sensitive_input` sits in the existing `ToolCategory::Secrets` beside
`set_secret`/`get_secret`. Availability is decided **per turn by the tool itself**, through a
new default method on the tool trait:

```rust
fn available(&self, ctx: &ToolContext) -> bool { true }
```

`ToolRouter::catalogue` takes the turn's `ctx` (it is already passed to `dispatch`) and drops
entries whose native tool says no. `turn_categories` drops a *native* category that has no
available tool this turn, so an unattended prompt never advertises an empty `questions` line.

**Rationale**: the category list is otherwise static, and both tools must vanish in dreams,
task runs, background pieces, inter-agent turns, side chats and (for the sensitive tool) every
non-WebUI channel. A name-based filter inside `turn_catalogue` would be a second place to
remember every condition; a trait method keeps the condition next to the tool (Principle II).

**Alternatives considered**:
- *Filter by name in `turn_catalogue`*: rejected — a `match` over tool names in shared code,
  which the constitution rejects and which drifts the moment a third conditional tool appears.
- *Put both tools in a client-declared category*: rejected — the WebUI declaring them would
  make them WebUI-only, and they must work on Discord and Telegram (FR-001, FR-007).
- *Do not filter, and fail at call time*: rejected — FR-007/FR-046 require the tool to be
  absent, because a tool the agent can see is a tool it will try; a dream would burn a round
  discovering it cannot ask.

**Cost**: `ToolRouter::catalogue` gains a parameter (3 call sites), `ToolContext` gains one
field (R2), and `VizierToolDyn`/`VizierTool` gain a defaulted method. No dispatch changes.

## R2 — What makes a turn interactive

**Decision**: add `ToolContext.interactive: bool`, computed once in the agent loop from the
request, with the answer being a whitelist:

```
interactive = matches!(content, Chat | AudioChat | ClientEvent | QuestionAnswer)
              && session.1.platform().is_some() || session.1.web_user().is_some()
```

`Prompt` (REST), `SilentRead`, `Unattended`, `BackgroundReport`, `Command` and `Btw` are *not*
interactive. The sensitive-input tool adds the stricter rule `session.1.web_user().is_some()`.

**Rationale**: the spec lists the unattended kinds explicitly (FR-007, FR-046) and side chats
are identified by request kind, not channel, so a channel-slug predicate alone
(`is_non_user_channel`) is insufficient. A whitelist fails closed: a future request kind is
unattended until someone decides otherwise.

**Alternative considered**: reuse `is_non_user_channel(session.1.to_slug())` — rejected: it
misses side chats and background-report turns, and it is `pub(crate)` to the storage layer.

## R3 — Storing a question set, and how the card survives a reload

**Decision**: one new table, `question_set`, holding the immutable set (JSON) and the mutable
state; a new `SessionHistoryContent::QuestionSet { … }` entry written when the set is presented,
whose `uid` **is** the set id. `state`, `answers`, `answerer` are filled in at read time from the
table by a `fill_question_sets` pass, exactly as `fill_client_calls` fills a client call's
`outcome`/`state` (`storage/sqlite/history.rs:115-168`).

**Rationale**: this is the shipped pattern for "a thing shown in the conversation whose state
changes later" (spec 019). It gives FR-016 (reload, second tab, another device), FR-020 (the
card shows pending/answered/declined), FR-061 (stored with the conversation, deleted with it)
and FR-025 (controls resolve from storage across a restart) with no new transport.

**Alternatives considered**:
- *Render the card from the `execute_python` report's `tool_calls`*: rejected — the report is
  a debugging artefact, its model copy drops `arguments`, and nothing identifies the set for
  an answer or updates its state.
- *A `SessionFrame` only, no history entry*: rejected — the frame is best-effort (lag drops)
  and a reload would lose the card, breaking FR-016 and US9.

## R4 — Getting the question to a channel

**Decision**: the tool presents **inside the call**, through a small registry in
`VizierDependencies`: each running `VizierChannel` reader registers an
`Arc<dyn QuestionSurface>` for its `(agent_id, platform)` while it runs, and `ask_question`
resolves the surface for the session's channel and awaits it. `QuestionSurface::present`
returns the platform message references (or an error) only after the controls are posted. The
WebUI needs no surface: its "presentation" is the history entry plus the `history_entry` frame
`record_step` already publishes.

**Rationale**: FR-010 is explicit that a delivery failure must fail the call and keep the turn
running, so the person never sees a card for a set the agent was told it presented. A
best-effort `SessionFrame` cannot report failure. The registry mirrors what
`VizierTransport::send_memory_op` already does (a channel + oneshot response between core and
a per-agent component).

**Alternatives considered**:
- *Publish a `SessionFrame` and let readers post asynchronously*: rejected for the failure
  reporting above; also silently no-ops when no reader is connected (a discord card would
  never appear and the agent would not know).
- *Have `ask_question` call the Discord/Telegram API itself*: rejected — the readers own the
  platform clients, the bot identity and the message-link bookkeeping; a second client path
  in core duplicates that, and a question must be one message edited by the reader afterwards.
- *Render controls after the turn's reply, driven by `stats.ended_by_tool`*: rejected — the
  answer would be unanswerable whenever the reply was not posted (channel error, abort).

## R5 — Discord controls

**Decision**: components on the question message, `custom_id` = `<short>.q<index>.<option>`
(≤ 100 bytes, the platform limit), where `<short>` is an 8-character reference indexed on
`question_set.short_id` and `<index>` is the question's position in the set — never its id, which
the agent may make 64 bytes long and which would not fit Telegram's 64-byte `callback_data`:

| Question type | Control | Limit that drives it |
|---|---|---|
| choose one | one action row of buttons (≤ 5) | 5 buttons per row, 5 rows per message |
| choose one, more options | one string `select` | 25 options per select |
| choose several | string `select` with `min_values`/`max_values` + a submit button | 25 options |
| confirm | two buttons | — |
| free text | a modal opened by a button on the question message (one text input per question, ≤ 5) | modal: 5 inputs, 4 000 chars each |

Answering edits the message into a record of the answers and removes the components (FR-024);
a stale press is answered ephemerally and starts no turn. Every interaction is acknowledged
with `InteractionResponseType::DeferredUpdateMessage` (or `ChannelMessageWithSource` ephemeral
for a refusal) inside Discord's 3-second window (FR-023).

**Rationale**: buttons and selects are what the existing `twilight` dependency and
`twilight_util::builder::InteractionResponseDataBuilder` already provide, so no new dependency
and no extra bot permission (FR-031). A modal is the platform's own text entry, which FR-022
asks for, and its `custom_id` carries the same short reference as a button.

**Alternatives considered**: a text rendering with a syntax (rejected in the spec); a thread
per question (extra channel spam, no way to edit the question); command-based answers
(`/answer 1:a` — the "typed syntax" the spec rejects).

## R6 — Telegram controls

**Decision**: one inline keyboard per question, posted as one message per question when a set
has more than one question (FR-029: split, never merged), all carrying the same `<short>`
reference. `callback_data` is capped at **64 bytes**, so it is the same short reference, the question's
position and the option index — never a JSON payload, and never the question id.

| Question type | Control |
|---|---|
| choose one | one row of `InlineKeyboardButton` per option |
| choose several | toggle buttons (`☐`/`☑`) whose press edits the keyboard, plus `Submit` |
| confirm | two buttons |
| free text | the person replies to the question message; the reply body is the answer (documented text form, FR-027) |

`AllowedUpdate::CallbackQuery` and an `UpdateKind::CallbackQuery` branch are added to the
reader; every press is acknowledged with `answer_callback_query` before any work (FR-023).

**Rationale**: Telegram has no multi-select and no modal, so a toggle row plus submit is the
closest native control and keeps one-press semantics; a set that does not fit the 64-byte /
keyboard limits falls back to the text form. Editing the message after the answer is a single
`edit_message_text` with the keyboard removed.

**Alternatives considered**: a single keyboard of `<n>` numeric buttons per question (loses
option labels); force the reply-to text form for everything (the "worse than prose" outcome
the spec rules out).

## R7 — The answer is a request, so it takes the normal path

**Decision**: a new `VizierRequestContent::QuestionAnswer(QuestionAnswer)` — display text
(`summary`), the structured answers, the set id, and the answerer. All three surfaces submit
the same value: the WebUI as a `question_answer` field on a normal request frame (pre-checked in
`handle_inbound`, like `check_client_event`), the channel readers by building the request after
their own press, then `transport.send_request` on the **session stored with the set**.

**Rationale**: FR-015 (same history, same queue, same attribution, no bypass) is then free,
and FR-062 (the agent sees a structured answer that is not a typed message) comes from one
`Display` implementation, like `ClientEvent` already does (`schema/request.rs:97-106`).

**Alternatives considered**: a `client_event` with `data`. Rejected: the agent cannot tell a
question answer from a drawn-UI event (FR-062), the name says "client" while Discord and
Telegram submit the same thing, and `source_call_id` means a client tool call.

**Claim and validation**: `questions::submit()` is one function: validate against the stored
set (required answered, choice value among the options, multi-select bounds, text length,
unknown/duplicate ids, total size), then claim atomically
`UPDATE question_set SET state='answered' … WHERE set_id=? AND state='pending'`. Zero rows
changed means already answered/declined/withdrawn → the surface refuses with "already
answered" (FR-017, SC-010). Every surface calls this function, so the rules cannot drift.

## R8 — A sensitive value has to reach a script that is already blocked

**Decision**: `ask_sensitive_input` is a native tool whose `call()` publishes
`SessionFrame::SensitiveAsk`, registers a oneshot in an in-memory registry keyed by `ask_id`,
and awaits it with `min(ASK_WAIT, ctx.deadline)`. `ctx.deadline` is the tool deadline the
sandbox already publishes (`tools/mod.rs:229-231`), and the script call is itself wrapped in
`tokio::time::timeout(tool_timeout)` by `RouterBridge` (`python/bridge.rs:123`), so the wait
must be strictly shorter than that or the script would see `timed out after …` instead of a
branchable timeout result.

**Rationale**: this is the only place in the system where a tool call may block on a person for
a long time, and the machinery for exactly that already exists in `ClientTools::round_trip`
(frame + `register` + `tokio::time::timeout` + outcome row). The difference is who may answer:
any socket on the conversation, not one declaring connection, so the registry keys on
`ask_id` and lives in `VizierDependencies` (reachable from the WebSocket handler via
`HTTPState.deps`).

**Alternatives considered**:
- *Defer to the next turn* (hand the value over as a message): rejected — it would put the
  value in exactly the copy that must stay clean (spec Context).
- *Poll storage for a value row*: rejected — the value must never be at rest.
- *A REST endpoint for the answer*: not needed (R11).

## R9 — Sensitive-ask lifecycle

**Decision**: a `sensitive_ask` row (`open` → `provided` | `declined` | `timed_out` |
`abandoned`) with **no value column**, plus an in-memory registry of open asks:

- On present: insert the row, publish the frame, register the oneshot, take the per-conversation
  slot (FR-053: one open ask per conversation; a second call waits for the first to close).
- On a new socket opening (`TopicConnection::open`): push every open ask of that session, so a
  second tab or a reload shows the dialog (FR-048).
- On answer: refuse unless `open` and the session matches; send on the oneshot; close the row.
- On decline/dismiss: same path with a declined outcome (FR-036, FR-050).
- On timeout: close as `timed_out`, script gets `{"status":"timeout"}` (FR-051).
- On turn abort/panic and on restart: `Questions::abandon(session)` closes open asks, called
  next to `seal_open_calls` (`agents/process.rs:293`) and in the startup sweeps
  (`dependencies.rs:187-190`), so an ask is never answerable against a dead turn (FR-049).

**Rationale**: the record is what the transcript shows ("a value was requested and provided",
FR/US4 #5, FR-054); the value itself is only ever a local in the tool and in the script. The
transcript entry is a second new history variant, `SessionHistoryContent::SensitiveAsk`, filled
at read time from this row exactly as `QuestionSet` is from `question_set`.

## R10 — Redaction: arm at receipt, and say what it cannot do

**Decision**: the value is armed with `ctx.secret_redactor.arm(&value)` the moment it arrives,
before the tool returns it to the script. `MIN_VALUE_BYTES`/`MAX_VALUE_BYTES` are reused, not
redefined. A value shorter than the minimum is still delivered — FR-043 only forbids silently
pretending it is protected: the dialog shows the warning *before* submission (`protected:
false`), and the tool returns `protected` so the script can say so too.

**Rationale**: `execute_python` scrubs the report before it becomes history, the model's copy,
the log line and the telemetry (`python/mod.rs:107-110`), and the bridge scrubs the nested
call's arguments and error (`python/bridge.rs:211-217`). Nothing else needs to change: the
value never crosses those boundaries in a form the redactor has not armed.

**Stated limits** (tool description + docs): exact-string only; a transformed form (base64,
hash, split) escapes it; a value the script writes to a file, memory, scratch or an outbound
body is outside the guarantee (FR-057); a person can still type the value into an ordinary
message (FR-058).

## R11 — The WebUI answers over the socket, not REST

**Decision**: two new inbound frame kinds on the existing topic WebSocket
(`question_answer` is a normal request whose content is the answer; `sensitive_input_answer`
is a small frame), and three new outbound ones (`question_set` state change,
`sensitive_ask_request`, `sensitive_ask_closed`). No new REST endpoint.

**Rationale**: the browser and the desktop app both drive `TopicConnection`
(`vizier-http/src/topic.rs:1-6`), so one path serves both, keeps the principal check, and
avoids a second surface to secure. A `client_error` refusal keeps the shipped error shape.

**Consequence to document**: the wire protocol is additive — clients must ignore frame kinds
they do not know (client-tools protocol §9), so an older WebUI keeps working and simply never
shows a dialog (the turn then times out and the script branches).

## R12 — Limits (one table, one home)

| Limit | Value | Why this value |
|---|---|---|
| questions per set | 10 | answered in one submission; keeps the card usable on a phone |
| options per question | 25 | Discord select maximum; beyond it the text form is used |
| prompt / help / label | 500 / 500 / 100 bytes | one card, readable, fits a Discord component label |
| option value | 100 bytes | fits `custom_id`/`callback_data` budgets with the short reference |
| text answer | 4 000 bytes | Discord modal input maximum |
| whole set payload | 64 KiB | matches the client-event data budget |
| whole answer payload | 64 KiB | same |
| sensitive value | 8 – 65 536 bytes | `MIN_VALUE_BYTES`/`MAX_VALUE_BYTES`, reused |
| sensitive ask wait | 120 s, capped by the tool deadline | a person is present; a script has a budget |
| open asks per conversation | 1 | FR-053 |
| question sets per turn | 1 | FR-008 |

## R13 — Compatibility and #82

New tables (`question_set`, `sensitive_ask`) and new enum variants are additive: the schema is
one `CREATE TABLE IF NOT EXISTS` batch (`storage/sqlite/mod.rs:68-1009`), so a new table needs
no migration and no `add_column_if_missing`. There is **no** back-compat code, so this feature
adds **nothing to #82** (same reasoning as `specs/019-client-tools/plan.md`). Serialised
`SessionHistoryContent` and `VizierRequestContent` gain variants; history rows written before
this feature simply never contain them, and `#[serde(default)]` is not needed because no
existing field changes.

## R14 — How this gets verified

- **Unit/integration (Rust)**: validation (every FR-005/FR-012 rule), the atomic claim
  (two submits, one wins), redaction of a supplied value through `execute_python`, the
  availability filter (interactive vs task/dream/side chat/background), short-id resolution,
  the text-form parser, and the read-time `question_set` merge.
- **End to end (`dummyplug`)**: `quickstart.md` scripts every check as a `{"tool":
  "execute_python", "arguments": {"code": …}}` message, per the constitution's e2e gate.
- **Not automatable offline**: the Discord and Telegram controls and their acknowledgement
  deadlines need a real bot; the sensitive dialog needs a person (or a browser). Both are
  scripted as manual steps with a stated expected outcome, and the plan's task list keeps them
  separate so a green `cargo test` is never mistaken for "channels work".
- Gate commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  --deny warnings` (CI flags), `cd webui && npm run typecheck`, `cd design && npm test`, and
  the `dummyplug` run from `quickstart.md`.

## R15 — Submodules, docs and drift

- `webui/` and `design/` are empty in this worktree: run `just checkout
  042-agent-asking-question` before touching them, and commit design → webui → vizier.
- `CLAUDE.md` says `secrets` is "the 16th and last built-in category"; it is 17 since `coding`
  (041) and becomes 18 with `questions`. Fix that line as part of this feature (it is a fact
  about the code this change edits, not a drive-by).
- User docs: `docs/src/api-integration/client-tools.md` gains the new frame kinds,
  `docs/src/configuration/channels.md` the channel behaviour, and a new
  `docs/src/configuration/questions.md` describes the two tools in agent terms.

## R16 — Rejected overall shapes

- **One tool for both** (`ask_question` with a `secret: true` flag): rejected — the two differ
  in turn-ending, delivery surface, storage and access rules; one tool would carry four
  conditionals and a description that must warn about half of itself.
- **Client tool for questions** (WebUI declares it, like `ui_render`): rejected — FR-001
  requires every channel, and a client tool is by construction one client's.
- **A new transport or store**: rejected — `SessionFrame` + SQLite + the request queue already
  carry every requirement (Principle I/III).
