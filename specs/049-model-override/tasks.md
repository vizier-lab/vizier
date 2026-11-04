# Tasks: Model Override for Chat Requests

**Input**: Design documents from `specs/049-model-override/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/model-override.md`, `quickstart.md`

**Organization**: Tasks are grouped by the three specified user stories. Tests are included because the plan and constitution require authorization, compatibility, WebUI and dummyplug end-to-end verification.

## Phase 1: Setup

**Purpose**: Prepare the workspace and WebUI submodule for the planned cross-repository changes.

- [x] T001 Check branch alignment with `just status` and create/switch the matching `049-model-override` submodule branch with `just checkout 049-model-override` before editing `webui/`; do not edit `design/` because the existing Picker and approved FigCraft placement are being reused.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define a backward-compatible request value and one shared server-side validator needed by all user stories.

- [x] T002 Add a default-absent optional `{ provider, model }` override value to `VizierRequest` in `crates/vizier-core/src/schema/request.rs` and to the REST `ChatRequest` in `crates/vizier-core/src/service/agents.rs`; keep old serialized requests valid.
- [x] T003 [P] Add the matching optional request field to `WebSocketMessage` in `webui/app/api/frames.ts` and the REST chat request type in `webui/app/api/endpoints/chat.ts`, keeping provider and full model ID separate.
- [x] T004 Add Rust serialization tests in `crates/vizier-core/src/schema/request.rs` proving old requests deserialize with no override and a model ID containing slashes round-trips unchanged; add WebUI payload coverage at the request-construction test seam or extract a pure helper in `webui/app/lib/modelOverride.ts` for testing.
- [x] T005 Implement one reusable server-side chat-choice validator in `crates/vizier-core/src/service/catalogue.rs` using provider summaries and `models_for(..., ModelCapability::Chat)`; require exact provider and complete model ID matches and return an explicit refusal instead of substituting the agent model.

**Checkpoint**: Existing clients still omit the override; the value round-trips without splitting model IDs, and backend validation has one shared implementation.

---

## Phase 3: User Story 1 - Choose a model for chat prompts (Priority: P1) 🎯 MVP

**Goal**: A permitted user selects a provider/model and subsequent prompts in the same conversation use it without changing the configured model or an in-flight turn.

**Independent Test**: With a role holding `model_override`, choose a model distinct from the configured model, send a prompt, and verify response stats/usage report the selected pair in the same conversation. A second prompt retains the selection; changing the selection affects only later prompts.

### Implementation for User Story 1

- [x] T006 Add `model_override` to `AVAILABLE_PERMISSIONS` in `crates/vizier-core/src/storage/user.rs` and pin the exact unscoped name with a unit test there; do not add scoped variants.
- [x] T007 Add the `Model override` permission label and a `Chat` group entry for `model_override` in `webui/app/lib/permissions.ts`; update `webui/app/lib/permissions.test.ts` to verify it renders as a named Chat permission rather than raw text under Other.
- [x] T008 [US1] Add a pure conversation preference helper in `webui/app/lib/modelOverride.ts` (or `webui/app/lib/remember.ts`) keyed by user, agent, channel and topic; cover read/write, Agent default, storage failure and separate-topic/user keys in `webui/app/lib/modelOverride.test.ts`.
- [x] T009 [US1] Create `webui/app/screens/chat/ModelOverridePicker.tsx` using the existing design `Picker` and `ProviderLogo`; provide one searchable dropdown, exact `<provider-id>/<full-model-id>` labels, provider icon fallback, loading/error/empty/unavailable states, and an explicit Agent default choice.
- [x] T010 [US1] Load provider summaries and eligible chat models through existing catalogue endpoints in `webui/app/screens/chat/ModelOverridePicker.tsx`; exclude media-only and unusable providers and preserve model IDs without parsing their slashes.
- [x] T011 [US1] Render the selector only when the current user has `model_override` in `webui/app/routes/chat.tsx`, immediately right of `ChatStatusStrip` with flexible space, on the existing row below Send; keep `ChatStatusStrip` styles and markup unchanged.
- [x] T012 [US1] Thread the selected pair through the composer send callback and `webui/app/stores/turns.ts` request construction for subsequent text and audio-chat requests; snapshot it at submission and omit the field for Agent default, commands, side chats, client events, project tool requests and non-WebUI channels.
- [x] T013 [US1] Create a per-turn `VizierModel` from the validated override in `crates/vizier-core/src/agents/agent/mod.rs` after restoring a stored request on resume; use that model consistently for completion rounds, usage scope, context/stats and response stats without mutating `self.model` or affecting non-chat work.
- [ ] T014 [US1] Add core turn tests in `crates/vizier-core/src/agents/agent/mod.rs` proving the selected provider/model supplies completion and response/usage identity, while an ordinary request uses the configured model and concurrent turn model choices do not leak.
- [x] T015 [US1] Add WebUI tests in `webui/app/screens/chat/ModelOverridePicker.test.tsx` and/or `webui/app/lib/modelOverride.test.ts` for permission-gated visibility, one-dropdown labels with slash-containing IDs, optional logos, preference restore and a changed choice applying only to subsequent sends.

