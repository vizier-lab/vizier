# Implementation Plan: Dummyplug Test Provider

**Branch**: `008-dummyplug-provider` | **Date**: 2026-09-27 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/008-dummyplug-provider/spec.md`

## Summary

Add an offline chat provider, `dummyplug`, for testing the full agent pipeline without a live model. It adds one `VizierModelTrait` implementation, `DummyplugModel`, which reads the latest user message and the agent's tool definitions, then does one of the following:
- For `tools`, it lists every tool.
- For an exact tool name, it returns a sample `{"tool", "arguments"}` JSON built from the tool's JSON Schema.
- For such a JSON, it emits a real `ToolCall`, so the **existing** agent loop runs the tool through hooks, history, and the UI. On the follow-up completion it echoes the tool result.
- For anything else, it returns random lorem ipsum.

Chat behavior lives in one new provider file. JSON Schema traversal (resolving `$ref`s, listing properties, generating sample values) goes in a new reusable `src/utils/json_schema.rs`. Frontmatter stripping reuses a new shared `utils::markdown::parse_markdown_str`. That function also replaces the two existing copies of the same parsing loop, in `read_markdown` and `memory_bundle::parse_markdown_bytes` (research R4a). The rest of the change is one registration arm per exhaustive `ProviderVariant` match, plus WebUI and doc entries. It needs no new dependencies, storage schema changes, or edits to the agent loop.

## Technical Context

**Language/Version**: Rust 2024 edition (existing crate); TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: `rig-core` 0.38.2 (message and tool types only, no client), `serde_json`, `rand` 0.10, `uuid`. All are already in `Cargo.toml`.

**Storage**: N/A. There are no schema changes, and `provider: "dummyplug"` is stored in the existing `AgentConfig.provider` field.

**Testing**: `cargo test` unit tests in `src/agents/agent/model/dummyplug.rs` (`#[cfg(test)]`), plus manual verification with [quickstart.md](./quickstart.md)

**Target Platform**: Same as Vizier: Linux/macOS/Windows, including the `musl` cross targets. It is pure Rust with no platform code.

**Project Type**: Single Rust binary with an embedded WebUI

**Performance Goals**: Replies in under 1 s, excluding tool execution time (SC-004). In practice they take microseconds, because there's no I/O.

**Constraints**: Must work fully offline with no credentials (FR-002). Must not change behavior for other providers (FR-012).

**Scale/Scope**:
- 1 small refactor: `utils/markdown.rs` gains `parse_markdown_str`, and `read_markdown` plus `storage/memory_bundle.rs::parse_markdown_bytes` delegate to it
- 2 new Rust files: `utils/json_schema.rs` (~150 lines, including tests) and `agents/agent/model/dummyplug.rs` (~300 lines, including tests)
- About 8 small edits across `config/provider.rs`, `agents/agent/model/mod.rs`, `channels/http/api/v1/providers/mod.rs`, `webui/app/interfaces/types.ts`, `webui/app/components/AgentForm.tsx`, `webui/app/routes/agent-settings.tsx`, and `docs/src/configuration/providers.md`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment | Status |
|-----------|------------|--------|
| **I. Lean by Default** | No new crates: lorem ipsum and schema sampling are hand-rolled over existing `rand`/`serde_json` (research R7, R9). `utils/json_schema.rs` has only one caller today. It is split out by an explicit product decision to reuse it (R7), and it is plain functions with no trait or generics, so it adds a file boundary and no abstraction. No new trait, config flag, storage variant, or Cargo feature. No `ProviderEntryConfig`/YAML entry, because there's nothing to configure (R2). | ✅ Pass |
| **II. DRY via Trait-Based Extensibility** | The provider is a new implementation of the existing `VizierModelTrait`. Registration is one arm in `VizierModel::new`/`new_with_override`, which are the module's provider constructors, the same way every other provider registers. The other added arms (`resolve_provider`, `upsert_provider`) exist only because those matches are compiler-enforced exhaustive over `ProviderVariant`; each arm is one line. The agent loop, hooks, and tool dispatch are **not** modified, and no provider-type branching is added to shared code (R1 rejected the hook/loop interception alternative for this reason). Frontmatter parsing would have been a third copy of the same loop, so it is merged into `utils::markdown::parse_markdown_str` and both existing copies delegate to it (R4a). | ✅ Pass |
| **III. Self-Contained, Zero-Dependency Runtime** | Makes no network calls and needs no external service or credentials. It adds a way to run the default path with *no* provider at all. | ✅ Pass (strengthens) |
| **IV. Portability by Default** | Pure Rust over existing crates, with no `cfg` and no OS calls. Cross targets are unaffected. | ✅ Pass |
| **V. Unified Errors & Observability** | `completion` keeps the existing trait's `anyhow::Result` signature. User input errors (bad JSON, unknown tool) are *replies*, not `Err`, so the agent stays usable (US4-3/4/5). Dispatch decisions are logged with `tracing::debug!`. There is no `unwrap()` outside tests. | ✅ Pass |

