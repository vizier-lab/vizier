# Research: Agent Secrets

**Feature**: `027-agent-secrets` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

The spec has no open clarifications. The user chose **script-only retrieval**: `get_secret` puts the value into the running `execute_python` script, but the model, saved history, logs and telemetry never see it. Each item below records a design decision taken while reading the code it touches.

## R1. Where secrets live: two SQLite tables

**Decision**: add `agent_secret` (the current value) and `agent_secret_event` (the change log) to the embedded SQLite database, behind a new `SecretStorage` trait composed into `VizierStorageProvider`, forwarded by `VizierStorage`, and implemented once by `SqliteStorage` in `src/storage/sqlite/secret.rs`. Values are `TEXT`, stored as supplied and protected by host/disk permissions, matching provider credentials.

**Rationale**: every non-memory entity already lives in SQLite (Principle III). A transaction gives "one entry per name" and "write the event with the value" atomically. The default SQLite connection is the only storage backend; there is no second implementation to justify a generic store.

**Alternatives considered**:
- *A memory document or CORE*: searchable by memory tools, visible in the memory UI and indexed into passages. Wrong namespace and wrong disclosure model.
- *`AgentConfig`*: updating it rebuilds the agent and aborts its turns; secrets must be changeable without either (FR-011).
- *Integration settings*: only integrations have settings; a secret is not an integration, and adding a fake kind would expose values through integration forms.
- *OS keyring*: adds a platform-specific runtime dependency and breaks the zero-dependency, cross-compiled single binary (Principles III/IV).
- *Application encryption*: key management is out of scope, and a key stored beside the database adds little. The spec explicitly relies on host protection, as provider credentials do.

## R2. Script-only retrieval, armed by the secret tools

**Decision**: `get_secret` returns the plaintext to the Python bridge. The model never receives it. A per-turn `SecretRedactor` on `ToolContext` starts empty; only a successful `get_secret` or `set_secret` adds a value. Before that, every scrub call returns its input unchanged. After it, the redactor stays armed for the rest of the turn, because later calls in the same script may use the value.

**Rationale**: code mode already lets a script pass a value to another tool without the model reading it. This gives the agent the credential it needs while keeping it out of model context, prompt-injection exfiltration, saved history, logs and the WebUI. Arming on the tools (rather than scanning every turn) is what makes ordinary turns behave exactly as today.

**Alternatives considered**:
- *Return the value to the model*: literal "get", but the model can then quote it into chat/history and an injected prompt can exfiltrate it. Rejected by the user's choice.
- *Never return the value*: the script could not use the secret, so the store would be write-only for the agent too.
- *An environment variable injected into the shell*: not available to the Python sandbox, and it leaks the value into process listings and shell configuration.
- *A separate secret-aware HTTP tool*: special-cases one consumer; the redactor keeps the generic tool surface.

## R3. Redaction mechanism and scope

**Decision**: `SecretRedactor` holds the values handled in this turn and replaces exact occurrences with `[redacted]`. Apply it at every value-bearing boundary introduced or touched by this feature:

1. `RouterBridge` nested-call `arguments` before the log line and `ToolInvocationRecord` are built.
2. Nested-call error strings before logging/recording.
3. `ExecutionReport::result`, `stdout`, `error.message`, `error.traceback`, and each nested `arguments`/`error` before the report is serialized, persisted or streamed.
4. `ToolRouter::call`'s `execute_tool` span arguments and recorded result when content capture is on.
5. `VizierTools::call`'s result recording where a `ToolContext` is available.
6. `DebugHook` stops logging raw nested arguments and tool responses; it logs the function name only.
7. The outer `execute_python` tool call's code and any message that supplied a value are **not** scrubbed: they are the input that set the secret, and the spec excludes them.

The report scrub runs after the sandbox returns and after nested invocations are collected. The script still holds the real value and the bridge returns the real value to the script; only the report copy is changed.

**Rationale**: tool results themselves are not in `ToolInvocationRecord` (only name, arguments, ok/error), so the main leaks are arguments, stdout/result and telemetry. Scrubbing at the boundary prevents a value from reaching history, the WebUI or an exporter even if a tool echoes it.

**Alternatives considered**:
- *Scrub only in the WebUI*: logs, telemetry and saved history still leak; the spec covers all of them.
- *Never record nested arguments/results*: loses useful debugging and the existing report shape, and does not cover stdout/result.
- *A taint-tracking Python type*: monty string values are plain data; a wrapper would have to be supported by the engine and every tool conversion. Exact-string scrubbing is the smallest mechanism that satisfies the stated guarantee.
- *Refuse to let scripts use the value in other tools*: defeats the feature.

