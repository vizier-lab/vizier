# Research: Model Override for Chat Requests

**Feature**: [spec.md](spec.md) | **Branch**: `049-model-override`

This research resolves the plan's technical questions from the current workspace source. No external dependencies or new runtime technologies are needed.

## 1. Per-turn model selection and shared-agent concurrency

**Decision**: Carry an optional `(provider, model)` choice on the user chat request and build a separate `VizierModel` for that turn. Do not mutate `VizierAgent.model` or persist the selection in `AgentConfig`.

**Rationale**: `VizierAgent` is shared while `agent_process` can run turns for distinct sessions concurrently (`crates/vizier-core/src/agents/process.rs`). Ordinary completions, response stats and usage recording currently use `self.model` in `crates/vizier-core/src/agents/agent/mod.rs` and `agents/agent/model/mod.rs`. The existing `VizierModel::new_with_override` (`agents/agent/model/mod.rs`) already routes a provider/model pair through the same `llm::resolve::resolve` path and creates model identity/usage data correctly. A shared mutable swap would let one conversation's choice leak to another concurrent conversation; a per-turn model avoids that class of bug.

**Alternatives considered**: Change the agent's saved config (wrong persistence scope and restarts/rebuilds the agent); mutate a shared model before each prompt (unsafe across concurrent sessions); add a second provider resolution implementation (duplicates resolver behavior).

## 2. Request boundaries, persistence and authorization

**Decision**: Use one default-absent optional model-override value on the shared `VizierRequest`, and accept it on the existing authenticated REST `ChatRequest` as well. Enforce it on both chat entry paths, and explicitly refuse a supplied override on any unsupported topic request kind or channel. The WebSocket/desktop path is `TopicConnection::handle_inbound`; REST chat is `service::agents::chat`. Never treat the WebUI visibility gate as authorization. Re-resolve the principal for an overridden topic request before checking permission so role revocation takes effect on the next submission on an already-open socket.

**Rationale**: `TopicConnection` is shared by WebSocket and embedded desktop IPC (`crates/vizier-http/src/embedded.rs`); the WebUI uses that path. An authenticated REST chat handler is another existing user-chat path and must not silently ignore an explicit override. `VizierRequest` is serialized with durable turns and replaced from stored history on restart in `agents/agent/mod.rs`, so the per-request value naturally survives queuing/resumption. New fields in the request schema follow `#[serde(default)]` compatibility patterns. Services resolve users/roles from storage (`service::Services::resolve`); a topic's opening `Principal` alone can be stale after a permission change.

**Alternatives considered**: Add a one-off REST endpoint for changing conversation selection (not needed; choice belongs to each sent request); store selection server-side (adds schema/storage/migration without a multi-device requirement); rely only on the selector hiding (not a security boundary); use the socket's initial role snapshot indefinitely (does not satisfy revocation while open).

## 3. Permission vocabulary and provisioning

**Decision**: Add only `model_override` to `crates/vizier-core/src/storage/user.rs::AVAILABLE_PERMISSIONS`; check it with the existing `access::require` helper. Do not make `all_agents:model_override` or `owned_agents:model_override` variants.

**Rationale**: `service::roles::available_permissions` and role validation both use this list. Startup reconciliation in `dependencies.rs` adds newly available permissions to the system role without rewriting other roles. `access::require` already supports an unscoped exact permission and system-role bypass. The WebUI session exposes permission strings for the presentation gate; the server remains authoritative.

**Alternatives considered**: Add agent-ownership variants (explicitly contrary to the spec); create a new permission framework or access trait (unnecessary for one direct permission); rely on front-end-only authorization (unsafe).

## 4. Eligible models and exact identifiers

**Decision**: Reuse provider summary and model listing services/APIs. Offer chat-capable providers that have a usable connection (and local providers that can serve their model list), then exact model IDs returned for `ModelCapability::Chat`. Validate the submitted provider/model pair against the current server-side list and resolve it through the normal resolver. Treat model IDs as opaque strings; only provider and model are separate fields.

**Rationale**: `service::catalogue::provider_summaries` reports provider kind, connection status and logo; `models_for` already handles catalogue, custom, local live models, dummyplug and chat-only inference-profile entries. The WebUI API's `chatProviders` currently filters media-only providers and `providerModels` loads models. The model id is kept whole by this structure, preserving embedded slashes. A listing check and the resolver serve different purposes: the list establishes current eligibility, while `llm::resolve` verifies actual provider configuration/credentials and constructs the right dialect/client.

**Alternatives considered**: Hard-code providers/models (stale and incompatible with custom providers); parse a combined display string at the server (ambiguous when model IDs contain `/`); accept any string the resolver can construct (can offer a model absent from the current eligible list); show every provider regardless of connection (gives choices that cannot be used).

## 5. Conversation preference and in-flight behavior

**Decision**: Persist the selector preference client-side with the existing guarded `remember` helpers, keyed by user ID, agent ID, channel ID and topic ID. Capture the selected value into each request when sent. Provide an Agent default choice to clear the override. If a stored model is no longer listed, keep it identifiable as unavailable, show feedback, and block sending with it until changed; do not silently clear or substitute it.

**Rationale**: `chat.tsx` owns the current route IDs and identity; topic state in `stores/turns.ts` is intentionally cleared on close. `lib/remember.ts` handles unavailable/blocked localStorage safely. No cross-device requirement is in the spec; browser-local persistence meets reopening the same conversation in this browser without creating server-side preference storage. A request snapshots the choice, so later selector changes affect only later sends, while queued and resumed turns keep their submitted choice.

**Alternatives considered**: Keep selection only in component state (lost on reopen); key by agent only (leaks between conversations); server-side conversation setting (requires storage/schema/API/migration and sync semantics not required); change a running turn when the dropdown moves (violates FR-006).

## 6. Dropdown composition and status strip

**Decision**: Use one searchable `@vizier/design` `Picker` with existing provider-logo support, positioned in the already reviewed FigCraft location to the right of the existing status strip. Wrap the strip and selector in a same-row container with flexible space; do not modify the strip itself or create separate provider/model controls.

**Rationale**: `chat.tsx` renders `ChatStatusStrip` directly below the composer. The design Picker is searchable, keyboard accessible, virtualized for long lists, and supports custom rendered rows; `ProviderLogo` has optional-logo fallback behavior. The saved FigCraft preview captures the exact placement decision and the user's approval.

**Alternatives considered**: Separate provider and model selectors (contradicts the spec); hand-written native select (poorly scales and cannot show provider icons consistently); changing the status strip or adding a new control row (not the approved layout).

## 7. Error handling and channel scope

**Decision**: Reject unauthorized overrides and unavailable/malformed choices before dispatch where possible; if provider credentials or model construction fail at turn execution, return the existing classified error response for that turn. Never retry using the agent's default model. Keep Discord/Telegram request creation unchanged and never add override state or UI to those readers.

**Rationale**: The existing WebSocket path sends client error frames before turns for refused input; core/provider failures are represented by existing `ServiceError` and response `ErrorKind` mechanisms. The feature's no-silent-fallback requirement applies both to stale list state and to a provider that fails between validation and use.

**Alternatives considered**: Fall back to the agent's configured model on failure (explicitly forbidden); add model choice to every `VizierRequest` producer including channels (widens release scope and changes channel behavior); invent new error kinds where existing error envelopes/classification can express the failure.
