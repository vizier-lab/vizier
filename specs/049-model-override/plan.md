# Implementation Plan: Model Override for Chat Requests

**Branch**: `049-model-override` | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/049-model-override/spec.md`

## Summary

Add a permission-gated, single model selector to the existing chat composer/status-strip row. A choice is a provider ID plus the complete model ID; the WebUI keeps it per user and conversation and captures it into each newly sent prompt. The server authorizes every override, verifies the provider/model is currently an eligible chat choice, and creates an isolated model for that turn using the existing resolver. It never mutates the agent's shared model or falls back when an explicit choice cannot be used. The implementation uses the existing provider catalogue, status-strip and searchable picker patterns; Discord and Telegram request construction remains unchanged.

## Technical Context

**Language/Version**: Rust 1.99.1, edition 2024; WebUI TypeScript/React 19 (workspace and WebUI package manifests).

**Primary Dependencies**: Existing `vizier-core` provider catalogue/resolver and `VizierModel`; existing `vizier-http` topic/service paths; WebUI `@vizier/design` `Picker`, `ProviderLogo`, and current catalogue API. No dependency additions planned.

**Storage**: No server schema or migration. The selected preference is client-side localStorage, keyed by signed-in user, agent, channel and topic. Each request itself carries the selected provider/model and is already journaled with durable turns, so queued or resumed turns retain the choice they were sent with.

**Testing**: Rust unit/service/HTTP tests for permission registration, authorization, model eligibility, request serialization and turn-local model use; WebUI unit tests for preference keys, selection/availability and request construction; `cd webui && npm run typecheck`; `cargo test` and `cargo clippy` for touched crates/workspace as CI requires. End-to-end: `quickstart.md` exercises permitted and denied requests with an offline dummyplug agent and verifies response stats identify the selected model.

**Target Platform**: WebUI in supported browsers and the embedded desktop WebUI; server on supported platforms. Discord/Telegram stay on their normal configured model.

**Project Type**: Rust multi-crate service/agent engine with bundled React WebUI.

**Performance Goals**: No additional provider-list request on every send; fetch catalogue/model choices when the selector is made available and validate the chosen model when a request is accepted. Model construction occurs once per overridden turn, not once per completion round.

**Constraints**: Preserve the status-strip's appearance and behavior; exactly one dropdown to its right with flexible space between. Keep provider and full model ID distinct on the wire; do not split model IDs on `/`. Enforce the unscoped `model_override` permission server-side and use current permissions per submission. Explicit invalid/unavailable overrides fail closed. No Discord/Telegram model field or behavior change. No schema migration, external service, or dependency.

**Scale/Scope**: One feature spanning `vizier-core`, `vizier-http`, WebUI submodule and feature documentation; no new crate or persistent server entity. Preference persistence is per browser/profile, not synchronized between devices.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default — PASS.** Reuse the current catalogue APIs, access helper, model resolver, searchable Picker and local preference helper; no new dependency, service, configuration or database state. A separate turn-local model is needed because one agent serves concurrent sessions and the agent's configured model must not be mutated.
- **II. Trait-Based Extensibility — PASS.** No provider-kind branching will be added to model dispatch. Provider/model resolution remains through `llm::resolve`; this is a selection value, not a new provider variant.
- **III. Self-Contained Default — PASS.** The feature adds no required network service; existing catalogue behavior and provider configuration remain authoritative. Users without the permission retain the current configured model path.
- **IV. Portability — PASS.** Browser preference storage uses the existing guarded localStorage helpers; Rust behavior uses existing cross-platform services and storage abstractions. No OS-specific code.
- **V. Unified Errors & Observability — PASS.** Failures use the existing `ServiceError`/HTTP error envelope or WebSocket client-error frame; no ad hoc fallback. Existing tracing and usage recording are preserved.
- **Workflow gates — PASS, subject to verification.** Run the touched Rust crate tests and clippy plus WebUI typecheck and feature tests; perform the dummyplug end-to-end path required for agent-observable behavior. No design-submodule change is anticipated; use the already approved FigCraft placement preview and preserve the drawn status-strip arrangement.

## Phase 0: Research

Findings and decisions are recorded in [research.md](research.md). The main architectural constraint is that `VizierAgent` is shared across concurrent sessions; model override must therefore be attached to an individual request/turn and cannot replace `self.model`. `VizierModel::new_with_override` and the common resolver already provide the required construction path. WebSocket/desktop topic requests share `TopicConnection`; HTTP REST chat uses the core service, so both authenticated user-chat entry points must enforce the same rule. The per-turn value remains serialized on durable request records for restart resumption.

## Phase 1: Design & Contracts

The entities, validation and lifecycle are in [data-model.md](data-model.md). The wire, authorization, catalogue eligibility and failure contract is in [contracts/model-override.md](contracts/model-override.md). Manual build/e2e checks are in [quickstart.md](quickstart.md).

### Intended implementation sequence

1. Register the single `model_override` permission in the available-permission list (system role receives newly registered permissions through existing startup reconciliation); add focused access/permission tests. Do not define scoped aliases.
2. Add a default-absent, serde-compatible model override value to the shared user chat request and the REST chat request. Accept it only for user `chat`/`audio_chat` topic requests and REST chat; reject it on other topic request kinds/channels. Add WebSocket/HTTP error behavior and test that ordinary requests, old clients, and channel-created requests continue to omit the value.
3. In core request handling, require current `model_override` authorization whenever an override is supplied, and reject overrides on unsupported request kinds or channels. Re-resolve a topic connection's principal for each overridden submission so permission revocation is effective without reconnecting. Reuse agent-view/project-channel authorization independently.
4. Validate provider/model against the currently available chat choices from `service::catalogue::provider_summaries` and `models_for(..., ModelCapability::Chat)`, allowing configured connected providers and working local providers. Require an exact provider ID and exact full model ID match; use the resolver as the final credential/configuration check. Return a typed refusal if unavailable, never continue with the default.
5. Create a model per overridden turn after resolving any stored request on resume; pass it only through that turn's completion, stats and usage scope. Leave the agent's shared model untouched for ordinary turns, health, side chats, dreams, handovers and channel-generated work. Cover concurrent requests and resumed turns.
6. Add the WebUI preference helper and one permission-gated searchable single dropdown in the approved position. Load eligible provider/model choices from existing APIs, display `<provider-id>/<full-model-id>` and provider logo when present, and retain text identification when absent. Capture the current value at submit (including audio chat); invalid persisted choices show an unavailable state and block that send until the user changes to another choice or Agent default.
7. Add focused tests and run the checks in quickstart; inspect the final diff for protocol scope, legacy request compatibility, and unchanged Discord/Telegram behavior.

### Project Structure

#### Documentation (this feature)

```text
specs/049-model-override/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/model-override.md
├── spec.md
└── checklists/requirements.md
```

#### Source code (anticipated)

```text
crates/vizier-core/src/
├── storage/user.rs                 # register the permission
├── schema/request.rs               # optional provider/model on a user request
├── service/agents.rs               # REST chat authorization and dispatch
├── service/catalogue.rs            # reuse current chat-choice list; add a validator only if a shared home is needed
├── access.rs                       # reuse require; no scoped permission logic
├── agents/agent/mod.rs             # use a turn-local model for eligible chat turns
└── agents/process.rs               # ensure model override survives queue/resume without affecting channel requests