**Limit**: exact-string matching cannot catch base64, URL encoding or hashing. The tool description tells the agent to pass the value to another tool rather than print it, and the docs state the limitation.

## R4. Tool names, category, registration

**Decision**: `set_secret` and `get_secret` in `src/agents/tools/secrets.rs`, category `secrets` (`ToolCategory::Secrets`, the 16th category, after `Integrations`), both on `default_toolset` in `BaseTools::new()`. They are not in `VizierTools::DREAM_TOOL_NAMES`.

**Rationale**: `set`/`get` mirror the user's words and the existing `read_core`/`write_core` naming. A category makes them discoverable through `list_tool_functions(category="secrets")`; placing it last keeps existing category lines byte-identical. Always-on matches the spec's "no per-agent switch". Dream exclusion prevents unattended reflection from reading or writing credentials.

**Consequence**: `secrets` becomes a reserved category name. An MCP server keyed exactly `secrets` becomes `secrets_mcp` under the existing `ToolRouter::catalogue()` rules. The commit message calls this out, as with `scratchpad`.

**Alternatives considered**: `secret_set`/`secret_get` (less natural), a list/delete tool (the spec requires neither; the WebUI lists and deletes), a per-agent grant (out of scope).

## R5. Access control: two permissions, managers only

**Decision**: `all_agents:secret` and `owned_agents:secret` in `AVAILABLE_PERMISSIONS`. A caller may manage an agent's secrets when they are the system role, hold `all_agents:secret`, or hold `owned_agents:secret` and own the agent. There is no view-only secret permission. Extract the existing `Caller` plus the "manages this agent" predicate from `integrations/access.rs` into `src/access.rs` and use it from both integrations and secrets.

**Rationale**: the privacy requirement is all-or-nothing; a person who can see the names but not change them is not asked for. The ownership rule and system bypass are exactly the integrations rule with different permission strings, so a second copy would be a defect (Principle II).

**Alternatives considered**: reuse `all_agents:edit`/`owned_agents:edit` (the user named the permissions explicitly), a separate `:view` permission (out of scope), creator-based access (integrations moved away from that; ownership is the model).

## R6. HTTP API shape

**Decision**:

- `GET /api/v1/agents/{agent_id}/secrets` — list metadata + masked values.
- `PUT /api/v1/agents/{agent_id}/secrets/{name}` — body `{ "value": string }`, create or replace.
- `DELETE /api/v1/agents/{agent_id}/secrets/{name}` — delete.
- `GET /api/v1/agents/{agent_id}/secrets/events?limit=20` — change log, newest first.

All require a secret-manager. A missing agent is `404`; a caller without a secret permission is `403`; validation failures are `422`; a missing secret on delete is `404`. Responses use the existing `APIResponse` envelope. A PUT returns `201` when created and `200` when replaced, with the `SecretItem` and a `status` field.

**Rationale**: `PUT` is an idempotent upsert at a name the caller chooses, matching the agent tool's set semantics. No endpoint returns a value, so there is no GET-by-name. Events are a separate list because they are append-only and can be paged independently.

**Alternatives considered**: `POST` create + `PATCH` replace (two calls and a conflict), one `POST /secrets` with the name in the body (less cacheable, and names are URL-safe), returning the value on PUT (violates write-only).

## R7. WebUI placement and permissions

**Decision**: add `secrets` to `SidebarPage` and the Misc group with the existing `key` icon. The page is `/:agentId/secrets` (`webui/app/routes/secrets.tsx`), reachable only when the current person can manage that agent's secrets. Add `hiddenPages?: SidebarPage[]` to the design `Sidebar` and filter items; `AppShell` hides `secrets` unless `canManageSecrets(agent)`. The page uses `PageFrame`, a simple list/table of names with masked values and last-change actor/time, `Dialog` for create/replace, `SecretField` for the saved masked value, and `ConfirmDialog` for delete. Role management's `PERMISSION_GROUPS` gains a Secrets group with readable labels.

**Rationale**: the page is a security surface, so it belongs under Misc rather than inside Config or Integrations. Existing design components already cover masking, dialogs and confirmation; adding only `hiddenPages` avoids a new component. Hiding is UX, not protection: every server route checks again.

**Alternatives considered**: a tab in Config (crowded and easy to miss), a section in Integrations (secrets are not integrations), always showing the nav item with a 403 page (violates "not offered"), a new custom Secrets component in `design/` (unnecessary).

## R8. Masking

**Decision**: move `mask_secret` from `src/integrations/mod.rs` to `src/utils/mod.rs` and use it from both integrations and secrets. It returns `••••` plus the last four characters for values longer than eight characters, otherwise only `••••`. `SecretStorage` computes the mask, so list/set DTOs never carry a raw value.

