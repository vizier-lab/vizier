# Data Model: Catalogue-Driven Providers and Models

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

Rust names are shown as they are planned. "Stored" means persisted in SQLite; "derived" means rebuilt from the catalogue on load.

---

## 1. `ProviderId` (replaces `ProviderVariant`)

A validated string newtype.

- **Format**: `^[a-z0-9][a-z0-9._-]{0,63}$`.
- **Serialisation**: as a plain string.
- **`Deserialize`**: accepts legacy enum names and maps them through [contracts/provider-id-migration.md](contracts/provider-id-migration.md). For example, `"gemini"` becomes `google` and `"opencode_zen"` becomes `opencode`.

Used in:
- `AgentConfig.provider`, `dream_provider` and `tools.read_image.settings.provider`;
- `provider_config.variant`;
- `custom_provider.id`;
- `VizierResponseStats.provider`;
- API paths.

**Validation**: an id must resolve to exactly one of a catalogue provider, a built-in provider, or a custom provider. Custom providers win on collision (R10). An id that resolves to nothing is an error at agent build: the agent goes to `Failed`, with the message `provider "<id>" is not available; pick another provider`.

## 2. Catalogue (derived; one per process)

```rust
struct CatalogueData {
    source: CatalogueSource,        // Bundled (build-time seed) | Cached (catalogue_snapshot row)
    fetched_at: DateTime<Utc>,
    checked_at: Option<DateTime<Utc>>,
    providers: BTreeMap<ProviderId, CatalogueProvider>,
    logos: HashMap<String, Bytes>,  // id → SVG; absent = no logo
}

struct CatalogueProvider {
    id: ProviderId,
    name: String,
    env: Vec<String>,               // credential env vars, in lookup order
    api: Option<String>,            // base URL; may contain ${VAR}
    npm: String,                    // protocol hint
    doc: Option<String>,
    models: BTreeMap<String, CatalogueModel>,
    // derived:
    dialect: Option<Dialect>,       // None = unsupported (R2)
    kind: ProviderKind,             // Cloud | Local (api host is localhost/127.0.0.1)
}

struct CatalogueModel {
    id: String,
    name: String,
    family: Option<String>,
    caps: Capabilities,
    limit: Limits,
    cost: Option<Cost>,
    status: ModelStatus,            // Active | Beta | Alpha | Deprecated
    release_date: Option<NaiveDate>,
    knowledge: Option<String>,
    lab: Option<String>,            // canonical_model_id prefix, for logos
    route: Option<RouteOverride>,   // per-model {npm, api, shape} (R2)
}

struct Capabilities { tool_call: Option<bool>, reasoning: Option<bool>, attachment: Option<bool>,
                      temperature: Option<bool>, input: Vec<Modality>, output: Vec<Modality> }
struct Limits { context: Option<u64>, input: Option<u64>, output: Option<u64> }   // 0 → None
struct Cost { input: f64, output: f64, cache_read: Option<f64>, cache_write: Option<f64> } // USD per 1M tokens
enum Dialect { OpenAiChat, OpenAiResponses, Anthropic, Gemini, Ollama }
```

**Rules**:
- **Absence is unknown**: a missing capability is "unknown", never "false". Unknown counts as supported (FR-013).
- **Image input**: `image_input()` is true when `input` contains `image`; it is unknown when the list is empty.
- **Picker default list** (FR-031): models where `tool_call != Some(false)` and `status != Deprecated`, sorted by `release_date` descending. The first entry is the preselected default.
- **Lifecycle**: an agent captures `Arc<CatalogueData>` when it is built. A refresh swaps the shared pointer, and running agents see the new data from their next build.

## 3. Built-in providers (code; not stored)

Each built-in has the same shape as `CatalogueProvider`, plus a `CredentialSource`. Built-ins never have catalogue models; Ollama and llama.cpp list theirs live.

| id | kind | dialect | credentials | notes |
|---|---|---|---|---|
| `ollama` | Local | `Ollama` | `base_url` (`OLLAMA_BASE_URL`, default `http://localhost:11434`) | models from `/api/tags`; existing model pull kept |
| `llama-cpp` | Local | `OpenAiChat` | `base_url` (`LLAMA_CPP_BASE_URL`, default `http://localhost:8080`) | models from `/v1/models` |
| `chatgpt` | Cloud | `OpenAiResponses` (SSE) | `access_token`, `account_id`, optional `base_url` (`CHATGPT_*`) | `ChatGptAccount` source |
| `dummyplug` | Local | its own `CompletionModel` | none | protocol unchanged |
| `elevenlabs` | media-only | none | `api_key` (`ELEVENLABS_API_KEY`) | never in the chat picker |
| `voyageai` | media-only | none | `api_key` (`VOYAGE_API_KEY`) | embeddings only |
| `hyperbolic` | media-only | none | `api_key` (`HYPERBOLIC_API_KEY`) | TTS and image generation only |

