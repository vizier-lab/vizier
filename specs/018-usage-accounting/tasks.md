---
description: "Task list for 018-usage-accounting"
---

# Tasks: Complete Usage Accounting and an Insightful Usage Page

**Input**: Design documents from `specs/018-usage-accounting/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/http-api.md](contracts/http-api.md), [contracts/webui.md](contracts/webui.md), [quickstart.md](quickstart.md)

**Tests**: The plan's Testing section and quickstart.md name specific unit tests. Those tests are included next to the code they cover, in `#[cfg(test)] mod tests` blocks in the same file, as the rest of the codebase does. There is no TDD-first ordering.

**Organization**: Tasks are grouped by user story (spec.md):
- **US1**: every call is counted (P1);
- **US2**: model tracking (P1);
- **US3**: spend at a glance, including the activity filter (P2);
- **US4**: costliest conversations and details (P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4; setup, foundational and polish tasks have no label

Research decisions are cited as `R#`, and data-model sections as `DM§#`.

---

## Phase 1: Setup

**Purpose**: Empty module homes, so later tasks can be parallel without touching the same `mod.rs` lines.

- [ ] T001 Create empty modules and register them:
  - `src/schema/usage.rs` (`pub mod usage;` plus re-exports in `src/schema/mod.rs`);
  - `src/storage/usage.rs` (`pub mod usage;` in `src/storage/mod.rs`);
  - `src/storage/sqlite/usage.rs` (`mod usage;` in `src/storage/sqlite/mod.rs`);
  - `src/agents/agent/model/usage.rs` (`pub mod usage;` in `src/agents/agent/model/mod.rs`);
  - `src/usage/mod.rs` (`mod usage;` in `src/main.rs`, with the other top-level `mod` lines).

  Run `cargo check`.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: The types, storage and call-site signature that every story builds on.

**⚠️ CRITICAL**: no user story work begins until this phase is complete.

- [ ] T002 [P] In `src/schema/usage.rs`, define `UsageActivity` (`Chat, Background, Scheduled, Dreams, Compaction, Vision, Other`):
  - serde `snake_case`, with `as_str()` / `FromStr`, and `JsonSchema` + `utoipa::ToSchema` derives;
  - `UsageEvent` with every `usage_event` column except `id` (DM§2), using `chrono::DateTime<Utc>` for `at` and `TokenCounts` for the four token fields.
- [ ] T003 [P] In `src/schema/metrics.rs`:
  - add `pub struct TokenCounts { input, cached, cache_write, output: u64 }`, with `Add`/`AddAssign` and `total()`;
  - change `estimate_cost` to take `&TokenCounts` instead of `&VizierResponseStats` (R7), and add `impl From<&VizierResponseStats> for TokenCounts` (using the `total_*` fields);
  - update the existing caller in `src/storage/sqlite/history.rs` to pass `&TokenCounts::from(stats)`, so behaviour stays identical until it is removed in T021;
  - add `cache_hit_rate(&TokenCounts, cache_outside_input) -> Option<f64>` (R7: `cached/input`, or `cached/(input+cached)` when outside; `None` when the denominator is 0);
  - unit tests: parity with the old function on two `VizierResponseStats` fixtures (cache inside and outside), and `cache_hit_rate` edge cases.
- [ ] T004 [P] In `src/schema/session.rs`, add `VizierChannelId::usage_activity(&self) -> UsageActivity` and `VizierSession::usage_place(&self) -> String` exactly per the DM§1 table:
  - Discord → `discord:{id}`; Telegram → `telegram:{id}`; HTTP → `http:{user}:{topic or ""}`;
  - Task → `task:{slug}`; Subagent/InterAgent → `background`; Dream → `dreams`; System → `system`.

  Add unit tests covering every variant.
- [ ] T005 [P] In `src/agents/agent/model/usage.rs`, define:
  - `UsagePurpose { Turn, Handover, Vision }`;
  - `TurnId(Uuid)` with `new()` and `Display`;
  - `UsageScope { session: Option<VizierSession>, purpose, turn: Option<TurnId>, round: Option<u32> }`, with constructors `UsageScope::turn(session, turn, round)`, `::handover(session)` and `::vision(session)`;
  - `UsageScope::activity(&self) -> UsageActivity`: Handover → Compaction, Vision → Vision, Turn → `session.1.usage_activity()`, no session → Other (R4).
- [ ] T006 Define the `UsageStorage` trait in `src/storage/usage.rs` per DM§3:
  - methods `record_usage`, `record_failed_turn`, `usage_groups`, `conversation_usage`, `insert_legacy_usage`;
  - types `UsageQuery`, `UsageGrouping`, `UsageGroup`, `UsageEventRow` (an event plus `failed: bool`);
  - add it to the `VizierStorageProvider` supertrait list in `src/storage/mod.rs`, and hand-forward every method on `VizierStorage` the same way the other traits are forwarded there.

  Depends on T002 and T003.
- [ ] T007 In `src/storage/sqlite/mod.rs`, add `pub fn init_usage_schema(conn: &Connection) -> Result<()>` with the exact DDL in DM§2 (both tables and all four indexes), and call it from `init_schema` after `init_integration_schema`.
- [ ] T008 Implement `UsageStorage for SqliteStorage` in `src/storage/sqlite/usage.rs`:
  - `record_usage` and `insert_legacy_usage` are plain inserts; the batch runs in one transaction;
  - `record_failed_turn` is `INSERT OR IGNORE`;
  - `usage_groups` builds one `SELECT … SUM(input_tokens), SUM(cached_input_tokens), SUM(cache_creation_input_tokens), SUM(output_tokens), COUNT(*), COUNT(DISTINCT turn_id), MAX(at) … GROUP BY <grouping cols>, provider, model, failed`:
    - `failed` is `e.turn_id IN (SELECT turn_id FROM usage_failed_turn)`;
    - filtered by `agent_id`, the `at` range and the optional `activity`;
    - the grouping columns per `UsageGrouping` are: `Day` → `date(at/1000,'unixepoch')`, activity; `Activity` → activity; `Model` → provider, model, served_model; `Place` → place; `Conversation` → channel, topic;
  - `conversation_usage` returns that conversation's rows in range with their failed flag and the total `COUNT(*)`. The caller prices and sorts them, so here they are ordered by `(input_tokens + output_tokens) DESC` and limited to `limit * 4` to leave room for re-sorting by cost;
  - `pub fn delete_agent(conn, agent_id)` deletes from both tables.

  Depends on T006 and T007.
- [ ] T009 Unit tests in `src/storage/sqlite/usage.rs` against an in-memory connection (stand it up with `init_usage_schema`, as `src/storage/sqlite/task_run.rs` tests do):
  - each `UsageGrouping` returns the expected sums for seeded rows;
  - the failed split via `usage_failed_turn`, and that `record_failed_turn` before the turn's events are inserted still marks them (the order-independence of R3);
  - the activity filter and the range bounds;
  - `delete_agent` removes only that agent's rows.
- [ ] T010 In `src/storage/sqlite/agent.rs` `delete_agent`, call `super::usage::delete_agent(&conn, agent_id)?` next to `core_revision::delete_agent` (FR-008). Confirm `delete_session` in `src/storage/sqlite/` does **not** touch usage tables.
- [ ] T011 Change `VizierModel` in `src/agents/agent/model/mod.rs` (R1, R2):
  - add an `agent_id: AgentId` field and a `storage: Arc<VizierStorage>` handle (from `deps.storage`) to the struct;
  - `new` stores the `agent_id` it already receives (rename `_agent_id`);
  - `new_with_override` gains an `agent_id: &AgentId` parameter;
  - `complete(&self, req, scope: &UsageScope)` and `complete_until(&self, req, deadline, scope: &UsageScope)`. After `inner.complete_until` returns `Ok(resp)`, build a `UsageEvent` with:
    - `at` = now, `activity` = `scope.activity()`;
    - `channel` / `topic` / `place` from `scope.session`; `place` = `"system"` when there is no session;
    - `turn_id` / `round` from the scope;
    - `provider` / `model` from `self.info`, `served_model` = `resp.model.clone()`;
    - tokens from `resp.usage`;
    - `duration_ms` measured around the inner call;
    - `legacy = false`.
  - `tokio::spawn` a task that calls `storage.record_usage(event)` and `tracing::warn!`s on error. Never await it in the call path, and never record on `Err`.
- [ ] T012 Update every caller so the crate compiles with the new signatures, passing a correct scope:
  - `src/agents/agent/mod.rs`:
    - the main `prompt` loop (`self.model.complete_until(…)` around line 829) → `UsageScope::turn(Some(ctx.session.clone()), turn_id, Some(turn_depth))`. Create a placeholder `let turn_id = TurnId::new();` at the top of the turn for now; T015 replaces it with the guard;
    - `dream_prompt` (`model.complete(…)` around line 1385) → the same, with its own `TurnId`;
    - `dream_turn`'s `VizierModel::new_with_override(deps, &self.config, …)` → pass the agent id;
  - `generate_handover_with_model` in `src/agents/agent/mod.rs`: add a `scope: &UsageScope` parameter and pass it to `complete`. `VizierAgent::generate_handover_message` passes `UsageScope::handover(Some(ctx.session.clone()))`, using the `_ctx` argument it already receives (rename it to `ctx`);
  - `src/agents/tools/mod.rs` (vision model build around line 1086) → pass the agent id;
  - `src/agents/tools/read_image/mod.rs` (`vision.complete(…)`) → `UsageScope::vision(Some(ctx.session.clone()))`, where `ctx` is the tool's `ToolContext`;
  - any test call sites in `src/agents/agent/model/dummyplug.rs` and `src/llm/` that go through `VizierModel` (not `CompletionModel` directly).

  Run `cargo check` and `cargo test`.

**Checkpoint**: every completion is recorded into `usage_event`, with nothing reading it yet.

---

## Phase 3: User Story 1 — Every model call is counted (P1) 🎯 MVP

**Goal**: The usage totals include dreams, compaction, failed and aborted rounds, and vision. Usage from before the upgrade is carried over. The page shows correct totals and an activity breakdown.

**Independent test**: quickstart.md §1, §2, §3.1, §4, §5, §6 and §7.1. Each one shows calls appearing under the right activity, failed rounds marked, and totals from before the upgrade unchanged.

- [ ] T013 [P] [US1] In `src/agents/agent/model/usage.rs`, add `TurnGuard { turn: TurnId, agent_id: AgentId, storage: Arc<VizierStorage>, done: bool }` with `new`, `turn()` and `succeeded(mut self)` (sets `done`). `Drop`: when `!done`, if `tokio::runtime::Handle::try_current()` is `Ok`, spawn `storage.record_failed_turn(&turn, &agent_id, Utc::now())`, logging any error with `tracing::warn!` (R3). Add a unit test (with `#[tokio::test]`) using an in-memory `SqliteStorage`:
  - dropping without `succeeded()` records one row;
  - `succeeded()` records none.
- [ ] T014 [P] [US1] Add `VizierModel::guard(&self) -> TurnGuard` in `src/agents/agent/model/mod.rs`, built from the model's own `agent_id` and `storage`, so call sites don't need to thread storage through.
- [ ] T015 [US1] In `src/agents/agent/mod.rs`, in both the main `prompt` and `dream_prompt`:
  - replace the T012 placeholder `TurnId` with `let guard = model.guard();`, and pass `guard.turn()` into every round's scope;
  - call `guard.succeeded()` on every path that returns `Ok`, including the depth-exhausted wrap-up answer;
  - leave every `Err` / `fail_turn` path, and the prompt-timeout path, to drop the guard, so it marks the turn failed. An aborted task drops the future and therefore the guard (#52).

  Depends on T013 and T014.
- [ ] T016 [US1] In `src/agents/agent/mod.rs` `maybe_checkpoint`, confirm the handover call (via `generate_handover_message`, T012) carries the turn's session, so automatic compaction is attributed to the same `place` as the turn. In `src/agents/process.rs`, the manual `checkpoint` command builds its `ToolContext` with `session: session.clone()` (around line 825), so `UsageScope::handover` gets the right session through `generate_handover_message`. Verify both; no new code if they already do.
- [ ] T017 [US1] Add a `UsageStorage`-backed carry-over in `src/dependencies.rs`, `async fn migrate_reply_usage(storage: &VizierStorage) -> Result<()>` (R6):
  - return early when the `state` key `usage.accounting_since` exists (use `StateStorage`, as the other migrations do);
  - otherwise iterate every agent (`AgentStorage::list_agents`). For each agent, read its `session_history` `Response` rows that carry `VizierResponseStats` (`Message { stats }` and `AudioReply(_, _, stats)`, the same two arms `aggregate_usage` matched), in batches of 1000;
  - map each row to a `UsageEvent`:
    - `legacy = true`, `at` = the history timestamp;
    - `activity` = `session.1.usage_activity()`, `place` = `session.usage_place()`;
    - `channel` = `session.1.to_slug()`, `topic` = `session.2`;
    - `provider` / `model` from the stats, or `""` when absent;
    - tokens from `TokenCounts::from(stats)`, `duration_ms` from `stats.duration`;
    - `turn_id` / `round` / `served_model` = `None`;
  - insert each batch with `insert_legacy_usage`; then write `usage.accounting_since` = `Utc::now().to_rfc3339()`;
  - call it from the startup migration sequence after the existing ones. A failure is logged with `tracing::error!` and does not block startup; it reruns next start because the key is unset.
- [ ] T018 [US1] Unit test for T017 in `src/dependencies.rs` (or a `#[cfg(test)]` helper next to it), using an in-memory `SqliteStorage`:
  - seed `session_history` with `Response` rows with stats across Discord, a task and a dream session;
  - run the migration;
  - assert that the grouped totals from `usage_groups(Activity)` equal what the old `aggregate_usage` returned for the same seed (write that expectation before T021 removes the function, or compute it by hand);
  - assert that a second run inserts nothing (SC-007, idempotence).
- [ ] T019 [US1] Report builder, first slice, in `src/usage/mod.rs`:
  - `pub async fn build_report(storage: &VizierStorage, catalogue: &Catalogue, q: UsageQuery) -> crate::Result<UsageReport>`;
  - a pricing helper `price(provider, model) -> Option<(Cost, bool)>`, moved from the closure in `agent_usage` (`src/channels/http/api/v1/agents/mod.rs`), so it has one home;
  - `summary` (all token fields, `calls`, `cost_usd` via `metrics::add_cost` semantics so it is `null` when nothing is priced, `cache_hit_rate`, `failed { cost_usd, tokens, turns }`) from `UsageGrouping::Activity` groups;
  - `daily` from `UsageGrouping::Day`, filling every date in range with an empty `by_activity`;
  - `by_activity` rows (label from the activity, `share` = cost/total);
  - `accounting_since` read from state;
  - `range` echoed.

  Leave `previous`, `by_model`, `by_place`, `costliest` and `unpriced` as empty or `None`; later stories fill them. Define the report types it returns in `src/schema/usage.rs` per DM§4 and contracts/http-api.md, with `Serialize`, `utoipa::ToSchema`, and every field present from the start.
- [ ] T020 [US1] Rewrite the `agent_usage` handler in `src/channels/http/api/v1/agents/mod.rs`:
  - parse `start_date` / `end_date` (defaults: end = now, start = end − 30 days) and `activity`;
  - validate (`400 invalid_range` when start > end or the range exceeds 366 days; `400 unknown_activity`);
  - call `usage::build_report`, and return `APIResponse<UsageReport>`;
  - update its `#[utoipa::path]`. In `src/channels/http/mod.rs`, replace `AgentUsageStats` and the old helper schemas in `components(schemas(…))` with `UsageReport` and its row types.
- [ ] T021 [US1] Remove the old path:
  - `aggregate_usage` from `HistoryStorage` (`src/storage/history.rs`, trait and `VizierStorage` forwarding), from `src/storage/sqlite/history.rs` (and `get_channel_type` if now unused), and from `src/storage/fs/history.rs`;
  - `AgentUsageStats`, `ChannelUsage`, `ChannelTypeUsage`, `ChannelTypeUsageDetail`, `DailyChannelTypeUsage`, the old `DailyUsage` and the old `UsageSummary` from `src/schema/metrics.rs` and the `src/schema/mod.rs` re-exports;
  - the `pricing` closure.

  Run `cargo clippy` and `cargo test`.
- [ ] T022 [P] [US1] In `webui/app/interfaces/types.ts`, remove `AgentUsageStats` and its helper interfaces, and add `UsageActivity`, `UsageReport`, `UsageSummary`, `FailedTotals`, `PeriodTotals`, `DailyUsage`, `BreakdownRow`, `ModelRow`, `ConversationRow`, `UnpricedNote`, `ConversationUsage`, `CallRow` and `UsageLink`, exactly mirroring contracts/http-api.md. In `webui/app/services/vizier.tsx`, change `getAgentUsage(agentId, start, end, activity?)` to send `activity` when set and return `UsageReport`.
- [ ] T023 [P] [US1] Create `webui/app/components/usageActivities.ts`: an ordered list of the seven activities, each with `id`, `label`, one-line `description` (the descriptions under "Where it went" in the mockup) and a `color`. Colours are CSS variable references or hexes per contracts/webui.md "Activity palette", with Chat as `var(--accent-dark)` and Other as `var(--color-text-muted)`. Export `activityMeta(id)`. Move `formatCost` from `webui/app/components/UsageBarChart.tsx` to `webui/app/utils/format.ts` (create it if absent), next to a `formatTokens` (K/M) helper.
- [ ] T024 [US1] Rewrite `webui/app/routes/usage.tsx`, first slice:
  - header with breadcrumb, title and period picker (7, 14, 30 or 90 days as `<button aria-pressed>` segments, state in the URL search param `period`);
  - headline card (cost, or "—" with "no known price"; tokens; calls; "biggest driver: ‹activity› (‹share›)");
  - the "Where it went" ranked list for **Activity only** (colour swatch, label, description, share bar with %, cost, tokens, calls);
  - the empty state "No usage in this period".

  Use `app.css` tokens (`var(--surface)`, `var(--border)`, `var(--text-secondary)`…), not literal hexes. Delete the old charts' usage from this route. Run `cd webui && npm run typecheck`.

**Checkpoint**: US1 is shippable. All calls are counted and visible by activity, with totals from before the upgrade preserved.

---

## Phase 4: User Story 2 — Know which model was used (P1)

**Goal**: A by-model breakdown with requested and served models, cache rate and failed tokens per model, and a note for unpriced tokens.

**Independent test**: quickstart.md §2.1 (vision model shown separately from the chat model) and §8.1 and §8.3 (served version, unpriced note). Switching an agent's model mid-period splits the days between the two models.

- [ ] T025 [US2] In `src/usage/mod.rs`, fill `by_model` from `UsageGrouping::Model` groups:
  - one `ModelRow` per `(provider, model)` with the `served_models` collected (distinct, non-null, sorted);
  - `cache_hit_rate` per model using that provider's `cache_outside_input` flag;
  - `failed_tokens`, and `priced` = the price lookup succeeded;
  - unpriced rows get `cost_usd: None` and `share: None`;
  - sort by cost descending, with unpriced rows last by tokens;
  - legacy rows with `provider == ""` show as "unknown model".

  Also fill `unpriced { tokens, models }`, summing every unpriced group, with models as `provider/model` strings.
- [ ] T026 [US2] In `src/usage/mod.rs`, add unit tests over a seeded in-memory storage:
  - two models across a mid-period switch give two rows with the right day split in `daily`;
  - a custom provider without a price lands in `unpriced` and is excluded from `summary.cost_usd`;
  - a served model differing from the requested one is listed in `served_models`.
- [ ] T027 [US2] In `webui/app/routes/usage.tsx`:
  - add the breakdown dimension switch (segmented `<button aria-pressed>`: Activity, Model; Channel is added in US3), held in the URL param `by`;
  - render Model rows with "‹provider› / ‹model›" as label and a detail line "served ‹versions› · cache ‹n›% · ‹f›% failed" ("No known price, not in the cost" when `!priced`);
  - add the unpriced note line under the headline when `unpriced.tokens > 0`.

  Run `npm run typecheck`.

**Checkpoint**: US1 and US2 together answer "what did it cost, on which model".

---

## Phase 5: User Story 3 — Understand spend at a glance (P2)

**Goal**: The change against the previous period, an activity filter scoping the whole page, one stacked daily cost chart, a Channel breakdown with readable names, and a partial-accounting notice.

**Independent test**: with seeded usage over two periods, the page shows the headline change, the daily chart stacked by activity, and the breakdown switchable across all three dimensions. Choosing an activity chip narrows every section (spec US3 scenarios 1–7).

- [ ] T028 [US3] In `src/usage/mod.rs`:
  - fill `previous` by running the Activity grouping over `[start − (end − start), start)` with the same `activity` filter. It is `None` when that period has no rows;
  - when `q.activity` is set:
    - leave `by_activity` empty;
    - set `summary.share_of_all` = this cost ÷ the unfiltered total for the same range (one extra Activity grouping without the filter);
    - keep the `daily` series only for that activity.
- [ ] T029 [US3] In `src/usage/mod.rs`, fill `by_place` from `UsageGrouping::Place`, with labels resolved only for the returned rows (R9):
  - `http:{user}:{topic}` → `SessionStorage::get_session_detail_by_topic(...).title`, else "WebUI topic";
  - `task:{slug}` → the task's title via `TaskStorage`, else the slug, with detail "Scheduled task";
  - `dreams` → "Dream cycle", with detail "Unattended reflection";
  - `background` → "Background jobs";
  - `discord:{id}` / `telegram:{id}` → a saved session title if present, else "Discord channel ‹id›" / "Telegram chat ‹id›", with detail "Discord" / "Telegram";
  - `system` → "System".

  No network calls.
- [ ] T030 [P] [US3] Create `webui/app/components/UsageDailyChart.tsx`: one Recharts `BarChart` stacked by activity over `daily`, with series coloured from `usageActivities.ts`, showing only activities present in range (or only the filtered one).
  - The tooltip shows the date, cost, tokens and calls.
  - Bars dated before `accounting_since` render at 0.45 opacity.
  - The y-axis is formatted as cost.
  - Add a visually hidden `<table>` (date, cost) for screen readers.

  Then delete `webui/app/components/UsageBarChart.tsx`.
- [ ] T031 [US3] In `webui/app/routes/usage.tsx`:
  - add the activity filter chips (All plus each activity; Other only if it has usage), held in the URL param `activity` and sent to `getAgentUsage`;
  - when filtered, drop "Activity" from the dimension switch, and switch `by` to `place` if it was `activity`;
  - add the Channel (`place`) dimension;
  - add the change pill from `previous` (hidden when null);
  - when filtered, the headline label becomes "‹Activity› · estimated cost · ‹range›" and the driver text becomes "‹share_of_all›% of all spend";
  - mount `UsageDailyChart`;
  - add the partial-accounting notice when `range.start < accounting_since` and (no filter, or filter in {dreams, compaction, vision}), with the wording from contracts/webui.md;
  - add "Custom" period with two date inputs;
  - keep the previous data on screen, dimmed, while a new fetch is in flight.

  Run `npm run typecheck`.

**Checkpoint**: the page answers cost, trend and drivers within seconds (SC-002), with at most one chart (SC-004).

---

## Phase 6: User Story 4 — Find what is expensive or wasteful (P3)

**Goal**: The cache-rate and failed-spend tiles, the top 10 costliest conversations, and a details drawer with calls ranked by cost, a plain-language insight, and a link to the conversation or a reason there isn't one.

**Independent test**: quickstart.md §1.3, §5, §6 and §7.2. Details for a WebUI topic link to chat; a task run links to Tasks; a dream links to the journal; a background piece links to its launching topic; a deleted topic shows `deleted`; a timed-out turn shows `failed: true` rounds.

- [ ] T032 [US4] In `src/usage/mod.rs`, fill `costliest` from `UsageGrouping::Conversation` groups:
  - price per `(provider, model)` sub-group and sum per `(channel, topic)`;
  - `activity` / `model` = the sub-group with the highest cost;
  - `failed_share` = failed cost ÷ cost;
  - `last_at`;
  - `exists` = `SessionStorage::get_session_detail_by_topic` finds it (for HTTP), or the slug still appears in `session_history` (for others; one `EXISTS` query per row; at most 10 rows);
  - label and detail via the same resolver as T029;
  - sort by cost descending (unpriced last by tokens) and take 10.
- [ ] T033 [US4] In `src/usage/mod.rs`, add `pub async fn conversation_usage(storage, catalogue, q, channel, topic, limit) -> crate::Result<ConversationUsage>`:
  - rows from `UsageStorage::conversation_usage`, priced, sorted by cost descending, truncated to `limit`, and `more` = total − returned;
  - `summary` computed over **all** of the conversation's rows in range (run the Activity grouping filtered to this channel/topic, or sum the rows);
  - `insight` only when failed cost ≥ 50% of cost, with the exact template from R10. "Longest" is the max `round` among failed turns, from a `MAX(round) … GROUP BY turn_id` over failed rows;
  - `link` / `no_link_reason` per R10:
    - HTTP → `chat` `/{agent}/chat/{topic}`;
    - `task__…` → `task_run` `/{agent}/tasks?task={slug}`;
    - `DREAM__…` → `dream` `/{agent}/dream`;
    - Subagent / InterAgent → look up the piece by session via `BackgroundJobStorage`, then its job's origin session; if that is an HTTP topic → `topic` `/{agent}/chat/{origin topic}`, else `no_web_page`;
    - Discord / Telegram → `no_web_page`;
    - session gone → `deleted`.

  If `BackgroundJobStorage` has no lookup by piece session, add `background_piece_by_session(agent_id, channel_slug)` to that trait and its sqlite implementation in `src/storage/sqlite/background_job.rs`.
- [ ] T034 [US4] In `src/channels/http/api/v1/agents/mod.rs`, add the route `.route("/{agent_id}/usage/conversation", get(agent_usage_conversation))`:
  - query `channel` (required → `400 missing_channel`), `topic`, `start_date`, `end_date`, `activity` and `limit` (1–100, default 20 → `400 invalid_limit`);
  - the same access check as `agent_usage`;
  - an unknown channel returns `200` with empty `calls` and `no_link_reason: "deleted"`;
  - `#[utoipa::path]`; register `ConversationUsage`, `CallRow` and `UsageLink` in `src/channels/http/mod.rs`.
- [ ] T035 [US4] Unit tests in `src/usage/mod.rs`:
  - the insight appears at 60% failed and not at 40%;
  - `more` is counted correctly;
  - the link mapping for HTTP, task, dream and Discord channel slugs;
  - `costliest` ranks by cost and caps at 10.
- [ ] T036 [P] [US4] In `webui/app/services/vizier.tsx`, add `getConversationUsage(agentId, { channel, topic, start, end, activity, limit })` returning `ConversationUsage`.
- [ ] T037 [P] [US4] Create `webui/app/components/UsageConversationDrawer.tsx` per contracts/webui.md:
  - `role="dialog" aria-modal="true"`, a right-side panel (full width under 640px), with Esc and the close button dismissing it and focus returning to the opener;
  - it fetches on open;
  - header with activity dot, label and detail;
  - four figures (cost, calls, cached input %, failed cost);
  - the insight callout;
  - the calls list (time, "‹activity› · round ‹n›", served model, or red "part of failed turn"; input, output, cost);
  - "+ ‹more› more calls";
  - footer link labelled by `link.kind` (Open in chat / Open run in Tasks / Open in Dream journal / Open in launching topic) as a React Router `<Link>`, or the `no_link_reason` sentence.
- [ ] T038 [US4] In `webui/app/routes/usage.tsx`:
  - add the **Cache hit rate** tile (`summary.cache_hit_rate` with a bar; "—" when null) and the **Spent on failed turns** tile (cost, share of spend, turn count, and a link scrolling to `#costliest`);
  - add the **Costliest conversations** section (rank, label and detail, activity dot, model, tokens, cost, "Failed" marker when `failed_share ≥ 0.5`, and a **Details** `<button>` opening `UsageConversationDrawer` with that row's `channel` / `topic` and the page's range and activity);
  - the empty list text "No conversations for this activity in the period."

  Run `npm run typecheck`.

**Checkpoint**: all four stories work independently and together.

---

## Phase 7: Polish & cross-cutting

- [ ] T039 [P] In `src/usage/mod.rs`, add `#[tracing::instrument(skip_all, fields(vizier.agent_id = %q.agent_id))]` on `build_report` and `conversation_usage`. Add no content attributes. Make sure recording errors use `tracing::warn!` with the agent id only.
- [ ] T040 [P] Update `CLAUDE.md`:
  - a short **Usage accounting** paragraph under Architecture: `VizierModel` records every answered completion with a required `UsageScope`; `TurnGuard` and `usage_failed_turn` mark failed turns; `usage_activity()` / `usage_place()` decide grouping; `src/usage/` builds reports; `migrate_reply_usage` carried old reply stats over; the `usage_event` / `usage_failed_turn` tables;
  - add `UsageStorage` to the `VizierStorageProvider` trait list there;
  - note that a new `VizierModel` call site must pass a scope.
- [ ] T041 [P] Add a usage section to the docs if `docs/src/` has a WebUI or usage page (check `docs/src/SUMMARY.md`): what each activity means, what is not counted (audio, TTS, STT, image generation, embeddings, the provider test ping), and the partial-accounting date.
- [ ] T042 Performance check (SC-006): seed about 100k `usage_event` rows for one agent over 90 days in a scratch data dir (a throwaway `#[ignore]` test in `src/storage/sqlite/usage.rs` is fine), and time `GET /agents/{id}/usage` for 90 days. It must be under 2s; if not, add the missing index or merge groupings.
- [ ] T043 Run `cargo clippy` and `cargo test`, and `cd webui && npm run typecheck`. Fix anything this feature introduced.
- [ ] T044 Run quickstart.md §1–§7 end to end against a running binary (`just run`) with a dummyplug agent, recording each step's result in the PR description. Run the live steps in §8 if a key is available, and mark them skipped otherwise.
- [ ] T045 Commit per story, as in Implementation strategy. The US1 commit is `feat: count every model call and rebuild the usage page on it [**breaking**]`, with a body noting the `GET /agents/{id}/usage` response change. The US4 commit notes the new `GET /agents/{id}/usage/conversation` endpoint.

---

## Dependencies & execution order

```text
Phase 1 (T001)
  └─► Phase 2: T002, T003, T004, T005 in parallel ─► T006 ─► T007 ─► T008 ─► T009, T010
                                                     T011 (needs T005, T006) ─► T012
       └─► US1: T013, T014 in parallel ─► T015 ─► T016
                T017 ─► T018
                T019 ─► T020 ─► T021
                T022, T023 in parallel ─► T024 (needs T020)
            └─► US2: T025 ─► T026; T027 (needs T024, T025)
            └─► US3: T028, T029 ─► T031 (needs T030, T024)
            └─► US4: T032, T033 ─► T034 ─► T035; T036, T037 in parallel ─► T038 (needs T034)
                 └─► Polish: T039–T045
```

- **US1 → everything**: US2–US4 extend `build_report` and the page US1 creates.
- **US2, US3 and US4 do not depend on each other.** After US1 they can go in any order or in parallel, except that they all edit `src/usage/mod.rs` and `webui/app/routes/usage.tsx`. Merge carefully, or sequence them.
- Within a story, Rust report work comes before the WebUI task that renders it.

## Parallel examples

- **Phase 2**: T002 (schema/usage.rs), T003 (schema/metrics.rs), T004 (schema/session.rs) and T005 (model/usage.rs) touch different files.
- **US1**: T013 + T014 (guard), alongside T017 (migration), alongside T022 + T023 (WebUI types and palette).
- **US3**: T030 (chart component) while T028 and T029 are done in Rust.
- **US4**: T036 + T037 (WebUI service and drawer) while T032 and T033 are done in Rust.

## Implementation strategy

- **MVP = Phase 1 + Phase 2 + US1.** Every call is counted, older usage is carried over, and the page shows totals and activity. This alone answers the original question ("is hidden usage counted?") and is shippable.
- Then **US2** (model) completes the P1 scope.
- **US3** and **US4** are the insight layer. Each is a separate, demoable increment.
- Commit per story (`feat:` …). Only the US1 commit carries `[**breaking**]`, because it is the one that changes the `GET …/usage` response.
