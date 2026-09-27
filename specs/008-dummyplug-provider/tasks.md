---

description: "Task list for the dummyplug test provider"
---

# Tasks: Dummyplug Test Provider

**Input**: Design documents from `specs/008-dummyplug-provider/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/dummyplug-protocol.md, quickstart.md

**Tests**: Included. plan.md explicitly asks for unit tests in `utils/markdown.rs`, `utils/json_schema.rs`, and `dummyplug.rs`, and for manual quickstart verification (the constitution's quality gate for runtime-affecting changes).

**Organization**: Tasks are grouped by user story, in priority order. US1 and US4 are both P1: US1 comes first because it is the MVP, and US4 second. US2 and US3 are P2. US4 comes before US2/US3 because it doesn't need them: a tester can hand-write a tool request.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US4 from spec.md
- Paths are relative to the repository root

## Key references (read before starting)

- **Provider seam**: `VizierModelTrait` in `src/agents/agent/model/mod.rs`:
  `completion(message, history, tools) -> anyhow::Result<(Option<String>, OneOrMany<AssistantContent>, Usage)>` and `context_window() -> Option<u64>`
- **Behavior contract**: `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` (§1 tools, §2 sample, §3 run, §4 echo, §5 lorem, and the provider API table)
- **Decisions**: `specs/008-dummyplug-provider/research.md`, R1–R13 (R4/R4a text extraction, R5 dispatch order, R6 request shape, R7 schema sampling, R8 echo, R9 lorem, R10 usage/ids)
- **rig-core 0.38.2 types**:
  - `rig_core::message::{Message, UserContent, AssistantContent, ToolCall, ToolFunction, ToolResult, ToolResultContent}`
  - `rig_core::completion::{ToolDefinition, Usage}` (`Usage::new()` is all zeros)
  - `rig_core::OneOrMany`
  - `ToolCall { id, call_id: Option<String>, function: ToolFunction { name, arguments: Value }, signature: Option<String>, .. }`. Check the struct in `~/.cargo/registry/src/*/rig-core-0.38.2/src/completion/message.rs` for any extra fields, such as `additional_params`.
- **Conventions**: `rand` 0.10 as used in `src/agents/agent/mod.rs` (`use rand::{RngExt, SeedableRng, rngs::StdRng}`, `random_range`). `tracing` for logs. No `unwrap()`/`expect()` outside tests.

---

## Phase 1: Setup

**Purpose**: Record a known-good baseline before the shared refactor touches the memory storage path.

- [X] T001 On branch `008-dummyplug-provider`, run `cargo test` and `cargo clippy`, and write down which tests pass (in particular the memory tests in `src/storage/memory.rs`). Any failure after Phase 2 that isn't on this list is a regression.

---

## Phase 2: Foundational (Blocking Prerequisites): one shared frontmatter parser (research R4a)

**Purpose**: Merge the two existing copies of the frontmatter parsing loop into one string-based function. US4 then reuses it to strip the header from user messages. This lands on its own so that any regression in memory storage is isolated from provider work.

**⚠️ CRITICAL**: US4, US2, and US3 depend on `parse_markdown_str`. US1 does not.

- [X] T002 Write the tests first: add a `#[cfg(test)] mod tests` to `src/utils/markdown.rs` for a not-yet-existing `parse_markdown_str::<serde_yaml::Value>(raw: &str)`. Each case asserts the `(frontmatter, body)` the current loop produces, with the loop's semantics applied: split on `['\n','\r']`, the first line must equal `---`, YAML runs until the next exact `---` line, and the body is the rest joined by `\n`. Cases:
  - (a) `"---\ntitle: a\n---\nhello\nworld"` → `({title: a}, "hello\nworld")`
  - (b) `"---\n---\nbody"`: the YAML is empty. Assert whatever `serde_yaml` does for an empty `Value` (`Null`) with body `"body"`.
  - (c) `"---\r\ntitle: a\r\n---\r\nhello"` produces the same frontmatter; the body has the empty segments CRLF splitting creates (`"\nhello"`).
  - (d) `"no header"` → `Err`
  - (e) `"---\ntitle: a\nhello"` (unclosed) → `Err`, not a panic
- [X] T003 Implement `pub fn parse_markdown_str<T: DeserializeOwned>(raw: &str) -> Result<(T, String), VizierError>` in `src/utils/markdown.rs`, with the exact semantics above. Return `Err` when the first line isn't `---`, the block is unterminated (don't `remove(0)` on an empty `Vec`), or the YAML fails to parse. Add a doc comment. Make T002 pass.
- [X] T004 Rewrite `read_markdown` in `src/utils/markdown.rs` as `std::fs::read_to_string(&path)` → `parse_markdown_str::<T>(&raw)`. When the header is missing, keep its existing error text, `failed to find frontmatter for <path>`. Keep the signature `read_markdown<T: DeserializeOwned + Clone>(path: PathBuf) -> Result<(T, String), VizierError>` unchanged. Remove the old loop.
- [X] T005 Rewrite `pub(crate) fn parse_markdown_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<(T, String)>` in `src/storage/memory_bundle.rs` as `String::from_utf8_lossy(bytes)` → `crate::utils::markdown::parse_markdown_str::<T>(..)`, mapping `VizierError` to `anyhow!(e.0)`. Keep the signature unchanged and remove the old loop. Don't edit any caller.
- [X] T006 Run `cargo test` and `cargo clippy`. The T002 tests and every test from the T001 baseline must pass, including the `src/storage/memory.rs` memory tests.

**Checkpoint**: One frontmatter parser, no behavior change except unclosed header → `Err` instead of panic. Commit it separately, e.g. `refactor: share one frontmatter parser across utils and memory storage`.

---

## Phase 3: User Story 1: Chat with an agent without a live provider (Priority: P1) 🎯 MVP

**Goal**: An agent with `provider: dummyplug` replies to every message with random lorem ipsum. It needs no keys, no network, and no provider entry.

**Independent Test**: Create a dummyplug agent in the WebUI, send `hello`, and see a lorem ipsum reply saved in session history. Sending it again gives different text. It works with no API keys set.

### Implementation for User Story 1

- [X] T007 [US1] Add the `dummyplug` variant as the last entry of `pub enum ProviderVariant` in `src/config/provider.rs`. Add nothing to `ProviderConfig`: there is no YAML config (research R2).
- [X] T008 [US1] Create `src/agents/agent/model/dummyplug.rs` with:
  - `pub struct DummyplugModel { context_window: Option<u64> }`
  - `impl DummyplugModel { pub fn new(agent_config: &AgentConfig) -> Self }`, which takes `agent_config.context_window` as-is (research R10)
  - `const WORDS: &[&str]`, the classic ~60-word lorem ipsum vocabulary
  - `fn lorem_ipsum() -> String`: 1–3 paragraphs separated by `\n\n`, each with 2–5 sentences of 6–14 words, the first word capitalized and ending in `.` (research R9). Use `rand` 0.10 as in `src/agents/agent/mod.rs`.
  - `#[async_trait::async_trait] impl VizierModelTrait for DummyplugModel`:
    - `completion` returns `Ok((None, OneOrMany::one(AssistantContent::text(lorem_ipsum())), Usage::new()))`. Use whatever `AssistantContent` text constructor rig 0.38 provides, such as `AssistantContent::text(..)` or `AssistantContent::Text(Text { text })`.
    - `context_window` returns the field.
  - A module doc comment: offline test provider, see `specs/008-dummyplug-provider/`.
- [X] T009 [US1] Register the provider in `src/agents/agent/model/mod.rs`:
  - add `mod dummyplug;`
  - in `resolve_provider`, add `ProviderVariant::dummyplug => Ok(ResolvedProvider { api_key: String::new(), base_url: None })`
  - in `VizierModel::new`, add `ProviderVariant::dummyplug => Self::build(dummyplug::DummyplugModel::new(agent_config))`
  - in `VizierModel::new_with_override`, add the same arm using `&override_config`
  Both `build` calls need `VizierModelTrait + Sync + Send + 'static`.
- [X] T010 [P] [US1] In `upsert_provider` in `src/channels/http/api/v1/providers/mod.rs`, add a `ProviderVariant::dummyplug` arm that returns early with `err_response(StatusCode::BAD_REQUEST, "dummyplug requires no configuration".into())` (research R3, contract "Provider API"). Follow the early return used by the `custom` arm when `base_url` is missing. `provider_to_response` needs no change.
- [X] T011 [P] [US1] In `webui/app/interfaces/types.ts`:
  - add `'dummyplug'` to the `ChatProvider` union and as the last entry of `CHAT_PROVIDERS`
  - add `dummyplug: 'dummyplug'` to `CHAT_PROVIDER_DEFAULT_MODELS` and `dummyplug: ['dummyplug']` to `CHAT_PROVIDER_MODELS`
  - add and export `export const chatProviderLabel = (p: string): string => p === 'dummyplug' ? 'dummyplug (testing)' : p` (research R12)
  Don't add it to the Settings → Providers `ALL_VARIANTS` list in `webui/app/routes/settingsRoot.tsx`.
- [X] T012 [P] [US1] In `webui/app/components/AgentForm.tsx`, change the 3 `CHAT_PROVIDERS.map((p) => <option key={p} value={p}>{p}</option>)` sites (main provider, dream provider, read-image provider; around lines 617, 964, and 1810) to render `{chatProviderLabel(p)}`, and import `chatProviderLabel`. Depends on T011.
- [X] T013 [P] [US1] Make the same change in `webui/app/routes/agent-settings.tsx` at its 3 `CHAT_PROVIDERS.map` `<option>` sites (around lines 727, 1072, and 1919). Depends on T011.
- [X] T014 [US1] Add `#[cfg(test)] mod tests` to `src/agents/agent/model/dummyplug.rs` (plan test 5, plus FR-009):
  - `lorem_ipsum()` is non-empty and ends with `.`
  - 5 calls are not all identical
  - `completion(prose_message, vec![], vec![])` returns exactly one `AssistantContent::Text` and `Usage::new()`
  - `context_window()` returns `Some(1234)` when the config has `context_window: Some(1234)`, and `None` otherwise. Build `AgentConfig` with `Default`, or the smallest constructor available.
  Use `#[tokio::test]` for the async cases.
- [X] T015 [US1] Verify:
  - `cargo build` and `cargo test dummyplug` pass
  - `cd webui && npm run typecheck` passes
  - Run quickstart.md §1–§2: `env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY just run`, then create an agent with provider `dummyplug (testing)`
  - Send `hello` twice and confirm two different lorem ipsum replies in the WebUI and in session history
  - `curl -X PUT .../api/v1/providers/dummyplug` returns 400

**Checkpoint**: MVP. Dummyplug agents chat offline.

---

## Phase 4: User Story 4: Trigger a tool by sending the JSON request (Priority: P1)

**Goal**: A message of the form `{"tool": "...", "arguments": {...}}` makes the agent run that tool through the normal path (hooks, history, UI). The final reply echoes the tool result, and errors are clear replies with nothing executed.

**Independent Test**: Send `{"tool": "read_core", "arguments": {}}`. The tool call and result appear in the UI and history, and the reply shows `**Tool result** (`read_core`):` with the CORE text. Also check that `{"tool": "nope"}` and `{"tool": ` return errors and run nothing.

**Depends on**: Phase 2 (`parse_markdown_str`) and US1 (dummyplug.rs exists and is registered).

### Implementation for User Story 4

- [X] T016 [US4] Implement `fn command_text(message: &Message) -> String` in `src/agents/agent/model/dummyplug.rs`, following research R4:
  1. Join the `UserContent::Text` items of a `Message::User` with `\n`, ignoring other content. For non-user messages, return an empty string.
  2. `crate::utils::markdown::parse_markdown_str::<serde_yaml::Value>(&text)`: on `Ok`, take the body; on `Err`, keep the whole text.
  3. Cut at the first `"\n\n# Attached Files\n"`.
  4. `trim()`. Then, if the text starts with ```` ``` ```` and ends with ```` ``` ````, drop the opening fence line (including any language tag such as `json`) and the closing fence.
  5. `trim()` again.
- [X] T017 [US4] Implement `fn tool_results(message: &Message) -> Vec<&ToolResult>`, which collects the `UserContent::ToolResult` items of a `Message::User`. Also implement `fn reply_tool_results(results: &[&ToolResult], history: &[Message]) -> String` (research R8, contract §4):
  - For each result, find the tool name by matching `result.id` against the `ToolCall.id`s in the **last** `Message::Assistant` in `history`, falling back to the id.
  - Render ``**Tool result** (`<name>`):\n<text>``, where the text joins the `ToolResultContent::Text` items and each `Image` becomes `[image]`.
  - Separate multiple results with `\n\n`.
- [X] T018 [US4] Implement `fn parse_tool_request(text: &str, tools: &[ToolDefinition]) -> Result<ToolCall, String>` (research R6, data-model "Tool request" validation table, contract §3). `Err` holds the user-facing reply text:
  - `serde_json::from_str::<Value>` fails → `Could not parse tool request: <err>`, followed by the expected-shape hint `{"tool": "<tool name>", "arguments": { ... }}`
  - not an object, `tool` missing or not a string, or `arguments` present but not an object → `Invalid tool request: <reason>` plus the same hint
  - `tool` not among `tools[*].name` → ``Unknown tool `<name>`. Send `tools` to list available tools.``
  - valid → a `ToolCall` with `id: format!("dummyplug-{}", uuid::Uuid::new_v4())`, `call_id: None`, `function: ToolFunction { name, arguments }` (`arguments` defaults to `json!({})`), `signature: None`, and any other fields at their defaults (research R10)
  Ignore extra top-level keys.
- [X] T019 [US4] Replace the body of `completion` in `src/agents/agent/model/dummyplug.rs` with the ordered dispatch from research R5, rules 1, 4 and 5 (rules 2 and 3 come in US2/US3; leave a comment marking where they go):
  - (1) if `tool_results(&message)` is non-empty → `Text(reply_tool_results(..))`
  - (4) else if `command_text(&message).starts_with('{')` → `ToolCall`, or `Text(err)`
  - (5) else → `Text(lorem_ipsum())`
  Log the chosen branch with `tracing::debug!`. Always return one `AssistantContent`, `None` as the message id, and `Usage::new()`.
- [X] T020 [US4] Add tests to `src/agents/agent/model/dummyplug.rs` (plan tests 1, 2 (JSON part), and 4):
  - **`command_text`**:
    - build a real message via `VizierRequest { content: VizierRequestContent::Chat("tools".into()), user: "tester".into(), metadata: json!({}), ..Default::default() }.to_message("")`, and assert `command_text == "tools"`
    - same for a ```` ```json\n{"tool":"x"}\n``` ```` fenced body → `{"tool":"x"}`
    - a plain `Message::user("hi")` with no frontmatter → `"hi"`
    - a prompt text that ends with the `# Attached Files` trailer → the trailer is removed
  - **Dispatch**, with a `tools` fixture of one `ToolDefinition { name: "echo", description: "Echo", parameters: json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}) }`:
    - a valid request → exactly one `AssistantContent::ToolCall` with name `echo`, the given arguments, and an id starting with `dummyplug-`
    - a request with `arguments` omitted gets `{}`
    - an unknown tool, malformed JSON, and `[1,2]` each give `Text` and no `ToolCall`
  - **Tool result**: a `Message::User` containing a `ToolResult` (with a matching `ToolCall` in a history `Assistant` message) gives `Text` containing `**Tool result** (`echo`)` and never a `ToolCall`
- [X] T021 [US4] Verify manually (quickstart.md §3 and §4):
  - `{"tool": "read_core", "arguments": {}}` shows a tool-call entry in the WebUI, then the echoed CORE.md
  - `{"tool": "nope"}` and `{"tool": ` return errors and no tool runs
  - a `write_memory` request (arguments written by hand from the tool's schema) creates a memory with a revision in its version history (US4-2)
  - a tool given bad arguments shows the tool's error in the echo, and the agent still replies to `hello` afterwards (US4-3)

**Checkpoint**: Tools can be tested end to end with no live model.

---

## Phase 5: User Story 2: Discover the agent's tools (Priority: P2)

**Goal**: `tools` (ignoring case and surrounding whitespace) lists every tool available to the agent, including MCP tools, with a short description.

**Independent Test**: Send `tools` and ` Tools ` to a dummyplug agent. Both list every tool in the agent's toolset. An agent with MCP servers also shows `mcp_<server>__<tool>` entries.

**Depends on**: US4's T016 (`command_text`) and T019 (dispatch skeleton).

### Implementation for User Story 2

- [X] T022 [US2] Implement `fn reply_tool_list(tools: &[ToolDefinition]) -> String` in `src/agents/agent/model/dummyplug.rs`, following contract §1:
  - the header `**Available tools** (N):`
  - one line per tool, in input order: `` - `<name>` — <first line of description, truncated to 120 chars with `…`> ``
  - the footer `Send a tool name to get a sample request.`
  - with zero tools, exactly `This agent has no tools available.`
- [X] T023 [US2] In `completion`, add dispatch rule 2 after rule 1 and before rule 4: `if command_text.eq_ignore_ascii_case("tools")` → `Text(reply_tool_list(&tools))`.
- [X] T024 [US2] Add tests to `src/agents/agent/model/dummyplug.rs`:
  - `tools`, ` Tools `, and `TOOLS` (sent through `VizierRequest::to_message`) all return the listing with every fixture tool name in input order
  - an empty `tools` gives the no-tools message
  - a multi-line description shows only its first line
  - a 200-char description is truncated

**Checkpoint**: Testers can discover exact tool names.

---

## Phase 6: User Story 3: Get a sample request for a specific tool (Priority: P2)

**Goal**: Sending an exact tool name returns its description, per-argument `` `name`: description `` lines grouped as Required/Optional, and a fenced JSON sample that round-trips through US4 unchanged.

**Independent Test**: Send `schedule_one_time_task`. The reply lists `schedule` under Required with its RFC3339 description. Sending the JSON block back unchanged invokes the tool; the tool may reject the placeholder date, and that's expected.

**Depends on**: US4 (`parse_tool_request`, for the round-trip) and US2's T023 (rule-order position).

### Implementation for User Story 3

- [X] T025 [P] [US3] Create `src/utils/json_schema.rs` and add `pub mod json_schema;` to `src/utils/mod.rs`. Implement:
  - `pub struct SchemaProperty { pub name: String, pub description: Option<String>, pub required: bool, pub schema: serde_json::Value }`
  - `pub fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value`: when `node` has a string `$ref` starting with `#/$defs/` or `#/definitions/`, return the target in `root` (or `node` when it's missing); otherwise return `node`
  Add a module doc: generic JSON Schema helpers, not tied to tools (research R7).
- [X] T026 [US3] Implement `pub fn properties(schema: &Value) -> Vec<SchemaProperty>` in `src/utils/json_schema.rs`:
  - resolve the root through `$ref`
  - if it has `allOf`, merge the `properties` and `required` of each (resolved) branch
  - ordering: the crate doesn't enable `serde_json`'s `preserve_order` (verified with `cargo tree -e features -i serde_json`), so `Map` is a `BTreeMap` and declared order is already lost. Return required properties first, in the order of the `required` array, which schemars emits in field order, then the optional ones alphabetically. Explain this in a comment. Don't enable `preserve_order`: it changes key order crate-wide.
  - for each property: `required` = the name is in `required`; `description` = its own `description`, else the resolved `$ref` target's `description`, folded to one line (split on whitespace, joined by single spaces); `schema` = the resolved sub-schema cloned
- [X] T027 [US3] Implement `pub fn sample_value(schema: &Value) -> Value` in `src/utils/json_schema.rs`, as a thin wrapper over a private `fn sample(node: &Value, root: &Value, name_hint: Option<&str>, depth: u8) -> Value` that follows research R7 in this order:
  1. `default`, then `examples[0]`, then `example`
  2. `const`, then `enum[0]`
  3. `$ref` → `resolve`, then recurse
  4. `anyOf`/`oneOf` → the first branch whose resolved form isn't `{"type":"null"}`; `allOf` → merge the object branches' `properties`, then sample as an object
  5. `type`: if it's an array, take the first entry that isn't `"null"`
     - `string` → `name_hint` or `"string"`
     - `integer` → `0`
     - `number` → `0.0`
     - `boolean` → `false`
     - `array` → `[sample(items)]`, or `[]` without `items`
     - `object` → sample every property, passing the property name as `name_hint`
     - anything else or missing → `null`
  6. `depth > 8` → `null`
  The root is the top-level schema.
- [X] T028 [US3] Add `#[cfg(test)] mod tests` to `src/utils/json_schema.rs`, using `schemars::schema_for!(T)` → `serde_json::to_value`. Define test-local types deriving `JsonSchema, Deserialize`: `NoArgs {}`, `WithOption { a: String, b: Option<u32> }`, `Nested { inner: Inner }` with `Inner { x: bool }`, `WithVec { items: Vec<String> }`, a unit enum `Mode { Fast, Slow }` field, `WithDefault` (`#[serde(default)]` or `#[schemars(default)]`), and a recursive `Node { children: Vec<Node> }`. Assert:
  - `sample_value` of each deserializes back into its type (`serde_json::from_value::<T>`), except that recursion only has to terminate without overflowing the stack
  - `NoArgs` → `{}`
  - `properties(WithOption)` gives `a` required and `b` optional, and a struct with required `z, a` and optional `y, b` comes back as `z, a, b, y`
  - a `///` doc comment on a field shows up as `description`, and a multi-line doc is folded
  - a field whose type has a doc comment but the field itself has none takes its description from the `$ref` target
  - a hand-written MCP-style schema (no `$defs`, `type: ["string","null"]`) samples as a string
- [X] T029 [US3] Implement `fn reply_tool_sample(tool: &ToolDefinition) -> String` in `src/agents/agent/model/dummyplug.rs`, following contract §2:
  - ``**`<name>`** — <full description>``
  - then `Required:` and `Optional:` blocks from `json_schema::properties(&tool.parameters)`, with lines `` - `name`: description `` (or `` - `name` `` when there's no description); omit a block when it's empty
  - then a ```` ```json ```` fence around `serde_json::to_string_pretty(&json!({"tool": name, "arguments": json_schema::sample_value(&tool.parameters)}))`. If the sample isn't an object, use `{}`.
  - then `Send the JSON back (edit the values first) to run the tool.`
- [X] T030 [US3] In `completion`, add dispatch rule 3 after rule 2 and before rule 4: `if let Some(tool) = tools.iter().find(|t| t.name == command_text)` → `Text(reply_tool_sample(tool))`.
- [X] T031 [US3] Add tests to `src/agents/agent/model/dummyplug.rs` (plan tests 3 and 6):
  - **Round-trip (SC-003)**: for tool fixtures built from `schemars::schema_for!` of the T028 types, send the name → reply; extract the text between the ```` ```json ```` fence markers; feed that fenced block, fences included, back as a new user message through `VizierRequest::to_message` → exactly one `ToolCall` with the same name
  - a tool with no parameters → `"arguments": {}` and no Required/Optional blocks
  - description lines: `` `name`: description `` for a described argument, a bare `` `name` `` otherwise, grouped under the right heading
  - rule order: a tool literally named `tools` still gets the listing

**Checkpoint**: All four stories work on their own. Testers can go list → sample → send → see the result in 4 messages or fewer (SC-005).

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T032 [P] Add a `dummyplug` row to the provider table in `docs/src/configuration/providers.md`: fields `—`, env var `—`, note "Offline test provider — no configuration; see below". Add a short "Dummyplug (testing)" subsection that summarizes contract §1–§5 (`tools`, tool name → sample, JSON → run, everything else → lorem ipsum) with one example exchange.
- [ ] T033 [P] Add `dummyplug` (offline test provider) to the provider list in the "Providers / models" section of `CLAUDE.md`.
- [ ] T034 Run `cargo clippy` and `cargo test` over the whole crate. Everything must be clean, and the T001 baseline must still pass.
- [ ] T035 Run `cd webui && npm run typecheck`.
- [ ] T036 Run the full quickstart.md, including:
  - config-less mode: `VIZIER_DATA_DIR=$(mktemp -d) cargo run -- run`, then create a dummyplug agent
  - §5 non-interactive paths: a one-time scheduled task completes with lorem ipsum, and a dream cycle for a dummyplug agent completes without errors (FR-010)
  - an agent on a real provider (if keys are available) still behaves as before (FR-012)

---

## Dependencies & Execution Order

### Phase dependencies

```text
Phase 1 (T001)
   └─▶ Phase 2 refactor (T002–T006)         ─┐
   └─▶ Phase 3 US1 (T007–T015)  [MVP]        ├─▶ Phase 4 US4 (T016–T021) ─▶ Phase 5 US2 (T022–T024) ─▶ Phase 6 US3 (T025–T031)
                                              ┘                                                                         └─▶ Phase 7 (T032–T036)
```

- **US1** depends only on Setup. It can be built in parallel with Phase 2, because it doesn't use `parse_markdown_str`.
- **US4** needs Phase 2 (`parse_markdown_str`, used by `command_text`) and US1 (the file exists and is registered).
- **US2** needs US4's `command_text` (T016) and dispatch skeleton (T019).
- **US3** needs US4's `parse_tool_request` (for the round-trip test) and US2's rule order (T023). **T025–T028 (`utils/json_schema.rs`) have no dependency on any other phase** and can start as soon as T001 is done.

### Within a story

- Tasks that edit `src/agents/agent/model/dummyplug.rs` (T008, T014, T016–T020, T022–T024, T029–T031) run in order, because they share one file.
- Tests are written right after the function they cover, in the same file, and must pass before the story's checkpoint. The exception is T002, which is test-first on purpose, to lock in the old behavior.

### Parallel opportunities

- **Phase 2 ∥ Phase 3 (US1)**: they touch different files (`utils/markdown.rs` and `storage/memory_bundle.rs` vs `config/provider.rs`, `model/*`, `providers/mod.rs`, and `webui/`).
- **Inside US1**, after T007–T009: T010 (HTTP API), T011 (types.ts), and then T012 ∥ T013 (the two TSX files).
- **`utils/json_schema.rs` (T025–T028)** can run alongside US1, US4, and US2. It's a new, standalone file.
- **Polish**: T032 ∥ T033.

### Parallel example: US1

```text
After T007 → T008 → T009 (sequential: enum, then model file, then registration):
  Task T010: upsert_provider 400 arm           (src/channels/http/api/v1/providers/mod.rs)
  Task T011: WebUI types + chatProviderLabel   (webui/app/interfaces/types.ts)
Then:
  Task T012: AgentForm.tsx option labels
  Task T013: agent-settings.tsx option labels
```

### Parallel example: early start on the schema utility

```text
While Phase 2 / US1 are in progress:
  Task T025 → T026 → T027 → T028  (src/utils/json_schema.rs, fully self-contained)
```

---

## Implementation Strategy

### MVP first (US1 only)

1. T001 baseline, then T007–T015.
2. **Stop and validate**: a dummyplug agent replies offline in the WebUI. This alone lets people test channels, sessions, history, and UI rendering without a provider.

### Incremental delivery

1. Phase 2 refactor, committed separately as `refactor:` so a memory regression is isolated.
2. US1, committed as `feat: add dummyplug offline test provider`, is the MVP.
3. US4: tools can run end to end (the other P1).
4. US2: tool discovery.
5. US3, plus `utils/json_schema.rs`: generated samples.
6. Polish: docs, CLAUDE.md, the full quickstart.

Each step leaves the binary working and the previous stories unchanged.

---

## Notes

- Total: 36 tasks. Each checkpoint is a good commit point, using conventional commits per the constitution.
- Never branch on `ProviderVariant::dummyplug` outside the four registration points: the enum, `resolve_provider`, `VizierModel::new`/`new_with_override`, and `upsert_provider`, plus the WebUI lists. All behavior lives in `DummyplugModel`.
- User input errors are replies, never `Err`. `completion` returns `Err` only for truly unexpected internal failures, and there are none by design.