## 4. `ProviderCredentials` (stored: `provider_config`)

```sql
-- unchanged table, re-keyed:
provider_config(variant TEXT PRIMARY KEY /* now a ProviderId string */, data TEXT /* ProviderCredentials JSON */)
```

```rust
struct ProviderCredentials {
    api_key: Option<String>,
    endpoint: Option<String>,      // azure resource endpoint
    base_url: Option<String>,      // local providers only (Kind::Local); refused for cloud (FR-027)
    access_token: Option<String>,  // chatgpt
    account_id: Option<String>,    // chatgpt
}
```

**Validation**:
- A `base_url` is accepted only when the provider is `Local` or a built-in that takes one.
- Fields the provider does not use are refused, with a 400 that names the field.

**Lookup** (FR-009, FR-016a):
1. The stored `ProviderCredentials` for the id.
2. Each entry of the provider's `env[]`, in order.
3. The legacy env var names for providers that had them (e.g. `GEMINI_API_KEY` for `google`, `ZAI_API_KEY` for `zai`, `COPILOT_API_KEY` for `github-copilot`), so existing deployments keep working.

The API reports where the credential came from: `saved` or `env:<NAME>` (FR-010a).

## 5. `CustomProvider` (stored: new `custom_provider` table)

```sql
CREATE TABLE IF NOT EXISTS custom_provider (id TEXT PRIMARY KEY, data TEXT NOT NULL);
```

```rust
struct CustomProvider {
    id: ProviderId,                     // fixed once saved
    name: String,
    protocol: CustomProtocol,           // OpenAiCompatible | Anthropic
    base_url: String,                   // http(s) URL, required
    api_key: Option<String>,            // never returned by the API
    headers: BTreeMap<String, String>,  // values never returned by the API
    models: Vec<CustomModel>,
    models_fetched_at: Option<DateTime<Utc>>,
}

struct CustomModel {
    id: String,
    context: Option<u64>,
    output: Option<u64>,
    tool_call: Option<bool>,
    image_input: Option<bool>,
    reasoning: Option<bool>,
    edited: bool,                       // set when the operator changes a row; a fetch never overwrites it
}
```

**State transitions**:

```
(none) ──create──▶ Saved ──edit──▶ Saved           (id immutable; key/header values replaced only when provided)
                     │──fetch models──▶ Saved      (merge by id: add new ids; keep edited rows; untouched rows refresh)
                     └──delete──▶ (none)           (warns if agents use it; agents on it then fail with "provider not available")
```

**Effective model data** (FR-024): use the custom model's fields where they are set. Fill the rest from the first catalogue model with the same id, preferring a provider whose dialect matches. Otherwise use name detection.

## 6. `ResolvedModel` (derived per agent build)

```rust
struct ResolvedModel {
    provider: ProviderId,
    model: String,
    dialect: Dialect,
    base_url: String,
    credential: Arc<dyn CredentialSource>,
    headers: Vec<(String, String)>,
    context_window: Option<u64>,     // R8 order
    max_output: Option<u64>,         // sent only if set or required by the dialect
    caps: Capabilities,
    cost: Option<Cost>,
}
```

It is built by `llm::resolve(&deps, provider_id, model_id, &agent_overrides)`. This is the only place that consults the catalogue, built-ins, custom providers, credentials and the route overrides. `VizierModel::new` becomes `resolve`, then `llm::build`, which returns `RetryingModel<HttpModel<D>>` as an `Arc<dyn CompletionModel>`. `caps` also drives the shared `prepare` step that handles attachments by capability (contract S6).

## 7. Agent config changes (`AgentConfig`, stored in `agent_config`)

| Field | Change |
|---|---|
| `provider`, `dream_provider`, `tools.read_image.settings.provider` | `ProviderVariant` becomes `ProviderId` (legacy names normalised on read, rewritten once on startup) |
| `context_window`, `max_tokens` | unchanged; they are now **overrides** of catalogue values. The WebUI leaves them empty by default |
| `tools.read_image` | unchanged shape; behaviour per FR-033 (see below) |
| `embedding.api_key`, `embedding.base_url` | no reader exists today. **Removed** (serde ignores the stored keys) |

