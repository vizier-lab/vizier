# Research: Dummyplug Test Provider

**Feature**: `008-dummyplug-provider` | **Date**: 2026-09-27

All Technical Context unknowns have been resolved. Every decision below is based on reading the current code, not assumed.

---

## R1. Where the provider plugs in

**Decision**: Implement `DummyplugModel` in a new file, `src/agents/agent/model/dummyplug.rs`, as a direct implementation of the existing `VizierModelTrait`:

```rust
async fn completion(&self, message: Message, history: Vec<Message>, tools: Vec<ToolDefinition>)
    -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)>;
fn context_window(&self) -> Option<u64>;
```

It does **not** use a rig-core `CompletionClient`. Registration is a single `ProviderVariant::dummyplug => Self::build(DummyplugModel::new(agent_config))` arm in both `VizierModel::new` and `VizierModel::new_with_override`.

**Rationale**: `VizierModelTrait` is the only thing the agent loop sees (`agents/agent/mod.rs:617`, `:1047`, `:1273`, and `tools/read_image/mod.rs:115`). It already receives the three things dummyplug needs:
- `message`: the latest user message, or the tool-result message when the loop continues after a tool call.
- `history`
- `tools`: `Vec<ToolDefinition>` with `name`, `description`, and the JSON-Schema `parameters`. This comes from `VizierTools::tools()`, which already merges the default toolset, the user toolset, and every MCP server's tools as `mcp_<server>__<tool>`.

When dummyplug returns `AssistantContent::ToolCall`, the existing loop at `agents/agent/mod.rs:699-900` runs the tool through the normal path: hooks `on_tool_call`/`on_tool_response`, timeout, session-file handling, and history. This satisfies FR-006 without touching the loop.

**Alternatives considered**:
- *Implement a rig `CompletionModel`/`CompletionClient`.* Rejected. It needs request/response types, a builder, and client plumbing for no benefit, because Vizier only uses the `VizierModelTrait` surface.
- *Mock at the HTTP level (a fake OpenAI-compatible server behind `custom`).* Rejected. It needs a running listener, which works against Principle III. It also can't see the tool list in a structured form.
- *Intercept "tools"/tool JSON in the agent loop or a hook.* Rejected. That branches on provider type inside shared dispatch code, which Principle II forbids, and it would change behavior for real providers.

## R2. Provider resolution and credentials

**Decision**: Add a `ProviderVariant::dummyplug` arm to `resolve_provider` that returns `ResolvedProvider { api_key: String::new(), base_url: None }` with no storage or env lookup.

The `dummyplug` variant has **no** `ProviderEntryConfig` variant, **no** `ProviderConfig` YAML field, and **no** stored provider entry.

**Rationale**: FR-002 requires zero configuration. Agent creation doesn't check that a provider entry exists: the only `get_provider` caller outside storage/keys is the Ollama special case at `agents/mod.rs:107`. So an agent with `provider: dummyplug` works as soon as the enum variant exists.

**Alternatives considered**: Adding a `ProviderEntryConfig::Dummyplug {}` storage variant. Rejected: it adds a storage variant, a settings-page row, and migration surface for a provider that has nothing to configure (Principle I).

## R3. Provider HTTP API (`/api/v1/providers/{variant}`)

**Decision**: `upsert_provider` gets a `ProviderVariant::dummyplug` arm that returns `400 Bad Request` with `"dummyplug requires no configuration"`. `provider_to_response` needs no change because it matches on `ProviderEntryConfig`, which gets no new variant. `GET`/`DELETE` already return 404 or a storage error for an unknown entry.

**Rationale**: The match over `ProviderVariant` is exhaustive, so an arm is mandatory. Rejecting the request is the honest answer.

## R4. What text the provider matches against

**Finding**: The user text doesn't reach the model as typed. `VizierRequest::to_message` (`schema/request.rs:246`) wraps it with `to_prompt()`:

```text
---
<yaml: sender, metadata>
---

<user content>

# Attached Files            ← only when attachments exist
- a.png (image/png)
...
```

Attachments follow as extra `UserContent` items. Session hooks (`on_request`) may also rewrite the request first.

