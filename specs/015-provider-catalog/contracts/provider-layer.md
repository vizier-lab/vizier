# Contract: `src/llm` Provider Layer (internal)

This is the seam that replaces rig. Everything outside `src/llm/` depends only on the items below; nothing outside it builds an HTTP request to a model provider.

The layer covers only what vizier uses: one non-streaming completion call with tools, embeddings, speech-to-text, text-to-speech and image generation. It is deliberately **not** a port of rig's API. The departures from rig are marked **[S1]–[S10]**, and *Simplifications versus rig* at the end summarises them.

## Types (`llm::types`)

```rust
pub enum Message {
    System(String),                                        // allowed anywhere; dialects normalise it [S8]
    User(Vec<Part>),                                       // non-empty, checked by constructors
    Assistant(Vec<AssistantPart>),
    ToolResult { call_id: String, content: Vec<Part> },    // its own message kind, not inside User [S4]
}
pub enum Part {
    Text(String),
    Attachment { mime: String, data: Vec<u8>, name: Option<String> },   // one attachment type [S6]
}
pub enum AssistantPart {
    Text(String),
    ToolCall { id: String, name: String, arguments: serde_json::Value,
               provider_data: Option<serde_json::Value> },             // one id [S5]
    Reasoning { text: String, provider_data: Option<serde_json::Value> }, // signatures etc. are opaque
}
pub struct ToolDefinition { pub name: String, pub description: String, pub parameters: serde_json::Value }
pub enum ToolChoice { Auto, None }
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64, pub total_tokens: u64,
                   pub cached_input_tokens: u64, pub cache_creation_input_tokens: u64 }

pub struct CompletionRequest {                             // a plain struct, no builder [S2]
    pub messages: Vec<Message>,                            // one list, no separate "prompt" [S3]
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: Option<ToolChoice>,                   // Some(None) = answer only [S7]
    pub max_tokens: Option<u64>,
}
pub struct CompletionResponse { pub id: Option<String>, pub content: Vec<AssistantPart>, pub usage: Usage }
```

**Kept compatible on purpose**:
- `ToolDefinition` serialises to the same JSON as rig's. `sandbox/docs.rs` builds `ToolFunctionDoc` from it, and the 014 fixtures must not change.
- `Usage` keeps rig's field names, so token accounting and the usage view don't change.
- Persisted history never stored rig types and is not touched.

**History mapping**: `schema/history.rs` maps one-to-one. `SessionHistoryContent::ToolCall` maps to an `AssistantPart::ToolCall`, and `ToolResult` maps to `Message::ToolResult`. The grouping and ungrouping of consecutive tool results inside user messages is deleted; dialects group as their wire format requires [S4].

## Errors (`llm::error`)

```rust
pub enum LlmError {
    Http { status: u16, retry_after: Option<Duration>, body: String },
    Transport(String),      // refused / reset / DNS / TLS
    Timeout,
    Decode(String),         // 2xx body we could not parse
    Config(String),         // missing credential, unsupported dialect, bad URL
    Unsupported,            // e.g. list_models on a provider without one
}
impl From<LlmError> for VizierError { … }
```

**Retry classification**:

| Error | Classified as |
|---|---|
| `Http` 429 | rate-limited |
| `Http` 408, 500, 502, 503, 504, 520–529 | unavailable |
| `Transport`, `Timeout` | unavailable |
| other `Http` codes, `Decode`, `Config`, `Unsupported` | permanent |

A 2xx response whose body is an error object counts as `Decode`, after `classify_text` is applied to it. The wait before a retry is `max(backoff, retry_after)`, capped by the turn's deadline.

## The completion seam

```rust
#[async_trait]
pub trait CompletionModel: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;
    async fn list_models(&self) -> Result<Vec<ListedModel>, LlmError> { Err(LlmError::Unsupported) }
    fn context_window(&self) -> Option<u64>;
}
pub struct ListedModel { pub id: String, pub context: Option<u64> }
```

It has exactly three implementations:

| Implementation | Role |
|---|---|
| `HttpModel<D: Dialect>` | the only completion type that touches the network |
| `DummyplugModel` | unchanged behaviour and protocol; no network |
| `RetryingModel<M: CompletionModel>` | a decorator that applies `CompletionRetryConfig`, attempt timeouts and the turn deadline (`complete_until`) [S10] |

