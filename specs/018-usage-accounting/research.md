# Research: Complete Usage Accounting and an Insightful Usage Page

**Feature**: `018-usage-accounting` | **Date**: 2026-10-16

What exists today (verified in code):

- Usage is derived at query time by `HistoryStorage::aggregate_usage` (`src/storage/sqlite/history.rs:242`). It reads every `session_history` row in range, deserialises its JSON and sums `VizierResponseStats` from `Response` rows only.
- Model calls that never produce such a row are invisible:
  - dream extraction and consolidation: `dream_turn` → `dream_prompt`, `agent/mod.rs:1385`; the stats are built and dropped, and only the journal text is saved;
  - checkpoint handovers: `generate_handover_with_model`, `agent/mod.rs:1739`, called from `maybe_checkpoint` and the manual `checkpoint` command (`process.rs:837`);
  - completed rounds of failed or aborted turns: `fail_turn` saves an `Error` row with no stats;
  - `read_image` in vision mode (`tools/read_image/mod.rs:115`).
- Every one of those calls goes through `VizierModel` (`src/agents/agent/model/mod.rs`), which already holds the resolved `provider` and `model` in `ModelInfo`. `CompletionResponse.model` carries the served model version, and nothing stores it.
- The provider "test connection" ping (`providers/usage.rs:219`) calls `llm::build` directly and stays uncounted (spec Assumptions).
- `dummyplug` returns `Usage::new()` (zeros) by design (`dummyplug.rs:85`, pinned by the test `prose_gets_one_text_reply_and_zero_usage`).

---

## R1 — Where to record: `VizierModel`, with an explicit `UsageScope` argument

**Decision**: Record each call in `VizierModel::complete_until`. This is the one agent-level funnel every chat, dream, handover and vision completion passes through, and it already knows the provider and model. `complete` and `complete_until` gain a required `&UsageScope` argument:

```rust
pub struct UsageScope {
    pub session: Option<VizierSession>,
    pub purpose: UsagePurpose,        // Turn | Handover | Vision
    pub turn: Option<TurnId>,         // set for Turn rounds; see R3
    pub round: Option<u32>,
}
```

`VizierModel` also gains the owning `AgentId`. `new` already receives it (as `_agent_id`), and `new_with_override` gains it as a parameter; its two callers, the dream model and the vision model, both have it in hand.

**Rationale**: There are four call sites: `prompt`, `dream_prompt`, `generate_handover_with_model` and `read_image`. With a required argument the compiler finds every one, and a future call site cannot forget to say what it is for. `RetryingModel` returns only the final answered attempt, so FR-005 (one record per answered call, never one per attempt) holds without extra code.

**Alternatives considered**:
- *Record in `llm::RetryingModel` / `HttpModel`*: rejected. `src/llm/` is deliberately agent-agnostic and has no idea of agent, session or purpose.
- *A `tokio::task_local!` scope set around turns*: rejected. Script-made tool calls go through `spawn_blocking` and the `RouterBridge`, where task-locals do not follow, so `read_image` called from `execute_python` would silently become "Other".
- *Read the context from the OpenTelemetry span*: rejected. It would make accounting depend on tracing configuration.
- *Keep summing in the turn loop and save the stats elsewhere*: rejected. It needs one save path per caller (the duplication Principle II forbids), and failed rounds would still be lost.

With the scope required, the spec's "Other" activity (FR-003) only arises from the `System` channel. It stays in the enum as the documented fallback.

## R2 — Never delaying the call: write after the answer, off the turn's path

**Decision**: After a successful completion, `VizierModel` builds a `UsageEvent` and hands it to `tokio::spawn(storage.record_usage(event))`. A failed write is logged with `tracing::warn!` and dropped (FR-006).

**Rationale**: The storage call takes the SQLite mutex, which a busy instance can hold for a moment. Spawning keeps that off the turn (SC-005) without a new long-lived subsystem. `run.rs` treats every long-lived task as one that must never return, and a per-write spawn needs none of that.

**Alternatives considered**:
- *A recorder task fed by a flume channel*: rejected for now. It is a sixth long-lived task for a write that is already cheap. Revisit if write contention shows up.
- *An inline `await`*: rejected because of SC-005.

## R3 — Failed and aborted turns: a drop guard and a separate table