crates/vizier-http/src/
└── topic.rs                        # authorize and reject invalid WS/desktop topic overrides before dispatch

webui/app/
├── api/endpoints/catalogue.ts      # reuse typed provider/model listing
├── lib/remember.ts or lib/modelOverride.ts  # user+agent+channel+topic preference
├── routes/chat.tsx                 # approved status-strip row placement and permission gate
├── screens/chat/ModelOverridePicker.tsx    # one dropdown using existing design components
├── stores/turns.ts                 # capture selected model in each chat/audio request
└── relevant tests                  # pure preference and payload/visibility behavior
```

Exact UI file placement may follow the current chat screen patterns, but the layout contract is fixed: leave `ChatStatusStrip` visuals unchanged, place one selector in flexible space to its right on the same row below Send. The design was already reviewed in the saved FigCraft preview; no new composition is authorized by this plan.

**Structure Decision**: Implement the per-turn model behavior in `vizier-core` (where requests are executed and model resolution/usage live), enforce user authorization at the shared core service/topic boundaries, adapt WebSocket/desktop frames in `vizier-http`, and keep preference and visual state in the WebUI. The server remains stateless with respect to the user's selector preference; the request carries the effective choice and the WebUI keys its saved preference by user and conversation.

## Constitution Check (post-design)

**Re-evaluation: PASS.** The design adds no dependencies, database changes, new service, or provider dispatch branch. The additional request field is optional/defaulted for backward compatibility. A model object is isolated per overridden turn to preserve concurrency and avoid changing the configured agent. The WebUI state is local to the browser and has no effect on the server's no-override path. The permission check is server-side and the same exact permission name is registered for role management. Existing unified errors and usage telemetry remain in use. No constitution exception or complexity-tracking entry is required.

## Complexity Tracking

No constitution violations; no entries.