**Checkpoint**: A permitted user can select and use a model in an existing conversation; no shared agent model mutation occurs.

---

## Phase 4: User Story 2 - Respect model-override permission (Priority: P1)

**Goal**: Users without permission cannot discover or submit an override; authorized submissions are checked against current permissions on every supported path, and administrators can grant the permission with a clear label.

**Independent Test**: Compare permitted and unpermitted roles. The unpermitted user sees no selector; an override sent directly over topic WebSocket/desktop or REST is rejected before model execution. A permission revoked while a topic remains open is rejected on the next override submission. In Settings → Roles, the permission is named and can be granted to a custom role.

### Implementation for User Story 2

- [x] T016 [US2] Enforce exact `model_override` authorization with `access::require` for supplied overrides in both `crates/vizier-http/src/topic.rs` and `crates/vizier-core/src/service/agents.rs`; preserve existing agent visibility and project-channel checks and return the contract's forbidden error without dispatch.
- [x] T017 [US2] Re-resolve the current principal from `self.services` for each override submitted through an open topic in `crates/vizier-http/src/topic.rs`; reject unsupported request kinds and non-WebUI channels with the contract's unsupported error rather than ignoring the override.
- [ ] T018 [US2] Add HTTP/topic integration tests in `crates/vizier-http/tests/topic.rs` and the relevant REST service tests in `crates/vizier-core/src/service/agents.rs` for granted, denied, revoked-on-open-socket, unsupported-request and unsupported-channel cases; assert denied requests never reach the agent.
- [x] T019 [US2] Verify permission registration flows through the existing role API by adding a service test in `crates/vizier-core/src/service/roles.rs` that `available_permissions` includes `model_override` and role creation accepts it but rejects scoped aliases.
- [x] T020 [US2] Verify role Settings grouping and display with a `groupsFor` test in `webui/app/lib/permissions.test.ts`: `model_override` appears in the Chat group with a human-readable label and can be toggled/saved by the dynamic role editor in `webui/app/routes/settings-role.tsx`.

**Checkpoint**: Role assignment is discoverable, and the server refuses every unauthorized or unsupported override independently of UI visibility.

---

## Phase 5: User Story 3 - Handle unavailable model choices (Priority: P2)

**Goal**: A stale, unavailable or unresolvable explicit selection gives clear feedback and never silently uses another model.

**Independent Test**: Select an eligible model, make it unavailable before submission, and verify the UI marks it unavailable and the server refuses it without a fallback completion. With no eligible models, the selector communicates an empty state and offers no unusable model.

### Implementation for User Story 3

- [x] T021 [US3] Use the shared catalogue validator from T005 before dispatch in `crates/vizier-http/src/topic.rs` and `crates/vizier-core/src/service/agents.rs`; map invalid or stale choices to the contract's unavailable error and never clear the explicit override or fall back.
- [x] T022 [US3] Preserve a saved-but-unavailable choice in `webui/app/screens/chat/ModelOverridePicker.tsx`, explain that it cannot be used, and block submission with it until the user selects a listed model or Agent default; expose distinct catalogue-load-failed and empty-list messages.
- [x] T023 [US3] Add validator tests in `crates/vizier-core/src/service/catalogue.rs` for exact provider/model matching, slashes in IDs, unknown provider/model, media-only provider, disconnected provider and a choice removed from the current list.
- [ ] T024 [US3] Add UI tests in `webui/app/screens/chat/ModelOverridePicker.test.tsx` for an empty eligible list, failed catalogue fetch, stale saved choice, explicit reset to Agent default and refusal to send while stale.

**Checkpoint**: An unavailable explicit choice is surfaced and refused; no failure path falls back to the configured model.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Verify all stories together and run project-required quality gates.

