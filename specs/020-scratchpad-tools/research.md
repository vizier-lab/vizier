# Research: Scratchpad Tools

**Feature**: `020-scratchpad-tools` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

The spec has no open clarifications. Each item below records a design decision taken while reading the code it touches.

## R1. Where scratch files live: a SQLite table

**Decision**: one new table, `scratch_file`, in the embedded SQLite database, behind a new `ScratchStorage` trait composed into `VizierStorageProvider` and forwarded by `VizierStorage`. This follows the `ClientCallStorage` pattern (`storage/client_call.rs` + `storage/sqlite/client_call.rs` + `init_client_call_schema`).

**Rationale**: the user asked for storage rather than RAM, because the total size is unbounded in practice (FR-008). SQLite is already the home of every non-memory entity (Principle III). Content is a `TEXT` column, so an append can be a single `content || ?` update that never reads the existing content into the process.

**Alternatives considered**:
- *Files on disk under the workspace* (`{workspace}/agents/{id}/scratch/...`): this adds a second place where conversation state lives, plus path handling and per-platform file semantics (Principle IV). It also gives up the atomic "check limits, then write" step that one locked SQLite connection provides for free.
- *In-process `HashMap`*: the user rejected this.
- *Reuse `session_file`*: session files are attachments people send. They carry a MIME type and bytes on disk, are listed by `list_session_files`, and must outlive the turn. Mixing them would leak scratch files into that tool and into the WebUI.

## R2. Key and the `NULL` topic

**Decision**: the primary key is `(agent_id, channel, topic, name)`, with `channel` = `VizierChannelId::to_slug()` and `topic TEXT NOT NULL`, storing `''` for a session without a topic.

**Rationale**: a `NULL` in a SQLite primary-key column is allowed and counts as distinct from every other `NULL`, so two writes to `(a, c, NULL, "x")` would make two rows. Mapping `None` to `''` keeps the upsert (`ON CONFLICT`) correct. No real topic is the empty string: topics are generated ids or channel-native ids.

## R3. Limits enforced atomically in storage

**Decision**: write and append run inside one transaction on the storage's locked connection. In order: validate the name (done before storage, in the tool); read this file's `size` and the session's file count; refuse if the new size would exceed `MAX_FILE_BYTES` (1 MiB = 1 048 576) or a new file would exceed `MAX_FILES` (64); otherwise upsert. The result is a `ScratchWrite` value, `Written { size }`, `TooLarge { limit, current }` or `TooMany { limit, names }`, never an `Err`.

**Rationale**: FR-011 requires an over-limit write to leave the content unchanged. Size is counted in **bytes** of UTF-8 (`str::len()` on the incoming content, plus the stored `size` column), not characters. SQLite's `length()` counts characters for `TEXT`, which is why the size is kept in a column rather than computed in SQL.

**Alternatives considered**: a `CHECK` constraint on size makes over-limit writes fail as SQL errors, which would then have to be mapped back into data. That is a worse fit for FR-012.

## R4. When a conversation is idle: the agent's process loop

**Decision**: scratch files are cleared in `src/agents/process.rs::agent_process`, which already owns the definition of busy (`session_is_busy`: a running turn, or a non-empty queue). There are three clear points, each an **awaited** `clear_scratch(&session)` call inside the loop:

1. **A turn ends** (`turns.join_next()`), whether current or not. After the existing hand-on logic, if `!session_is_busy(...)`, clear. This covers completed, failed, panicked and timed-out turns, and an aborted turn's task finally ending.
2. **`/abort`** (`abort_session_notify`). The queue is dropped and the turn aborted, so the session is idle at once. Clear there too, so a message sent straight after the abort starts with an empty scratchpad, even if the aborted task's join comes after the new turn has started (in which case point 1 finds it busy and skips).
3. **The instance stops** (update, restart, delete). After `turns.stop_all`, clear every stopped session that has no queued requests. Sessions whose queue is handed on to the next instance keep their scratch files, because their turns continue (spec edge case "Agent update/restart").

**Rationale**: the spec defines a chat's end as "no running turn and no queued turn", and this loop is the only place that knows both. Awaiting the clear inline, rather than `tokio::spawn`-ing it the way `update_thinking_state` is spawned, guarantees ordering: no request for the session can start between "found idle" and "cleared", because the loop handles one event at a time. The `DELETE` uses the primary-key prefix, so its cost is one indexed delete per idle transition, often of zero rows.

**Residual race (accepted)**: a script whose turn is aborted stops at its next host call (`sandbox/runtime.rs::CancelOnDrop`), but one tool call already inside `block_on` can still complete after the clear. That leaves at most one stale file in the same conversation. The next idle transition or startup removes it. It never crosses conversations.

