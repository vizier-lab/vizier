# Research: Prune old tool outputs (060)

## R1. What the placeholder id is, and how the loop knows it

**Decision**: the history uid of the stored `ToolResult` entry. `Message::ToolResult` gains
`record: Option<String>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`), never
encoded by a dialect. `history_entries_to_messages` sets it to `entry.uid`. In `prompt_with`,
`StepRecorder::record` returns the uid `record_step` already gets back (`Option<String>`), and the
loop stores it on the result before pushing it into `history`.

**Rationale**: call ids are not unique. The Gemini and Ollama dialects use the tool's name as the
id (`llm/dialect/gemini.rs:116`, `ollama.rs:98`), so every call is `execute_python`. The uid is
unique installation-wide, and `get_session_history_entry(session, uid)` is a primary-key read
already scoped to the conversation. Putting the id on the message means it travels with the
message through checkpoint rebuilds, held messages and resume, with no parallel bookkeeping.

**Alternatives considered**:
- *Provider call id as the key*: rejected (not unique, see above).
- *A side map from call id or position to uid*: positions shift on checkpoint rebuilds and
  `repair_tool_pairing`, and call ids collide. It would be fragile and duplicate what the message
  already carries.
- *A wrapper type around every history message*: a far larger change to `prompt_with`'s signature
  and every caller, for the same information.

## R2. Where the rule runs

**Decision**: `completion_request` builds `prune_for_model(&messages_for_model(history))` and then
appends the current message (narrowed only). It takes a `prune: bool`. `prompt_with`, the side
chat and the tests pass `true`, `dream_prompt` passes `false`. The handover summary
(`generate_handover_with_model`) summarises the pruned view.

**Rationale**: one place covers every turn kind that runs the loop (FR-008). The current message is
always the newest round, so leaving it out of the walk changes nothing. The side chat replays
a host turn's prefix, so it prunes the same way and shares the cache. The handover must use the
pruned view: compaction triggers on the pruned context size, so the full view could exceed the
window the handover runs in. Dreams have no `recall_history` (FR-015), so they must not see
placeholders.

**Alternatives considered**: pruning inside `messages_for_model` (would reach dreams). Pruning in
the dialects (seven copies, against Principle II).

## R3. The rule: one forward walk, prefix-stable

**Decision**: walk the messages in order. Count rounds (each `Assistant` message starts one). Each
*eligible* result is appended to an unpruned queue and its estimated tokens are added to a running
total. When the total passes `HIGH` (60,000), pop from the front of the queue while the total is
above `LOW` (30,000) and the front result belongs to an earlier round than the one being walked,
marking each popped result pruned. Exact text in [contracts/prune-rule.md](contracts/prune-rule.md).

**Rationale**: a decision made at a result depends only on the messages before it, so appending
messages never reverses one. That gives FR-005 (one-way) and FR-006/SC-004 (live and replay agree,
and turn N+1 starts with turn N's last prefix) by construction. The two marks give SC-005: after a
prune, at least `HIGH − LOW` new tokens must arrive before the next one. The same-round guard keeps
the newest round whole (FR-007).

**Alternatives considered**:
- *Keep the newest N tokens, recomputed each request*: one more result pruned almost every round,
  a cache miss every round.
- *Persisting a pruned flag on stored entries*: a write per prune, a migration, and the replay
  would still need the rule to decide.

## R4. Eligibility

**Decision**: a result is eligible when all hold:
1. it has a `record` (it was saved);
2. all its parts are text (images are never pruned);
3. its narrowed text is at least `MIN_BYTES` (2,048);
4. it is not a failed `execute_python` report (`ok: false`);
5. it is not a report that ended the turn (`ExecutionReport::ending_call` is `Some`).

Ineligible results are neither pruned nor counted toward the total.

**Rationale**: these are the spec's edge cases. Counting ineligible results would let unprunable
content push the rule into pruning everything else. A plain-string tool error (`Ok(Err(e))`,
timeouts) is short in practice and rule 3 keeps it. The size is measured on the narrowed text,
because that is what the model is sent.

## R5. Token estimate

**Decision**: `(bytes + 3) / 4` of the narrowed text. The placeholder's `<size>` is the narrowed
text's size, human-formatted (`812 B`, `14 KB`, `1.2 MB`, base 1,024, one decimal under 10).

**Rationale**: the marks are budgets, not limits. Being exact per provider buys nothing, and a
tokenizer would be a dependency (Principle I).

## R6. `recall_history`

**Decision**: `VizierTool` in `agents/tools/history.rs`, category `ToolCategory::History`, on
`default_toolset`, not in `DREAM_TOOL_NAMES`. Input `{ id }`. It calls
`get_session_history_entry(&ctx.session, id)`; a `ToolResult` entry returns
`{status: "found", id, output}`, where `output` is the stored text parsed as JSON when it parses
and the text otherwise, uncapped (FR-010). Anything else (no entry, an entry that is not a tool
result, a uid from another conversation) returns `{status: "not_found", id}` (FR-011). A result
saved before a checkpoint is still in `session_history`, so it is recallable (FR-012).

**Rationale**: returning parsed JSON lets a script index a report directly
(`r["output"]["result"]`). The stored text is the full report, including the per-call arguments
that the model's view drops. That is "exactly as stored". The session scoping comes from the
storage query (agent and channel slug, topic included), so no extra check is needed.

## R7. The category and the guidance sentence

**Decision**: append `History` to `ToolCategory` and to the end of `ALL` (the convention `a_new_category_is_appended_to_the_end_of_the_list` pins: older prompts and scripts already saw the order), with the name `history`
and the description "Recall tool outputs pruned from this conversation's view; recall them in a
script and return only what you need". Add `recall_history` to `every_tool_is_in_its_category`.

**Rationale**: built-in category names are already reserved through `ToolCategory::ALL`
(`reserved_category_name`), so `history` becomes reserved with no extra code (FR-014). The
description is the category line in `SANDBOX.md`, which satisfies FR-016 without new prompt text.
This changes `SANDBOX.md` once for every agent (spec Assumptions).

## R8. Measurement

**Decision**: `prune_for_model` returns `PruneStats { pruned, tokens_removed }`. `invoke_agent_span`
declares `vizier.prune.outputs` and `vizier.prune.tokens_removed`, and `prompt_with` records the
latest request's stats on that span after each completion. Recalls are ordinary tool calls inside
the script, already traced through `ToolRouter::call` (`execute_tool recall_history`) and listed in
the report (FR-018).

**Rationale**: the last request's numbers are the turn's state, because the rule is one-way.
Span fields are already how per-turn facts are reported (`vizier.turn.ended_by_tool`).
