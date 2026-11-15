# Research: Steer a Busy Turn

Decisions behind `plan.md`. Each entry: **Decision**, **Rationale**, **Alternatives considered**.

## R1 — Where a waiting message lives: the session queue, shared with the turn

**Decision**: The per-session queue in `agent_process` (`SessionQueue`, today a `VecDeque` owned by the loop) becomes a shared handle, `Arc<std::sync::Mutex<VecDeque<Queued>>>`, with each `Queued { request, response_tx, turn, mode }` carrying a `mode` (`Steer` | `Queue`, R3). The loop keeps pushing to the back and popping from the front exactly as today; the running chat turn holds a clone of its session's handle (`SteerInbox`, passed through `SessionJobs` into `VizierAgent::chat` and onto `ToolContext.steer`), and at each round boundary takes every `Steer` item out under the lock.

**Rationale**: One queue keeps FR-004's "in arrival order, as today" for free: a steer message the turn never reached stays exactly where it was and runs as its own turn when the loop pops it, behind or ahead of `/queue` messages and handovers as it arrived. Taking under the lock is the in-memory half of exactly-once (the loop cannot pop what the turn took, and vice versa). `std::sync::Mutex` is enough: both critical sections are a few `VecDeque` operations with no `.await` inside.

**Alternatives considered**: (a) a second per-session "steer box" beside the queue — two structures to keep in order with each other, and a leftover steer message would need re-inserting into the queue at the right place; (b) a bus subject the turn subscribes to — the queue is process-local on purpose (only the agent's owner holds it), and a subscription would duplicate it; (c) persisting the queue and claiming from `session_turn` only — the in-memory queue is already the source of truth for ordering and response senders, which the DB row cannot hold.

## R2 — The durable half of exactly-once: one transaction

**Decision**: A new fenced storage method, `TurnStorage::steer_turns(session, items) -> Vec<SteeredTurn>`, runs in **one transaction**: for each `(turn_id, request)` it deletes the `session_turn` row *if it is still there and `queued`*, and only for rows it deleted inserts the request's `SessionHistoryContent::Request` entry (with `mid_turn: true`, R5) through the same insert path as `save_session_history` (artifact references, Workspace index). Items whose row is gone (an `/abort` raced the boundary) are dropped. A request without a journal row (non-durable) is saved without the delete. The body of `save_session_history` is split into a `write_history_entry(tx, …)` helper both use.

**Rationale**: Today a queued message has a `session_turn` row (`accept_turn` in `send_request`) that recovery re-sends. If the history save and the row delete were separate, a crash between them either loses the message (row gone, no history) or answers it twice (history saved, row re-sent). One transaction removes both windows — SC-002's "0 lost, 0 answered twice" across restarts. The guarded delete (`WHERE id = ? AND state = 'queued'`) doubles as the race guard against `/abort`'s `cancel_session_turns`.

**Alternatives considered**: (a) save first, delete after, and de-duplicate on recovery by looking the turn id up in history JSON — a scan on every recovered row, and CLAUDE.md forbids history-JSON scans at read time elsewhere; (b) mark the row `absorbed` and keep it — another state for recovery to understand, for no gain; (c) the detached-script pattern (claim, then record) — it accepts a lost delivery on a crash between the two, which is fine for a background report but not for a person's message.

## R3 — Steer or queue: a field, set once, before the queue

**Decision**: `VizierRequest.queue: bool` (`#[serde(default, skip_serializing_if = "is_false")]`): `true` means "run as a new turn". It is set by the WebUI on Ctrl+Enter / Cmd+Enter, or by the engine when the text starts with `/queue` (R4). The decision whether a queued item is `Steer` or `Queue` is the pure `steer::mode(&request, &session) -> Mode`, called once in `agent_process` when it queues an item:

- `Queue` if `request.queue`, or the content is not `Chat`, or the text is a skill command (`skill::invocation::parse` ≠ `Plain`), or a command (`/checkpoint`, `/lobotomy`, … already arrive as `Command`), or the session has no person in it (`Task`, `Dream`, `Subagent`, inter-agent), or the request is `transient`;
- `Steer` otherwise.

`AudioChat` (voice messages) queues as today in this version: its transcription runs in the turn that handles it (Chat arm), and doing it at a boundary would hold the running turn on STT. `ClientEvent` and `QuestionAnswer` queue as today (FR-007).

**Rationale**: The mode must survive a restart (the request is journaled, so the field rides the row) and must not change after send (FR-011b) — a field set at send time and a pure function of the stored request give both. One predicate keeps the eligibility rules in one testable place (like `records_session_activity`).

**Alternatives considered**: (a) the WebUI sending the text `/queue …` for Ctrl+Enter — it would show `/queue` in the person's own message and in history; (b) a new request content variant `QueuedChat` — every `Chat` match arm would need a twin.

## R4 — `/queue`: parsed once, in core

**Decision**: `steer::strip_queue_command(&mut request)` runs in `agent_process` on receipt, before `mode`: a `Chat` text matching `^/queue(@\S+)?(\s+|$)` (case-insensitive; the `@bot` suffix is Telegram's) has the prefix removed and `queue` set. `/queue` with nothing after it is answered directly ("Usage: /queue <message> — runs it after the current turn.") with no turn and no history, like `/skills`. `queue` joins `skill::invocation::RESERVED`, so a skill named `queue` gets no command of its own and stays reachable as `/skill queue`; `channels/commands.rs` adds a `queue` builtin (Discord: a `message` option, required; Telegram: in the menu), turned back into `Chat("/queue …")` like a skill command. In a Workspace, `workspaces/commands.rs` parses `/queue @<agent-id> <message>` into the text the agent receives, `/queue <message>`, like the other commands.

**Rationale**: Every channel already delivers commands as `Chat` text and core tells them apart once (spec 021's rule); parsing `/queue` in four channel readers would copy it four times. Stripping on receipt means the history entry, the frontmatter and the WebUI show the message without the command, wherever it came from.

**Alternatives considered**: parsing in each channel — rejected (Principle II).

## R5 — How a steered message is stored and told apart

**Decision**: It is saved as an ordinary `SessionHistoryContent::Request` with `Chat` content, its original `timestamp` (the server's receipt time, `sent_at`) and a new engine-only field `VizierRequest.mid_turn: bool` set to `true`. The history row's own timestamp is the time it was read, which places it after the round's last `ToolResult` and before the next step, so stored order is display order (FR-005, reload-stable). `mid_turn` is stripped from anything a client sends (with the other receipt-time fields in `topic.rs::stamp_on_receipt`, and forced `false` in `agent_process` before queuing). One predicate, `VizierRequest::opens_turn()` (false for a `DetachedDelivery` and for `mid_turn`), replaces the ad-hoc checks that look for a turn's start (`steps.rs::seal_open_calls`, and any other `rposition(Request)` found in the tasks pass); the WebUI's `groupHistory` reads the same field.

The frontmatter of a steered message adds `via: mid_turn` beside the person's `sender`, and `BOOT.md` gains one sentence: a message marked `via: mid_turn` arrived while you were working; take it into account before your next step (FR-006). The sentence is fixed text, so the system prompt stays byte-identical across turns.

**Rationale**: Keeping the content `Chat` means notifications, Workspace indexing, reactions, the WebUI bubble and attribution treat it as the person's message with no new arm; the flag only changes *where* it sits in a turn. A `via` key is how the frontmatter already distinguishes client events and worker messages.

**Alternatives considered**: (a) a new `VizierRequestContent::Steer(String)` — every place that handles a person's `Chat` needs a twin (Principle II); (b) a key in `metadata` — metadata is client-shaped and shown to the agent verbatim, so a client could forge it, and it is not typed.

## R6 — What the model is sent at a boundary

**Decision**: The round boundary in `prompt_with` (`agents/agent/mod.rs`, where `deps.deliveries.claim` runs today) becomes: take steer items (R1) → `steer_turns` (R2) → for each saved message, build its model text with the same helper the `Chat` arm uses (frontmatter via `to_prompt`, attachments moved into session files with `attachments_note`, project `@` mentions resolved, Workspace `catch_up::for_activation` run first so silent reads land before it) → one `Message::User` holding, in order, the background-delivery text (if any) and then each person message as its own `Part::Text`. The `chat` turn's request-preparation code is extracted into one helper (`prepare_person_request`) used by the turn start and the boundary, so they cannot drift. Each steered message is pushed to watchers as `SessionFrame::HistoryEntry` through the turn's `StepRecorder`, as deliveries are.

On replay, `history_entries_to_messages` already holds a mid-round request until the round's results are in; it must also **merge consecutive user-side entries** after a round (a delivery plus steered messages, or several steered messages) into one user message, so a replayed turn sends the provider what the live turn sent and the prompt prefix matches (provider cache, spec 060's agreement rule). A unit test pins live/replay equality.

**Rationale**: Anthropic and Bedrock want alternating roles (Bedrock already merges); sending one user message at the boundary is valid for every dialect and is what deliveries do today.

**Alternatives considered**: one user message per steered entry — two adjacent user messages after the tool results are refused or reordered by some providers.

## R7 — The steered message's own sender

**Decision**: Each queued request carries its own `response_tx` (the channel reader or WebUI socket waiting on it). When the message is steered, that sender gets one new terminal frame, `VizierResponseContent::Steered`, and is dropped; `VizierResponse::ends_turn` is true for it. The Discord and Telegram renderers end quietly on it (stop typing, post nothing): the reply is posted once, by the running turn's renderer (FR-012). The WebUI ignores it — it learns from the `history_entry` frame (R8).

**Rationale**: A sender dropped with no frame reads as "the agent stopped" (the end frame the bus publishes on drop), which channels report as unavailable. An explicit ending frame keeps that signal meaningful.

**Alternatives considered**: attaching the steered message's sender to the running turn so it receives the reply too — Discord and Telegram would post the reply twice.

## R8 — WebUI: waiting, then the split trail

**Decision**:
- **Send**: the composer's `onSubmit` gains a `queue` flag; design's `Composer` maps Ctrl+Enter / Cmd+Enter to it (Enter unchanged; neither fires while the mention/suggestion list is open, as today for Enter), and shows a one-line hint *only while busy* through a new `hint` prop. `connectionStore` puts `queue: true` on the request. `/queue …` typed in the WebUI needs nothing: core strips it (R4).
- **Waiting**: the existing `queued` list in `lib/frames.ts` already draws a message sent while busy below the live trail and drops it when the stored request arrives (matched by text or stamp). A steered message's `history_entry` arrives mid-turn and clears it the same way; a queued one clears when its own turn starts. The match must compare the text *after* `/queue` is stripped.
- **Split**: `groupHistory` (`lib/trail.ts`) adds a `{ kind: 'steer', entry }` block to the open turn for a `Request` whose `mid_turn` is true, instead of opening a new turn. The turn renderer cuts the trail at each steer block: trail part, the person's message bubble, next trail part, then the reply; each part gets the ordinary "Worked · N steps · duration" header from its own steps (FR-010). Background deliveries keep their in-trail steps (FR-010a). Everything is derived from stored history, so live, reload and second tab agree.

**Rationale**: Reuses the queue display and the history-driven trail (spec 023 builds the running trail from history, not live frames), so the split needs no new frame.

**Alternatives considered**: a live-only `steered` frame — would not survive a reload, violating FR-010.

## R9 — Abort, stop, restart, delete

**Decision**: Unchanged code paths, checked against the shared queue:
- `/abort` (`abort_session_silent`) also **clears** the shared `VecDeque` (the running turn may hold a clone), then `cancel_session_turns` deletes the rows; a boundary racing it finds its rows gone (R2) and saves nothing (FR-009).
- Instance stop for update/restart: the running turn is resumed on the next instance first, and `hand_on_queued` re-sends the queue in order; the resumed turn's next boundary steers the re-sent steer items. Already-steered messages are history of the resumed turn (`split_at_request` replays them).
- Crash: `recover_agent` re-sends the running row (`resumed`) then the queued rows; same as above.
- Delete: as today.

**Rationale**: R1 + R2 make the existing lifecycle code correct without new branches.

## R10 — End-to-end check with dummyplug

**Decision**: Two additions to the dummyplug protocol (submodule `dummyplug`, documented in `contracts/dummyplug-steer.md` and §3/§8 of the protocol file):
1. an optional `delay_ms` key on the run shape (`{ "tool": …, "arguments": …, "delay_ms": 1500 }`, 0–10 000): dummyplug waits that long before answering with the tool call, which gives a test a window to send a message while the turn is busy;
2. §8: when the request's newest messages after the last tool result include user text (a steered message), the closing reply is `**Read mid-turn**:` followed by each such message's text, one per line, instead of the §4 echo.

**Rationale**: Deterministic and offline (constitution e2e gate); no real model needed to prove the message reached the very next completion.

**Alternatives considered**: a slow `execute_python` loop — timing depends on machine speed and the sandbox's CPU limit.

## R11 — Design pass

**Decision**: The composer hint and the split trail are UI changes. FigCraft was unreachable this session, so the mockups are ASCII in `contracts/webui.md`, reviewed before UI code (constitution v2.2.0). If FigCraft is reachable at implementation time, the `Chat` page of `Vizier.figcraft` is updated first.