**Rationale**: one masking rule for every secret surface, no duplicate. Computing it in storage creates a boundary where values cannot accidentally be serialized.

**Alternatives considered**: a second copy in `secrets` (duplicate), no last-four hint (loses the ability to distinguish two values), returning the length (leaks information).

## R9. Change events

**Decision**: one `agent_secret_event` row per successful create/replace/delete, with a UUID, agent id, name, `SecretAction` (`created`/`replaced`/`deleted`), actor as `Requester` JSON, and RFC 3339 timestamp. Insertion is in the same transaction as the value change. The list is newest first, default limit 20, max 100. Events are visible only to secret managers.

**Rationale**: FR-019/FR-021. `Requester` is the established person-or-agent actor type; `Requester::Agent(agent_id)` records the agent's own changes, and HTTP resolves usernames/agent names for display. Storing no value makes the event table safe to inspect and export.

**Alternatives considered**: a free-form actor string (loses the person/agent distinction), reusing `integration_event` (different entity, and its fields assume an integration), a log file (not queryable or durable with the agent).

## R10. Lifecycle

**Decision**: `set` is an `INSERT ... ON CONFLICT(agent_id, name) DO UPDATE` inside a transaction; `created_at` survives a replace, `updated_at` and `updated_by` change. `delete` removes the row and writes an event only when a row existed. `SqliteStorage::delete_agent` deletes both tables for that agent. Agent updates, restarts and vizier restarts need no hook: the next read hits the current row. No startup migration or backfill is required.

**Rationale**: storage is the source of truth and every get goes to SQLite, so a replacement is visible to the next script immediately with no refresh or restart (FR-011). There is no in-memory cache to invalidate. New tables need no migration.

**Alternatives considered**: caching values on the agent instance (stale after WebUI rotation, and it keeps secrets in process memory longer), a startup sweep (no incomplete states exist), a notification/refresh (unnecessary).

## R11. Validation and limits

**Decision**: one validator in `src/secrets/mod.rs`, called by `secrets::service` for both tools and HTTP:

- Name: 1–128 bytes, ASCII `[A-Za-z0-9._-]`, case-sensitive, no other rule.
- Value: 8–65,536 bytes of UTF-8, preserved exactly (leading/trailing whitespace and line breaks included).

Refusals explain the rule and are data results (tools) or `422` (HTTP); nothing is written.

**Rationale**: the name rule is URL-safe for the `PUT`/`DELETE` path. The 8-byte minimum is the smallest length at which exact substring replacement is unlikely to corrupt ordinary report text. A single validator keeps the two surfaces identical.

**Alternatives considered**: 1-byte minimum (redaction would replace single letters everywhere), regex crate (a hand-written byte loop is enough), trimming values (breaks tokens with spaces or line breaks), case-insensitive names (surprising and collision-prone).

## R12. Set input is outside the non-disclosure guarantee

**Decision**: document and test that a value supplied to `set_secret` exists in the script or message that supplied it. The set tool's nested arguments are redacted, and all later retrievals are protected, but the outer `execute_python` code (the "supplying script") and the person's chat message are not rewritten.

**Rationale**: the model or person must know the value to set it. Retroactively rewriting the turn's input would be a history mutation, would not remove it from the model's context, and could corrupt the user's message. Being explicit is more honest than promising an impossible guarantee.

**Alternatives considered**: a secret value input that never passes through the model (a special client tool or form) — out of scope, and the WebUI form already avoids the conversation; history rewrite — rejected.

## R13. Testing strategy

**Decision**:

- `cargo test`: storage CRUD/replace/isolation/events/agent-delete in `src/storage/sqlite/secret.rs`; validator/access tests; redactor tests (no-op when empty, exact replacement, JSON/report scrub); tool result shapes; category table and dream exclusion.
- HTTP handler behavior is covered by the dummyplug/quickstart checks plus any existing test harness patterns; the project has no general handler test suite.
- `cargo clippy` and `cd webui && npm run typecheck`.
- `quickstart.md` dummyplug steps: set, get via a local echo endpoint, report redaction, restart, replace, delete, permissions, cross-agent isolation, events, log scan, and agent deletion.
- Dream exclusion is a `cargo test` assertion that neither name is in `DREAM_TOOL_NAMES`; a live-provider dream step is optional and marked as such.

**Rationale**: the security guarantees are mostly in pure functions and storage transactions, which unit tests cover cheaply; the end-to-end path needs a real agent loop and is scripted for dummyplug per the constitution.
