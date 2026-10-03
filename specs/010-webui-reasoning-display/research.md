# Phase 0 Research: WebUI Reasoning & Tool Activity Display

Eight decisions. The first three carry all the risk; the rest are small.

---

## Decision 1: How the ordering position is stored and assigned

**Decision**: Add a nullable `seq INTEGER` column to `session_history`, indexed, assigned on insert
as `(SELECT IFNULL(MAX(seq), 0) + 1 FROM session_history)` — a single global monotonic counter.

**Rationale**: The insert already runs under `self.conn.lock()` (`src/storage/sqlite/history.rs:76`),
so the read-then-write is serialized and cannot race. An index on `seq` makes `MAX(seq)` an index
lookup rather than a scan, so the cost is one extra B-tree seek per history insert.

Global rather than per-conversation because a global counter is monotonic within *every* subset of
rows, including the cross-channel reads (`history.rs:383` reads by agent across channels) that a
per-conversation counter would not order. Per-conversation would also have to restate the
session-matching predicate — including the `topic IS NULL` variance already spelled out three times
in this file — which is exactly the duplication Principle II exists to prevent.

Nullable because FR-006 forbids a backfill: pre-existing rows keep `NULL` forever.

**Alternatives considered**:
- *Implicit `rowid` tie-break* — free and needs no column, but a table rebuild (`VACUUM INTO`, a
  future schema change that recreates the table) renumbers rowids and silently reorders history. The
  user chose the explicit column for exactly this reason.
- *`PRAGMA user_version` schema versioning* — the "proper" migration framework, but introducing a
  version-counter scheme to add one column is the anticipatory abstraction Principle I forbids. When
  a second column change arrives, revisit.
- *Per-conversation counter* — better locality, no benefit, more duplicated predicate.

---

## Decision 2: How the column gets added to an existing database

**Decision**: A small `add_column_if_missing(conn, table, column, decl)` helper that checks
`PRAGMA table_info(<table>)` and issues `ALTER TABLE … ADD COLUMN` only when absent. Called from the
same startup path that runs the `CREATE TABLE IF NOT EXISTS` batch.

**Rationale**: This is the **first column added to an existing table in the project's history** —
`grep -rn "ALTER TABLE\|table_info\|user_version" src/` returns nothing. The whole schema is one
`execute_batch` of `CREATE TABLE IF NOT EXISTS` (`src/storage/sqlite/mod.rs:180-211`), which by
construction does nothing to a table that already exists. So a fresh database gets `seq` from the
`CREATE TABLE`; an existing one needs the `ALTER`.

A ~10-line helper is preferred over swallowing SQLite's "duplicate column name" error because
swallowing hides real failures, and preferred over a migration framework because there is one column
to add.

**Note**: this helper belongs beside the schema, not in `dependencies.rs`. The `dependencies.rs`
migrations move *data* between representations; this is schema shape, and it must run before any
query references the column.

**Alternatives considered**:
- *Run the `ALTER` unconditionally and ignore the error* — fewer lines, but it swallows every other
  failure mode of that statement too.
- *Recreate the table with the column and copy rows* — a rebuild of an unbounded table at startup,
  for a nullable column. No.

---

## Decision 3: Making narration replayable — the part that can break turns

**Decision**: Two coordinated changes.

**(a) Record narration before its sibling tool calls.** In `messages_to_history_entries`
(`src/schema/history.rs:70-94`), the `Message::Assistant` arm currently pushes `ToolCall` entries
*inside* its content loop and the `AssistantMessage` only *after* it. Restructure to collect text and
tool calls first, then push `AssistantMessage` and then the `ToolCall` entries.

**(b) Replay must merge narration and its tool calls into one assistant message.** Extend the
`pending` accumulation in `history_entries_to_messages` with a `pending_text: Option<String>`, and
have `flush_pending_tool_calls` emit a single `Message::Assistant` whose content is
`[Text(pending_text), ToolCall…]`. An `AssistantMessage` entry sets `pending_text` instead of
flushing and pushing its own message.

