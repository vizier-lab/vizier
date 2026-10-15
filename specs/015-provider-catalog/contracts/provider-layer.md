# Contract: `src/llm` Provider Layer (internal)

This is the seam that replaces rig. Everything outside `src/llm/` depends only on the items below; nothing outside it builds an HTTP request to a model provider.

## Types (`llm::types`)

```rust
pub enum Message {
    System(String),
    User(Vec<UserPart>),                                       // non-empty, checked by constructors
    Assistant { id: Option<String>, content: Vec<AssistantPart> },
}
pub enum UserPart {
    Text(String),
    Image { data: Media, media_type: String },
    Document { data: Media, media_type: String, name: Option<String> },
    ToolResult { call_id: String, content: Vec<ToolResultPart> },
}
pub enum ToolResultPart { Text(String), Image { data: Media, media_type: String } }
pub enum Media { Base64(String), Url(String) }
pub enum AssistantPart {
    Text(String),
    ToolCall { id: String, call_id: Option<String>, name: String, arguments: serde_json::Value },
    Reasoning { text: String, signature: Option<String>, provider_data: Option<serde_json::Value> },
}
pub struct ToolDefinition { pub name: String, pub description: String, pub parameters: serde_json::Value }
pub enum ToolChoice { Auto, None }
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64, pub total_tokens: u64,
                   pub cached_input_tokens: u64, pub cache_creation_input_tokens: u64 }

pub struct CompletionRequest {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tokens: Option<u64>,
}
pub struct CompletionResponse { pub id: Option<String>, pub content: Vec<AssistantPart>, pub usage: Usage }
```

**Field names**:
- `ToolDefinition` and `Usage` keep rig's field names, so call-site changes are just `use` paths.
- `ToolDefinition` serialises to the same JSON as rig's. `sandbox/docs.rs` builds `ToolFunctionDoc` from it, and the 014 fixtures must not change.

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

**Retry classification** (`agents::agent::model::retry`):

| Error | Classified as |
|---|---|
| `Http` 429 | rate-limited |
| `Http` 408, 500, 502, 503, 504, 520–529 | unavailable |
| `Transport`, `Timeout` | unavailable |
| other `Http` codes, `Decode`, `Config`, `Unsupported` | permanent |

A 2xx response whose body is an error object counts as `Decode`, after `classify_text` is applied to it. The wait before a retry is `max(backoff, retry_after)`, capped by the turn's deadline.

## Traits

```rust
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;
    async fn list_models(&self) -> Result<Vec<ListedModel>, LlmError> { Err(LlmError::Unsupported) }
}
pub struct ListedModel { pub id: String, pub context: Option<u64> }

#[async_trait]
pub trait CredentialSource: Send + Sync {
    /// Headers / query params to attach to one request. May refresh a cached token.
    async fn authorize(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder, LlmError>;
    /// Called after a 401 so token-exchange sources can drop their cache once.
    async fn invalidate(&self) {}
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync { async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f64>>, LlmError>; }
#[async_trait]
pub trait SttProvider: Send + Sync { async fn transcribe(&self, audio: &[u8], filename: &str, language: Option<&str>) -> Result<String, LlmError>; }
#[async_trait]
pub trait TtsProvider: Send + Sync { async fn speak(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, LlmError>; }
#[async_trait]
pub trait ImageGenProvider: Send + Sync { async fn generate(&self, prompt: &str, size: Option<(u32, u32)>) -> Result<(Vec<u8>, String), LlmError>; }
```

The existing `VizierEmbeddingModel`, `VizierSttModel`, `VizierTtsModel` and `VizierImageGenModel` traits keep their signatures. Their remote implementations become thin wrappers over these, so callers don't change.

## Completion dialects (one `CompletionProvider` implementation each)

| Dialect | Endpoint | Tool calls | Reasoning round-trip | Notes |
|---|---|---|---|---|
| `OpenAiChat` | `POST {base}/chat/completions` | `tools[]`, `tool_choice: "auto"\|"none"`, `tool_calls[]`; results as `role: tool` | `reasoning_content` (DeepSeek and similar) | `list_models`: `GET {base}/models` |
| `OpenAiResponses` | `POST {base}/responses` | `function_call` / `function_call_output` items | reasoning items (encrypted content kept in `provider_data`) | ChatGPT: `stream: true`, `store: false`, body read whole and parsed as SSE |
| `Anthropic` | `POST {base}/v1/messages` | `tool_use` / `tool_result` blocks | thinking blocks with `signature` | `system` is a separate field; `max_tokens` required (R8); automatic `cache_control` on the last message, the system prompt and the tools; `anthropic-version` header |
| `Gemini` | `POST {base}/models/{model}:generateContent` | `functionDeclarations` / `functionCall` / `functionResponse` | `thoughtSignature` | tool schemas are sanitised the way rig did (no `$ref`, `additionalProperties`, …); key in the `x-goog-api-key` header |
| `Ollama` | `POST {base}/api/chat` | `tools[]` / `tool_calls[]` | `thinking` field | `list_models`: `GET /api/tags`; context from `POST /api/show` |

**`answer_only`** (`ToolUse::Off`) sends `tool_choice: none` wherever the dialect supports it, and keeps tools defined, as today.

**Every dialect** is a pair of pure functions, `encode(&CompletionRequest, &ResolvedModel) -> serde_json::Value` and `decode(serde_json::Value) -> Result<CompletionResponse, LlmError>`, around one shared `http::send_json`. The pure functions are what the fixture tests target.

## Media dialects (curated providers; FR-016)

| Kind | Implementation | Used by |
|---|---|---|
| STT | OpenAI `/audio/transcriptions`, multipart | openai, groq, mistral |
| STT | Gemini `generateContent` with inline audio | gemini |
| STT | Hugging Face raw inference | huggingface |
| STT | (existing, unchanged) | elevenlabs, whisper |
| TTS | OpenAI `/audio/speech` (MP3 → WAV, unchanged) | openai, openrouter, xai |
| TTS | Hyperbolic `/audio/generation` | hyperbolic |
| TTS | (existing, unchanged) | elevenlabs, kokoro |
| Image | OpenAI `/images/generations` | openai, xai |
| Image | Hyperbolic `/image/generation` | hyperbolic |
| Image | Hugging Face inference | huggingface |
| Embeddings | OpenAI `/embeddings` | openai, openrouter, together, mistral, github-copilot, voyageai |
| Embeddings | Gemini `batchEmbedContents` | google |
| Embeddings | Cohere `/v2/embed` | cohere |
| Embeddings | Ollama `/api/embed` | ollama |
| Embeddings | fastembed (local) | implements `VizierEmbeddingModel` directly |

Each endpoint, request field and default is taken from the rig 0.39 source for that provider. Embeddings must return the same vectors as rig for the same input (FR-020); a parity test checks this before rig is removed.

## Resolution (`llm::resolve`)

`resolve(deps, &ProviderId, model_id, overrides) -> Result<ResolvedModel, VizierError>` is the only place that reads:

- the catalogue, including a model's route override;
- built-ins;
- custom providers;
- credentials (saved, then the catalogue's env vars, then legacy env vars);
- endpoint rules: Azure templating, and extra headers such as opencode-go's.

Dialect code never matches on a provider id.
