---
description: "Task list for 015: catalogue-driven providers and models, and removing rig"
---

# Tasks: Catalogue-Driven Providers and Models (and Removing rig)

**Input**: Design documents from `specs/015-provider-catalog/`

**Prerequisites**:
- plan.md
- spec.md (US1–US8)
- research.md (R1–R15)
- data-model.md
- contracts/: `http-api.md`, `provider-id-migration.md`, `provider-layer.md`, `catalogue-snapshot.md`
- quickstart.md

**Tests**: The spec asks for no TDD. The unit tests below are the ones plan.md lists under Testing:
- dialect `encode`/`decode` fixtures;
- golden fixtures recorded from rig 0.39;
- transport tests against an in-process axum server;
- catalogue parse and validation;
- the `ProviderId` legacy map;
- the custom-model merge;
- context-window resolution.

Each sits next to the code it checks. End-to-end checks are the quickstart's dummyplug and stub-server steps (constitution gate).

**Organization**: tasks are grouped by user story. rig is removed in **one cutover** (US1), after the new layer is built in Phase 2 alongside the untouched rig code (plan, *Phases*). There is no shim, and the branch is merged as a whole.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, and no dependency on an unfinished task)
- **[Story]**: US1–US8 from spec.md
- Paths are relative to the repository root
- "Contract X" means `specs/015-provider-catalog/contracts/X`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Record the baseline, add the build inputs (the catalogue seed and build-time gzip), and create the offline test fixture every story uses.

- [X] T001 Confirm the baseline on branch `015-provider-catalog`:
  - Run `cargo build`, `cargo test`, `cargo clippy` and `cd webui && npm run typecheck`, and record any pre-existing failures in the PR description so they aren't blamed on this feature.
  - Save `cargo tree -e normal | wc -l` for comparison after the cutover.
- [X] T002 In `Cargo.toml`:
  - Enable the `reqwest` features `json` and `multipart`, keeping the existing version 0.13.
  - Add `flate2` under `[build-dependencies]`, using the version already in `Cargo.lock`.
  - Add `"/assets/catalogue/**/*"` to the package `include` list.
  - Do **not** remove `rig-core` yet; that is T052.
- [X] T003 [P] Create `scripts/catalogue_update.py` per contract `catalogue-snapshot.md` § `scripts/catalogue_update.py`:
  - Use only the standard library, with `urllib`, run as `python3 -I`, and a `--keep-on-failure` flag.
  - Fetch `https://models.dev/api.json` and validate it against the committed `assets/catalogue/api.json` when one exists: it must parse, keep at least 50% of the provider count, and have at least one provider whose `npm` maps to a supported dialect per research R2.
  - Trim it to the kept provider and model fields.
  - Write `assets/catalogue/api.json` with keys sorted, one model per line. Write `meta.json` (`schema: 1`, `source_url`, `fetched_at` ISO-8601, the response `etag`, `providers`, `models`, `no_logo`).
  - Download `https://models.dev/logos/{id}.svg` for every provider id and every `canonical_model_id` prefix, 8 at a time, into `assets/catalogue/logos/`. Detect the fallback image by comparing against the bytes for `__none__`, and record those ids in `no_logo`. Remove logos for ids that no longer exist.
  - Print the counts and the total size.
  - On any error: with `--keep-on-failure`, print a `::warning::` line plus the age of the current snapshot and exit 0; without it, exit 1.
- [X] T004 [P] Add a `catalogue-update` recipe to `justfile` that runs `python3 -I scripts/catalogue_update.py`.
- [X] T005 Run `just catalogue-update` to produce the first seed in `assets/catalogue/`: `meta.json`, `api.json` and `logos/`. Commit it as `chore: add model catalogue seed` (depends on T003, T004).
- [X] T006 Extend `build.rs`, after the existing WebUI step:
  - Gzip `assets/catalogue/api.json` with `flate2` into `$OUT_DIR/catalogue.json.gz`, and emit `cargo:rerun-if-changed=assets/catalogue`.
  - If the file is missing or not valid JSON, `panic!` with a message telling the developer to run `just catalogue-update`.
  - Never access the network (depends on T002, T005).
- [X] T007 [P] Create `specs/015-provider-catalog/fixtures/stub_llm.py` per quickstart.md § Fixtures. It uses only the standard library (`http.server`, `json`, `argparse`), takes `--port`, and is run as `python3 -I`. It serves:
  - **`GET /v1/models`**: returns `stub-tools` and `stub-vision` in OpenAI list format.
  - **`POST /v1/chat/completions`**: OpenAI chat format. It replies with text `ECHO <compact JSON of the last user message's content parts>`, or, when the last user text starts with `CALL <tool> <json>`, with a single tool call. After a `role: tool` message, it replies `ECHO tool-result <content>`.
  - **`POST /v1/messages`**: the same behaviour in Anthropic Messages format (`tool_use` and `tool_result` blocks).
  - **Error injection**: when the request carries `X-Stub-Status: <code>` and optionally `X-Stub-Retry-After: <s>`, it fails the first request with that status and `Retry-After` header, then succeeds on the next.
  - **`GET /broken`**: returns `{}`.
  - **Request log**: every request body is printed to stderr, so the quickstart can inspect what was sent.

---

## Phase 2: Foundational (the provider layer and catalogue core, built alongside rig)

**Purpose**: All new code from plan Phase 1, with **nothing wired to it yet**. rig stays untouched and vizier behaves exactly as before. The golden fixtures are recorded here, while rig is still present.

**⚠️ CRITICAL**: No user-story work can begin until this phase is complete. US1 (the cutover) consumes everything here.

### `src/llm` core types and transport

- [X] T008 Create the `src/llm/` module and add `mod llm;` in `src/main.rs`. In `src/llm/types.rs`, implement exactly the types in contract `provider-layer.md` § Types:
  - `Message` with `System`, `User(Vec<Part>)`, `Assistant(Vec<AssistantPart>)` and `ToolResult { call_id, content: Vec<Part> }`. Its constructors (`Message::user_text`, `Message::user(parts)`, …) reject empty content with a `VizierError`.
  - `Part`: `Text`, `Attachment { mime, data: Vec<u8>, name }`.
  - `AssistantPart`: `Text`, `ToolCall { id, name, arguments, provider_data }`, `Reasoning { text, provider_data }`.
  - `ToolDefinition`, with serde output byte-identical to rig's `{name, description, parameters}`.
  - `ToolChoice`, and `Usage` with the field names `input_tokens`, `output_tokens`, `total_tokens`, `cached_input_tokens`, `cache_creation_input_tokens`.
  - `CompletionRequest` and `CompletionResponse`.
  - A unit test that `serde_json::to_value(ToolDefinition)` equals the JSON rig produces for the same values.
- [X] T009 [P] Create `src/llm/error.rs`:
  - `LlmError` with `Http { status, retry_after, body }`, `Transport`, `Timeout`, `Decode`, `Config` and `Unsupported`.
  - `impl From<LlmError> for crate::VizierError`.
  - `fn parse_retry_after(&HeaderValue) -> Option<Duration>`, accepting delta-seconds or an HTTP date, with unit tests (depends on T008).
- [X] T010 [P] Create `src/llm/http.rs`. It holds a shared `reqwest::Client`, built once and held on `VizierDependencies` (field added in T012) with `connect_timeout` 10s, and these functions:
  - `send_json(client, req: RequestBuilder) -> Result<(u16, HeaderMap, Bytes), LlmError>`, which maps non-2xx responses to `LlmError::Http` with `retry_after`, connect/reset/DNS/TLS failures to `Transport`, and timeouts to `Timeout`;
  - `send_multipart`, the same for multipart requests;
  - `redact(&str) -> String`, which hides `Bearer …`, `x-api-key`, `api-key`, `x-goog-api-key` and `key=` values for `tracing::debug!` request logs (depends on T009).
- [X] T011 [P] Create `src/llm/sse.rs`: `parse_sse_body(&str) -> Vec<(Option<String> event, String data)>`, which handles multi-line `data:` fields, comments and `[DONE]`, with unit tests. It is used only by the ChatGPT backend (research R6) (depends on T009).
- [X] T012 Add `pub http: reqwest::Client` to `VizierDependencies` in `src/dependencies.rs`, built with `llm::http::client()`. Nothing uses it yet (depends on T010).
- [X] T013 [P] Create `src/llm/auth.rs`, per contract `provider-layer.md` § Auth:
  - the `CredentialSource` trait (`authorize`, `invalidate`);
  - `StaticKey { style: Bearer | Header(&'static str) | Query(&'static str) | None, key }`;
  - `CopilotExchange`, ported from `~/.cargo/registry/src/*/rig-core-0.39.0/src/providers/copilot/auth/native.rs`. It GETs `https://api.github.com/copilot_internal/v2/token` with the GitHub token, caches the returned token until `expires_at` minus 60s behind a `tokio::sync::Mutex`, adds the editor headers rig sends, and `invalidate()` drops the cache;
  - `ChatGptAccount { access_token, account_id }`, which sends a bearer token plus the `chatgpt-account-id` header, as rig's `chatgpt` module does (depends on T010).