**Rationale**: (a) alone is not enough, and getting this wrong is the one failure in this feature
that breaks agents outright rather than looking wrong.

With today's write order, replay produces `assistant(tool_use) → assistant(text) → user(tool_result)`
— the tool result no longer follows its tool call, which providers that enforce adjacency reject.
That is the latent reason narration is filtered out on both the write path
(`src/agents/agent/mod.rs:459,495`) and the read path
(`src/channels/http/api/v1/agents/channel.rs:137`).

But fixing only the write order gives `assistant(text) → assistant(tool_use) → user(tool_result)`:
adjacency is restored, yet it emits **two consecutive assistant messages**, which Anthropic also
rejects. The original model response was a single assistant message carrying a text block and
tool_use blocks together, so replay has to reconstruct that shape. Hence (b).

`flush_pending_tool_calls` already builds the assistant message from a content vector
(`src/schema/history.rs:201-209`), so prepending a text block is a two-line change to a function
that exists.

**Verification**: SC-005 — 20 multi-tool turns against a live Anthropic-family provider with 0
replay rejections. This cannot be verified on dummyplug alone, because dummyplug does not enforce
message-shape rules; the quickstart marks it as a live-provider step.

**Alternatives considered**:
- *Write order only* — produces consecutive assistant messages. Rejected above.
- *Keep narration out of history and show it from the live stream only* — would make the trail
  inconsistent between a fresh turn and a reloaded one, for no saving.
- *Synthesize narration back into the final `Response` text* — changes what the agent said. No.

---

## Decision 4: Suppressing a script's nested tool calls from the live stream

**Decision**: Add `on_nested_tool_call` to `VizierSessionHook`, defaulting to delegating to
`on_tool_call`. `RouterBridge::dispatch` calls the nested variant. `ToolCallsHook` and `ThinkingHook`
override it to pass name and arguments through without emitting a frame.

**Rationale**: Dropping the `tool_calls` list from `ExecutionReportView` is *not sufficient* for
FR-034. `RouterBridge::dispatch` runs every nested call through `self.ctx.hooks.on_tool_call(…)`
(`src/agents/tools/python/bridge.rs:78-84`), and `ToolCallsHook::on_tool_call` emits a `ToolChoice`
frame for anything that is not `think`. So today a loop of twelve nested calls streams twelve
`ToolChoice` frames into the WebUI live — the per-call noise exists in two places, not one.

The trait method is the Principle II shape: no caller branches on "am I nested", each hook states its
own nested behaviour, and the default keeps every existing hook behaving exactly as it does now. The
default delegating to `on_tool_call` matters — `debug.rs` and any future hook keep seeing nested
calls without being touched.

`ThinkingHook` is included because a script calling `think` is equally script-internal.

**Scope note**: nested calls are **not** persisted to history. They go through `ToolRouter` directly
and never enter the model's message list, so `messages_to_history_entries` never sees them. The
history-sourced trail therefore shows no nested calls for free; only the live path needed fixing.

**Non-issue confirmed**: a script cannot recursively call `execute_python`. `RouterBridge::resolve`
searches `default_toolset`, `user_toolset` and the MCP catalogue; `sandbox_toolset` lives on
`VizierTools`, not on `ToolRouter`, so the sandbox tools are unreachable from a script.

**Alternatives considered**:
- *Give `RouterBridge` a `ToolContext` with `ToolCallsHook` removed* — `VizierSessionHooks` is an
  opaque `Vec` with no removal API, and `ExecutePython` has no way to identify which hook is which.
  Adding that introspection is worse than one trait method.
- *A `nested: bool` on `on_tool_call`* — every implementor must then branch on it, which is the
  type-tag branching Principle II forbids.

---

## Decision 5: Where the displayed trail comes from

**Decision**: One normalized trail model with two producers. The live WebSocket stream produces it
during a turn; a pure grouping function over stored history produces it on load. No refetch after a
turn completes — the events already collected are attached to the turn that just closed.

**Rationale**: The WebUI cannot refetch even if it wanted to: `historyVersion`
(`webui/app/routes/chat.tsx:266`) is `useState(0)` with no setter destructured, so the history effect
never re-runs after mount. Adding a refetch per turn would also cost a round trip and a flicker on
every answer.