`VizierModel` is `Arc<dyn CompletionModel>`. It is built as `RetryingModel<HttpModel<D>>`, or as plain dummyplug. The separate `completion` / `answer_only` methods, the `(message, history, tools)` signature and the tuple return value are all gone [S3, S7].

### `Dialect`: pure translation, no I/O [S1]

```rust
pub trait Dialect: Send + Sync + 'static {
    fn path(&self, m: &ResolvedModel) -> String;                                  // e.g. "/chat/completions"
    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> serde_json::Value;
    fn decode(&self, body: serde_json::Value) -> Result<CompletionResponse, LlmError>;
    fn read_body(&self) -> BodyKind { BodyKind::Json }                            // Sse for the ChatGPT backend
    fn list_models_path(&self) -> Option<&'static str> { None }
    fn decode_models(&self, _body: serde_json::Value) -> Vec<ListedModel> { vec![] }
}
```

`HttpModel<D>` does everything around the dialect, once for all five:

1. Prepare the request (below).
2. Call `D::encode`.
3. Authorise through `CredentialSource`.
4. Send through the shared `reqwest::Client`.
5. Map the status to `LlmError`.
6. Read the body as JSON or SSE.
7. Call `D::decode`.

On a 401 from a token-exchange source, `HttpModel` calls `invalidate()` and retries once. Dialects never see HTTP, so their fixture tests are pure `encode`/`decode` checks.

### Request preparation: one shared step before encoding [S6, S8]

`prepare(req, &ResolvedModel) -> PreparedRequest` runs before every dialect:

- **Attachments by capability** (FR-013, FR-033):
  - **Images**: kept when `caps.image_input != Some(false)`. Otherwise the image is replaced by the text `[image attached: <name>, but this model cannot view images]`.
  - **PDFs and other documents**: kept when the model accepts `pdf`. Otherwise the same kind of note, naming the file.
  - **Audio**: never sent (out of scope); always replaced by a note.

  Read-image routing to a separate vision model happens before this step, in the agent (data-model §7).
- **System messages** (`normalize_system`), run according to the dialect's declared `SystemStyle`:
  - **`Field`** (Anthropic, Gemini): leading `System` messages are joined into one system field. Later `System` messages become user text: `<system>…</system>`.
  - **`Inline`** (OpenAI chat, Ollama): system messages pass through as `role: system` messages.
  - **`Instructions`** (Responses): leading system messages go into `instructions`; later ones become developer messages.

`PreparedRequest` is a `CompletionRequest` that has passed both checks, plus the extracted `system: Option<String>` for the `Field` and `Instructions` styles.

### The five dialects

| Dialect | Path | Tool calls and results | Reasoning round-trip | Notes |
|---|---|---|---|---|
| `OpenAiChat` | `/chat/completions` | `tools[]`, `tool_choice: "auto"\|"none"`, `tool_calls[]`; each `ToolResult` becomes a `role: tool` message | `reasoning_content` (DeepSeek and similar) | models: `GET /models` |
| `OpenAiResponses` | `/responses` | `function_call` / `function_call_output` items; `ToolCall.id` is the `call_id` | reasoning items; encrypted content in `provider_data` | always `store: false`, so no item ids are kept [S5]. ChatGPT: `stream: true`, body read as SSE |
| `Anthropic` | `/v1/messages` | `tool_use` / `tool_result` blocks; consecutive `ToolResult`s grouped into one user turn | thinking blocks, with the signature in `provider_data` | `max_tokens` required (R8); automatic `cache_control` on the last message, the system prompt and the tools; `anthropic-version` header |
| `Gemini` | `/models/{model}:generateContent` | `functionDeclarations` / `functionCall` / `functionResponse`; consecutive results grouped into one turn | `thoughtSignature` in `provider_data` | tool schemas sanitised the way rig does it (`$ref`, `additionalProperties`, …); key in the `x-goog-api-key` header |
| `Ollama` | `/api/chat` | `tools[]` / `tool_calls[]`; results as `role: tool` | `thinking` field | models: `GET /api/tags`; context from `POST /api/show` |

Dialect code never matches on a provider id. Provider-specific behaviour comes only from `ResolvedModel`: base URL, headers, credential source and capabilities.

## Auth

```rust
#[async_trait]
pub trait CredentialSource: Send + Sync {
    async fn authorize(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder, LlmError>;
    async fn invalidate(&self) {}      // after a 401, for token-exchange sources
}
```

