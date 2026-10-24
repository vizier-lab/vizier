# Tasks: Agent Secrets

**Input**: Design documents from `/specs/027-agent-secrets/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

**Tests**: Included as regression coverage for the security guarantees; the spec did not request a TDD-first order. The `dummyplug` end-to-end checks are in `quickstart.md` and are required by the constitution.

**Organization**: Tasks are grouped by user story. Phase 2 is the shared foundation; each story phase is a testable increment. The WebUI work follows the submodule order `design/` → `webui/` → parent pin bump.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: `[US1]`, `[US2]`, `[US3]`, `[US4]`
- Every task names the exact file(s) to touch.

## Path Conventions

Single Rust project at the repository root, with the bundled React app in `webui/` and the design library in `design/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Declare the new modules and establish a clean baseline. This feature adds no project, package or dependency.

- [X] T001 Create the module skeletons in `src/main.rs`, `src/access.rs`, `src/secrets/mod.rs`, `src/schema/secret.rs`, `src/storage/secret.rs`, `src/storage/sqlite/secret.rs`: declare `mod access;`, `mod secrets;`, `pub mod secret;` in `src/schema/mod.rs`, `pub mod secret;` in `src/storage/mod.rs`, and `mod secret;` in `src/storage/sqlite/mod.rs`; give each new file a one-line module doc comment.
- [X] T002 [P] Run the baseline checks before editing behavior in `src/` and `webui/`: `cargo test`, `cargo clippy`, `cd webui && npm run typecheck`; record any pre-existing failure in the implementation notes so it is not mistaken for a regression.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Storage, validation, access and redaction primitives every story uses.

**⚠️ CRITICAL**: No user story work can begin until this phase compiles and its tests pass.