Two producers feeding one renderer is the DRY requirement here: if the live trail and the reloaded
trail were built by separate code they would drift, and the user would see a turn change appearance
on refresh.

The grouping function — ordered `SessionHistory[]` → `Turn[]` — is pure, which makes it the one piece
of this feature that is cheaply unit-testable, and it is where the ordering guarantees actually
become visible.

**Alternatives considered**:
- *Refetch history when a turn closes* — simpler state, but a wasted round trip per answer, visible
  flicker, and it still needs the grouping function.
- *Render the live stream only and leave history unrendered* — the page-lifetime option the spec
  rejected.

---

## Decision 6: Where `intent` lives

**Decision**: `intent` is a required field on `ExecutePythonInput`, and nothing else changes.
`ExecutionReport` is untouched.

**Rationale**: The intent is already persisted and already streamed, because it travels in the tool
call's arguments: history stores `ToolCall { name: "execute_python", arguments: { code, intent } }`,
and the live path sends `ToolChoice { name, args }`. Adding `intent` to `ExecutionReport` as well
would duplicate it into the report the *model* reads, costing tokens to tell the model something it
just wrote (the opposite of what commit 3d5b213 did), and creating a second copy that can disagree
with the first.

So FR-033 ("recorded alongside the run") is satisfied by the existing tool-call record.

**Pairing**: the WebUI must pair a run's intent with its report. From history this is exact — both
entries carry the same `call_id`. From the live stream there is no id: `VizierResponseContent::
ToolChoice` has only `{ name, args }`, so pairing is positional, matching the report to the most
recent unpaired `execute_python` choice. This is what today's code already does implicitly by
appending two sequential inline events, and adding a correlation id to the WebSocket protocol for a
display nicety is not worth the protocol change.

**Alternatives considered**:
- *`intent` on `ExecutionReport`* — duplicate state, tokens spent echoing the model's own input.
- *A correlation id on `ToolChoice`* — exact live pairing, but a protocol change affecting every
  channel for a cosmetic gain.

---

## Decision 7: Paging cursor

**Decision**: `HistoryQuery` gains an optional `before_seq`; the predicate becomes
`timestamp < ?t OR (timestamp = ?t AND seq < ?s)` when both are supplied, and stays as today when
only `before` is given.

**Rationale**: FR-005 requires it, and the current `AND timestamp < ?` (`history.rs:111`) can split a
tie group across a page boundary. Additive and backward compatible: an existing caller passing only
`before` keeps today's behaviour.

**Honest scoping**: this is API correctness, not a visible bug today. The WebUI calls
`getTopicHistory(agentId, resolvedTopicId)` with no paging parameters at all, so no user currently
hits the boundary case. It is implemented because the requirement is real and the fix is small, not
because anything is broken for a user right now. SC-003 verifies it directly against the endpoint.

---

## Decision 8: Out of scope, and why it is worth naming

**Finding**: `formatToolChoice` (`webui/app/routes/chat.tsx:102`) was never updated for the memory
tool rename in spec 009. It has a `memory_read` case labelled with the *old* search semantics that
reads `args.query`, no `memory_search` case at all, and a `memory_detail` case for a retired tool.
Since `memory_read` now takes `(bundle, path)`, it renders as `🔍 Searching memory for 'undefined'`.

**Decision**: not fixed here. It is a pre-existing defect from another feature, and folding it in
would mix an unrelated repair into this diff.

**Why it is named anyway**: this feature changes its severity. Today the wrong label flashes past in
an ephemeral indicator nobody re-reads. Once trails persist, it becomes permanent wrong text in every
stored transcript, and it is the single most frequently called tool family. It should be fixed before
or soon after this lands, as its own change.

---

## Resolved: no outstanding unknowns

The spec carried no `[NEEDS CLARIFICATION]` markers into this phase — the three forking decisions
(durability, backfill, `intent` requiredness) were settled with the user before the spec was
committed, and are recorded in its Decisions section. Nothing in Phase 0 reopened them.