### The completion seam, request preparation and dialects

- [X] T014 Create `src/llm/model.rs`, per contract `provider-layer.md` § The completion seam:
  - `trait CompletionModel { complete, list_models, context_window }`.
  - `ListedModel`.
  - `struct HttpModel<D: Dialect> { dialect, resolved: Arc<ResolvedModel>, http }`. Its `complete` runs `prepare`, then `D::encode`, `credential.authorize`, `send_json` to `base_url + D::path`, and `D::read_body`, then `D::decode`. On a 401 from a source whose `invalidate` cleared a cached token, it retries once.
  - `RetryingModel<M>`: **move** the retry logic from `src/agents/agent/model/retry.rs` into it. That covers backoff, `attempt_timeout`, the deadline, the `PROMPT_TIMED_OUT`/`RATE_LIMITED`/`PROVIDER_UNAVAILABLE` prefixes, and `complete_until(req, deadline)`. Classify on `LlmError` per the contract's table, apply `classify_text` only to `Decode`, and wait `max(backoff, retry_after)`, capped by the deadline.
  - Keep `retry.rs`'s unit tests, ported to the new module. Leave `retry.rs` in place until T060 (depends on T008, T010, T013).
- [X] T015 Create `src/llm/dialect/mod.rs`:
  - the `Dialect` trait (`path`, `encode`, `decode`, `read_body`, `list_models_path`, `decode_models`);
  - `enum DialectKind { OpenAiChat, OpenAiResponses, Anthropic, Gemini, Ollama }`;
  - `enum SystemStyle { Field, Inline, Instructions }`, with `fn system_style()` on `Dialect`;
  - `BodyKind { Json, Sse }`;
  - `PreparedRequest` (depends on T008).
- [X] T016 Create `src/llm/prepare.rs`, per contract `provider-layer.md` § Request preparation, with unit tests for each rule: `prepare(req, &ResolvedModel, SystemStyle) -> PreparedRequest`.
  - **Attachments**: images are kept unless `caps.image_input == Some(false)`. PDFs are kept only when the input modalities contain `pdf`. Audio is always replaced. A replaced attachment becomes `Part::Text("[image attached: <name>, but this model cannot view images]")`, or a similar note for a document or audio file.
  - **`normalize_system`**:
    - `Field`: leading `System` messages are joined with `\n\n` into `system`, and later ones become a user `Part::Text("<system>…</system>")`.
    - `Inline`: unchanged.
    - `Instructions`: leading messages go to `system`; later ones are kept for the Responses dialect to emit as developer messages (depends on T015).
- [X] T017 [P] Implement `src/llm/dialect/openai_chat.rs` (`OpenAiChat`, `SystemStyle::Inline`), per the contract's dialect table:
  - `encode`: messages to `role: system|user|assistant|tool`. Attachments become `image_url` data URLs, or `file` parts for PDFs. Each `ToolResult` is its own `role: tool` message with `tool_call_id`. Assistant `ToolCall`s become `tool_calls[{id, type: function, function: {name, arguments: <stringified>}}]`. `Reasoning` with `provider_data.reasoning_content` is sent back as `reasoning_content`. Also `tools[]`, `tool_choice: "auto"|"none"`, and `max_tokens` when set.
  - `decode`: `choices[0].message` to parts, including `reasoning_content` as `Reasoning`; `usage` including `prompt_tokens_details.cached_tokens`; an error object inside a 2xx is `LlmError::Decode`.
  - `list_models_path: "/models"`.
  - Port each field choice from `rig-core-0.39.0/src/providers/openai/completion/` and `…/deepseek.rs`.
  - Fixture tests in `src/llm/dialect/fixtures/openai_chat/` cover: text, a tool call, parallel tool calls, a tool result, an image, reasoning round-trip, `tool_choice none`, and cached usage (depends on T016).
- [X] T018 [P] Implement `src/llm/dialect/anthropic.rs` (`Anthropic`, `SystemStyle::Field`):
  - path `/v1/messages`, plus the `anthropic-version: 2023-06-01` header;
  - consecutive `ToolResult`s grouped into one `user` turn of `tool_result` blocks;
  - `tool_use` blocks;
  - thinking blocks round-tripped, with the `signature` in `provider_data`;
  - `max_tokens` always set: `req.max_tokens`, else `resolved.max_output`, else 2048;
  - **automatic prompt caching**: `cache_control: {type: ephemeral}` on the last tool, the system prompt and the last message block, ported from rig's `with_automatic_caching`;
  - `usage` read from `input_tokens`, `cache_read_input_tokens` and `cache_creation_input_tokens`;
  - `list_models_path: "/v1/models"`.

  Port from `rig-core-0.39.0/src/providers/anthropic/completion.rs`. Fixture tests as in T017, plus a cache-control placement test (depends on T016).
- [X] T019 [P] Implement `src/llm/dialect/gemini.rs` (`Gemini`, `SystemStyle::Field`):
  - path `/models/{model}:generateContent`, with the key in the `x-goog-api-key` header (via `StaticKey::Header`);
  - `systemInstruction`, `contents[]` with roles `user`/`model`;
  - `functionDeclarations`, with parameters sanitised the way rig's `gemini` module does (strip `$schema`, `$ref`/`definitions` by inlining, `additionalProperties`, unsupported `format`s);
  - `functionCall` / `functionResponse`, with consecutive results grouped;
  - `thoughtSignature` kept in `provider_data`;
  - `inlineData` for attachments;
  - `toolConfig.functionCallingConfig.mode = NONE` for `ToolChoice::None`;
  - `usageMetadata` read from `promptTokenCount`, `candidatesTokenCount` and `cachedContentTokenCount`;
  - `list_models_path: "/models"`, decoding `inputTokenLimit` as the context window.

  Port from `rig-core-0.39.0/src/providers/gemini/completion.rs`, and test schema sanitising with a fixture taken from a real MCP tool schema (depends on T016).
- [X] T020 [P] Implement `src/llm/dialect/ollama.rs` (`Ollama`, `SystemStyle::Inline`):
  - path `/api/chat`, with `stream: false`;
  - `tools[]` / `tool_calls[]`, giving each call a synthesised id `call_<n>` when Ollama omits one, as rig does;
  - `role: tool` results;
  - `images[]` as base64;
  - `thinking` round-trip;
  - usage read from `prompt_eval_count` and `eval_count`;
  - `list_models_path: "/api/tags"`;
  - plus `fn show_context(base_url, model)`, which POSTs `/api/show` and reads `model_info.*.context_length`.

  Port from `rig-core-0.39.0/src/providers/ollama.rs` (depends on T016).
- [X] T021 [P] Implement `src/llm/dialect/openai_responses.rs` (`OpenAiResponses`, `SystemStyle::Instructions`):
  - path `/responses`, always `store: false`;
  - `instructions` from the leading system prompt; later system messages become `role: developer` input items;
  - input items: `message`, `function_call` (with `call_id` = `ToolCall.id`), `function_call_output`, and reasoning items carrying `encrypted_content` from `provider_data`, requested with `include: ["reasoning.encrypted_content"]`;
  - `tool_choice`;
  - `decode` handles the `output[]` items, and the usage fields `input_tokens`, `input_tokens_details.cached_tokens` and `output_tokens`;
  - `read_body` returns `Sse` when `resolved.stream_required` is set, which is the case for ChatGPT. The SSE body is aggregated through `sse::parse_sse_body` into the final `response.completed` payload, as rig's `responses_api::streaming::parse_sse_completion_body` does.

  Port from `rig-core-0.39.0/src/providers/openai/responses_api/` and `…/chatgpt/mod.rs` (depends on T016, T011).

### Media clients (implementing vizier's existing traits directly)