- [x] T025 [P] Update `specs/049-model-override/quickstart.md` with any implementation-specific steps discovered while exercising UI, permissions, stale choices, REST/topic paths and queued/resumed requests.
- [x] T026 Run `cd webui && npm test` and `npm run typecheck`; record results for the permission group, preference/picker and request payload tests.
- [ ] T027 Run `cargo test -p vizier-core` and `cargo test -p vizier-http`, then the CI-appropriate workspace `cargo clippy`/`cargo test` gates; report any unavailable Windows-only or service-dependent checks accurately.
- [ ] T028 Run the real-binary dummyplug end-to-end steps in `specs/049-model-override/quickstart.md`, proving selected model identity, unauthorized refusal, stale-model no-fallback, and unchanged Discord/Telegram request behavior.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Must verify matching WebUI submodule branch before changing submodule files.
- **Foundational (Phase 2)**: Depends on Setup; defines the shared request value and catalogue validator before story implementations.
- **User Story 1 (Phase 3)**: Depends on foundational request types/validator and setup. Delivers the selector and per-turn model path.
- **User Story 2 (Phase 4)**: Depends on US1's request path; completes server authorization, revocation handling, and Settings role presentation. Both P1 stories are required before releasing any override path.
- **User Story 3 (Phase 5)**: Depends on US1 and US2 plus the validator from Phase 2; completes stale/unavailable feedback and fail-closed behavior.
- **Polish (Phase 6)**: Depends on all three stories; performs test/typecheck/lint and dummyplug e2e verification.

### User Story Dependencies

- **US1 (P1)**: Starts after foundational work; model selection and use in a permitted conversation.
- **US2 (P1)**: Requires US1's override field/path to exercise the permission boundary; the server-side authorization tasks are a release blocker, not optional follow-up.
- **US3 (P2)**: Requires the selected-model pipeline and authorization boundary; hardens the available-choice and stale-selection behavior.

### Parallel Opportunities

- After T001, T002 and T003 can proceed in parallel (Rust schema and WebUI request types are separate files); T006 and T007 can also proceed independently once story work begins.
- In US1, T008 and T009 can be developed in parallel after T007; T013/T014 backend turn work can proceed independently of WebUI picker work, while T011/T012 depend on the picker/preferences.
- In US2, T019 (role API test) and T020 (WebUI label/assignment test) can proceed in parallel; T016 and T017 touch separate server files but must agree on the shared contract.
- In US3, T023 backend validator tests and T024 WebUI state tests can run in parallel; implementation T021 and T022 follows the corresponding validation results.
- In Polish, WebUI gates and Rust gates can run in parallel; the dummyplug e2e follows successful builds.

## Parallel Examples

```text
Foundational:
Task: T002 Add backward-compatible Rust request override fields in crates/vizier-core/src/schema/request.rs and service/agents.rs
Task: T003 Add matching WebUI request types in webui/app/api/frames.ts and the chat endpoint type file

User Story 1:
Task: T008 Implement conversation-keyed model preference and tests in webui/app/lib/modelOverride.ts
Task: T009 Build searchable model dropdown and provider-logo rows in webui/app/screens/chat/ModelOverridePicker.tsx
Task: T013 Implement turn-local override model use and stats in crates/vizier-core/src/agents/agent/mod.rs

User Story 2:
Task: T019 Test registered permission and role assignment in crates/vizier-core/src/service/roles.rs
Task: T020 Label/group model_override in the WebUI role permission catalogue in webui/app/lib/permissions.ts

User Story 3:
Task: T023 Test exact current model eligibility in crates/vizier-core/src/service/catalogue.rs
Task: T024 Test stale and empty selector states in webui/app/screens/chat/ModelOverridePicker.test.tsx
```

## Implementation Strategy

### MVP First

Complete Setup and Foundational work, then implement US1 together with the mandatory server authorization in US2 before making the selector usable. Validate a permitted dummyplug-backed prompt and a direct unauthorized submission. Do not ship a UI-only model switch or an unprotected backend override.

### Incremental Delivery

1. Add the default-absent request field, permission registry entry and shared model eligibility validator.
2. Deliver US1 selector and isolated per-turn model execution.
3. Complete US2's current-permission enforcement, Settings role label and denial/revocation tests before release.
4. Deliver US3 stale/unavailable states and no-fallback checks.
5. Run the automated gates and real-binary dummyplug workflow; leave Discord/Telegram behavior unchanged.

## Notes

- Every task uses a checkbox, sequential ID, a `[P]` marker only where independent, and a `[US1]`/`[US2]`/`[US3]` label only in user-story phases.
- The already reviewed FigCraft layout is reused; no design-submodule component is planned.
- No implementation code has been changed by task generation.