**Alternatives considered**:
- *Clear at the start of each burst* (when a request reaches an idle session): this fixes the straggler race, but it wipes the scratch files of requests handed on across an agent update. Those requests arrive at the new instance as fresh requests on an idle session, so they would lose state the spec says they keep. It would also leave scratch content sitting in storage until the conversation is next used, which goes against the "could grow" concern behind R1.
- *A per-burst generation id stamped on rows*: correct, but it adds state to the session loop and a column, only to close a millisecond window within one conversation.

## R5. Restart, conversation deletion, agent deletion

**Decision**:
- **Startup**: `VizierDependencies` gains `clear_orphaned_scratch`, run next to `interrupt_orphaned_client_calls`. It calls `ScratchStorage::clear_all_scratch()` (`DELETE FROM scratch_file`). Failure is logged and does not stop boot, as with the other sweeps. This is not compatibility code, so no #82 entry is needed.
- **`delete_session`** (`storage/sqlite/session.rs`) and **`delete_agent`** (`storage/sqlite/agent.rs`) delete the matching rows on the connection they already hold, as they already do for `client_tool_call`.

## R6. Category, registration, naming

**Decision**: add `ToolCategory::Scratchpad`, named `scratchpad`, placed **after `Files`** in `ToolCategory::ALL` (15 variants). Its description is "Temporary notes private to this conversation, deleted once it goes idle". The three tools go in `src/agents/tools/scratchpad.rs` and are registered on `default_toolset` in `BaseTools::new()` (always on, no config). They are added to the `every_tool_is_in_its_category` table.

**Consequence**: `scratchpad` becomes a reserved category name. An existing MCP server whose key is exactly `scratchpad` gets category `scratchpad_mcp`, and its tools are renamed to match by the existing `ToolRouter::catalogue()` rules. That is the same treatment every built-in category already gets, and it is called out in the commit message.

**Rationale**: the tool names come from the user and are the dispatch key. A new category keeps `SANDBOX.md` byte-stable across turns, because the categories are built once per instance.

## R7. The dream cycle

**Decision**: add `write_tmp`, `append_tmp` and `read_tmp` to `VizierTools::DREAM_TOOL_NAMES`. Dreams are not in code mode: `dream_tools` offers these definitions directly, and `dream_call` dispatches through `router.dispatch` with the dream session as `ctx.session`.

**Scope that follows from the code**: every dream stage is sent as its own request on its own session (`VizierChannelId::Dream(source, Extraction)` per source conversation, and `Dream(system, Consolidation)`), all through `transport.send_request`, so they run through the same process loop. A "dream run" in the spec's sense is therefore one stage's conversation. Each extraction and the consolidation get separate scratchpads, cleared when that stage's turn ends. Extraction scratch files do not carry over to consolidation. Consolidation receives the extractions' answers in its prompt, which is the existing hand-off.

## R8. Tool results are data

**Decision**: every tool returns `Ok(..)` with a tagged result. Write and append return `{"status":"written","name","size"}`, or `{"status":"refused","reason":"invalid_name"|"too_large"|"too_many","message", …}`. Read returns `{"status":"found","name","content","size"}` or `{"status":"not_found","name","existing":[…]}`. Only a storage failure (SQLite error) is an `Err`, which a script sees as a catchable exception, as with any tool.

**Rationale**: FR-012. In code mode an `Err` becomes a Python exception, which would end an unguarded script. Returning data keeps a missing file or a full scratchpad recoverable by the agent.

## R9. Name rule

**Decision**: 1–128 bytes, ASCII letters, digits, `-`, `_` and `.`, and not `.` or `..`. Checked by a hand-written `fn valid_scratch_name(&str) -> bool` with a unit test. No regex crate is needed. Names are case-sensitive.

## R10. Testing

**Decision**:
- Unit tests in `storage/sqlite/scratch.rs`, against an in-memory connection via `init_scratch_schema`: overwrite, append creating the file, append joining exactly, byte-size limit refusal leaving content intact, count limit refusal while overwrite still works, isolation by session and by agent, `None` topic upsert, `clear_scratch` and `clear_all_scratch`.
- Unit tests in `tools/scratchpad.rs` for the name rule and the not-found listing. `every_tool_is_in_its_category` and a check that the three names are in `DREAM_TOOL_NAMES`.
- End to end: dummyplug steps in [quickstart.md](quickstart.md). The queued-burst case is timing-dependent over a socket, so it is checked by sending two messages back to back with a slow first script. If that ever proves flaky, R4's idle rule is still covered by the restart and new-message steps.