- [X] T022 [P] Create `src/llm/media/openai_shaped.rs`: `OpenAiShaped { http, base_url, credential, model }`, implementing these existing traits:
  - `crate::stt::VizierSttModel`: multipart `/audio/transcriptions` with `file`, `model` and optional `language`;
  - `crate::tts::VizierTtsModel`: `/audio/speech` with `{model, input, voice, speed}`, passed through the existing `tts::mp3_to_wav`;
  - `crate::image_generation::VizierImageGenModel`: `/images/generations` with `{model, prompt, size: "WxH"}`, decoding `b64_json` or fetching `url`, and returning `image/png`;
  - `crate::embedding::VizierEmbeddingModel`: `/embeddings` with `{model, input: [..]}`, returning `Vec<Vec<f64>>` in input order.

  Port request fields and defaults from the matching rig 0.39 modules for openai, groq, mistral, openrouter, xai, together, copilot and voyageai (depends on T010, T013).
- [X] T023 [P] Create these media clients, each implementing the existing `Vizier*Model` trait(s), ported from the corresponding rig 0.39 modules:
  - `src/llm/media/gemini.rs`: STT by sending inline audio to `generateContent` with a transcription prompt, as rig's gemini transcription does; embeddings via `batchEmbedContents`;
  - `src/llm/media/cohere.rs`: embeddings via `/v2/embed` (`input_type: search_document`, `embedding_types: ["float"]`), as rig does;
  - `src/llm/media/ollama.rs`: embeddings via `/api/embed`;
  - `src/llm/media/hyperbolic.rs`: TTS and image generation, using the endpoints and bodies from `rig-core-0.39.0/src/providers/hyperbolic.rs`;
  - `src/llm/media/huggingface.rs`: STT and image generation via the inference API, as in `rig-core-0.39.0/src/providers/huggingface/` (depends on T010, T013).

### Catalogue core and provider ids

- [X] T024 Create `src/catalogue/data.rs` with the types from data-model.md §2: `CatalogueData`, `CatalogueProvider`, `CatalogueModel`, `Capabilities`, `Limits` (where `0` becomes `None`), `Cost`, `ModelStatus`, `RouteOverride`, `CatalogueSource { Bundled, Cached }` and `ProviderKind { Cloud, Local, Media }`. Implement:
  - `fn parse(json: &[u8]) -> Result<CatalogueData>`: skips (and `debug!`-logs) individual bad models and ignores unknown fields;
  - `fn dialect_for(provider, model) -> Option<(DialectKind, Option<String> api)>`: the research R2 mapping, including per-model `provider.{npm, api, shape}` overrides;
  - the research R3 default base-URL table;
  - `ProviderKind::Local` when the `api` host is `localhost` or `127.0.0.1`.

  Unit tests against a small fixture JSON cover: an override wins; an unsupported provider gives `None`; a missing `api` uses the R3 table; `limit.input` is preferred (depends on T015).
- [X] T025 [P] Create `src/catalogue/builtin.rs`: the built-in providers from data-model.md §3 (`ollama`, `llama-cpp`, `chatgpt`, `dummyplug`, `elevenlabs`, `voyageai`, `hyperbolic`), each with its kind, dialect (or none), credential fields, env vars and defaults (depends on T024).
- [X] T026 Create `src/catalogue/seed.rs`, which embeds the seed: `include_bytes!(concat!(env!("OUT_DIR"), "/catalogue.json.gz"))`, plus a `rust_embed` folder for `assets/catalogue/meta.json` and `logos/`. Implement:
  - `fn load_seed() -> CatalogueData`;
  - `fn seed_logo(id) -> Option<&'static [u8]>`;
  - `fn seed_no_logo(id) -> bool`.

  Add a unit test that the seed parses and has at least 200 providers (depends on T006, T024).
- [X] T027 Create `src/catalogue/mod.rs`, holding `pub struct Catalogue(Arc<parking_lot::RwLock<Arc<CatalogueData>>>)` with:
  - `snapshot() -> Arc<CatalogueData>`;
  - `swap(new)`;
  - query helpers for later phases:
    - `providers(filter)`: selectable providers (supported dialects, plus built-ins and custom providers passed in);
    - `models(provider_id, all: bool)`: the FR-031 default list and its ordering;
    - `model(provider_id, model_id)`;
    - `find_model_by_id(model_id, prefer_dialect)`, for custom fill-in (FR-024).

  Add `pub catalogue: Catalogue` to `VizierDependencies` in `src/dependencies.rs`, initialised from `load_seed()`. The DB cache comes in US4 (depends on T026).
- [X] T028 Replace `ProviderVariant` with `ProviderId` in `src/config/provider.rs`:
  - a `pub struct ProviderId(String)` newtype that validates `^[a-z0-9][a-z0-9._-]{0,63}$`;
  - `Serialize` as a plain string, and a `Deserialize` that maps legacy names through the full table in contract `provider-id-migration.md` § Name map (e.g. `gemini`→`google`, `opencode_zen`→`opencode`, `copilot`→`github-copilot`, `together`→`togetherai`, `mimo`→`xiaomi`, `moonshot`→`moonshotai`, `llama_cpp`→`llama-cpp`);
  - `galadriel` and `mira` are kept as given;
  - `utoipa::ToSchema` as a string;
  - `pub const fn` constants or `ProviderId::new_static` for the built-in ids used in code (`ProviderId::OLLAMA`, `DUMMYPLUG`, …).

  Keep a `type ProviderVariant = ProviderId;` alias **only** until T049, so the tree still compiles. Unit tests: every legacy name maps as in the contract, and invalid ids are refused (depends on T025).
- [X] T029 Create `src/llm/resolve.rs`: `ResolvedModel` per data-model.md §6, plus `stream_required: bool` and `system_style` coming from the dialect. Implement `async fn resolve(deps, &ProviderId, model_id, overrides: &ModelOverrides) -> Result<ResolvedModel, VizierError>`:
  - **Provider lookup**: custom provider (T035 storage, behind a trait method that returns `None` until then), then catalogue, then built-in. Anything else errors with `provider "<id>" is not available; pick another provider`.
  - **Model**: catalogue model, else the custom model with FR-024 fill-in. An unknown model id is allowed.
  - **Dialect and base URL**: from `dialect_for` or the built-in, with `${VAR}` expanded from env or saved credentials (Azure).
  - **Credential source** (contract `provider-layer.md` § Auth): `StaticKey` with the header style the dialect needs; `CopilotExchange` for `github-copilot`; `ChatGptAccount` for `chatgpt`.
  - **Endpoint rules**: Azure's URL template, and the `x-opencode-session` header for `opencode-go`, minted once per resolve.
  - **Context window and max output**: the research R8 order.
  - **Capabilities and cost.**

  Credential lookup follows data-model §4 (saved, then catalogue `env[]`, then legacy env vars), using the existing `provider_keys.rs` functions for now. Unit tests cover: context-window order (explicit → `limit.input` → `limit.context` → none); max-output rules; route override; Azure expansion (depends on T013, T027, T028).
- [X] T030 Create `src/llm/mod.rs` with `pub fn build(resolved: ResolvedModel, http, retry: CompletionRetryConfig) -> Arc<dyn CompletionModel>`. It matches `DialectKind` once, as the module constructor does, to construct `HttpModel<D>`, wrapped in `RetryingModel`. Re-export the public types (depends on T014, T017–T021, T029).

### Golden fixtures from rig (rig still present)

- [X] T031 Add a temporary `#[cfg(test)] mod golden_capture` in `src/llm/golden_capture.rs`. Its single `#[ignore]` test starts an axum capture server on `127.0.0.1:0` that records request bodies and returns canned responses. It points the **rig 0.39** clients at that server through their `base_url` builders: openai Responses, OpenAI chat (`CompletionsClient`), anthropic, gemini, ollama, deepseek, the ChatGPT SSE path, and the rig media clients for each `OpenAiShaped`/gemini/cohere/ollama/hyperbolic/huggingface path. For every scenario in T017–T023 it writes these files to `src/llm/dialect/fixtures/golden/<dialect>/<scenario>.{request,response,parsed}.json`:
  - the request body rig sent;
  - the canned response;
  - rig's parsed result, normalised to `llm::types` JSON.

  For embeddings it also records the returned vectors. Run it once with `cargo test -- --ignored golden_capture` and commit the fixtures (depends on T017–T023).