**Post-design re-check (after Phase 1)**: Still passes with no changes. The contracts add no API surface beyond one `400` arm, and the data model adds no persisted entities.

## Project Structure

### Documentation (this feature)

```text
specs/008-dummyplug-provider/
├── plan.md                         # This file
├── research.md                     # Phase 0: decisions R1–R13
├── data-model.md                   # Phase 1: entities, validation, turn state machine
├── quickstart.md                   # Phase 1: manual verification script
├── contracts/
│   └── dummyplug-protocol.md       # Phase 1: user-facing chat protocol + provider API behavior
├── checklists/
│   └── requirements.md             # From /speckit-specify
└── tasks.md                        # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
src/
├── config/
│   └── provider.rs                       # + ProviderVariant::dummyplug
├── utils/
│   ├── mod.rs                            # + pub mod json_schema;
│   ├── markdown.rs                       # + parse_markdown_str(); read_markdown() delegates to it; + equivalence tests
│   └── json_schema.rs                    # NEW: generic JSON Schema traversal (resolve, properties, sample_value) + unit tests
├── storage/
│   └── memory_bundle.rs                  # parse_markdown_bytes() delegates to utils::markdown::parse_markdown_str (signature unchanged)
├── agents/agent/model/
│   ├── mod.rs                            # + mod dummyplug; + arm in new(), new_with_override(), resolve_provider()
│   └── dummyplug.rs                      # NEW: DummyplugModel (VizierModelTrait) + unit tests
└── channels/http/api/v1/providers/
    └── mod.rs                            # + upsert_provider arm → 400 "requires no configuration"

webui/app/
├── interfaces/types.ts                   # + 'dummyplug' in ChatProvider, CHAT_PROVIDERS, *_MODELS; + chatProviderLabel()
├── components/AgentForm.tsx              # 3 <option> sites use chatProviderLabel()
└── routes/agent-settings.tsx             # 3 <option> sites use chatProviderLabel()

docs/src/configuration/providers.md       # + dummyplug row / short section
```

**Structure Decision**: Single-project layout (existing). Chat behavior lives in `src/agents/agent/model/dummyplug.rs`, next to the other provider code in `model/`. Schema traversal lives in `src/utils/json_schema.rs`, which is provider-agnostic and reusable, alongside `utils/markdown.rs` and `utils/tar.rs`. Everything else is registration only.

### `utils/json_schema.rs` public API

```text
pub struct SchemaProperty { name, description: Option<String>, required: bool, schema: Value }
pub fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value   // local $ref (#/$defs, #/definitions)
pub fn properties(schema: &Value) -> Vec<SchemaProperty>             // top-level, $ref + allOf aware; required (in `required` order) then optional (alphabetical)
pub fn sample_value(schema: &Value) -> Value                         // placeholder conforming in type/shape (R7 rules)
```

### `dummyplug.rs` internal layout

