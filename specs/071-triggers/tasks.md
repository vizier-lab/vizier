# Tasks: Triggers

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/)

`[P]` can run in parallel. `[USn]` is the spec's user story. Paths under `core/` are
`crates/vizier-core/src/`; `http/` is `crates/vizier-http/src/`. Tests are written with their task.

## Phase 1: Foundation (blocks everything)

- [x] T001 [P] `Trigger`, `TriggerEvent`, `TriggerRun`, states, `ToolScope` in `core/schema/trigger.rs`
- [x] T002 [P] `VizierChannelId::Trigger` with UTC time and `trigger__` slug in `core/schema/session.rs`; test one conversation per firing
- [x] T003 `TriggerStorage` and SQLite tables in `core/storage/trigger.rs`, `core/storage/sqlite/trigger.rs`
- [x] T004 PostgreSQL migration `core/storage/sql/postgres/migrations/0020_triggers.sql`
- [x] T005 `admit` and `start_run` transactions (queue caps, overlap, limit, cooldown, monitor state) with concurrency tests (depends T003)
- [x] T006 Fenced writes on `control`, one test per write in `core/storage/sqlite/fence_tests.rs`
- [x] T007 `TriggerKind`, `TriggerKinds::builtin()` in `core/triggers/kind.rs`; a test-only kind
- [x] T008 `VizierRequest.tool_scope` and `trigger`; `ToolContext.tool_scope`; clear both on client receipt
- [x] T009 Apply the scope in `turn_catalogue` and `turn_categories`, including the `memory_read` entry; test every memory tool is classified (`core/agents/tools/mod.rs`)
- [x] T010 Carry the scope to workers, wakes, consultations, custom tools; one test per path
- [x] T011 `Triggers::fire`, dispatch, close from the reply stream, retry of waiting events when a window reopens, in `core/triggers/mod.rs`
- [x] T012 Recovery: resume or interrupt runs, pick up pending and waiting events, in `core/recovery.rs`
- [x] T013 [P] Framing in `core/agents/agent/system_prompt/trigger_run.rs`, chosen on the channel; no question tools
- [x] T014 [P] Refuse detached scripts in trigger runs (`core/agents/detached/mod.rs`)
- [x] T015 [P] `is_non_user_channel`, not wakeable, `UsageActivity::Trigger`
- [x] T016 [P] Once-per-run delivery to inbox and destination, hourly trigger notices (`core/triggers/delivery.rs`, `core/notifications/`)
- [x] T017 Secret redaction and caps on stored payloads
- [x] T018 Six permissions in `core/storage/user.rs`, rules in `core/triggers/access.rs`, labels in `webui/app/lib/permissions.ts`

**Check**: a test kind fires end to end; scope and limit tests pass.

## Phase 2: Webhook (US1, US2) - first release

- [x] T019 [US1] Webhook kind: settings, secret, signature, header and body filter, delivery id from header or JSON field, forwarded headers (`core/triggers/webhook.rs`)
- [x] T020 [US1] Encrypt secrets at the storage boundary; mask in reads
- [x] T021 [US1] Service operations and authorization in `core/service/triggers.rs`
- [x] T022 [US1] Public route in `http/api/v1/hooks.rs`: order, 256 KiB limit, one `404`, `413`, `415`
- [x] T023 [US2] Miss limiter by `client_address`; collapse storm drops into one record
- [x] T024 [US1] Test the address secret never reaches logs or spans
- [x] T025 [US1] Signed-in routes in `http/api/v1/agents/triggers.rs`, OpenAPI, conformance vectors
- [x] T026 [US2] Delete cascades for trigger and agent; delivery expiry
- [x] T049 [US7] Deliver to the setter only: resolve `delivers_to`, owner fallback, audience on notices; destination failure reported once
- [x] T050 [US7] `nothing_to_report` tool and the summary timer (`core/triggers/delivery.rs`)
- [x] T051 [US2] Per-kind full-queue policy (`when_full`); failed-run retry once
- [x] T052 [US1] Optional timestamp check; payload redaction rules; 3-day payload sweep
- [x] T046 [US7] Triage: batching, call and queue limits, fenced untrusted payloads, verdict parsing, fail open, usage (`core/triggers/triage.rs`)
- [x] T047 [US7] Triage and destination settings in the API, destination access check, events route; model choice permission; needs-attention on a removed model
- [x] T048 [US7] dummyplug e2e: mixed events, one delivery, malformed verdict, storm within limits
- [x] T027 [US1] dummyplug e2e: webhook firing, framing, scope (`crates/vizier-core/tests/dummyplug_e2e.rs`)
- [x] T028 [US1] `docs/src/configuration/triggers.md`: webhooks, security, proxies and access rules

**Check**: quickstart steps 1-9, 14 and 17-19.

## Phase 3: Monitor (US3)

- [x] T029 [US3] `AgentsClient::run_check`, host handling, contract update, fake client
- [x] T030 [US3] Run a script with the listed tools only, no model-backed or turn-ending tools, wall-clock timeout, redaction
- [x] T031 [US3] Monitor kind and shared source: schedule, skip while running, result shape, state cap (`core/triggers/monitor.rs`)
- [x] T032 [US3] Backoff, failed state, owner notification, reset on edit
- [x] T033 [US3] Test endpoint that neither fires nor saves
- [x] T034 [US3] dummyplug e2e and docs for monitors

**Check**: quickstart steps 10-12.

## Phase 4: Agent tools and addons (US4, US6)

- [x] T035 [US4] `ToolCategory::Triggers` appended; eight tools in `core/agents/tools/triggers/`
- [x] T036 [US4] Person-behind-the-turn permission check; availability rules; secret redacted from history
- [x] T045 [US4] Look up a user by platform id; direct messages only, none on zero or several matches; tests (`core/triggers/access.rs`, `core/storage/user.rs`)
- [x] T037 [US4] Approval for every agent-made trigger (triage model chosen at approval), return to it on change, 20-trigger cap, owner notices; narrow script tools for agent-made monitors
- [x] T038 [US4] Run-reading tools in `DREAM_TOOL_NAMES`
- [x] T039 [US6] `IntegrationKind::triggers()`; deactivate on removal; test push kind and shared pull kind

**Check**: quickstart step 13.

## Phase 5: WebUI (US5)

- [x] T040 [US5] FigCraft mockup of the Triggers page; **review before any code**
- [x] T041 [US5] Components and stories in `design/`
- [x] T042 [US5] Page, forms from kind listings, triage and destination, events with verdicts, runs, monitor health, approval, permission gating in `webui/`

## Phase 6: Finish

- [ ] T043 Run the whole quickstart; `just check`; both storage backends (quickstart done on SQLite
  except 15 and 16, see quickstart.md; `just check` as one command not run)
- [x] T044 Update `CLAUDE.md`; add to #82 only if compatibility code appeared

## Dependencies

Phase 1, then 2. Phase 3 needs 1 only, but ships after 2. Phase 4 needs 2 and 3. Phase 5 needs the
API from 2 and 3; the mockup (T040) can start at any time.