- [X] T003 [P] Add `SecretAction` (`Created`/`Replaced`/`Deleted`, snake-case serde) and `AgentSecretEvent` (id, agent_id, name, action, `Requester` actor, RFC 3339 `at`) in `src/schema/secret.rs`; re-export both from `src/schema/mod.rs`.
- [X] T004 Add `SecretOutcome`, `SecretRecord` (name, `masked`, created_at, updated_at, updated_by; **no value field**), `SecretWrite`, and the `SecretStorage` trait (`set_agent_secret`, `get_agent_secret`, `list_agent_secrets`, `delete_agent_secret`, `list_agent_secret_events`) in `src/storage/secret.rs`; use the exact signatures in `data-model.md` §4–5.
- [X] T005 Add `SecretStorage` to the `VizierStorageProvider` supertrait and forward every method from `VizierStorage` to `self.0` in `src/storage/mod.rs`.
- [X] T006 Add `init_secret_schema(conn)` in `src/storage/sqlite/mod.rs`: create `agent_secret` with primary key `(agent_id, name)` and `agent_secret_event` with the `agent_secret_event(agent_id, at DESC)` index, matching `data-model.md` §1–2; call it from `init_schema` after `init_turn_schema`.
- [X] T007 Implement `SecretStorage for SqliteStorage` in `src/storage/sqlite/secret.rs` (after T014): a set is one transaction that upserts the value, preserves `created_at` on replace, updates `updated_at`/`updated_by`, and inserts `created` or `replaced`; get returns only the value; list masks with `utils::mask_secret` and sorts by name; delete is one transaction that inserts a `deleted` event only when a row existed and returns whether it did; events are newest first with the given limit. Store `Requester` as JSON and timestamps as RFC 3339. Add tests for create, replace, list/mask, get, delete, and event actions.
- [X] T008 Delete both `agent_secret` and `agent_secret_event` for the agent in `SqliteStorage::delete_agent` in `src/storage/sqlite/agent.rs`, alongside the existing `scratch_file`/`client_tool_call` cleanup.
- [X] T009 [P] Add `MAX_NAME_BYTES = 128`, `MIN_VALUE_BYTES = 8`, `MAX_VALUE_BYTES = 65_536`, `validate_name` (1–128 bytes of ASCII `[A-Za-z0-9._-]`, case-sensitive) and `validate_value` (8–65,536 UTF-8 bytes, no trimming) with unit tests in `src/secrets/mod.rs`.
- [X] T010 [P] Implement `SecretRedactor` in `src/secrets/redact.rs`, declare `pub mod redact;` and re-export `SecretRedactor` in `src/secrets/mod.rs`: `Mutex<Vec<String>>`, `arm` (deduplicate, ignore values shorter than 8 bytes, cap at 64), `is_armed`, `text` (exact replacement with `[redacted]`, longest value first), `json` (recursively scrub strings including map keys), and `report` (scrub `result`, `stdout`, `error.message`, `error.traceback`, and every nested `arguments`/`error` in `ExecutionReport`); add tests for the no-op unarmed case, exact replacement, JSON values/keys, and a report with a nested call.
- [X] T011 Move `Caller` and the ownership predicate out of `src/integrations/access.rs` into `src/access.rs`: `Caller { user_id, is_system, permissions }`, `From<&AuthenticatedUser>`, `has`, `owns`, and `manages(agent, all_permission, owned_permission)` (system bypass, all permission, or owned permission plus ownership). Re-export `Caller` from `src/integrations/access.rs` and keep `can_manage_on_agent` behavior identical; move its tests.
- [X] T012 Add `MANAGE_ALL = "all_agents:secret"`, `MANAGE_OWNED = "owned_agents:secret"`, and `can_manage(caller, agent)` in `src/secrets/access.rs`, declare `pub mod access;` in `src/secrets/mod.rs`, and add unit tests for system, all-agents, owned/not-owned and neither-permission callers.
- [X] T013 [P] Add exactly `all_agents:secret` and `owned_agents:secret` to `AVAILABLE_PERMISSIONS` in `src/storage/user.rs`; do not change the default permissions of any non-system role.
- [X] T014 [P] Move `MASK` and `mask_secret` from `src/integrations/mod.rs` to `src/utils/mod.rs`, keep the existing masking rule (`••••` plus the last four characters only when longer than eight characters), re-export from `src/integrations/mod.rs` if needed, update `src/integrations/kind.rs`, and move/update the tests.
- [X] T015 Add `SecretError` (`InvalidName`, `InvalidValue`, `NotFound`, `Storage`) and the `set`, `get`, `delete`, `list`, `events` facade in `src/secrets/service.rs`, declare `pub mod service;` in `src/secrets/mod.rs`; validate name and value before touching storage, validate the name on get/delete, map a missing get/delete to `NotFound`, clamp the event limit to 1–100, and add tests for validation, not-found and storage mapping.
- [X] T016 Add `pub secret_redactor: Arc<SecretRedactor>` to `ToolContext` in `src/agents/tools/mod.rs` and update every `ToolContext` literal (`src/agents/agent/mod.rs`, `src/agents/process.rs`, `src/agents/background/mod.rs`, and the test constructor) to start with `Arc::new(SecretRedactor::default())`; the ordinary chat turn must create one redactor and pass the same `Arc` to `execute_python` through its cloned context.
- [X] T017 Add `ToolCategory::Secrets` as the 16th variant after `Integrations` in `src/agents/tools/mod.rs`, extend `ALL` to 16, and give it name `secrets` and the description from `contracts/agent-tools.md`; update any exhaustive category tests.

**Checkpoint**: `cargo test` covers storage, validation and access; `cargo clippy` is clean. User stories can start.

---

## Phase 3: User Story 1 - An agent saves a credential and uses it later (Priority: P1) 🎯 MVP

**Goal**: `set_secret` and `get_secret` are available to every ordinary, scheduled and background turn; `get_secret` returns the value into the script while the report is redacted; dream turns never see either tool.

**Independent Test**: On `dummyplug`, set `AGENT_TOKEN`, then in a later turn retrieve it, print it, and send it to a local echo endpoint. The echo log proves the script received the exact value; the report/history show `[redacted]`. Restart and repeat. Replace the value and confirm the next get sees the new one.

### Implementation

