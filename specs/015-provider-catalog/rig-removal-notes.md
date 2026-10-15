# Draft Notes: Remove rig-core, Own the Provider Layer

> Pre-spec draft, written before [spec.md](spec.md). It is input to `/speckit-plan`, not the plan. Where the two disagree, the spec wins:
> - The provider table is now generated from the models.dev catalogue, not hand-written.
> - Removal order: the shim and staged per-provider switch-over described below are **not** used. The new layer is built first, then rig is removed in one cutover (plan Phases 1–2).
> - D1 is decided the other way: Hyperbolic, Galadriel and Mira are dropped as chat providers (FR-014).

**Branch**: `015-own-provider-layer` (not created yet) | **Date**: 2026-10-15 | **Issue**: [#69](https://github.com/blinfoldking/vizier/issues/69)

**Input**: Issue #69, plus a survey of every `rig_core` use in `src/` (about 60 files, about 276 message-type sites).

> No `spec.md` exists yet. This plan comes first because the change is internal and keeps existing behaviour; there are no user stories to specify. Run `/speckit-specify` later if you want the full artifact set (spec, research, tasks).

## Summary

Replace `rig-core` 0.39 with an in-tree provider layer, `src/llm/`. It is agent-agnostic, like `src/sandbox/`, and has four parts:

1. **Our own conversation types**: `Message`, the user and assistant parts, `ToolDefinition`, `ToolChoice` and `Usage`. They replace `rig_core::message`, `OneOrMany` and `completion::ToolDefinition` everywhere: `schema/`, history, tools, MCP, the sandbox, the agent loop and dummyplug.
2. **A small number of wire dialects**, one implementation each behind one `CompletionProvider` trait:
   - OpenAI Chat Completions
   - OpenAI Responses
   - Anthropic Messages
   - Gemini `generateContent`
   - Ollama `/api/chat`
3. **A provider table.** Each `ProviderVariant` becomes a row: dialect, base URL, auth style, env var, extra headers, model-listing support. This replaces the two 25-arm `match`es in `model/mod.rs`, the 25 `VizierModelBuilder` impls in `model/provider.rs`, and most of `resolve_provider`.
4. **Our own HTTP and error layer**, built on the `reqwest` we already depend on. It reports the status code, the `Retry-After` header and the body as a typed `LlmError`. That fixes the limitation `retry.rs` documents today: rig drops the status code, so failures are classified by matching error text.

Embeddings, STT, TTS and image generation move onto the same transport, using the same pattern: one dialect, many providers as rows.

Persisted data is unaffected. `SessionHistoryContent` already stores our own shapes and never rig types, and `ProviderVariant`'s serde names stay the same. So the change needs **no storage migration and no API change**. We work from the pinned rig 0.39 source in `~/.cargo/registry` as the reference implementation. It runs correctly in production today, so every dialect is a port of known-good behaviour, not something written from provider docs alone.

## Technical Context

**Language/Version**: Rust, edition 2024

**Primary Dependencies**:
- Removed: `rig-core`.
- Kept and used directly: `reqwest` 0.13 (enable the `json`, `multipart` and `stream` features), `serde`/`serde_json`, `schemars`, `tokio`, `async-trait`, `fastembed`, `rmcp`.
- New crates: none planned. SSE parsing is hand-rolled, about 80 lines; only the ChatGPT Codex backend needs it today (R6).

**Storage**: none changed.

**Testing**:
- Pure `encode`/`decode` unit tests per dialect, against JSON fixtures.
- Transport tests against an in-process `axum` server on `127.0.0.1:0` (axum is already a dependency).
- A temporary rig-parity harness (Phase 3).
- `#[ignore]`d live smoke tests gated on env keys.
- The dummyplug e2e quickstart.

**Target Platform**: unchanged, including the musl and cross targets. Dropping rig's transitive dependency tree is likely to help those targets.

**Project Type**: single Rust binary with an embedded WebUI. The WebUI is untouched.

**Constraints**:
- The dummyplug chat protocol must stay byte-compatible (constitution, test-harness contract).
- Tool names, history shape and the `ProviderVariant` serde names must not change.
- Anthropic automatic prompt caching must keep working, since cost regressions are silent.

**Scale/Scope**:
- Deleted: about 2,000 lines (`model/provider.rs`, most of `model/mod.rs`, and 40 small adapter files under `embedding/`, `stt/`, `tts/` and `image_generation/`).
- Added: roughly 3,000–4,000 lines in `src/llm/`. About half of that is the dialect encoders and decoders, plus fixtures.

## Where rig is used today

| Area | Files | rig surface |
|---|---|---|
| Message types | `schema/{request,response,history}.rs`, `agents/process.rs`, `agents/agent/mod.rs`, `agents/agent/system_prompt/context.rs`, `agents/tools/read_image/` | `Message`, `UserContent`, `AssistantContent`, `ToolCall`, `ToolResult`, `OneOrMany`, media types |
| Tool definitions | `agents/tools/mod.rs`, `agents/mcp/mod.rs`, `sandbox/docs.rs` | `completion::ToolDefinition` |
| Completion | `agents/agent/model/{mod,provider,retry,dummyplug}.rs` | `CompletionClient`, `CompletionModel`, `ModelListingClient`, `CompletionError`, `http_client`, `Usage`, `ToolChoice`, 25 provider clients |
| Embedding | `embedding/*.rs` (9 adapters + fastembed) | `EmbeddingsClient`, `embeddings::EmbeddingModel` |
| STT / TTS / Image | `stt/{openai,groq,mistral,gemini,huggingface}.rs`, `tts/{openai,openrouter,xai,hyperbolic}.rs`, `image_generation/{openai,xai,hyperbolic,huggingface}.rs` | transcription, audio-generation and image-generation clients |

`elevenlabs`, `whisper`, `kokoro` and `fastembed` already have their own implementations and only touch rig through trait glue.

Two behaviours are worth keeping in mind:

- **Completions are not streamed today.** One request goes out and one response comes back. We keep that. Streaming is out of scope (D3).
- **Reasoning blocks are not persisted.** `messages_to_history_entries` drops `AssistantContent::Reasoning` (`_ => {}`). Within a single turn, though, rig round-trips reasoning back to the provider, because the turn's `Vec<Message>` stays in memory. Anthropic thinking signatures and DeepSeek `reasoning_content` need that round-trip during a tool loop, so our types must carry reasoning **in memory** even though history keeps dropping it.

## Constitution Check

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | ✅ Net removal of a 43k-line dependency and about 2k lines of glue. The provider table replaces about 50 near-identical match arms and impls. No streaming API, no generic "agent" abstraction, no client builders: only what the five dialects and the existing call sites need. |
| **II. DRY via Trait-Based Extensibility** | ✅ Each wire format is exactly one `CompletionProvider` impl (likewise for embeddings, STT, TTS and images). Providers that differ only in URL, auth or headers are **data rows**, not code. Adding an OpenAI-compatible provider is one row; a new wire format is one new trait impl. No dispatch `match` grows. |
| **III. Self-Contained Runtime** | ✅ No new dependency or service. Network calls go only to the provider the user configured, as before. |
| **IV. Portability** | ✅ Pure Rust over `reqwest` (already used, rustls). This drops rig's transitive dependencies from the musl and cross builds. |
| **V. Unified Errors & Observability** | ✅ `LlmError` gets a `From` into `VizierError`. The `ErrorKind::classify` prefixes (`PROMPT_TIMED_OUT`, `RATE_LIMITED`, `PROVIDER_UNAVAILABLE`) are kept. Request and response debug logging goes through `tracing`, with API keys redacted. |
| **Gate: dummyplug e2e** | ✅ Dummyplug moves to the new types, and its protocol stays unchanged. The quickstart re-runs the 008 and 014 scripts unchanged as a regression check. |
| **Gate: conventional commits** | Not breaking if D1 keeps every variant (`refactor:`/`feat:`). If any variant is dropped, it becomes `refactor!:` with `[**breaking**]`. |

**Result**: PASS, subject to decisions D1 and D2 below.

## Design

### Module layout

```
src/llm/
  mod.rs            // re-exports; CompletionProvider, EmbeddingProvider, … traits
  types.rs          // Message, UserPart, AssistantPart, ToolDefinition, ToolChoice, Usage, CompletionRequest/Response
  error.rs          // LlmError { Http{status, retry_after, body}, Transport, Timeout, Decode, Config }
  http.rs           // shared reqwest::Client, Auth styles, send_json/send_multipart, status→LlmError, key redaction
  sse.rs            // minimal SSE line parser + "aggregate to final response" (Responses dialect)
  registry.rs       // ProviderSpec table: ProviderVariant → { dialect, base_url, auth, env, headers, listing }
  dialect/
    openai_chat.rs      // /chat/completions  — most providers
    openai_responses.rs // /responses         — openai, chatgpt (SSE), copilot codex-class models
    anthropic.rs        // /v1/messages       — anthropic (+ automatic cache_control)
    gemini.rs           // :generateContent   — gemini
    ollama.rs           // /api/chat          — ollama
  embed/   openai.rs, gemini.rs, cohere.rs, ollama.rs
  audio/   openai_stt.rs, openai_tts.rs, gemini_stt.rs, hf.rs, hyperbolic.rs
  image/   openai.rs, hyperbolic.rs, hf.rs
```

`agents/agent/model/` keeps `VizierModel`, `retry.rs`, `registry.rs` (context-window detection) and `dummyplug.rs`. It shrinks to "look up the `ProviderSpec`, build the dialect, wrap it in retry." Each of `embedding/`, `stt/`, `tts/` and `image_generation/` keeps its `Vizier*Model` trait and its `from_agent_settings` factory. The per-provider files collapse into table lookups.

### Core types (`llm/types.rs`)

```rust
pub enum Message {
    System(String),
    User(Vec<UserPart>),                                   // non-empty, checked at construction
    Assistant { id: Option<String>, content: Vec<AssistantPart> },
}
pub enum UserPart {
    Text(String),
    Image { data: Media, media_type: String },             // Media = Base64(String) | Url(String)
    Document { data: Media, media_type: String, name: Option<String> },
    ToolResult { call_id: String, content: Vec<ToolResultPart> },
}
pub enum ToolResultPart { Text(String), Image { data: Media, media_type: String } }
pub enum AssistantPart {
    Text(String),
    ToolCall { id: String, call_id: Option<String>, name: String, arguments: serde_json::Value },
    Reasoning { text: String, signature: Option<String>, provider_data: Option<serde_json::Value> },
}
pub struct ToolDefinition { pub name: String, pub description: String, pub parameters: serde_json::Value }
pub enum ToolChoice { Auto, None }
pub struct Usage { input_tokens, output_tokens, total_tokens, cached_input_tokens, cache_creation_input_tokens }
pub struct CompletionRequest { system: Option<String>, messages: Vec<Message>, tools: Vec<ToolDefinition>,
                               tool_choice: Option<ToolChoice>, max_tokens: Option<u64> }
pub struct CompletionResponse { id: Option<String>, content: Vec<AssistantPart>, usage: Usage }

#[async_trait] pub trait CompletionProvider: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> { Err(LlmError::Unsupported) }
}
```

- `Vec` replaces `OneOrMany`. The non-empty invariant is checked where messages are built, not encoded in the type, because `OneOrMany` adds friction at every one of the 276 sites.
- Field names follow rig's where the meaning is the same (`Usage`, `ToolDefinition`), so the migration is mostly a change of `use` paths.
- `VizierModelTrait::completion` keeps its `(message, history, tools)` shape but returns `CompletionResponse` in place of the `(Option<String>, OneOrMany<AssistantContent>, Usage)` tuple.

### Provider table (`llm/registry.rs`)

| Dialect | Providers |
|---|---|
| `openai_chat` | deepseek, groq, mistral, xai, perplexity, moonshot, zai, minimax, together, cohere (compatibility API), huggingface (router), hyperbolic, galadriel, mira, mimo, openrouter, llama_cpp, custom, opencode_zen, opencode_go (+ session header), azure (deployment URL + `api-key` header + `api-version`), copilot (conversational models) |
| `openai_responses` | openai, chatgpt (Codex backend, SSE, `chatgpt-account-id`), copilot (Codex-class models) |
| `anthropic` | anthropic |
| `gemini` | gemini |
| `ollama` | ollama |

Each row is a `ProviderSpec`:

```rust
ProviderSpec {
    variant,
    dialect,
    default_base_url,
    auth: Bearer | Header(name) | QueryKey | None,
    env_key,
    base_url_env,
    extra_headers,
    lists_models,
}
```

`resolve_provider` becomes a lookup of `spec.env_key`. The special resolvers for ChatGPT, Azure, custom and local providers stay, because those carry more than a key.

**Copilot** needs its token exchange ported from rig's `copilot/auth/native.rs`: the GitHub token is traded at `api.github.com/copilot_internal/v2/token` for a short-lived token, which is cached and refreshed. We also port rig's per-model routing between Chat Completions and Responses.

### Errors and retry

- `http.rs` maps every non-2xx response to `LlmError::Http { status, retry_after, body }`. Connection, reset and socket-timeout failures become `LlmError::Transport`.
- `retry.rs::classify` switches on that type. A typed status now decides first, and `classify_text` remains only as the fallback for providers that report errors as HTTP 200 with an error body.
- `Retry-After` is honoured: the wait is `max(backoff, retry_after)`, capped by the turn's deadline.
- Update the module comment in `retry.rs` that blames rig.

## Phases

Each phase compiles, passes `cargo clippy` and `cargo test`, and could ship on its own. rig stays until Phase 7, so any provider can fall back to rig while it is unported.

### Phase 0: Research (`research.md`)

- **R1**: For each dialect, read the rig 0.39 source and record every quirk we rely on:
  - Anthropic: the system prompt as a separate field, `cache_control` placement for automatic caching, how thinking blocks are round-tripped, `max_tokens` required.
  - OpenAI Responses: item-shaped input and output, `function_call_output`, reasoning items.
  - Gemini: `functionDeclarations` schema sanitising, `thoughtSignature`.
  - Ollama: `options`, how tool-call ids are synthesised.
  - DeepSeek: `reasoning_content`.
  - Mistral: the tool-call id format.
- **R2**: Schema sanitising. Some providers (Gemini especially) reject `schemars` output that contains `$ref`, `definitions` or `additionalProperties`. Find out what rig rewrites, so our MCP and tool schemas keep working.
- **R3**: Copilot token exchange and model routing (see the Design section).
- **R4**: ChatGPT Codex backend: required headers, `store: false`, `instructions`, forced `stream: true`, and SSE event types.
- **R5**: Media endpoints:
  - STT: OpenAI, Groq and Mistral use `/audio/transcriptions`; Gemini uses inline audio; HF uses the raw inference API.
  - TTS: what rig sends for OpenRouter, xAI and Hyperbolic TTS.
  - Images: OpenAI and xAI use `/images/generations`, plus the Hyperbolic and HF endpoints.
- **R6**: Confirm a hand-rolled SSE parser is enough, or justify `eventsource-stream`.
- **R7**: Model-listing endpoints and the `context_length` fields for the nine providers that list models today.
- **R8**: Check that galadriel and mira are still operating (input to D1).

### Phase 1: Own the types (rig stays as transport)

- Add `src/llm/types.rs`.
- Add `From` conversions both ways between our types and rig's, in **one** file (`model/rig_shim.rs`) that sits at the `VizierModelImpl` boundary.
- Move every non-model call site onto our types: `schema/`, `history.rs`, `process.rs`, `agent/mod.rs`, `context.rs`, `read_image`, `tools/mod.rs`, `mcp/mod.rs`, `sandbox/docs.rs` and `dummyplug.rs`.
- Keep `AssistantPart::Reasoning` in memory; `history.rs` still skips it.
- Done when rig is referenced only in `model/`, `embedding/`, `stt/`, `tts/` and `image_generation/`. The dummyplug quickstarts for 008 and 014 still pass unchanged.

This phase is the largest diff but purely mechanical, which is why it lands on its own.

### Phase 2: Transport and errors

- Add `llm/{http,error,sse}.rs`.
- Write axum-backed transport tests for: 429 with `Retry-After`; 500, 503 and 529 classified as unavailable; a 400 failing at once; a connection refused; a slow server hitting `attempt_timeout`; an SSE stream aggregated to a final response.
- Teach `retry.rs` about `LlmError`. It still understands rig errors, for the providers not yet ported.

### Phase 3: Completion dialects, one at a time

Order, chosen by coverage and risk:

1. `openai_chat`, which unlocks about 20 providers.
2. `anthropic`.
3. `ollama`.
4. `gemini`.
5. `openai_responses` for openai, then chatgpt.
6. azure and copilot specifics.

For each dialect:

- **Unit tests** for `encode(&CompletionRequest) -> Value` and `decode(Value) -> CompletionResponse`. Fixtures live in `src/llm/dialect/fixtures/<dialect>/`. They cover: plain text, a tool call, parallel tool calls, a tool result with an image, reasoning round-trip, `tool_choice: none`, and usage including cached tokens.
- **Parity test** (temporary, removed in Phase 7). Point the rig 0.39 client and our client at the same axum capture server with the same input, then assert the two request bodies are semantically equal, after normalising known-intentional differences. Feed both the same recorded response and assert the decoded `CompletionResponse` is equal. This is how we verify wire formats without keys.
- **Switch over.** Flip the variants that use this dialect from `VizierModelImpl<rig::…>` to the provider-table path, with `lists_models` and context-window detection preserved.

### Phase 4: Collapse construction

- Delete `VizierModelBuilder`, `model/provider.rs` and the duplicated `new` / `new_with_override` matches. Both become `registry::spec(variant)` followed by `dialect::build(spec, resolved, config)`.
- Move `fetch_context_window_from_api` onto `CompletionProvider::list_models`.

### Phase 5: Embeddings

- Add an `EmbeddingProvider` trait with these dialects:
  - `openai_embeddings`: openai, openrouter, together, mistral, copilot, and voyageai (compatible request shape).
  - `gemini` (`batchEmbedContents`).
  - `cohere` (`/v2/embed`, or its compatibility API).
  - `ollama` (`/api/embed`).
- The fastembed adapter implements `VizierEmbeddingModel` directly, dropping the rig trait.
- Check vector equality: the same input text must give the same vector through rig and through us, to the float. Otherwise existing `document_index` rows silently stop matching new queries.

### Phase 6: STT, TTS and images

Port each rig-backed adapter onto `llm/audio` and `llm/image` dialects, using multipart via `reqwest`. Each `Vizier{Stt,Tts,ImageGen}Model` trait and factory keeps its signature. The `expect("failed to create … client")` calls in today's constructors go away (constitution V).

### Phase 7: Remove rig

- Delete `rig-core` from `Cargo.toml`, along with `rig_shim.rs`, the parity tests and rig error handling in `retry.rs`.
- Quiet the `rig` crate directive in `main.rs`.
- Update `CLAUDE.md`: the Providers section ("Runs on `rig-core`"), the "rig drops status codes" note, and the "Where rig is used" notes.
- `cargo tree | grep rig` must be empty.

### Phase 8: Verification

- `cargo clippy` and `cargo test`.
- `cd webui && npm run typecheck` (sanity check only; nothing changes there).
- Dummyplug quickstart (008 and 014 scripts unchanged), plus a new `quickstart.md` step: send a tool request and confirm `context` and tool results still round-trip.
- Live smoke tests (`#[ignore]`, run with `cargo test -- --ignored llm_smoke`). Each sends one text turn, one tool call and one tool result for every dialect whose key env var is set, plus ollama when `OLLAMA_BASE_URL` answers. Run at least openai, anthropic, gemini, deepseek and ollama before merging. Anything not live-tested is listed in the PR as "parity-tested only".
- Anthropic caching check: on a two-turn live run, the second turn reports `cached_input_tokens > 0`.

## Decisions needed

- **D1: Which providers stay first-class?** **Recommendation: keep every `ProviderVariant`.** Once they are table rows, an OpenAI-compatible provider costs about five lines. Keeping them all also avoids a breaking change for stored agent configs; an unknown variant would fail deserialisation. If R8 shows galadriel or mira are defunct, drop them in a separate, clearly marked breaking commit, with a startup check that names any agent still configured with one.
- **D2: OpenAI on Responses or Chat Completions?** **Recommendation: Responses, which is what we use today.** We need the Responses dialect for ChatGPT and Copilot Codex models anyway, and switching would change reasoning-model behaviour mid-refactor. Chat Completions for OpenAI remains available as the `custom` path.
- **D3: Is streaming in scope?** **Recommendation: no.** Completions aren't streamed today, and the WebUI streams turn frames, not tokens. The types leave room for a later `stream()` method. SSE is used internally only where the backend requires it (ChatGPT).
- **D4: In-tree module or separate crate?** **Recommendation: in-tree `src/llm/` first**, like `src/sandbox/`. Split it into a workspace crate later if the boundary holds.

## Risks

| Risk | Mitigation |
|---|---|
| Silent wire drift: a field rig sent that we forgot, such as cache control or a schema rewrite | Parity harness per dialect; R1 and R2 checklists; live smoke tests on the main five |
| Gemini rejects tool schemas from MCP servers | R2: port rig's schema sanitiser, with a fixture taken from a real MCP tool |
| Reasoning round-trip breaks tool loops (Anthropic thinking signatures, DeepSeek) | `AssistantPart::Reasoning` keeps `signature` and `provider_data`; fixture tests cover the round-trip |
| Embedding vectors differ, so existing indexes go stale | Vector-equality check in Phase 5; same model, same input normalisation |
| Copilot token refresh mishandled | Port rig's logic directly; cache with expiry; transport test for a 401 followed by a refresh |
| Phase 1 diff is large and conflicts with other work | Land Phase 1 quickly and on its own; it is mechanical and changes no behaviour |

## Out of scope

- Token streaming to channels.
- New providers.
- Changes to `ProviderEntryConfig` or the providers WebUI.
- Moving MCP off `rmcp`. It doesn't depend on rig; we only swap the `ToolDefinition` type at its edge.