| Source | What it does |
|---|---|
| `StaticKey` | a bearer token, a named header (`x-api-key`, `api-key`, `x-goog-api-key`) or a query key; or nothing at all |
| `CopilotExchange` | trades the GitHub token for a short-lived Copilot token, cached until it expires |
| `ChatGptAccount` | an access token plus the `chatgpt-account-id` header |

## Media and embeddings: vizier's existing traits, implemented directly [S9]

There is no second trait layer. The media clients in `src/llm/media/` implement `VizierEmbeddingModel`, `VizierSttModel`, `VizierTtsModel` and `VizierImageGenModel` directly. Their signatures are unchanged, so callers don't change. Each client is configured by base URL, credential source and model:

| Client | Implements | Serves (curated providers) |
|---|---|---|
| `OpenAiShaped` | STT (`/audio/transcriptions`, multipart), TTS (`/audio/speech`, MP3 → WAV unchanged), image generation (`/images/generations`), embeddings (`/embeddings`) | STT: openai, groq, mistral. TTS: openai, openrouter, xai. Images: openai, xai. Embeddings: openai, openrouter, together, mistral, github-copilot, voyageai |
| `Gemini` | STT (inline audio through `generateContent`), embeddings (`batchEmbedContents`) | google |
| `Cohere` | embeddings (`/v2/embed`) | cohere |
| `Ollama` | embeddings (`/api/embed`) | ollama |
| `Hyperbolic` | TTS, image generation | hyperbolic |
| `HuggingFace` | STT, image generation (raw inference API) | huggingface |
| *(existing, unchanged)* | — | elevenlabs (STT and TTS), whisper, kokoro, fastembed (implements `VizierEmbeddingModel` directly, without its rig adapter) |

These clients share `http.rs` (send, status to `LlmError`, redaction) with `HttpModel`. Each request field and default is taken from rig 0.39 and pinned by golden fixtures. Embedding vectors must equal the recorded rig vectors (FR-020).

## Resolution (`llm::resolve`)

`resolve(deps, &ProviderId, model_id, overrides) -> Result<ResolvedModel, VizierError>` is the only place that reads:

- the catalogue, including a model's route override;
- built-ins;
- custom providers;
- credentials (saved, then the catalogue's env vars, then legacy env vars);
- endpoint rules: Azure templating, and extra headers such as opencode-go's.

It returns a `ResolvedModel` (data-model §6). `llm::build(resolved) -> VizierModel` then chooses `HttpModel<D>` from `resolved.dialect` and wraps it in `RetryingModel`. The dialect enum is matched in this one constructor, exactly as a module constructor registers its implementations.

## Simplifications versus rig

| # | rig | here | effect |
|---|---|---|---|
| S1 | each provider has its own client, model, request builder and response type (~25 modules, ~43k lines) | `Dialect` (pure `encode`/`decode`) plus one `HttpModel<D>` | HTTP, auth and errors written once; dialect tests are pure |
| S2 | `completion_request(..).messages(..).tools(..).build()` through `VizierModelImpl<T: CompletionClient>`, 25 builder impls and two 25-arm matches | a plain `CompletionRequest` and `dyn CompletionModel` | no generics or builders; faster builds |
| S3 | separate "prompt" message and history | one `messages` list | retry and the call sites stop juggling three arguments |
| S4 | tool results are content inside a `User` message | `Message::ToolResult` | `history.rs` grouping logic deleted; dialects group as needed |
| S5 | `ToolCall { id, call_id, function: { name, arguments } }` | `ToolCall { id, name, arguments, provider_data }` | one id; Responses sent with `store: false` |
| S6 | `Image` / `Audio` / `Video` / `Document` parts, with media-type enums and source kinds | `Part::Attachment { mime, data }` plus `prepare` | capability handling in one place, not at every call site |
| S7 | `completion` and `answer_only` methods | `tool_choice` on the request | one method; no retry branching |
| S8 | per-provider handling of system messages (involved Anthropic logic for mid-conversation ones) | one `normalize_system` with three styles | written and tested once |
| S9 | rig's media traits under vizier's `Vizier*Model` traits | the clients implement `Vizier*Model` directly; ~30 adapter files become ~6 clients | no double layer |
| S10 | retry passed alongside the model | `RetryingModel<M>` decorator | `VizierModel` is just the trait object |