```text
pub struct DummyplugModel { context_window: Option<u64> }
impl DummyplugModel { pub fn new(agent_config: &AgentConfig) -> Self }
impl VizierModelTrait for DummyplugModel { completion(), context_window() }

fn command_text(message: &Message) -> String              // R4: strip frontmatter / attachments trailer / fence, trim
fn tool_results(message: &Message) -> Vec<&ToolResult>     // R5 rule 1
fn reply_tool_results(results, history) -> String          // R8
fn reply_tool_list(tools) -> String                        // contract §1
fn reply_tool_sample(tool) -> String                       // contract §2: uses json_schema::{properties, sample_value}
fn parse_tool_request(text, tools) -> Result<ToolCall, String>   // contract §3; Err = user-facing reply text
fn lorem_ipsum(rng) -> String                              // R9

#[cfg(test)] mod tests
```

### Unit tests

**`utils/markdown.rs`** (`cargo test markdown`): an equivalence test for `parse_markdown_str` covering normal, empty-frontmatter, CRLF and no-header input (same `(frontmatter, body)` as the old loop), plus unclosed-header input (`Err`, not a panic). The existing memory tests in `src/storage/memory.rs` must pass unchanged.

**`utils/json_schema.rs`** (`cargo test json_schema`), run against schemars-generated schemas and a hand-written MCP-style schema:

- `sample_value` covers: no properties → `{}`; `Option<T>` → the inner type; a nested struct via `$defs`; `Vec<T>` → one item; a unit enum → its first variant; `default`/`examples` preferred; a recursive type stops at the depth limit with no stack overflow.
- `sample_value` output deserializes back into the Rust type the schema was generated from. This is the type-conformance check.
- `properties` covers: required-then-optional ordering; `required` flags are correct; a `$ref` target's description is used as the fallback; multi-line descriptions are folded; `allOf` is merged.

**`dummyplug.rs`** (`cargo test dummyplug`):


1. `command_text` strips real `VizierRequest::to_prompt()` output: frontmatter via `parse_markdown_str`, the attachments trailer, and a ```` ```json ```` fence. It also falls back to the whole text when there is no frontmatter.
2. Dispatch: `tools` (case and whitespace variants), exact tool name, valid JSON producing a `ToolCall`, unknown tool, malformed JSON, and prose producing non-empty lorem ipsum.
3. **Round-trip (SC-003)**: for schemars-generated schemas of representative input types, the fenced JSON extracted from `reply_tool_sample` passes `parse_tool_request` and yields a `ToolCall` with the same name. The representative types cover no args, `Option<T>`, a nested struct via `$defs`, `Vec<T>`, a unit enum, and a recursive type.
4. A message containing a `ToolResult` produces text only, never a `ToolCall`.
5. `Usage` is zero, and `context_window` passes through the config.
6. Argument lines in the sample reply show `` `name`: description `` under Required/Optional, or `` `name` `` alone when there is no description. The description lookup itself is tested in `json_schema`.

## Implementation Order

0. Refactor: `utils::markdown::parse_markdown_str` with its equivalence tests. Point `read_markdown` and `parse_markdown_bytes` at it, and run `cargo test` (the memory tests must stay green). This lands first and on its own, so a regression is isolated from the provider work.
1. `ProviderVariant::dummyplug` plus the compiler-driven arms (`resolve_provider`, `new`, `new_with_override`, `upsert_provider`). At this stage, `DummyplugModel` returns lorem ipsum only. **This is US1 (P1) and an MVP.**
2. `command_text` and the `tools` listing (US2).
3. `utils/json_schema.rs` (with its tests), then the tool sample reply (US3).
4. `parse_tool_request`, the `ToolCall` emission, and the tool-result echo (US4, P1).
5. WebUI entries and label, and docs.
6. Unit tests, `cargo clippy`, `npm run typecheck`, and the manual quickstart run.

## Complexity Tracking

No constitution violations. This section is intentionally empty.
