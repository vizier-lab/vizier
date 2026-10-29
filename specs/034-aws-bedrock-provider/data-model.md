# Data Model: AWS Bedrock Provider

**Feature**: `034-aws-bedrock-provider` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

This feature adds one provider's credential fields, one provider listing, three capability selections (chat, embeddings, images) and one static setup guide. It adds **no database table and no stored row type**: the credentials live in the existing `provider_config` JSON, the chat model is an existing agent field, and the guide is static provider metadata.

## 1. Bedrock credentials (`ProviderCredentials`)

Extends the existing generic shape (`src/schema/provider.rs`). Every field is `Option<String>`; absence means unset.

| Field | Secret | Encrypted at rest | Source | Validation |
|---|---|---|---|---|
| `access_key_id` | No | No | Saved, `AWS_ACCESS_KEY_ID` | required with the secret; non-empty |
| `secret_access_key` | Yes | Yes | Saved, `AWS_SECRET_ACCESS_KEY` | required with the id; non-empty |
| `session_token` | Yes | Yes | Saved, `AWS_SESSION_TOKEN` | optional; only sent with the signature |
| `region` | No | No | Saved, `AWS_REGION`, `AWS_DEFAULT_REGION` | required; `^[a-z]{2,4}(-[a-z0-9]+)+-\d+$` |

Rules:

- `is_empty()` is true only when all fields (old and new) are empty.
- `merged_with()` treats a non-empty incoming value as replacing the stored one; this is how a PUT that omits a field keeps the stored value.
- A bearer-only configuration is detected by `api_key` being set while the key pair is absent; it is refused with the "Bedrock API keys are not supported" message. `AWS_BEARER_TOKEN_BEDROCK` maps to `api_key` only so this message can be produced.
- A save that would leave exactly one of `access_key_id`/`secret_access_key` set is refused, naming the missing field; setting the region (or a session token) before the pair is allowed. A stored incomplete pair fails at request time with the same message.
- The provider API accepts only the fields the listing declares. For Bedrock that is the four fields above; `api_key`, `base_url`, `endpoint`, `access_token` and `account_id` are refused (`404`/`400` as today).
- `ProviderResponse` exposes `access_key_id` and `region` in the clear, `has_secret_access_key` and `has_session_token` as booleans, and never a secret. `source` is `saved`, `env:AWS_ACCESS_KEY_ID`, … from the existing resolver.

`CredentialField` additions: `AccessKeyId`, `SecretAccessKey`, `SessionToken`, `Region` (serialised `access_key_id`, `secret_access_key`, `session_token`, `region`). For `amazon-bedrock` the listing returns exactly those four, not `api_key`; the WebUI labels are `Access key ID`, `Secret access key`, `Session token (optional)`, `Region`, with the identifier and region in the clear.

Environment precedence (unchanged shape): saved row wins field by field, then the catalogue's env list (plus `AWS_SESSION_TOKEN`, `AWS_DEFAULT_REGION`), then the legacy table. The origin label is the first variable used.

## 2. Resolved Bedrock connection

`ResolvedModel` (existing; no new persisted type) carries:

| Field | Bedrock value |
|---|---|
| `provider` | `amazon-bedrock` |
| `model` | the catalogue/agent id, unchanged (may be a cross-region profile or an ARN) |
| `dialect` | `DialectKind::Bedrock` |
| `base_url` | `https://bedrock-runtime.{region}.amazonaws.com` |
| `credential` | `SigV4Credential { access_key_id, secret_access_key, session_token, region, service: "bedrock" }` |
| `context_window`, `max_output`, `caps`, `cost` | from the catalogue, then the agent's overrides (existing rule) |
| `max_tokens` | the agent's value, sent as `inferenceConfig.maxTokens` |
| `headers`, `query` | empty |
| `stream_required` | false |

Resolution order: `region` from credentials → `AWS_REGION` → `AWS_DEFAULT_REGION` → error; `access_key_id` and `secret_access_key` required; `session_token` optional. An invalid region or an incomplete pair is a `Config` error before any request. A model id is never rewritten; the URL path encodes it once (`%XX` for everything outside the unreserved set) and the signer double-encodes `%` for the canonical URI.

## 3. Converse translation (domain types, not stored)

`DialectKind::Bedrock` maps the existing `PreparedRequest`/`CompletionResponse` types:

| Vizier | Converse |
|---|---|
| `system` | `system: [{text}]` |
| `Message::User` | `{role: "user", content: [text, image]}` |
| `Message::Assistant` | `{role: "assistant", content: [text, toolUse, reasoning]}` |
| `Message::ToolResult` | `{role: "user", content: [{toolResult: {toolUseId, content}}]}`; consecutive results are grouped into one message |
| `ToolDefinition` | `{toolSpec: {name, description, inputSchema: {json: parameters}}}` |
| `ToolChoice::Auto` | omitted / `{auto: {}}` |
| `ToolChoice::None` | omitted (Converse has no none; tools stay defined) |
| `Part::Attachment` (image) | `{image: {format: png/jpeg/gif/webp, source: {bytes: base64}}}` |
| `Part::Attachment` (other) | replaced by the shared capability note before encoding |
| `AssistantPart::Text` | `{text}` |
| `AssistantPart::ToolCall` | `{toolUse: {toolUseId, name, input}}` |
| `AssistantPart::Reasoning` | `{reasoningContent: {reasoningText: {text, signature?}}}` when a signature exists; dropped otherwise |
| `Usage` | `inputTokens`, `outputTokens`, `totalTokens`, `cacheReadInputTokens`, `cacheWriteInputTokens` |
| `finish_reason` | `end_turn`→stop, `tool_use`→tool_call, `max_tokens`→length, `content_filtered`/`guardrail_intervened`→content_filter, others as returned |