**Decision**: Extract the "command text" like this:
1. Concatenate the `UserContent::Text` items of `message`. Ignore image, audio, and document items (spec edge case: attachments are ignored).
2. Strip the frontmatter with the shared `utils::markdown::parse_markdown_str::<serde_yaml::Value>(text)` and keep the body (see **R4a**). On `Err`, meaning the text has no header, use the whole text. That is the case for some internal prompts.
3. Cut at the first `\n\n# Attached Files\n`, if present.
4. Strip one surrounding Markdown code fence (```` ``` ```` or ```` ```json ````). The sample reply is fenced, and chat clients (Discord, WebUI) keep the fences when a user copies it.
5. `trim()`.

**Alternatives considered**: Reading `VizierRequestContent` directly. Rejected: the model layer only gets `Message`, and threading the raw request through would change the trait for one provider.

## R4a. One shared frontmatter parser

**Finding**: The "`---` line, YAML lines until the next `---`, then the body" loop already exists twice:

| Location | Input | Problem |
|----------|-------|---------|
| `utils::markdown::read_markdown<T>(path: PathBuf)` (`src/utils/markdown.rs:17`) | File path; reads from disk | Takes only a path. **Panics** on an unclosed header, because it calls `content.remove(0)` in a loop with no bound. |
| `storage::memory_bundle::parse_markdown_bytes<T>(bytes)` (`src/storage/memory_bundle.rs:143`) | Bytes | `pub(crate)` in memory storage. A model provider shouldn't depend on storage internals. |

A copy in dummyplug would be the third. Principle II requires merging it before new work builds on it.

**Decision**: Add one string-based parser to `src/utils/markdown.rs`:

```rust
/// Split `---\n<yaml>\n---\n<body>` and parse the YAML frontmatter as `T`.
/// Err when the first line isn't `---`, the block is unterminated, or the YAML doesn't parse as `T`.
pub fn parse_markdown_str<T: DeserializeOwned>(raw: &str) -> Result<(T, String), VizierError>
```

It keeps the existing line semantics **exactly**, so current callers see no difference:
- Split on `['\n', '\r']`.
- The first line must be exactly `---`.
- The YAML is the lines up to the next exact `---`.
- The body is the remaining lines joined with `\n`.

The only intended difference is that an unclosed header returns `Err` instead of panicking.

Both existing functions become thin wrappers around it:
- **`read_markdown`**: `std::fs::read_to_string(path)` → `parse_markdown_str`. The "failed to find frontmatter for <path>" error text stays, by mapping the missing-`---` case.
- **`parse_markdown_bytes`**: `String::from_utf8_lossy(bytes)` → `parse_markdown_str`, with the error mapped to `anyhow`. It keeps its signature, so no `memory_bundle` call site changes.

The callers are:
- `read_markdown`: skills (`skill/mod.rs`, `skill/install.rs`), the legacy fs task and dream-journal readers used by the startup migration, and `dependencies.rs`'s legacy memory migration.
- `parse_markdown_bytes`: the memory read, write and history path.

**Risk and mitigation**: `parse_markdown_bytes` is on the memory write/history path from spec 006. The mitigations are:
- the existing memory tests (`src/storage/memory.rs`) must pass unchanged
- a new equivalence test in `utils/markdown.rs` feeds normal, empty-frontmatter, CRLF, no-header and unclosed-header inputs to `parse_markdown_str`, and asserts the `(frontmatter, body)` the old loop produced, or `Err` for the unclosed-header case

**Alternatives considered**:
- *Calling `parse_markdown_bytes` from dummyplug.* Rejected, because it makes the model layer depend on storage internals and leaves two copies of the loop.
- *A frontmatter crate (e.g. `gray_matter`).* Rejected per Principle I; the loop is ~15 lines.

## R5. Dispatch order

**Decision**: Evaluate these checks in order, and let the first match win:

| # | Condition | Reply |
|---|-----------|-------|
| 1 | `message` contains any `UserContent::ToolResult` | Text echo of each tool result (FR-007). Never another tool call. |
| 2 | Command text, lowercased, `== "tools"` | Tool listing (FR-003) |
| 3 | Command text `==` some `tool.name` (exact, case-sensitive) | Sample request (FR-004) |
| 4 | Command text starts with `{` | Parse as a tool request. Emit a `ToolCall`, or an explanatory error (FR-006, FR-008). |
| 5 | Anything else | Lorem ipsum (FR-009) |