- [X] T032 [P] Add `src/llm/dialect/golden_tests.rs`. It loads every fixture under `fixtures/golden/` and asserts that our `encode` produces a request body semantically equal (as JSON values) to rig's, after normalising documented intentional differences, which are listed in a comment with the reason for each (e.g. `store: false`). It also asserts that our `decode` of the canned response equals rig's parsed result. For media, it uses an axum server to replay the canned responses and compares our output, with vectors equal element-for-element (FR-020) (depends on T031).
- [X] T033 [P] Add transport tests in `src/llm/transport_tests.rs` (`#[cfg(test)]`), using an axum server on `127.0.0.1:0`:
  - a 429 with `Retry-After: 2` makes `RetryingModel` wait at least 2s and then succeed;
  - 500, 503 and 529 are retried; 400 fails at once;
  - a refused connection (an unused port) gives `Transport`;
  - a slow handler exceeding `attempt_timeout` gives `Timeout` and is retried;
  - the SSE body aggregation works;
  - `CopilotExchange`: a 401 triggers one token refresh, then success (depends on T014, T030).

**Checkpoint**: `cargo test` passes with the new layer fully unit-tested. vizier still runs on rig, unchanged.

---

## Phase 3: User Story 1 - Existing deployments keep working after upgrade (Priority: P1) 🎯 MVP

**Goal**: The cutover. Every call site moves to `src/llm`, legacy provider names and credentials are migrated, YAML `providers` is retired, and **rig is deleted**. Agents behave as before (FR-014–FR-020, FR-026, FR-028).

**Independent Test**: quickstart.md §1: start the new build on a workspace created by `master` that has dummyplug, gemini, custom, galadriel and moonshot (`.cn`) agents.
- Healthy agents stay healthy, under their new ids.
- `gal-old` fails with the "not available" message.
- The migration logs once.
- The 008 and 014 dummyplug scripts pass unchanged.
- §5: a `Retry-After` from the stub is honoured.

### Storage and migration

- [X] T034 [US1] In `src/schema/provider.rs`:
  - Replace the 29-variant `ProviderEntryConfig` with `pub struct ProviderCredentials { api_key, endpoint, base_url, access_token, account_id }`, all `Option<String>`.
  - Change `ProviderEntry` to `{ id: ProviderId, credentials: ProviderCredentials }`.
  - Keep a private `LegacyProviderEntryConfig` (the old tagged enum, copied verbatim) used **only** by the migration in T037.
  - Add `CustomProvider`, `CustomProtocol { OpenAiCompatible, Anthropic }` and `CustomModel`, per data-model.md §5.
- [X] T035 [US1] In `src/storage/provider.rs` and `src/storage/sqlite/provider.rs`:
  - Re-key `ProviderStorage` by `&ProviderId`, reading and writing `ProviderCredentials` JSON in `provider_config(variant, data)`.
  - Add `trait CustomProviderStorage { list_custom_providers, get_custom_provider, upsert_custom_provider, delete_custom_provider }`, backed by a new table `custom_provider(id TEXT PRIMARY KEY, data TEXT NOT NULL)`, created in the schema init in `src/storage/sqlite/mod.rs`.
  - Compose `CustomProviderStorage` into `VizierStorageProvider` and forward it in `VizierStorage`, in `src/storage/mod.rs`.
  - Update the read-only `src/storage/fs/provider.rs` legacy reader so it still compiles. It returns legacy rows for the migration only (depends on T034).
- [X] T036 [US1] Collapse `src/provider_keys.rs` into `async fn resolve_credentials(storage, &ProviderId, catalogue_env: &[String]) -> Option<(ProviderCredentials, CredentialOrigin)>`, where `CredentialOrigin` is `Saved` or `Env(name)`. The order is the saved row, then each catalogue env var, then the legacy env vars from contract `provider-id-migration.md`. Keep the ChatGPT/Azure/local-provider special fields. Delete the per-variant `resolve_*` functions once T029's resolve uses this (depends on T035).
- [X] T037 [US1] Add `migrate_provider_ids_v1` to `src/dependencies.rs`. It is idempotent, guarded by the `global_config` key `migration.provider_ids_v1`, and implements contract `provider-id-migration.md` exactly:
  - read legacy `provider_config` rows through `LegacyProviderEntryConfig`;
  - map names, applying the base-URL rule against catalogue `api` values;
  - write `ProviderCredentials` rows under the new ids;
  - create custom providers for the legacy `custom` row (or the `CUSTOM_BASE_URL`/`CUSTOM_API_KEY` env vars) and for unmatched base URLs (`<legacy>-custom`);
  - delete `galadriel` and `mira` credential rows;
  - rewrite `agent_config.data` (`provider`, `dream_provider`, `tools.read_image.settings.provider`);
  - log each change at `info` (`warn` for custom-provider creation).

  Run it in the startup migration sequence after storage opens, before agents load. Unit tests use an in-memory SQLite database seeded with legacy rows covering every row of the name map plus both footnotes (depends on T035, T036, T027).