- [X] T018 [US1] Implement `SetSecret` in `src/agents/tools/secrets.rs`: input `{name, value}`, validate through `secrets::service::set` with the captured `agent_id` and `Requester::Agent`, arm `ctx.secret_redactor` only after a successful write, and return `{status:"created"|"replaced", name}` or `{status:"refused", reason, name, message}` as data (never `Err` for invalid input).
- [X] T019 [US1] Implement `GetSecret` in `src/agents/tools/secrets.rs`: input `{name}`, validate/get through `secrets::service`, arm `ctx.secret_redactor` before returning the real `{status:"found", name, value}`, and return `{status:"not_found", name}` or `{status:"refused", reason, name, message}` as data; the tool description must say the value is script-only and must not be printed.
- [X] T020 [US1] Register both tools on `default_toolset` in `BaseTools::new()` in `src/agents/tools/mod.rs` (add `mod secrets;` and import `SetSecret`/`GetSecret`); add them to the `every_tool_is_in_its_category` table under `Secrets`; add the `secrets` → `secrets_mcp` reserved-category assertion.
- [X] T021 [US1] Add tests in `src/agents/tools/mod.rs` asserting neither `set_secret` nor `get_secret` is in `VizierTools::DREAM_TOOL_NAMES` and that the dream tool list does not include them; add a category-list test that `secrets` is the last built-in category.
- [X] T022 [P] [US1] In `src/agents/tools/python/bridge.rs`, redact the nested call `arguments` before the `tracing::info!` line and before constructing `ToolInvocationRecord`, and redact the error string before storing it or returning `BridgeError::Tool`; keep the value returned to the script unchanged.
- [X] T023 [P] [US1] In `src/agents/tools/python/mod.rs`, call `ctx.secret_redactor.report(&mut report)` after `bridge.take_parts()` and before the `python script finished`/`failed` log and before `serde_json::to_value(report)`; this must cover `result`, `stdout`, traceback and nested calls.
- [X] T024 [US1] Make telemetry redactor-aware in `src/agents/tools/mod.rs`: create the span without recording arguments for `ToolRouter::call`, record redacted arguments after dispatch, pass `Some(&ctx.secret_redactor)` to `record_tool_result`, and keep the direct `traced_tool_call` path (`execute_python`'s own code is the excluded set input) working; update every `execute_tool_span`/`record_tool_result` caller, including `dream_call`.
- [X] T025 [P] [US1] Stop `DebugHook` in `src/agents/hook/debug.rs` from logging raw tool arguments and tool-response payloads: log the function name and response variant/tool name only. Leave request/response logging unchanged.
- [X] T026 [US1] Add tool tests in `src/agents/tools/secrets.rs` using an in-memory `SqliteStorage` with `init_secret_schema`: create then get, replace returns `replaced`, invalid name/value returns `refused` and writes nothing, missing returns `not_found`, and a value with spaces/newlines round-trips exactly.
- [X] T027 [US1] Walk steps 0–2 of `quickstart.md` on a running `dummyplug` agent: `set_secret`, get via the local echo endpoint, confirm the echo log has the value while the report/history shows `[redacted]`, and confirm a missing name returns `not_found`.

**Checkpoint**: The MVP works without the WebUI: an agent can store a secret, use it from a script, and never shows the stored value to the model or transcript.

---

## Phase 4: User Story 2 - A person manages an agent's secrets in the WebUI (Priority: P1)

**Goal**: A secret manager can list, create, replace and delete secrets in the WebUI; values are always masked; both permissions appear in role management and are enforced server-side.

**Independent Test**: As a holder of `all_agents:secret`, open an agent's Secrets page, create a secret, reload and confirm it is listed masked, replace it, delete it, and confirm persistence. Repeat as a user with neither permission and confirm every read/write is refused.

### Implementation

- [X] T028 [US2] Add the DTOs (`SecretItem`, `SecretList`, `SecretWriteResponse`, `SetSecretRequest`) and `list`/`put`/`delete` handlers in `src/channels/http/api/v1/agents/secrets.rs`; load the agent and return `404` when absent, check `secrets::access::can_manage` and return `403` with both required permissions, map validation to `422`, use `200`/`201` for replace/create, return `404` for a missing delete, and resolve `updated_by_name` through the existing `requester_name`/`usernames`/`agents_by_id` helpers. No response may contain a value.
- [X] T029 [US2] Add `mod secrets;` and `.nest("/{agent_id}/secrets", secrets::secrets())` in `src/channels/http/api/v1/agents/mod.rs`; register the router's `GET` list, `PUT /{name}`, and `DELETE /{name}` routes.
- [X] T030 [US2] Add the three handlers and their DTOs to the `paths(...)`/`components(schemas(...))` lists in `src/channels/http/mod.rs`; keep every response schema free of a raw value.
- [X] T031 [P] [US2] Add `canManageSecrets(agent)` to `webui/app/lib/access.ts` with the same system/all/owned rule the server uses, and add `webui/app/lib/access.test.ts` cases for no user, all-agents, owned-and-owned, owned-and-not-owned, and neither permission.
- [X] T032 [P] [US2] Add the `Secrets` group to `PERMISSION_GROUPS` in `webui/app/lib/permissions.ts` after Agents, with labels `Manage every agent's secrets` and `Manage secrets of agents they own`; confirm `groupsFor` still filters available permissions.
- [X] T033 [P] [US2] Add `SecretItem`, `createSecret`/`replaceSecret` (the same `PUT`), `deleteSecret`, and `listSecrets` to `webui/app/api/endpoints/secrets.ts`, matching `contracts/http-api.md`; URL-encode the name.
- [X] T034 [US2] Add `secrets` to `SidebarPage`, the Misc group with the `key` icon, and an optional `hiddenPages?: SidebarPage[]` filter in `design/src/components/Sidebar/Sidebar.tsx`; update `Sidebar.stories.tsx` `PAGES` and add a hidden-page story/control.
- [X] T035 [US2] Add `secrets: '/secrets'` to `PAGE_PATH`, hide `secrets` in `Sidebar` unless an agent is selected and `canManageSecrets(agent)` is true, and add `route(':agentId/secrets', 'routes/secrets.tsx')` in `webui/app/layouts/AppShell.tsx` and `webui/app/routes.ts`.
- [X] T036 [US2] Implement `webui/app/routes/secrets.tsx` with `PageFrame`: loading/error/empty states; rows showing name, `SecretField` with the server mask and `onReplace`, last-change relative time/actor, and Delete; a create `Dialog` with `TextField` name (mono) and password value (8–65,536 bytes helper), a replace `Dialog` with locked name, and a `ConfirmDialog` for delete. A `403` must render a clear permission error, not a blank page or a value. Do not add a reveal action.
- [X] T037 [US2] Commit the design submodule change, then the webui submodule change (`npm run typecheck` in `webui/`), then stage the updated `design`/`webui` gitlinks in the parent; do not edit `legacy/`.
- [X] T038 [US2] Walk step 4 of `quickstart.md`: as `all_agents:secret`, create/reload/replace/delete a secret, confirm the mask and actor, verify both role labels, and verify a user with neither permission gets no nav item and a `403` on list/put/delete.

**Checkpoint**: Both P1 stories are independently usable: the agent path (US1) and the people path (US2).

---

## Phase 5: User Story 3 - Rotate or remove a secret without disruption (Priority: P2)

**Goal**: Replacement is immediate for the next retrieval, deletion is confirmed and permanent, and neither restarts the agent nor aborts a turn.

**Independent Test**: Retrieve a secret, replace it from the WebUI while the agent is idle, and confirm the next retrieval returns the new value with no restart; delete it and confirm the next get is `not_found`.

### Implementation

- [X] T039 [US3] Add regression tests in `src/storage/sqlite/secret.rs` for replacement: one row remains, `created_at` is preserved, `updated_at`/`updated_by` change, a second get returns the new value, deleting then setting creates a fresh row, and deleting a missing name returns `false` with no event.
- [X] T040 [P] [US3] Add tests in `src/secrets/service.rs` proving there is no cache: `set` then `get` in the same storage returns the new value; a second `set` changes it immediately; `delete` makes the next `get` `NotFound`; the service never sends an `AgentCommand` or touches the transport.
- [X] T041 [US3] Add handler-level tests/helpers in `src/channels/http/api/v1/agents/secrets.rs` for `201` on create, `200` on replace, `404` on missing delete, and the `created_at`/status mapping; keep the write path free of restart or refresh calls.
- [X] T042 [US3] Finish the confirmation UX in `webui/app/routes/secrets.tsx`: the delete dialog names the secret and says the agent's next get returns `not_found`; the replace dialog says the next retrieval sees the new value and no restart is needed; disable confirm while saving and surface a server error in the dialog.
- [X] T043 [US3] Walk step 3 of `quickstart.md`: replace from the WebUI and from a script, confirm the next get and echo log show the new value with no restart/abort, delete after confirmation, confirm `not_found`, and set the same name again as a fresh secret.

**Checkpoint**: Rotation and removal are safe and immediate; deletion is not a hidden one-click action.

---

## Phase 6: User Story 4 - Secrets stay private to their agent and accountable (Priority: P2)

**Goal**: Cross-agent access is indistinguishable from not-found; change records show actor/action/name/time but never a value; reports, logs and telemetry are clean.

**Independent Test**: Put the same name on agents A and B with different values; each script reads only its own, and the other agent gets `not_found`. After set/get, inspect the report, stdout, nested arguments, event list and logs for the value and find none.

### Implementation

- [X] T044 [US4] Add the `events` handler in `src/channels/http/api/v1/agents/secrets.rs` with `limit` default 20/max 100, actor name resolution, and an `items` response containing no value/mask/hash; add its `GET /{agent_id}/secrets/events` route and OpenAPI entry in `src/channels/http/api/v1/agents/mod.rs`/`src/channels/http/mod.rs`.
- [X] T045 [US4] Add `listSecretEvents` to `webui/app/api/endpoints/secrets.ts` and a read-only **Recent changes** section to `webui/app/routes/secrets.tsx` showing action, name, actor name and time for the newest 20; show an inline error when it fails, never a value.
- [X] T046 [US4] Add storage tests in `src/storage/sqlite/secret.rs` for cross-agent isolation (same name, different values; A cannot read B), event rows containing no value, and `delete_agent` removing both tables' rows.
- [X] T047 [P] [US4] Add non-disclosure tests in `src/secrets/redact.rs` and `src/agents/tools/mod.rs` for a report's `result`, `stdout`, traceback, nested arguments/error, and an `execute_tool` result; assert the raw value is absent and `[redacted]` is present after arming.
- [X] T048 [P] [US4] Add `src/secrets/access.rs` tests for system bypass, all-agents, owned/not-owned and neither permission, including a revoked permission being absent from the caller's list.
- [X] T049 [P] [US4] Add tool-level isolation tests in `src/agents/tools/secrets.rs`: two captured agent ids with the same name get different values, and a name that exists only on the other agent returns `not_found` with no hint.
- [X] T050 [US4] Walk steps 5–7 of `quickstart.md`: prove both agents' echo requests carry their own value, cross-agent names return `not_found`, events show actors and no value, and `grep` the debug log for a WebUI-only value after a get finds no match.

**Checkpoint**: Isolation, accountability and non-disclosure are verified end to end, not only by unit tests.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, generated artifacts and the full quality gate.

- [X] T051 [P] Write `docs/src/configuration/secrets.md` (purpose, script-only retrieval, limits, redaction limitation, set-input caveat, permissions, rotation/deletion, no encryption/expiry), add it to `docs/src/SUMMARY.md` as 2.13, and update `docs/src/api-integration/rest-api.md` and `docs/src/configuration/agents.md` with the routes/permissions/tool category.
- [X] T052 [P] Update `CLAUDE.md` with the `secrets` category and tools, their non-dream status, the `agent_secret`/`agent_secret_event` tables, the script-only redaction boundary, and the two permission names.
- [X] T053 Regenerate `webui/openapi.json` and `webui/app/api/schema.d.ts` against a running server with `node scripts/openapi.mjs` (or `VIZIER_URL=… node scripts/openapi.mjs`); confirm the four secret routes and their schemas are present.
- [X] T054 Run the full gate for `src/` and `webui/`: `cargo fmt --check`, `cargo test`, `cargo clippy`, `cd webui && npm run typecheck`, and `cd webui && npm test`; fix every failure introduced by this feature.
- [X] T055 Run `quickstart.md` end to end on a fresh `dummyplug` pair (steps 0–10), including restart persistence, rotation, deletion, permissions, isolation, events, log/telemetry scan and agent deletion.
- [X] T056 Do the final security review against `spec.md`: search `src/` and `webui/` for the test value after a set/get, confirm no value field in any DTO, no secret in `AgentConfig`, `secrets` reserved as an MCP category, no dream tool entry, and no migration or #82 entry is needed because the tables are new.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: starts immediately.
- **Foundational (Phase 2)**: depends on T001; blocks all stories. T003→T004→T005 and T003→T006→T014→T007→T008; T009 and T010 can run alongside T003–T008; T011→T012; T013 is independent; T015 needs T004/T007/T009; T016 needs T010; T017 is independent.
- **US1 (Phase 3)**: needs T015/T016/T017. T018→T019 (same file), then T020/T021; T022/T023/T025 can follow once T010/T016 exist; T024 follows T020; T026 after T018–T025; T027 after the code.
- **US2 (Phase 4)**: needs T007/T012/T015 for the server and T034/T035/T036 for the UI; T031/T032/T033 can start as soon as their files exist. T037 after T034–T036; T038 after T037.
- **US3 (Phase 5)**: depends on US1 and US2.
- **US4 (Phase 6)**: T044 depends on US2; T045 on T033/T036/T044; T046–T049 can run in parallel once their files exist; T050 after all.
- **Polish (Phase 7)**: after all desired stories.

### User Story Dependencies

- **US1 (P1)**: independent of US2–US4 and is the MVP.
- **US2 (P1)**: depends on the storage/access foundation; its WebUI page is independently testable with the server routes.
- **US3 (P2)**: depends on US1 + US2 because it exercises both the agent and the page; no new store behavior beyond the read-through design.
- **US4 (P2)**: depends on US1 + US2; its event surface is additive.

### Within Each Story

- Rust models/traits before implementations; storage before service; tools before registration.
- Redaction wiring before tool-level end-to-end checks.
- Design component before the webui page; typecheck before the parent pin bump.
- Tests accompany the increment; the e2e quickstart steps close each story.

---

## Parallel Opportunities

- **Setup**: T001 and T002 are independent.
- **Foundational**: T003/T009/T010/T011/T013/T014 touch different files; after T003, T006 and T009 can run together.
- **US1**: T022 (bridge), T023 (execute_python report) and T025 (debug hook) touch different files and can follow the tool implementation in parallel; T024 must follow T020.
- **US2**: T031, T032 and T033 are independent files; T034 (design) must precede T035/T036.
- **US4**: T046 (storage tests), T047 (redaction tests) and T048 (access tests) are independent files.

### Parallel Example: US2

```text
Task: "Add canManageSecrets to webui/app/lib/access.ts"
Task: "Add the Secrets group to webui/app/lib/permissions.ts"
Task: "Add the secrets API client to webui/app/api/endpoints/secrets.ts"
```

### Parallel Example: US1 redaction

```text
Task: "Redact bridge arguments/errors in src/agents/tools/python/bridge.rs"
Task: "Scrub ExecutionReport in src/agents/tools/python/mod.rs"
Task: "Remove raw payload logging from src/agents/hook/debug.rs"
```

---

## Implementation Strategy

### MVP First (US1 only)

1. Complete Phase 1 and Phase 2.
2. Complete Phase 3 (T018–T027).
3. **STOP and VALIDATE** with quickstart steps 0–2: set, get via echo, redacted report, restart persistence.
4. This is the agent-facing secret store; ship it before the management UI if needed.

### Incremental Delivery

1. Foundation → US1: agent can store and use a secret (MVP).
2. US2: people can list/create/replace/delete in the WebUI with permissions.
3. US3: rotation/deletion guarantees.
4. US4: isolation, events, non-disclosure audit.
5. Polish: docs, generated OpenAPI, full gate, final quickstart.

### Notes

- Tests are regression coverage, not a TDD requirement; write them with the increment they protect.
- `secrets` is a reserved category; `secrets_mcp` is the MCP fallback.
- The set input (the message or script that supplies a value) is outside the non-disclosure guarantee; nested `set_secret` arguments and every later retrieval are inside it.
- No migration or #82 entry is needed: `agent_secret` and `agent_secret_event` are new tables and no legacy name/field is kept.