**Rationale**:
- Rule 1 comes first, so a tool result can never be mistaken for a new command. That guarantees the turn ends with exactly one tool call per user request, with no loop.
- Rule 2 comes before rule 3, so a hypothetical tool literally named `tools` can't shadow the command.
- Tool names are case-sensitive identifiers such as `read_core` or `mcp_fs__read_file`, so rule 3 matches exactly.
- Rule 4 keys on a leading `{`. That separates "meant as a tool request but malformed" (FR-008) from ordinary prose, which falls through to lorem ipsum.
- Non-interactive prompts start with frontmatter plus prose, not `{`. That covers dream cycles, scheduled tasks, handover generation, and `read_image`. They land on rule 5 and complete normally (FR-010).

## R6. Tool request JSON format

**Decision**: One canonical shape, used both for the sample (output) and the request (input):

```json
{ "tool": "<tool name>", "arguments": { ... } }
```

- `tool`: required string that must match a name in `tools`.
- `arguments`: optional object. It defaults to `{}` when omitted.
- Unknown extra top-level keys are ignored.

**Rationale**: FR-005 requires the sample to round-trip as input. A single, small shape is easiest to type by hand. Other shapes are rejected on purpose: accepting `name`/`args`, OpenAI-style `function.arguments` strings, and so on would add surface without testing value (Principle I).

## R7. Sample arguments from JSON Schema

**Decision**: Put the JSON Schema traversal in a new general-purpose module, **`src/utils/json_schema.rs`**, not inside the provider. It works on a plain `serde_json::Value` and knows nothing about tools or dummyplug, so later features can use it too. Examples include arguments for skills or subtasks, validation hints, or WebUI forms built from a schema. Dummyplug is only its first caller. The public API is plain functions, with no trait:

```rust
/// A top-level property of an object schema.
pub struct SchemaProperty {
    pub name: String,
    pub description: Option<String>, // property's own, else its `$ref` target's; folded to one line
    pub required: bool,              // listed in the schema's `required`
    pub schema: Value,               // the property's (ref-resolved) sub-schema
}

/// Resolve a local `$ref` (`#/$defs/X`, `#/definitions/X`) against `root`; non-ref nodes are returned as-is.
pub fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value;

/// Top-level properties of an object schema (following `$ref` and merging `allOf`):
/// required ones first in `required`-array order, then optional ones alphabetically
/// (serde_json is built without `preserve_order`, so declared key order isn't available).
pub fn properties(schema: &Value) -> Vec<SchemaProperty>;

/// A placeholder value that conforms to `schema` in type and shape.
pub fn sample_value(schema: &Value) -> Value;
```

`sample_value` walks the tool's `parameters` schema, which schemars generates for built-in tools and MCP servers supply for their tools. It uses the schema itself as the `$ref` root. For each node, it takes the first rule that applies:

1. `default`, then `examples[0]` or `example`, if present.
2. `const`, then `enum[0]`.
3. `$ref`: resolve `#/$defs/X` or `#/definitions/X` against the root schema.
4. `anyOf`/`oneOf`: take the first branch that isn't `{"type":"null"}`. `allOf`: merge the object branches.
5. `type`. If it's an array such as `["string","null"]`, take the first non-null entry. Then map:
   - `string` → `"<name>"`, using the property name as a hint, or `"string"`.
   - `integer` → `0`
   - `number` → `0.0`
   - `boolean` → `false`
   - `array` → `[sample(items)]`
   - `object` → every entry in `properties`, sampled recursively
   - missing type → `null`
6. Stop at depth 8 and return `null`, which protects against recursive `$ref`s.