**Read-image behaviour** (FR-033):

| Read-image enabled? | Explicit vision model? | Main model `image_input` | What happens |
|---|---|---|---|
| no | — | — | the attachment is injected as today; if `image_input` is `false`, FR-013 replaces the image with a note |
| yes | yes | any | that vision model describes the image (today's behaviour) |
| yes | no | not `false` | the image goes straight to the main model |
| yes | no | `false` | the image is replaced with a note, and the agent health shows a warning to configure a vision model |

## 8. Media and embedding providers (curated; enums kept)

`TtsProvider`, `SttProvider`, `ImageGenProvider` and `EmbeddingProvider` stay as enums with curated model lists (FR-016). Each gains `fn credentials(&self) -> ProviderId`. This is one curated table, not dispatch logic:

| Media provider | Uses credentials of |
|---|---|
| `Openai` | `openai` |
| `Gemini` | `google` |
| `Together` | `togetherai` |
| `Copilot` | `github-copilot` |
| `Groq`, `Mistral`, `Cohere`, `Openrouter`, `Huggingface`, `Xai` | the same-named provider |
| `Elevenlabs`, `Voyageai`, `Hyperbolic` | the media-only built-ins |
| `Ollama` | the `ollama` built-in (its `base_url`) |
| `Local`, `Kokoro`, `Whisper` | none |

## 9. `VizierResponseStats` additions

| Field | Type | Notes |
|---|---|---|
| `provider` | `Option<ProviderId>` | added with `serde(default)`, set by `VizierAgent::chat` |
| `model` | `Option<String>` | as above |

`AgentUsageStats` gains `estimated_cost_usd: Option<f64>` on its summary, on each day, and on each channel bucket. It is `None` when no response in the bucket has a known price (FR-021).

## 9a. Catalogue cache (stored: `catalogue_snapshot`, `catalogue_logo`)

The DDL is in [contracts/catalogue-snapshot.md](contracts/catalogue-snapshot.md#cache-sqlite).

- `catalogue_snapshot` is a single row: `schema`, `source_url`, `etag`, `fetched_at`, `checked_at`, `last_error`, `failed_at`, and `data` (the gzipped trimmed JSON).
- `catalogue_logo` holds `id`, `svg` (`NULL` means no logo exists) and `fetched_at`.

**States**:

```
(no row) ──first successful 200──▶ Cached
Cached ──304──▶ Cached (checked_at bumped)
Cached ──200 + valid──▶ Cached (data, etag, fetched_at replaced)
Cached ──error / invalid──▶ Cached (last_error, failed_at set; data unchanged)
```

## 10. Catalogue settings (config file)

| Key | Env var | Default | Notes |
|---|---|---|---|
| `vizier.catalogue.refresh` | `VIZIER_CATALOGUE_REFRESH` | `true` | R13 |
| `vizier.catalogue.ttl` | `VIZIER_CATALOGUE_TTL` | `24h` | freshness window for the lazy check |
| `vizier.catalogue.url` | — | `https://models.dev/api.json` | logos are fetched from the same origin |

`vizier.providers` is **removed** (FR-028). A leftover key is ignored, with a startup warning.

## 11. `src/llm` types

The full shapes, and the S1–S10 simplifications versus rig, are in [contracts/provider-layer.md](contracts/provider-layer.md). In short:

- **Messages**: `Message` is `System` / `User(Vec<Part>)` / `Assistant(Vec<AssistantPart>)` / `ToolResult { call_id, content }`.
- **Parts**: `Part` is `Text` / `Attachment { mime, data, name }`.
- **Assistant parts**: `AssistantPart` is `Text` / `ToolCall { id, name, arguments, provider_data }` / `Reasoning { text, provider_data }`.
- **Unchanged JSON and field names**: `ToolDefinition` (same JSON as rig's) and `Usage` (same field names).
- **Request and response**: `CompletionRequest { messages, tools, tool_choice, max_tokens }` and `CompletionResponse`.
- **Errors**: `LlmError`.
- **Traits**: `CompletionModel` (`HttpModel<D>`, `DummyplugModel`, `RetryingModel<M>`), `Dialect` (pure `encode`/`decode`, five implementations) and `CredentialSource` (three implementations).
- **Media**: the media clients implement the existing `Vizier*Model` traits directly.

**History mapping**: `SessionHistoryContent::ToolCall` and `ToolResult` map one-to-one to `AssistantPart::ToolCall` and `Message::ToolResult`. Reasoning is still not persisted.