Rules: roles must alternate — consecutive same-role messages are merged; tool results are user content; an empty `end_turn` response becomes empty text; unknown content blocks are ignored; a non-2xx body keeps the existing HTTP error path.

## 4. Embedding selection (`EmbeddingConfig`)

`EmbeddingProvider::Bedrock` (new variant) with `model: String`:

| Family | Recognised ids | Default | Request | Response | Dimension |
|---|---|---|---|---|---|
| Titan V2 | `amazon.titan-embed-text-v2:0` | yes | `{inputText, normalize: true}` | `embedding` | 1024 |
| Titan V1 | `amazon.titan-embed-text-v1` | — | `{inputText}` | `embedding` | 1024 |
| Cohere Embed v3 | `cohere.embed-english-v3`, `cohere.embed-multilingual-v3` | — | `{texts, input_type: "search_document"}` | `embeddings` | 1024 |

Rules:

- Empty model → Titan V2; an explicit model is stored unchanged.
- Family detection ignores an optional cross-region prefix (`us.`, `eu.`, `apac.`, `global.`) before matching the model id.
- Unknown id → embedder build error naming the supported families and models.
- Titan embeds one text per call; `embed_texts` loops. Cohere batches in chunks of 96 and requires one vector per input.
- The existing `VizierEmbeddingModel` trait is unchanged; errors become `anyhow`/`VizierError` as today.
- `SqliteIndexer::new` checks the existing `document_index` declared dimension against the probe vector. A mismatch is a build error naming both dimensions and the reindex requirement; no wrong-sized vector is inserted.

## 5. Image selection (`ImageGenToolSettings`)

`ImageGenProvider::Bedrock` (new variant) with optional `model` and `size`:

| Family | Recognised ids | Default | Default size | Size rules |
|---|---|---|---|---|
| Nova Canvas | `amazon.nova-canvas-v1:0` | yes | 1024×1024 | each side 320–4096, multiple of 16, aspect 1:4–4:1, total < 4,194,304 px |
| Titan Image Generator V2 | `amazon.titan-image-generator-v2:0` | — | 1024×1024 | documented fixed size table; `1024×1024` is valid |

Rules:

- Empty model → Nova Canvas; typed id stored unchanged.
- `size` is `WxH`; absent → 1024×1024; malformed → error naming the format; provider validation is surfaced.
- Image blocks accept PNG/JPEG/GIF/WEBP; an image attachment in any other format is replaced by the shared attachment note rather than sent as an invalid block.
- Request: `{taskType: "TEXT_IMAGE", textToImageParams: {text}, imageGenerationConfig: {numberOfImages: 1, quality: "standard", width, height, cfgScale: 8.0}}`; response `{images: [base64], error?}`. The first image is decoded; a non-empty `error` is a tool error and writes no file.
- MIME is sniffed (PNG/JPEG/WEBP) and returned to the existing tool, which derives the extension from it.
- Generation uses the existing 300 s media timeout; no usage row is written (media accounting rule).

## 6. Provider setup guide

A static, serialisable value attached to `ProviderSummary` for `amazon-bedrock`:

| Field | Type | Rules |
|---|---|---|
| `intro` | `String` | One sentence: use an IAM access key pair with Bedrock permissions. |
| `steps` | `[{title, body}]` | Ordered: create key, attach policy, enable model access, choose region, paste values; each body names the exact AWS page or value. |
| `policy` | `{language: "json", code}` | The exact minimal policy: `bedrock:InvokeModel` + `bedrock:ListFoundationModels`, `Resource: "*"`; copied verbatim. |
| `links` | `[{label, url}]` | AWS IAM users, Bedrock model access, AWS supported models, and `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock`. |

The guide is not stored in SQLite, not user-editable and not included in any agent config. The docs section is the same content; the form links to its anchor. The form never receives or renders a saved secret in the guide.

## 7. Usage and cost

| Field | Source |
|---|---|
| input/output/total tokens | Converse `usage` |
| cached input/output tokens | `cacheReadInputTokens` / `cacheWriteInputTokens` where present |
| provider/model | the resolved provider (`amazon-bedrock`) and requested model |
| estimated cost | catalogue `Cost` for the model, through the existing usage report |

No `usage_event` schema change; no embedding/image accounting (existing rule). A Bedrock model without a catalogue price is recorded without a cost estimate.

## 8. Error model

| Situation | Surface |
|---|---|
| Missing region | `Config`: names the saved setting, `AWS_REGION`, `AWS_DEFAULT_REGION` |
| Invalid region | `Config`: names the value and the expected form |
| Incomplete key pair | `Config`: names the missing field |
| Bearer-only (`AWS_BEARER_TOKEN_BEDROCK`) | `Config`: explains Bedrock API keys are unsupported and names the access-key variables |
| Wrong region / DNS | `Transport`: includes the endpoint host (which contains the region) |
| Model not in region | HTTP 404 `ResourceNotFoundException` body |
| Model access / policy | HTTP 403 `AccessDeniedException` body |
| Bad model/params/size | HTTP 400 `ValidationException` body |
| Throttling / unavailable | HTTP 429/503; existing retry/backoff |
| Expired session token | HTTP 403 body naming expiry |
| Test (control-plane listing) | `TestResult { ok, model_count, status?, error? }`; never starts an agent |