The sample reply lists the top-level arguments in two groups, *required* (from the schema's `required`) and *optional*, above the fenced JSON. Each line reads `` `name`: description ``:
- The description is the property's schema `description`, or the `description` of the `$ref` target when the property has none.
- It is folded to one line.
- The `: description` part is left off when neither exists.

All properties go into the JSON. That satisfies acceptance scenario US3-3.

**Why descriptions**: Placeholder values only match on type, so a string that must follow a format gets a value the tool will reject. For example, `schedule_one_time_task.schedule` needs RFC3339. That format lives only in the argument's description ("in RFC3339 format (e.g., 2024-12-25T10:30:00Z)"). Showing descriptions tells the user how to edit the placeholders before sending, and it adds no new logic, because the descriptions are already in the schema dummyplug receives.

**Alternative considered**: Parsing `e.g.` examples out of descriptions to fill in placeholder values. Rejected, because free-text parsing is brittle, and showing the description gets the same result.

The sample reply is built from `properties()`, which provides the name, description and required flag for each argument line, and from `sample_value()`, which provides the JSON.

**Rationale**: No new dependency. The schemas are small, and ~100 lines of `serde_json::Value` walking covers what schemars 1.x emits (`$defs`, `anyOf` for `Option<T>`, `enum` for unit enums) and typical MCP schemas. Schema traversal is a separate concern from chat dispatch, so keeping it in `utils/` makes it easy to test on its own, and the provider file stays about protocol behavior.

**Alternatives considered**:
- *A JSON-Schema faker or walker crate.* Rejected per Principle I and the dependency-weight rule.
- *Keeping these as private helpers in `dummyplug.rs`.* This would be the Principle I default for a single call site. It was overridden by an explicit product decision: schema traversal is expected to be reused. The module is plain functions, so it adds no abstraction, only a file boundary.

## R8. Tool-result echo (closing the turn)

**Decision**: When `message` contains `ToolResult` items, reply with one text block per result:

```text
**Tool result** (`<tool name>`):
<text content of the result>
```

The tool name comes from the matching `ToolCall` in the last `Assistant` message in `history`, found by `id`. If no match is found, use the id. `ToolResultContent::Image` items render as `[image]`.

## R9. Lorem ipsum generation

**Decision**: Build it by hand from a static `const WORDS: &[&str]` (the ~60-word classic lorem ipsum vocabulary) and `rand` (already a dependency, `rand = "0.10"`, used in `agents/agent/mod.rs`). Generate 1–3 paragraphs of 2–5 sentences, 6–14 words each. Capitalize the first word, and end each sentence with `.`.

**Alternatives considered**: The `lipsum` crate. Rejected, because a new dependency isn't justified for 20 lines (Principle I).

## R10. Usage and context window

**Decision**:
- **Usage**: Return `Usage::new()` (all zeros). The spec allows zero.
- **Context window**: `context_window()` returns `agent_config.context_window`, which is `None` unless the user overrides it. With `None`, the checkpoint logic in `agents/agent/mod.rs:653` never triggers. That is correct, because no real context is consumed.
- **Message ID**: `message_id` is `None`.
- **Tool-call IDs**: `format!("dummyplug-{}", uuid::Uuid::new_v4())` (`uuid` is already a dependency), with `call_id: None` and `signature: None`.

## R11. Model name

**Decision**: Dummyplug accepts any `model` string and ignores it. The WebUI default model and the only listed model are both `"dummyplug"`.

## R12. Making it recognizable as a test provider (FR-011)

**Decision**:
- In the WebUI, add `'dummyplug'` to `ChatProvider`, `CHAT_PROVIDERS` (last entry), `CHAT_PROVIDER_DEFAULT_MODELS`, and `CHAT_PROVIDER_MODELS`.
- Add a tiny `chatProviderLabel(p)` helper in `interfaces/types.ts` that returns `"dummyplug (testing)"` for dummyplug and `p` otherwise. Use it in the six `<option>` render sites in `AgentForm.tsx` and `agent-settings.tsx`.
- Leave dummyplug out of the Settings → Providers `ALL_VARIANTS` list (`settingsRoot.tsx`), because there is nothing to configure.
- Add a row to `docs/src/configuration/providers.md`.

## R13. Build profile

**Decision**: Always compile dummyplug, with no Cargo feature. It's opt-in per agent (spec assumption), it has no dependencies, and gating it would need `cfg` in the provider match (Principle IV).