**Decision**:
- `prompt` and `dream_prompt` each open a `TurnGuard { turn: TurnId (uuid), agent_id }`, and every round's scope carries that `turn`.
- On success the guard is marked done.
- If it is dropped without being marked done, because of an `Err` return through `fail_turn`, the prompt timeout or the supervisor aborting the task (#52; tokio drops the future, so `Drop` runs), its `Drop` spawns `record_failed_turn(turn, agent_id, now)`. It uses `tokio::runtime::Handle::try_current()`, so a drop during shutdown is a no-op rather than a panic.
- Failure is a row in its own table, `usage_failed_turn`, joined at query time.

**Rationale**: Both the event inserts and the failure mark are fire-and-forget (R2), so their order isn't guaranteed. An `UPDATE usage_event SET failed = 1` could run before the last round's insert. A separate keyed row makes the result independent of order. `Drop` is the only hook that also covers aborts, where no code after the abort point ever runs.

**Known gap (documented, accepted)**: a process crash mid-turn leaves that turn's rounds counted as succeeded. `vision` calls made inside a turn carry no `turn` (`ToolContext` has none), so a failed turn's image reads are not marked failed.

## R4 — Activity: decided once, at write time

**Decision**: Store `activity` on each row:
- `UsagePurpose::Handover` → **compaction**;
- `UsagePurpose::Vision` → **vision**;
- `UsagePurpose::Turn` → `VizierChannelId::usage_activity()`, a new method next to `to_slug()` / `platform()`:
  - Discord, Telegram, HTTP → chat;
  - Task → scheduled;
  - Subagent, InterAgent → background;
  - Dream → dreams;
  - System → other.

**Rationale**: The channel already says what kind of conversation a turn is in, and the method sits with the other per-variant facts about `VizierChannelId`, not in call sites (Principle II). Storing the result keeps every query a plain `GROUP BY`.

A background *report* turn (spec 012) runs in the originating session and is counted as **chat**. That's where its reply appears, and it keeps the report turn tied to the conversation it woke. Consults (`consult_agent`) run in an `InterAgent` session and count as **background** for the agent that ran them (spec Assumptions).

## R5 — Storage: a `UsageStorage` trait and two tables

**Decision**:
- New `src/storage/usage.rs` with a `UsageStorage` trait (data-model.md §3), added to the `VizierStorageProvider` supertrait and hand-forwarded by `VizierStorage`.
- It is implemented only by `SqliteStorage`, in `src/storage/sqlite/usage.rs`, with tables `usage_event` and `usage_failed_turn` created by `init_usage_schema` (the same split-out pattern as `init_task_run_schema`).
- `delete_agent` calls `usage::delete_agent(&conn, agent_id)` on the connection it already holds (FR-008, like `core_revision::delete_agent`).
- `delete_session` does **not** touch usage (FR-008).
- `HistoryStorage::aggregate_usage` and its fs implementation are removed, along with `AgentUsageStats` and its helper types.

**Alternatives considered**: putting usage columns on `session_history` was rejected. Most of the calls this feature exists to count have no history row.

## R6 — Carrying pre-upgrade usage over (FR-007, FR-019, SC-007)

**Decision**: A one-time startup migration in `dependencies.rs`, `migrate_reply_usage`:
- It runs when the state key `usage.accounting_since` is absent.
- In batches, it copies each `Response` history row that has stats into `usage_event` with `legacy = 1`:
  - provider and model come from the stats when they are present; rows saved before spec 015 have none and are copied with provider and model empty, so they count as unpriced, exactly as today;
  - activity comes from `usage_activity()` of the row's session;
  - no turn id; the stored totals become one row.
- When it finishes it writes `usage.accounting_since = now`. The report returns that timestamp, and the page shows the partial-accounting notice for any range that starts before it.

**Rationale**: SC-007 (pre-upgrade totals unchanged) holds by construction: the same numbers go through the same pricing. A single source table keeps every query simple. Reading both tables at query time would mean two code paths for every view.

**Alternatives considered**: *No carry-over* was rejected, because it empties the page on upgrade (Edge Cases). *Query-time union with `session_history`* was rejected for the duplication and the JSON deserialisation cost that this feature removes.

## R7 — Pricing and aggregation: SQL groups, Rust prices

**Decision**:
- SQL returns token sums grouped by the needed dimensions plus `(provider, model)`, and Rust prices each group with the catalogue.
- `metrics::estimate_cost` is generalised from `&VizierResponseStats` to a small `TokenCounts { input, cached, cache_write, output }`, so there is still one pricing function.
- The previous-period comparison is the same query over `[start − (end − start), start)`.
- The cache hit rate uses the existing `cache_outside_input` flag per provider: `cached / input` when cached tokens are counted inside input, `cached / (input + cached)` when outside (Anthropic).

**Rationale**: Prices live in the swappable catalogue snapshot, not in SQLite. Grouping first keeps pricing work proportional to the number of distinct `(day, activity, provider, model)` cells, not to the number of rows.

**Performance (SC-006)**: indexes `(agent_id, at)` and `(agent_id, channel, topic)`. A 90-day report is about five grouped scans over at most around 100k rows, well under the 2-second budget on SQLite.

## R8 — HTTP surface: replace the response and add one details endpoint

**Decision**:
- `GET /api/v1/agents/{agent_id}/usage` keeps its path and `start_date`/`end_date`, gains `activity`, and returns the new `UsageReport` (contracts/http-api.md).
- New `GET /api/v1/agents/{agent_id}/usage/conversation` takes `channel`, `topic`, the range and `activity`, and returns a `ConversationUsage`.
- Both keep today's access rule unchanged (FR-020).

**Rationale**: The only consumer of the old shape is the bundled WebUI page, which this change rewrites. The change is still marked `[**breaking**]` in the commit, for API users.

## R9 — Readable names and "places"

**Decision**:
- At write time each row stores `place`, the grouping key for the Channel breakdown:
  - Discord channel id, Telegram chat id, or WebUI topic;
  - `task:<task slug>` (all runs of one task together);
  - `dreams`;
  - `background`;
  - `system`.
- Labels are resolved at read time, only for the rows returned (at most around a dozen places plus 10 conversations):
  - `session_detail.title` for WebUI topics and any titled session;
  - the task's title from `TaskStorage` for `task:` places;
  - "Dream cycle" and "Background jobs" as fixed labels;
  - Discord and Telegram: a saved session title if present, otherwise "Discord channel ‹id›" or "Telegram chat ‹id›".

**Rationale**: No live platform lookups, so the page works when a bot is down and adds no network calls on a person's read. Bounding lookups to the returned rows keeps them cheap.

**Revised (2026-10-16)**: Discord and Telegram names *are* looked up live when the page loads, by request: through a new `IntegrationKind::conversation_names` hook (Discord: channel + guild; Telegram: chat title), using the agent's own enabled connection, for at most 25 places per report, in parallel, 3 s timeout each, cached in memory for 10 min (1 min after a failure). A conversation whose bot is gone or unreachable keeps its id.

## R10 — Conversation details: link and plain-language insight

**Decision**: The details endpoint returns a `link`, or a `no_link_reason`, resolved server-side:

| Conversation | Link |
|---|---|
| WebUI topic | `chat` → `/{agent}/chat/{topic}` |
| Task run | `task_run` → `/{agent}/tasks?task={slug}` |
| Dream | `dream` → `/{agent}/dream` |
| Subagent or InterAgent piece | `topic` of the launching session, via `BackgroundJobStorage` (piece session → job → origin), if it is a WebUI topic |
| Discord / Telegram | none: `no_web_page` |
| Session no longer in `session_detail` | none: `deleted` |

The `insight` is a deterministic template, filled only when failed-turn cost is at least 50% of the conversation's cost: "‹n› turns failed (errored, timed out or were aborted) after ‹m› answered rounds; ‹$x› of ‹$y› bought no answer. The longest ran ‹k› rounds." No model is asked; this is accounting, not a turn.

## R11 — Verifying with dummyplug when it reports zero tokens

**Decision**: Keep dummyplug's zero usage. The protocol is a test-harness contract (constitution), and the zero-usage test pins it. The quickstart checks **calls**: count, activity, model, served model, failed marking and links. Token and cost arithmetic is covered by unit tests on seeded `usage_event` rows. One live-provider step, marked as such, confirms that real token counts arrive.

## R12 — WebUI

**Decision**:
- Rewrite `webui/app/routes/usage.tsx` to the mockup (spec Assumptions).
- Keep Recharts, already a dependency, for the one stacked daily-cost chart, and remove the old `ChannelTypeStackedChart` and the toggles from `UsageBarChart.tsx`.
- The details drawer fetches `usage/conversation` on demand.
- State is the period, activity and dimension, held in the URL search params so a view can be linked to. No new store.

## Resolved unknowns

There were no open `NEEDS CLARIFICATION` items in the spec. Every technical unknown above is resolved, and no new crates are needed.