- [X] T038 [US1] Retire YAML providers (FR-028), per contract `provider-id-migration.md` § YAML:
  - In `src/config/mod.rs`, remove `VizierConfig.providers`, but keep a `#[serde(default)] providers: Option<serde_yaml::Value>` captured only to detect leftovers. Remove `ProviderConfig` and its `*ProviderConfig` structs from `src/config/provider.rs`.
  - Delete `migrate_providers` from `src/dependencies.rs`.
  - At startup, when the captured value is a non-empty map, `warn!` once per key, naming the catalogue env var(s) for that provider and "Settings › Providers".
  - Update `src/cli/onboard.rs` to stop writing `providers`. After the provider choice, print the env var to set (catalogue `env[0]`, or the built-in's) and that the key can be added in Settings › Providers.
  - Remove the `providers:` blocks from the repo's example config files, if any are tracked.

### Call-site cutover

- [X] T039 [US1] Move the message types onto `crate::llm::types` in `src/schema/request.rs`, `src/schema/response.rs` and `src/agents/agent/system_prompt/context.rs`:
  - `VizierRequest::to_message` builds `Message::User(vec![Part::Text(prompt), Part::Attachment{mime, data, name}…])`, and attachment reading keeps its existing file-loading code.
  - `VizierResponse` tool-result conversion produces `Message::ToolResult`.
  - `with_context` operates on `Message::System`.
  - Remove `OneOrMany` and the rig `use` lines.
- [X] T040 [US1] Rewrite `src/schema/history.rs` conversions for the one-to-one mapping (contract S4):
  - `messages_to_history_entries`: `Assistant` parts become `AssistantMessage`/`Response`/`ToolCall` entries, with the same narration-before-calls ordering rule (FR-011 of spec 010); `Message::ToolResult` becomes `ToolResult`; `Reasoning` is skipped.
  - `history_entries_to_messages`: consecutive `ToolCall` entries plus the pending narration become one `Assistant` message; each `ToolResult` becomes one `Message::ToolResult`.
  - Delete the grouping and ungrouping of tool results inside user messages, and port the module's existing unit tests (depends on T039).
- [X] T041 [US1] Switch `ToolDefinition` to `crate::llm::types::ToolDefinition` in `src/agents/tools/mod.rs`, `src/agents/mcp/mod.rs` and `src/sandbox/docs.rs`. Run the 014 unit tests: the `ToolFunctionDoc` output must be unchanged.
- [X] T042 [US1] Replace `src/agents/agent/model/mod.rs`:
  - `VizierModel` becomes a thin `Clone` wrapper around `Arc<dyn llm::CompletionModel>`.
  - `VizierModel::new(agent_id, deps, &AgentConfig)` = `llm::resolve(deps, &config.provider, &config.model, &ModelOverrides{context_window, max_tokens})`, then `llm::build(resolved, deps.http.clone(), config.completion_retry.clone())`. Dummyplug is special-cased only by being the built-in whose builder returns `DummyplugModel`.
  - `new_with_override` does the same with an overridden provider and model.
  - Expose `complete(req)` and `complete_until(req, deadline)`.
  - Delete `fetch_context_window_from_api`, `VizierModelBuilder`, `VizierModelImpl`, `resolve_provider` and both 25-arm matches.
  - **Delete** `src/agents/agent/model/provider.rs` (depends on T030, T036).
- [X] T043 [US1] Port `src/agents/agent/model/dummyplug.rs` to `impl llm::CompletionModel for DummyplugModel`, reading tool results from `Message::ToolResult` and replying with `AssistantPart`. The dispatch rules and the reply text must stay byte-identical (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`), and its unit tests are ported unchanged (depends on T042).
- [X] T044 [US1] Update `src/agents/agent/mod.rs` and `src/agents/process.rs`:
  - Build `CompletionRequest { messages: system + history + new message, tools, tool_choice, max_tokens: None }`.
  - Call `model.complete_until(req, deadline)`; the "answer only" path sets `tool_choice: Some(ToolChoice::None)` (contract S7).
  - Read `CompletionResponse.content`/`usage`/`id` in place of the old tuple.
  - Iterate `AssistantPart::ToolCall { id, name, arguments }`, and append `Message::ToolResult { call_id, content }`.
  - Keep `Reasoning` parts in the turn's in-memory message list, so they round-trip within the turn.
  - Port the `mod.rs` unit test at about line 1573 (depends on T040–T043).
- [X] T045 [US1] Update `src/agents/tools/read_image/mod.rs` to build its single-turn vision request as `CompletionRequest` with a `Part::Attachment`, and to read the text from `AssistantPart::Text`. Update `src/agents/tools/mod.rs`'s `build_read_image_vision` to use `VizierModel::new_with_override` with `ProviderId` (depends on T042).
- [X] T046 [US1] Rewrite the media factories to build `src/llm/media` clients, and delete their per-provider rig adapter files:
  - **`src/embedding/mod.rs`**: delete `cohere.rs`, `copilot.rs`, `gemini.rs`, `mistral.rs`, `ollama.rs`, `openai.rs`, `openrouter.rs`, `together.rs` and `voyageai.rs`.
  - **`src/stt/mod.rs`**: delete `openai.rs`, `groq.rs`, `mistral.rs`, `gemini.rs` and `huggingface.rs`.
  - **`src/tts/mod.rs`**: delete `openai.rs`, `openrouter.rs`, `xai.rs` and `hyperbolic.rs`.
  - **`src/image_generation/mod.rs`**: delete `openai.rs`, `xai.rs`, `hyperbolic.rs` and `huggingface.rs`.

  Each factory resolves credentials through `fn credentials(&self) -> ProviderId` on its enum (`SttProvider`, `TtsProvider`, `ImageGenProvider`, `EmbeddingProvider` in `src/schema/agent.rs`), implemented exactly as the data-model.md §8 table, and `provider_keys::resolve_credentials`. Base URLs come from the catalogue or the R3 table for that provider id.

  Port `src/embedding/fastembed.rs` to implement `VizierEmbeddingModel` directly, dropping the rig `EmbeddingModel` impl. Remove the `expect("failed to create … client")` calls. `elevenlabs.rs`, `whisper.rs` and `kokoro.rs` are unchanged (depends on T022, T023, T036).
- [X] T047 [US1] Remove `EmbeddingConfig.api_key` and `base_url` from `src/schema/agent.rs`, and any reader. There is none today, so stored keys are simply ignored by serde (data-model.md §7).
- [X] T048 [US1] Update `src/agents/supervisor.rs`:
  - Ollama model pull before build uses the `ollama` built-in's resolved `base_url`, from `ProviderCredentials` or `OLLAMA_BASE_URL`.
  - A `resolve` failure with "not available" becomes the agent's `last_error`, through the existing `Failed` path (Story 1, scenario 4).
- [X] T049 [US1] Replace every remaining `ProviderVariant` use with `ProviderId`, in:
  - `src/schema/agent.rs` (`provider`, `dream_provider`, `ReadImageToolSettings.provider`);
  - `src/agents/tools/mod.rs`;
  - `src/channels/http/api/v1/agents/mod.rs`;
  - `src/channels/http/api/v1/providers/mod.rs`: routes keyed by `Path<ProviderId>`; `GET` lists stored credentials only; the dummyplug `PUT` returns 400 and `GET` returns 404 as before; `PUT` accepts `ProviderCredentials`. Full behaviour comes in US5.

  Then delete the `ProviderVariant` alias from T028. `grep -rn ProviderVariant src` must be empty (depends on T042–T048).

### Retry, errors and rig removal

- [X] T050 [US1] Delete `src/agents/agent/model/retry.rs`. Its logic lives in `llm::model::RetryingModel` (T014). Keep re-exports of `PROMPT_TIMED_OUT`, `RATE_LIMITED` and `PROVIDER_UNAVAILABLE` from `src/agents/agent/model/mod.rs`, so `ErrorKind::classify` and its callers are unchanged (depends on T044).
- [X] T051 [US1] Delete `src/llm/golden_capture.rs` (T031). The committed fixtures and `golden_tests.rs` stay as regression tests with no rig involved (depends on T032).
- [X] T052 [US1] Remove `rig-core` from `Cargo.toml` and the `rig` log directive from `src/main.rs`. Then confirm:
  - `cargo tree | grep -c rig-core` prints `0`;
  - `grep -rn "rig_core\|rig::" src` is empty;
  - `cargo build`, `cargo test` and `cargo clippy` pass;
  - the `cargo tree -e normal | wc -l` delta is recorded in the PR (depends on T039–T051).
- [X] T053 [US1] Run quickstart.md §1 by hand: upgrade a `master` workspace, check the log lines, health, `vizier agent ps` and provider rows, restart for idempotence, and confirm the old-client `PUT`. Run §5 (Retry-After via `stub_llm.py`) and the 008 and 014 dummyplug scripts, all unchanged. Record the results in the PR (depends on T052).

**Checkpoint**: rig is gone. Every pre-existing agent works on the new layer. This is the MVP: it ships issue #69 on its own.

---

## Phase 4: User Story 3 - Agents use accurate model limits and capabilities (Priority: P1)

**Goal**: Catalogue context windows and output limits are effective and visible; capability warnings are shown on save; images are handled by capability; read-image configures itself (FR-012, FR-013, FR-033).

**Independent Test**: quickstart.md §2 (the `ant-1` context-window and override rows, and the warning rows) and §3 (the `img-yes`/`img-no` agents against `stub_llm.py`).

- [X] T054 [US3] Add `model: Option<AgentModelInfo { provider: ProviderId, model: String, context_window: Option<u64>, max_output: Option<u64>, context_source: "override"|"catalogue"|"listing"|"name"|"unknown" }>` to `AgentHealthStatus` in `src/schema/commands.rs`. Fill it in `src/agents/supervisor.rs` from the agent's `ResolvedModel` when it is built, and show it in `vizier agent ps` (`src/cli/agent.rs`).
- [X] T055 [US3] In `src/llm/resolve.rs`, complete the R8 fallback chain past the catalogue: when the catalogue and overrides give no context window, call `list_models` on the built model (OpenRouter, OpenAI-style `/models` with `context_length`) or `ollama::show_context`, once per build, with a 5s timeout. Then fall back to `agents::agent::model::registry::detect_context_window`. Record `context_source` (depends on T054).
- [X] T056 [US3] Implement the FR-033 read-image rules in `src/agents/tools/mod.rs` (`build_read_image_vision`) and `src/agents/agent/mod.rs`, per the data-model.md §7 table:
  - read-image enabled, no explicit vision model, and the main model's `caps.image_input != Some(false)`: images stay as attachments for the main model, and no vision model is built;
  - the same with `image_input == Some(false)`: `prepare` replaces the image with the note (T016), and the agent health gets the warning `read_image: main model cannot view images; configure a vision model`;
  - an explicit vision model keeps today's behaviour.
- [X] T057 [US3] Add `warnings: Vec<AgentWarning { code, message }>` to the create and update agent responses in `src/channels/http/api/v1/agents/mod.rs`, computed from the catalogue for `provider`/`model`:
  - `no_tool_calling` when `tool_call == Some(false)`;
  - `no_image_input` when read-image is enabled with no vision model and `image_input == Some(false)`;
  - `unknown_model` when the model is not in the catalogue and is not a custom-provider model.

  Warnings never block a save (FR-013, contract `http-api.md` § Agents).
- [X] T058 [US3] Run quickstart.md §2 (the context-window and warning rows) and §3 against `stub_llm.py`, and record the results in the PR (depends on T054–T057, and on T066 for the custom provider `stub`, so run it after US6's API or create the provider through the storage API in a test).

**Checkpoint**: US1 and US3 both work. Agents use accurate limits, and images never fail a turn.

---

## Phase 5: User Story 2 - Operator picks a provider and model from an accurate catalogue (Priority: P1)

**Goal**: A read-only catalogue API with logos, the single merged agent form (a prerequisite, FR-029), and one shared provider/model picker replacing the hard-coded lists (FR-001, FR-005, FR-005a, FR-006–FR-011, FR-030).

**Independent Test**: quickstart.md §2 (the catalogue endpoint rows) and §7 (the "One form", "Picker" and "Defaults" bullets), offline.

### Catalogue read API

- [X] T059 [US2] Create `src/channels/http/api/v1/catalogue.rs` with `GET /catalogue`, `GET /catalogue/providers`, `GET /catalogue/providers/{id}/models` and `GET /catalogue/providers/{id}/models/{model}`, with the exact response shapes in contract `http-api.md` § Catalogue:
  - `connection` from `resolve_credentials`: status and source label only, never a secret;
  - `fields` from the provider kind and built-in definition;
  - sort order: connected, then custom, then "popular" (the legacy-mapped ids), then A–Z;
  - for `kind: local` providers, the server's live models are merged in, with `origin: "server"`.

  Mount it in `src/channels/http/api/v1/mod.rs` under `require_auth` only, and add the utoipa annotations. The lazy refresh trigger is added in US4.
- [X] T060 [US2] Add `GET /catalogue/logos/{id}.svg` to `src/channels/http/api/v1/catalogue.rs`, **outside** `require_auth`. It serves the seed logo (`seed_logo`), or `404` for `seed_no_logo` and unknown ids, with exactly the headers in contract `catalogue-snapshot.md` § Logos (CSP, `nosniff`, `image/svg+xml`, `Cache-Control`). The DB-cached and lazily fetched logos come in US4.

### Agent form merge (FR-029; prerequisite for the picker)

- [ ] T061 [US2] **Merge the forms**, as a `refactor:` commit with no behaviour change:
  - In `webui/app/components/AgentForm.tsx`, implement `mode="edit"` fully. Load `initialData`, keep Agent ID read-only in edit mode, replace the per-tab Save buttons with one sticky "Save changes" bar shown when there are unsaved edits, and accept `extraTabs: {key, label, icon, render}[]`.
  - Port every settings-only field and behaviour from `webui/app/routes/agent-settings.tsx` into `AgentForm`, after a field-by-field comparison of the two files' Config, Prompt and Tools tabs.
  - Reduce `agent-settings.tsx` to loading the agent, rendering `<AgentForm mode="edit" … extraTabs={[Sharing, Danger]}>`, and keeping its existing Sharing and Danger Zone sections as the extra tabs.
  - Verify with `npm run typecheck` and a manual walk through every setting on `/agents/<id>/settings`, saving and reloading.
- [ ] T062 [P] [US2] Add `webui/app/components/ProviderLogo.tsx`: `<img src="/api/v1/catalogue/logos/{id}.svg" alt={name}>` at a fixed box size. `onError` falls back to an initials badge of the same size. A dark-theme `filter: invert(1)` applies to logos marked as `currentColor`, with a prop for the 10 full-colour ones, read from `/catalogue/providers` `logo` metadata.
- [ ] T063 [US2] Add the catalogue API calls and types to `webui/app/services/vizier.tsx` and `webui/app/interfaces/types.ts`, matching contract `http-api.md`: `getCatalogue`, `listCatalogueProviders`, `listCatalogueModels(id, all)` and `getCatalogueModel`. Change the `provider` type on agent requests from the `ChatProvider` union to `string`.
- [ ] T064 [US2] Add `webui/app/components/ProviderModelPicker.tsx`, a provider dropdown plus a model dropdown:
  - **Providers**: searchable by name and id; grouped CONNECTED / CUSTOM / ALL PROVIDERS; each row shows the logo and connection state.
  - **Models**: searchable; fetched with `all=false`, plus a "Show all models" toggle; each row shows the lab logo, context, a 🔧👁💭 capability row and price; deprecated models are badged; filtering is client-side with a virtualised list, so OpenRouter's 380+ models filter within 2s (SC-008); a model id that isn't listed can be typed in (FR-011).
  - **Below the selector**: the selected model's card (limits, capabilities, price, knowledge).
  - **Props**: `value {provider, model}`, `onChange`, `filter?: {toolCallOnly?, imageInputOnly?}`, `preselectDefault?: boolean`.
- [ ] T065 [US2] Use `ProviderModelPicker` for the main model in `AgentForm.tsx`, and for the dream model (with `filter.toolCallOnly`) and the read-image vision model (with `filter.imageInputOnly`). Delete `CHAT_PROVIDERS`, `CHAT_PROVIDER_MODELS`, `CHAT_PROVIDER_DEFAULT_MODELS` and `chatProviderLabel` from `webui/app/interfaces/types.ts`, plus all their imports. The TTS, STT, image-generation and embedding curated lists stay.

**Checkpoint**: one form, with catalogue-driven pickers that work offline.

---

## Phase 6: User Story 6 - Operator adds their own custom providers (Priority: P2)

**Goal**: CRUD, test and fetch-models for custom providers, with OpenAI-compatible and Anthropic protocols, wired into resolve and the UI (FR-022–FR-027).

**Independent Test**: quickstart.md §4, against `stub_llm.py`.

- [X] T066 [US6] Add `src/channels/http/api/v1/providers/custom.rs` with every endpoint in contract `http-api.md` § Custom providers: `GET`, `POST`, `PUT`, `DELETE` (with `?confirm=true` and a 409 that lists the agents using it), `POST /{id}/fetch-models` and `POST /test`.
  - Validate ids with the `ProviderId` regex. A clash with a catalogue, built-in or other custom id returns 409, and the message names the clash.
  - The id is immutable on `PUT`; a different id in the body returns 400.
  - An omitted `api_key` or header value keeps the stored value; a header value set to `null` removes it.
  - A row that differs from the stored one is saved with `edited: true`.
  - Responses never contain the key or header values.
  - Fetch-models merges by id as data-model.md §5 describes: add new ids, keep edited rows, refresh the others. Unit-test the merge.

  Mount it under `/providers/custom` with `settings:providers` (depends on T035).
- [X] T067 [US6] Wire custom providers into `src/llm/resolve.rs`: look up `CustomProviderStorage` first. The dialect is `OpenAiChat` for `OpenAiCompatible` and `Anthropic` for `Anthropic`; the base URL and headers come from the record, with `StaticKey` Bearer (OpenAI) or `x-api-key` (Anthropic) when a key is set. Model data follows FR-024: the custom row's fields, then `catalogue.find_model_by_id(model, prefer_dialect)`, then name detection. Add a unit test for the fill-in.
- [X] T068 [US6] Handle catalogue id collisions in `src/catalogue/mod.rs` `providers()`: a catalogue provider whose id equals a custom provider's id is hidden, and the custom provider wins (spec edge case).
- [ ] T069 [P] [US6] Add `webui/app/components/providers/CustomProviderForm.tsx`, the two-step form from the agreed mockup:
  - **Step 1, connection**: name; id, auto-slugged and editable until first save; a protocol radio card for OpenAI-compatible or Anthropic; base URL with an "Requests go to …" hint; API key; "Extra headers" expander; and a "Test connection" button calling `POST /providers/custom/test`.
  - **Step 2, models**: "Fetch from server", and an editable table with columns id, context, output, tools, vision and reasoning. Fields filled from the catalogue are marked ⓒ. A warning appears for rows with tool calling unknown or false.
  - **Edit mode**: the id is read-only, and the key and header values show as `•••• [Replace]`.
- [ ] T070 [US6] Add the custom-provider API calls to `webui/app/services/vizier.tsx`, and render custom providers in the CUSTOM group of `ProviderModelPicker.tsx`, each with a "custom" badge.
- [X] T071 [US6] Run quickstart.md §4 against `stub_llm.py` (both protocols, id clash, immutable id, edited-row survival, secret masking, delete warning, the FR-027 base-URL refusal), then the deferred §3 from T058. Record the results in the PR.

**Checkpoint**: self-hosted and proxy endpoints work, with no code change and no file editing (SC-009).

---

## Phase 7: User Story 5 - Connecting a new OpenAI-compatible provider needs no code change (Priority: P2)

**Goal**: A providers API and settings page that work for any selectable provider: generic credentials, env-var detection, "used by" counts, reachability and test (FR-009, FR-010, FR-010a–c, FR-016a).

**Independent Test**: quickstart.md §7, "Providers page" bullet, plus a live run on one provider vizier didn't support before (§8).

- [X] T072 [US5] Complete `src/channels/http/api/v1/providers/mod.rs` per contract `http-api.md` § Providers:
  - **`GET /providers`** returns connected providers: saved rows plus env-detected ones from `resolve_credentials`, with `source`, `has_api_key`, `endpoint`, `base_url`, `migrated_from` (recorded by T037 in a `global_config` key `migration.provider_ids_v1.map`) and `used_by`.
    - `used_by` is computed by scanning agent configs: chat includes the main, dream and read-image models; STT, TTS, image generation and embeddings go through each enum's `credentials()`.
    - `reachable` applies to local and custom providers. It is checked in a background task with a 2s timeout, cached for 30s in a `parking_lot::Mutex<HashMap>`, and the endpoint never waits on it.
  - **`PUT /providers/{id}`** returns 400 for a field the provider does not use, and for a `base_url` on a cloud provider (FR-027).
  - **`DELETE /providers/{id}`** returns 409 with the affected agents unless `?confirm=true`.
  - **`POST /providers/{id}/test`** makes one model-listing call, or a 1-token Anthropic completion.
  - The dummyplug rules stay as they are.
- [ ] T073 [P] [US5] Add `webui/app/components/providers/ConnectedList.tsx`. Each row shows the logo, name, id, source badge (`Saved key` / `From env NAME` / `migrated from …`), model count, "Used by: chat (3 agents) · TTS (1) …" and reachability for local and custom providers. Its actions are View models, Edit and ⋯ (Re-fetch for custom providers, Duplicate, Delete). Delete opens `DeleteWarning.tsx`, which lists the affected agents.
- [ ] T074 [P] [US5] Add `webui/app/components/providers/AddProvider.tsx` and `ConnectForm.tsx`:
  - a search box over `/catalogue/providers?connected=false`, with All / Chat / Media & embeddings / Local filters;
  - Popular first, then A–Z;
  - "+ Custom provider" opens `CustomProviderForm`;
  - a no-match search offers "Create custom provider …";
  - `ConnectForm` asks only for the provider's `fields`, shows the env-var alternative and the catalogue endpoint, and has Test and Save.
- [ ] T075 [P] [US5] Add `webui/app/components/providers/ModelsDrawer.tsx`: a read-only list of a provider's models (all of them, with deprecated ones marked), using the same row component as the picker.
- [ ] T076 [US5] Replace the providers section of `webui/app/routes/settingsRoot.tsx` with a header showing the catalogue status line and a "Refresh now" button (wired in US4), then `ConnectedList` and `AddProvider`. Delete the hard-coded `ALL_VARIANTS` list and the per-variant form code (depends on T073–T075).

**Checkpoint**: any catalogue provider can be connected from the UI. Keys found in env vars and usage are visible.

---

## Phase 8: User Story 7 - Agent setup is simpler because vizier knows the models (Priority: P2)

**Goal**: The Advanced section, checkpoint token count, automatic read-image UI, smart defaults, inline connect and the Review card (FR-031–FR-035; FR-029 was done in T061).

**Independent Test**: quickstart.md §7, "Defaults", "Read-image", "Inline connect" and "Review" bullets.

- [ ] T077 [US7] In `webui/app/components/AgentForm.tsx`, move Context window and Max output into an "Advanced" collapsible section, empty by default, with placeholders `"<value> · from catalogue"` taken from the selected model. Move Thinking depth, Prompt timeout and Silent-read chance into the same section. Next to Checkpoint threshold, show `≈ <threshold% × effective context> tokens`.
- [ ] T078 [US7] In `ProviderModelPicker.tsx` and `AgentForm.tsx` (create mode), when a provider is selected and the model is empty, preselect the model marked `default: true` by `/catalogue/providers/{id}/models` (FR-031).
- [ ] T079 [US7] In `AgentForm.tsx`'s Tools › Read image section, follow FR-033:
  - main model with `caps.image_input !== false`: show "✓ <model> can see images. They are sent to it directly", and no picker;
  - `image_input === false`: show a warning and the picker with `filter.imageInputOnly`;
  - an existing explicit vision model: show it as an override with a "Remove" button.

  In the TTS, STT and image-generation sections, show "✓ uses your <Provider> key" or a "Connect <Provider>" link, through `credentials()` mapped on the client.
- [ ] T080 [US7] Add inline connect to `ProviderModelPicker.tsx` (FR-034):
  - **With `settings:providers`**: choosing a provider whose `connection.status` is `missing` opens `ConnectForm` inline, which saves with `PUT /providers/{id}` and then re-fetches the provider list.
  - **Without it**: show "Ask an admin to connect <Provider>".

  Reuse `ConnectForm` from T074.
- [ ] T081 [US7] Render the model card on the Review tab in `AgentForm.tsx` (create mode): limits with the checkpoint token count, capabilities, price, dream and read-image routing, and the agent's `warnings` (computed on the client from the catalogue model, using the same rules as T057) (FR-035).

**Checkpoint**: creating an agent on a catalogue model needs no typed numbers and no vision-model choice for vision models (SC-010).

---

## Phase 9: User Story 4 - Catalogue stays current without breaking offline installs (Priority: P2)

**Goal**: A SQLite cache, a lazy conditional refresh triggered only by people, lazy logos, manual refresh, and the release-workflow seed fetch (FR-002–FR-004).

**Independent Test**: quickstart.md §6.

- [X] T082 [US4] Add the `catalogue_snapshot` and `catalogue_logo` tables, with the DDL from contract `catalogue-snapshot.md` § Cache, to the schema init in `src/storage/sqlite/mod.rs`. Add `trait CatalogueCacheStorage { get_snapshot, put_snapshot(tx), touch_checked, record_failure, get_logo, put_logo }` in `src/storage/catalogue.rs`, compose it into `VizierStorageProvider`, and forward it in `VizierStorage`.
- [X] T083 [US4] Create `src/catalogue/cache.rs`. At startup, in `src/dependencies.rs`, when the `catalogue_snapshot` row parses with `schema == 1` and its `fetched_at` is newer than the seed's, use it (`source: Cached`). A corrupt row logs a `warn` and the seed is used. Startup makes no network request.
- [X] T084 [US4] Create `src/catalogue/refresh.rs`, per contract `catalogue-snapshot.md` § Lazy refresh:
  - `fn maybe_refresh(deps)` checks `catalogue.refresh`, the TTL against `checked_at`, the 1h `failed_at` backoff, and single-flight (an `AtomicBool`). It spawns one background task and returns at once.
  - `async fn refresh_now(deps) -> Result<CatalogueStatus>` waits up to 30s.
  - The task sends a GET with `If-None-Match` and `Accept-Encoding: gzip`, decompressing with `flate2`:
    - **304**: `touch_checked`.
    - **200**: trim to the kept fields, then validate (parses; a supported provider exists; at least 50% of the provider count; every provider id used by stored agents, credentials and media settings is present). On success, `put_snapshot` and `catalogue.swap`. On failure, `record_failure`.
  - Add the config keys `catalogue.refresh` (default `true`), `catalogue.ttl` (default `24h`) and `catalogue.url` to `src/config/mod.rs`, plus `VIZIER_CATALOGUE_REFRESH` and `VIZIER_CATALOGUE_TTL` in `docker-entrypoint.sh` and the `apply_overrides` CLI/env handling.
  - Unit tests run against an axum server serving 200, 304, invalid bodies and timeouts.
- [X] T085 [US4] Call `catalogue::refresh::maybe_refresh(&deps)` at the start of every handler in `src/channels/http/api/v1/catalogue.rs` and of `GET /providers` and `GET /providers/custom`. Never call it from agent, scheduler or dream code (add a doc comment on `maybe_refresh` stating this rule). Add `POST /catalogue/refresh` (`settings:providers`): 200 with the status, 502 on failure, and 409 when refresh is disabled.
- [X] T086 [US4] Extend the logo handler in `catalogue.rs` to follow the contract's lookup order: the `catalogue_logo` row, then the seed, then `seed_no_logo`. Otherwise, when refresh is enabled, spawn a background fetch of `https://models.dev/logos/{id}.svg`, storing the svg, or NULL if it equals the fallback, and return 404 this time.
- [ ] T087 [US4] Fill in `GET /catalogue`'s `source`, `fetched_at`, `checked_at` and `refresh.{enabled, ttl, in_progress, last_error, failed_at}`. In `settingsRoot.tsx`, show "Catalogue: models.dev · as of <fetched_at> (<source>)", with the last error if any, and wire the "Refresh now" button to `POST /catalogue/refresh`.
- [X] T088 [P] [US4] In `.github/workflows/release.yml`, job `prepare-release`, add a "Refresh model catalogue" step after "Update Cargo.toml versions" that runs `python3 -I scripts/catalogue_update.py --keep-on-failure`, and add `assets/catalogue` to the `git add` in "Commit version bump". In `.github/workflows/publish.yml`'s "Verify package" step, fail when `cargo package --allow-dirty --no-verify --list` lacks `assets/catalogue/api.json`.
- [X] T089 [US4] Run quickstart.md §6: refresh off and offline; agents running with no request; a lazy check on read; 304 with `VIZIER_CATALOGUE_TTL=1s`; broken URL with backoff; offline restart from SQLite. Record the results in the PR.

**Checkpoint**: the catalogue stays current for people using it, stays visible offline, and a headless install never phones home.

---

## Phase 10: User Story 8 - Model prices feed the usage view (Priority: P3)

**Goal**: Estimated cost per agent, day and channel, never shown as zero when unknown (FR-021).

**Independent Test**: quickstart.md §7, "Usage view" bullet.

- [X] T090 [US8] Add `provider: Option<ProviderId>` and `model: Option<String>` (`serde(default)`) to `VizierResponseStats` in `src/schema/response.rs`, set in `VizierAgent::chat` (`src/agents/agent/mod.rs`) from the agent's resolved provider and model.
- [X] T091 [US8] In `src/storage/sqlite/history.rs` `aggregate_usage` and `src/schema/metrics.rs`, add `estimated_cost_usd: Option<f64>` to the summary, each `by_day` entry and each `by_channel_type` entry. For each response with `provider`/`model`, look up the catalogue cost, or the custom model's (none for now), and compute `(input − cached) × input + cached × cache_read (or input) + cache_creation × cache_write (or input) + output × output`, divided by 1e6. A bucket with no priced responses stays `None`. Pass the catalogue snapshot to `aggregate_usage` from the usage handler in `src/channels/http/api/v1/agents/mod.rs`, and unit-test the arithmetic.
- [ ] T092 [US8] In `webui/app/routes/usage.tsx`, show the estimated cost next to token totals, per day and per channel, labelled "est.", with `—` when it is `null`.

**Checkpoint**: all stories are done.

---

## Phase 11: Polish & Cross-Cutting Concerns

- [ ] T093 [P] Update `CLAUDE.md`:
  - The **Providers/models** section: `src/llm` (dialects, `HttpModel`, `RetryingModel`, `prepare`, media clients), `src/catalogue` (seed, cache, lazy refresh), `ProviderId`, custom providers; remove "Runs on `rig-core`" and the "rig drops status codes" note.
  - The **Storage** section: `custom_provider`, `catalogue_snapshot`, `catalogue_logo`, and `provider_config` holding `ProviderCredentials`.
  - The **Config layering** section: no YAML `providers`; the `catalogue.*` keys.
  - The **Tools** section: `ToolDefinition` now lives in `llm::types`.
  - The **Commands** section: `just catalogue-update`.
- [ ] T094 [P] Update `docs/` (the mdBook) pages on providers, configuration and Docker env vars: no `providers:` YAML; `VIZIER_CATALOGUE_*`; custom providers; legacy env vars still honoured.
- [ ] T095 [P] Add the live-provider smoke tests to `src/llm/smoke_tests.rs`, `#[ignore]` and named `llm_smoke_*`. Each test runs one text turn, one tool call and one tool result through `llm::build`, for each of openai (Responses), anthropic (asserting `cached_input_tokens > 0` on the second turn), google, deepseek (reasoning plus a tool loop), ollama, and one catalogue provider vizier didn't support before, skipping when its env key is not set.
- [ ] T096 Run `cargo build`, `cargo test`, `cargo clippy` and `cd webui && npm run typecheck`, and fix everything.
- [ ] T097 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk every quickstart.md dummyplug and stub-server step (§1–§7) in one session, and record the results in the PR.
- [ ] T098 Run the live steps in quickstart.md §8 with the keys available, and list every dialect not run live as "golden-fixture-tested only" in the PR.
- [ ] T099 Run `release-and-publish.yml` with `dry_run: true`. Confirm that the prepare-release log shows the catalogue step, and that `cargo package --list` contains `assets/catalogue/api.json`.
- [ ] T100 Write the PR description:
  - **Breaking changes**: Galadriel and Mira are dropped; YAML `providers` is removed; provider ids are renamed and legacy names are still accepted.
  - The baseline and dependency-count delta (T001, T052).
  - The suggested PATCH clarification to the constitution's Distribution constraint (research R13).
  - **Commit types**: `feat!:` with `[**breaking**]` for the breaking changes, `refactor:` for T061, and `chore:` for catalogue seed refreshes.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** comes first. T005 needs network access once.
- **Foundational (Phase 2)** depends on Setup and blocks every story. All of it is new code, and rig is untouched.
- **US1 (Phase 3)**, the cutover, depends on Foundational. **Every later story depends on US1**, because they extend `ProviderId`, `resolve` and the new storage.
- **US3 (Phase 4)** depends on US1. Its quickstart step T058 also needs US6's custom-provider API (T066), or it can be run after Phase 6.
- **US2 (Phase 5)** depends on US1. T061 (the form merge) must come before T064 and T065.
- **US6 (Phase 6)** depends on US1. Its WebUI parts (T069, T070) depend on US2's picker (T064).
- **US5 (Phase 7)** depends on US1 and US2 (T063), and reuses `CustomProviderForm` from US6 (T069).
- **US7 (Phase 8)** depends on US2 (the picker and the merged form) and US5 (`ConnectForm`, T074).
- **US4 (Phase 9)** depends on US2 (the catalogue endpoints, to add triggers to) and on US5's settings page (T076) for the Refresh button. T082–T084 and T088 can start right after US1.
- **US8 (Phase 10)** depends only on US1.
- **Polish** comes last.

### Story dependency graph

```
Setup → Foundational → US1 (cutover, rig removed) ─┬─→ US3
                                                   ├─→ US2 ─┬─→ US6 ─→ US5 ─→ US7
                                                   │        └──────────────→ US4 (UI bits)
                                                   ├─→ US4 (backend: T082–T084, T088)
                                                   └─→ US8
```

### Within each phase

Storage, then resolve, then call sites, then the API, then the UI. Each quickstart run task comes last in its phase.

## Parallel opportunities

- **Setup**: T003, T004 and T007 are independent files.
- **Foundational**: T009, T010, T011 and T013 after T008; the five dialects T017–T021 in parallel after T016; the media clients T022 and T023 in parallel; T025 alongside the dialects; T032 and T033 in parallel after T031.
- **US2**: T062 alongside T059/T060; T063 after T059.
- **US5**: T073, T074 and T075 are independent components.
- **After US1**: US8 (T090–T092) and US4's backend (T082–T084, T088) can run alongside US2 and US3.
- **Polish**: T093, T094 and T095 are independent.

## Parallel Example: Foundational dialects

```bash
# After T016 (prepare) lands, write all five dialects at once:
Task: "Implement src/llm/dialect/openai_chat.rs (T017)"
Task: "Implement src/llm/dialect/anthropic.rs (T018)"
Task: "Implement src/llm/dialect/gemini.rs (T019)"
Task: "Implement src/llm/dialect/ollama.rs (T020)"
Task: "Implement src/llm/dialect/openai_responses.rs (T021)"
# And the media clients:
Task: "Create src/llm/media/openai_shaped.rs (T022)"
Task: "Create src/llm/media/{gemini,cohere,ollama,hyperbolic,huggingface}.rs (T023)"
```

## Parallel Example: User Story 5

```bash
Task: "Add webui/app/components/providers/ConnectedList.tsx (T073)"
Task: "Add webui/app/components/providers/AddProvider.tsx + ConnectForm.tsx (T074)"
Task: "Add webui/app/components/providers/ModelsDrawer.tsx (T075)"
```

---

## Implementation Strategy

### MVP first (through US1)

1. **Phase 1 and Phase 2**: the new layer, fully unit- and golden-tested, with vizier unchanged.
2. **Phase 3 (US1)**: the cutover. rig is deleted, migrations run, and every existing agent keeps working.
3. **Stop and validate**: quickstart §1 and §5, plus the 008 and 014 dummyplug scripts. This alone resolves issue #69.

### Incremental delivery (the branch merges as a whole; these are internal milestones)

1. **US3**: accurate limits and capability handling at runtime.
2. **US2**: the catalogue API, the merged form and the pickers. This is the headline UI change.
3. **US6, then US5**: custom providers, then the full providers page.
4. **US7**: form simplifications.
5. **US4**: SQLite cache, lazy refresh and the release workflow.
6. **US8**: cost.
7. **Polish**: docs, smoke tests, the full quickstart and a release dry run.

### Notes

- **[P]** tasks touch different files and have no unfinished dependencies.
- **Commits**: commit after each task or logical group, with conventional commits.
  - The cutover (T052) is the `refactor!:`/`feat!:` commit carrying `[**breaking**]` for the dropped providers and YAML `providers`.
  - T061 is `refactor:`.
  - Seed refreshes are `chore:`.
- Never add a `ProviderVariant`-style `match` over provider ids in dialect or call-site code (Principle II). Quirks go through `CredentialSource` and the endpoint rules in `resolve`.
