# Research: AWS Bedrock Provider

**Feature**: `034-aws-bedrock-provider` | **Researched**: 2026-10-28 | **Plan**: [plan.md](./plan.md)

Provider facts were checked against AWS's Bedrock API reference and model-parameter guides on **2026-10-28**. Model availability, model access and per-region offerings remain the operator's AWS state; the catalogue is the source of chat models and prices.

## R1 — Provider identity, visibility and dialect mapping

**Decision**: Use the catalogue id `amazon-bedrock` (display name **Amazon Bedrock**) as the only Bedrock entry. Add `DialectKind::Bedrock` and map the catalogue's `npm: "@ai-sdk/amazon-bedrock"` to it in `catalogue/data.rs::dialect_for_npm`. Add `amazon-bedrock` to `MEDIA_PROVIDERS` so the media/embedding provider filters include it. Do not add a catalogue seed or a built-in provider.

**Rationale**: The catalogue already ships 200 Bedrock entries with ids, names, context/output limits, tool/vision capabilities and prices (verified in `assets/catalogue/api.json`). The provider is hidden today only because `dialect_for` returns `None` for `@ai-sdk/amazon-bedrock`. Mapping the npm is the same data-driven rule that maps `@ai-sdk/anthropic` and `@ai-sdk/azure`; no provider-id branch is needed in the picker, model listing or agent form. Keeping the catalogue id preserves the models, logos, search and usage views.

**Alternatives rejected**:
- A built-in `BuiltinProvider`: built-ins shadow catalogue providers and expose no models (`builtin_listing` sets `model_count: 0`), so Bedrock would lose its 200 models. A built-in would only be justified if the catalogue had no entry.
- A custom provider: loses catalogue data and requires a second service or URL.
- A second provider id for embeddings/images: the connection is one credential and one region; separate ids would duplicate settings.
- Adding a `ProviderKind::Bedrock`: `ProviderKind` describes local/cloud/media, not protocol; the dialect already carries the protocol.

**Test impact**: `catalogue/data.rs` and `catalogue/mod.rs` tests currently use `bedrock` as the example *unsupported* provider. They must switch that example to another catalogue provider with no dialect (`galadriel` is already used elsewhere) and assert that `amazon-bedrock` is listed with its fields.

## R2 — Credentials: four fields, one pair, env fallback

**Decision**: Extend the one generic credential shape with four fields:

| Field | Secret? | Source |
|---|---|---|
| `access_key_id` | No (identifier) | Settings, `AWS_ACCESS_KEY_ID` |
| `secret_access_key` | Yes (encrypted) | Settings, `AWS_SECRET_ACCESS_KEY` |
| `session_token` | Yes (encrypted) | Settings, `AWS_SESSION_TOKEN` (temporary credentials only) |
| `region` | No | Settings, `AWS_REGION`, then `AWS_DEFAULT_REGION` |

`CredentialField` gains `AccessKeyId`, `SecretAccessKey`, `SessionToken`, `Region`; for Bedrock the listing returns exactly those four (not `api_key`), the same shape as the existing Azure rule that adds `Endpoint`. `ProviderCredentials::{is_empty, merged_with}` include the new fields. `provider_keys.rs` maps the AWS names explicitly before its `contains("KEY")` heuristics (those heuristics would otherwise map both AWS key variables to `api_key` and silently drop the region), and one `extra_env` helper appends `AWS_SESSION_TOKEN` and `AWS_DEFAULT_REGION` both in `provider_env` (resolution) and in the catalogue listing (the UI and `connections`), because the catalogue lists only the first three plus the bearer variable.

**Why not a bearer token**: The catalogue's env list includes `AWS_BEARER_TOKEN_BEDROCK`, but the feature deliberately supports IAM access keys only (spec Assumptions). The resolver detects a bearer-only configuration and returns a specific error pointing at `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, instead of a generic "no key". The API refuses `api_key` for Bedrock because `check_fields` only accepts the listed fields.

**Alternatives rejected**:
- Reusing `api_key` for the access key ID or the bearer token: hides which secret is which and makes "incomplete pair" impossible to detect.
- A nested JSON `bedrock: {access_key_id, ...}` credential: the storage shape is one flat `ProviderCredentials`; a nested shape would need a second serializer and break the generic provider API.
- Reading `~/.aws` profiles: out of scope (spec Assumptions).

**Validation**: `access_key_id` and `secret_access_key` are both required for a request; one half is an error naming the missing field, and a save that would leave exactly one half set is refused for the same reason (setting the region alone before the keys is allowed). `region` is required and lightly validated (`^[a-z]{2,4}(-[a-z0-9]+)+-\d+$`, e.g. `us-east-1`, `us-gov-west-1`, `eusc-de-east-1`); anything else gets "invalid AWS region" before a request. A session token is optional and only sent when present.

## R3 — SigV4: one pure signer, one body-aware credential source

**Decision**: Add `src/llm/sigv4.rs` with a pure `sign` function and `src/llm/auth.rs::SigV4Credential`. Signing rules:

- Service `bedrock`, region from the connection, host from the request URL (`host[:port]` when non-default).
- Algorithm `AWS4-HMAC-SHA256`; `x-amz-date` = `YYYYMMDDTHHMMSSZ` UTC; credential scope `YYYYMMDD/region/bedrock/aws4_request`.
- Payload hash = lowercase hex SHA-256 of the **exact body bytes sent**; the body is hashed in the canonical request (no `x-amz-content-sha256` header is needed for Bedrock).
- Signed headers: `content-type`, `host`, `x-amz-date`, and `x-amz-security-token` when a session token is present; sorted lowercase, values trimmed, multi-values comma-joined.
- Canonical URI: the path as sent, then the SigV4 "double URI encode" step (percent-encode `%` to `%25`), matching the AWS SDK's `double_uri_encode = true` for Bedrock. The model id is a single path segment encoded with the Smithy `BASE_SET`: unreserved `A-Za-z0-9-._~` pass; `/`, `:`, `%`, `,` and the other sub-delims are `%XX` (uppercase).
- Signing key: HMAC-SHA256 chain (`AWS4` + secret → date → region → service → `aws4_request`); signature = hex HMAC-SHA256(signing key, string-to-sign).
- `Authorization: AWS4-HMAC-SHA256 Credential=<id>/<scope>, SignedHeaders=<list>, Signature=<hex>`.

`HttpModel::send_once` changes from `.json(body)` to serializing the body once, setting `content-type`, signing those bytes and sending the same bytes. The `CredentialSource` trait gains a default `authorize_with_body(req, info, method, url, body)` that delegates to `authorize`; `StaticKey` is unchanged, `SigV4Credential` signs, and `SigV4Credential::authorize` (no body) returns a config error so an accidental unsigned request is impossible. Media uses `Endpoint::post_json`, which signs its serialized body the same way. The `Authorization` and `x-amz-security-token` values are added to `http::redact` so a trace/debug log cannot leak them.

**Evidence**:
- The AWS Converse reference path is `POST /model/{modelId}/converse`; `InvokeModel` is `POST /model/{modelId}/invoke`.
- SigV4 canonical-request rules (URI-encode every path segment, include `host` and every `x-amz-*` header, include `content-type` when present) are the AWS "Create a signed AWS API request" guide.
- The AWS SDK Rust generated `converse.rs` sets `signing_options.double_uri_encode = true` and `normalize_uri_path = true`; its label formatter percent-encodes `:` and `/`. Following the official SDK avoids the widely reported signature mismatch from single-encoded paths.
- `ring` is already a direct dependency (`Cargo.toml`) and provides `hmac::sign` and `digest::digest`; `chrono` is already used for dates.

**Alternatives rejected**:
- The AWS SDK for Rust: a large dependency tree for four HMACs; the constitution prefers the smallest change and no unnecessary crate.
- `hmac`/`sha2` crates: `sha2` is present but HMAC is not; `ring` already covers both.
- Putting signing inside the dialect: `Dialect` is pure and has no URL/headers/body bytes; auth belongs in `CredentialSource`.
- `UNSIGNED-PAYLOAD`: only for S3; Bedrock signs the body.
- Encoding the model id raw: the official SDK encodes it; raw ARNs work in some clients but are not what the SDK sends.

**Tests**: the AWS SigV4 test-suite vectors (`get-vanilla`, `post-vanilla`, `post-vanilla-query`, session-token case) with fixed clock/credentials; a Bedrock-shaped POST with a fixed body and expected signature generated independently; model-id encoding cases (`…-v1:0`, cross-region profile, ARN) and canonical double-encoding.

## R4 — Chat dialect: Converse

**Decision**: `src/llm/dialect/bedrock.rs` implements `Dialect` for Converse:

- `system_style = Field` (the shared `prepare` joins leading system messages into one string); `path = /model/{encoded model}/converse`; `headers = content-type: application/json, accept: application/json`; `BodyKind::Json`.
- Request body:
  - `messages`: roles `user`/`assistant`; content blocks keyed by shape — `{text}`, `{image: {format, source: {bytes: base64}}}`, `{toolUse: {toolUseId, name, input}}`, `{toolResult: {toolUseId, content: [...]}}`.
  - **Strict alternation**: consecutive same-role messages are merged into one message with concatenated content blocks. This is required by Converse and covers both consecutive user turns and the N tool results of one assistant turn (the Anthropic dialect's grouping rule, generalised).
  - `system: [{text}]` when non-empty; `inferenceConfig: {maxTokens}` when the agent or catalogue supplies one; `toolConfig: {tools: [{toolSpec: {name, description, inputSchema: {json: parameters}}}]}` when tools exist.
  - `toolChoice`: `auto` is the default; `ToolChoice::None` has no Converse equivalent, so no `toolChoice` is sent (the model may call tools). Tools stay defined because history may hold tool calls. This is documented.
  - Images are only sent when `prepare` kept them (catalogue `image_input` is not false); non-image attachments are already replaced by the shared note.
- Response:
  - `output.message.content`: text → `Text`; `toolUse` → `ToolCall` with `input` as the arguments; `reasoningContent` → `Reasoning` when a text/signature is present, otherwise dropped; `citationsContent`/unknown blocks are ignored.
  - Empty content with `end_turn` → empty text (Anthropic's documented empty-reply case); otherwise an empty response is a decode error.
  - `stopReason`: `end_turn`/`stop_sequence` → `stop`, `tool_use` → `tool_call`, `max_tokens` → `length`, `content_filtered`/`guardrail_intervened` → `content_filter`, others lowercased as returned.
  - `usage`: `inputTokens`, `outputTokens`, `totalTokens` (or input+output when absent), `cacheReadInputTokens`, `cacheWriteInputTokens`.
  - `model` is the resolved model (Converse does not echo it); `id` is absent.
  - An error object in a 2xx body is a decode error; HTTP errors keep the existing status/body path.

**Evidence**: the Converse API reference (request/response syntax, `Message`, `ContentBlock`, `ToolConfiguration`, `ImageBlock`, `ToolResultBlock`), the tool-use guide, and the catalogued model capabilities. The ImageBlock note "only Anthropic Claude 3" is stale; the Nova and Claude model pages both document image input through Converse, so capability data decides.

**Alternatives rejected**:
- Reusing the Anthropic dialect with `anthropic_version` in the body: Bedrock's Anthropic models use a different path, `anthropic_version` location and model-id scheme; Converse is the unified surface.
- Per-family dialects (Claude/Nova/Llama): duplicates the message translation and contradicts the catalogue-driven capability model.
- Streaming (`ConverseStream`): not used anywhere in the provider layer; the turn waits for the full response.

**Tool-choice limitation**: Converse exposes only `auto`, `any` and `tool`. `ToolChoice::None` becomes auto; the spec's "where the model supports it" wording covers this. No attempt is made to force text by dropping tool definitions, which would risk rejecting histories that contain tool calls.

## R5 — Resolver: region, endpoint, auth and errors

**Decision**: In `llm/resolve.rs`, after the common limits/capabilities are filled:

1. Resolve region: `credentials.region` → `AWS_REGION` → `AWS_DEFAULT_REGION`; missing → `Config("no AWS region for \"amazon-bedrock\": set the region in Settings › Providers or AWS_REGION")`.
2. Validate region format; invalid → a message naming the value and the expected form, before any request.
3. Reject a bearer-only configuration (`api_key` set, no access key/secret) with "Bedrock API keys are not supported; set an access key ID and secret access key (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY)".
4. Require `access_key_id` and `secret_access_key`; a half pair names the missing field.
5. `base_url = https://bedrock-runtime.{region}.amazonaws.com`; `credential = SigV4Credential`; no `base_url` override is accepted for the catalogue provider (the generic `check_fields` already refuses it for cloud providers).
6. Chat model ids are sent exactly as the catalogue/agent gives them; cross-region prefixes (`us.`, `eu.`, `apac.`, `global.`) are not rewritten. The catalogue's model list is the picker's source; a typed id is accepted and the provider decides.
7. `fill_context_from_provider` does not call a model listing for Bedrock (no `list_models`); the catalogue's limits are authoritative, and an unlisted model gets the existing name-based/unknown fallback.

`provider_env` returns the catalogue's env list plus `AWS_SESSION_TOKEN` and `AWS_DEFAULT_REGION` for this provider. `provider_keys::credentials_from_env` maps AWS names explicitly (R2).

**Rationale**: `resolve.rs` is the only place that turns provider/model/overrides into a dialect, URL, credential and limits (its module doc says so). The Bedrock rule is the same kind of data-endpoint rule as Azure's resource URL and Copilot's token exchange, not a dialect branch.

**Error surface**: missing region/credentials are build-time `Config` errors (the agent reports a failed build with the message); a wrong region is a DNS/transport error naming the host; a model not in the region is a 404 `ResourceNotFoundException` body; model access is 403 `AccessDeniedException`; bad model/parameters are 400 `ValidationException`; throttling is 429; service unavailable is 503. The existing retry classifier retries 429/5xx/timeouts and never 400/403/404.

## R6 — Embeddings

**Decision**: Add `EmbeddingProvider::Bedrock` (display `bedrock`) and `VizierEmbedder` support:

| Family | Model ids (prefix) | Request | Response |
|---|---|---|---|
| Titan V2 (default) | `amazon.titan-embed-text-v2:0` | `{inputText, normalize: true}` (dimensions omitted → 1024) | `{embedding, inputTextTokenCount}` |
| Titan V1 | `amazon.titan-embed-text-v1` | `{inputText}` | `{embedding, inputTextTokenCount}` |
| Cohere Embed v3 | `cohere.embed-english-v3`, `cohere.embed-multilingual-v3` | `{texts: [..], input_type: "search_document"}` | `{embeddings: [[..]], id, response_type}` |

- An empty model resolves to `amazon.titan-embed-text-v2:0` (documented default); an explicit id is stored and used as typed.
- Unknown ids fail at embedder build with the supported families named, before any request.
- Titan has one text per call; `embed_texts` loops. Cohere batches in chunks of 96 (the documented maximum) and rejects a length mismatch.
- All supported models return 1024-dimensional vectors by default, so they are compatible in one index. No dimension setting is added.
- `Endpoint` is `/model/{encoded}/invoke`, content-type JSON, accept JSON; the shared `Endpoint::post_json` signs the exact bytes.
- `Cohere`'s `input_type` stays `search_document` for both indexing and queries, matching the existing `CohereEmbeddings` behaviour; distinguishing `search_query` is an existing cross-provider limitation, out of scope.

**Index dimension guard**: `SqliteIndexer::new` creates `document_index` with the dimension of a probe embedding. Today a later model change with a different dimension produces an opaque insert error. Add a check after the probe: read the existing virtual table DDL and compare the declared `float[N]` with the probe length; if they differ, return a clear error naming both sizes and saying the index must be rebuilt/reindexed. This is provider-neutral and satisfies FR-023.

**Alternatives rejected**:
- A per-provider embedding enum or dimension setting: the existing enum + fixed model default is enough; dimensions are a property of the model.
- Cohere-only or Titan-only: both are common on Bedrock and the difference is one request shape; supporting both costs little and matches the spec.
- Parallel Titan calls: memory saves already chunk passages; unbounded parallelism would multiply provider throttling. Sequential is predictable.

## R7 — Image generation

**Decision**: Add `ImageGenProvider::Bedrock`, default `amazon.nova-canvas-v1:0`; support Nova Canvas and Titan Image Generator V2:

| Family | Model ids (prefix) | Request | Notes |
|---|---|---|---|
| Nova Canvas (default) | `amazon.nova-canvas-v1:0` | `{taskType: TEXT_IMAGE, textToImageParams: {text}, imageGenerationConfig: {numberOfImages: 1, quality: "standard", width, height, cfgScale: 8.0}}` | sides 320–4096, multiples of 16, aspect 1:4–4:1, < 4,194,304 px |
| Titan Image Generator V2 | `amazon.titan-image-generator-v2:0` | same shape | documented sizes include 1024×1024; default width/height 1408 |

- Default size 1024×1024 when the agent leaves `size` empty. A provided `WxH` is parsed strictly; a malformed value is an error naming the expected form. The provider decides whether the dimensions are supported; its `ValidationException` is surfaced.
- Response `{images: [base64, ...], error?}`; the first image is decoded; a non-empty `error` (RAI output deflection) is a tool error and no file is written. MIME is sniffed from the bytes (`PNG`/`JPEG`/`WEBP` magic) and returned to the existing `image_generate` tool, which already picks the filename extension from it; no default to PNG when the bytes are another format.
- Prompt length and content are validated by the provider; no client-side truncation or negative-prompt fields are added (out of scope).
- The existing 300 s media timeout covers generation.

**Alternatives rejected**:
- Adding Stability AI models (different request schemas) now: out of scope; the family guard names the supported families.
- Reusing the OpenAI image client: different path, auth and body shape; it would need a provider check.
- Sending `size` as an aspect-ratio convenience: Bedrock takes explicit width/height; parsing `WxH` is exact.
- Multiple images: the agent's contract is one image per tool call; request one and save the first.

## R8 — Provider Test

**Decision**: `providers/usage.rs::check` gets a Bedrock branch that calls the control-plane `ListFoundationModels` (`GET https://bedrock.{region}.amazonaws.com/foundation-models`) with the same SigV4 credential, a 10–15 s timeout and no retries; it reports `model_count` on success. This verifies the access key, the region and the signed request without requiring model access or spending tokens. The spec's permission assumption is amended to include `bedrock:ListFoundationModels` for this action; the setup guide's copyable policy includes both actions.

**Rationale**: The existing Anthropic branch uses a 1-token completion because that is the best check for a key; for Bedrock, a completion would require the account to have enabled a specific model (a different, noisy failure). The control-plane listing is cheap, read-only and region-scoped, and returns a count the existing `TestResult` can show. It is an existing AWS API (`bedrock:ListFoundationModels`), not a new service.

**Alternatives rejected**:
- A Converse ping with `amazon.nova-lite-v1:0`: fails with "model access" when the operator enabled only Claude, which looks like a credential failure and violates the actionable-error goal.
- A bare credential-shape check: cannot detect a wrong region or a revoked key.
- `GetFoundationModel` per model: more calls, same permission class, no advantage over the listing.

## R9 — Usage, cost and accounting

**Decision**: Converse's `usage` feeds the existing `Usage` struct (`inputTokens`, `outputTokens`, `totalTokens`, cache read/write) and `VizierModel::complete_until` records it exactly as for every other provider. The catalogue's Bedrock prices feed `metrics::estimate_cost` through the saved reply's provider/model; a model with no catalogue price shows no invented cost. Embeddings and image calls stay outside chat token accounting, like every existing media call.

**Rationale**: No new accounting path or columns are needed. The existing `VizierResponseStats` already carries provider/model and the usage report groups by them.

## R10 — Setup guide, WebUI and docs

**Decision**: Add a serializable `ProviderGuide { intro, steps: [{title, body}], policy: {language, code}, links: [{label, url}] }` to `ProviderSummary`, sourced from a static `catalogue::guide::for_provider(&ProviderId)` table (only `amazon-bedrock` for now). The generic `ConnectForm` renders it above the fields in a `ToggleSection`/`<details>`: steps, a `<pre>` policy with a copy button, and the links. It is open for a new connection, collapsed while editing, and always reachable. No provider-id branch is added to the form.

Content (also the `docs/src/configuration/providers.md` section, anchor `#amazon-bedrock`):

1. Create an IAM user and access key (IAM console → Users → Security credentials → Create access key; *Application running outside AWS*; the secret is shown once).
2. Attach the minimal policy:
   ```json
   {"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["bedrock:InvokeModel","bedrock:ListFoundationModels"],"Resource":"*"}]}
   ```
3. Enable model access in Bedrock → Model access for the models to use (Anthropic may require a use-case form; embeddings/images need their models enabled too).
4. Choose a Bedrock region where the model is available (e.g. `us-east-1`); cross-region inference profile ids (`us.`, `eu.`, `apac.`, `global.`) are accepted as model ids.
5. Paste the access key ID, secret access key and region; add the session token only for temporary credentials.

Links: AWS IAM users, Bedrock model access, AWS supported models, and `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock`. `webui/app/lib/providers.ts` gains labels (`Access key ID`, `Secret access key`, `Session token (optional)`, `Region`) and marks `access_key_id`/`region` as plain fields; `credentialText` says "Saved keys" / "Keys from AWS_ACCESS_KEY_ID" and shows the region.

**Docs**: `providers.md` gets the section (also covering environment variables, the single region, the supported embedding/image families and defaults, and the absence of Bedrock TTS/STT); `encryption.md` adds `secret_access_key` and `session_token` to the protected provider fields; `tools-embedding.md` lists Bedrock for embeddings and images with the defaults. The WebUI is a submodule: no design component change is needed; `npm run typecheck` and the existing tests cover it. The form's guide text is the docs section, kept in one place by a stable anchor.

**Alternatives rejected**:
- Hard-coding a Bedrock panel in `ConnectForm`: violates the provider-agnostic form and duplicates the docs.
- A help modal or a separate onboarding page: more surface for five steps; the form is where the values are entered.
- Putting the policy in the docs only: the spec requires first-time setup without leaving the form.

## R11 — Encryption and secret handling

**Decision**: Extend `encryption/fields.rs::{protect_provider_entry, decrypt_provider_entry}` to protect `credentials.secret_access_key` and `credentials.session_token` with the same record-bound AAD (`provider_config`, record id, field path). `access_key_id` and `region` stay plaintext. `http::redact` adds the SigV4 `Authorization` value and `x-amz-security-token` header. The provider response exposes `access_key_id`, `region`, `has_secret_access_key` and `has_session_token`; it never returns a secret. The setup guide never includes a real key.

**Rationale**: The existing provider encryption is field-level and explicit; adding two fields is the smallest change. `access_key_id` is an identifier (it is not enough to sign) and showing it is how the person confirms the connection. No migration is needed: existing rows deserialize with `None` for the new fields, and the encryption write path applies to any new value.

**Tests**: protect/decrypt round trip with all four fields, sibling `access_key_id`/`region` unchanged, redaction of the SigV4 header and session token.

## R12 — Verification and risks

**Verification plan** (detailed in [quickstart.md](./quickstart.md)):

- **Offline**: `cargo test` (SigV4 vectors, dialect, resolver, credentials, encryption, embeddings/images, index guard), `cargo clippy`, `npm test`, `npm run typecheck`; `GET /catalogue/providers` shows Bedrock with four fields and the guide; `PUT /providers/amazon-bedrock` accepts/masks values; startup logs/connection attempts show no AWS traffic; a dummyplug agent still answers.
- **Live AWS (volunteer, marked)**: the project has no Bedrock account. A volunteer with access runs the special-release checklist: create an IAM user with the policy, enable a chat model, Nova Canvas and Titan Text Embeddings V2; connect, Test (model count), chat, tool call, image, memory write/search, usage/cost; negative checks for a missing secret, missing region, wrong region, model access and bearer-only config; verify the exact signed request works (no signature mismatch) and that a cross-region profile id is sent unchanged. This is a release gate, not a CI job.

**Risks and mitigations**:

| Risk | Mitigation |
|---|---|
| SigV4 canonicalisation mismatch (path encoding, headers) | Follow the AWS SDK's label encoding + double-encoded canonical path; pin the algorithm with the AWS test vectors; make the live check the first implementation step after the signer. |
| Model access / regional availability surprises | Use the catalogue's models for chat; surface `AccessDeniedException`/`ResourceNotFoundException` verbatim; document model access; the Test uses the control plane, not a chat model. |
| Image size family differences | Strict `WxH` parse, default 1024×1024, provider validation surfaced; supported families named in errors. |
| Embedding dimension change against an existing index | Add the provider-neutral dimension check in `SqliteIndexer::new` and fail with both sizes + reindex guidance. |
| Tool choice `None` cannot be expressed | Send no `toolChoice` (auto), document the limitation, keep tools defined for histories with tool calls. |
| Catalogue drift (model retired, region entry removed) | Treat the catalogue as a snapshot; typed ids remain valid; the provider's error is authoritative; no silent substitution. |
| 200 model rows clutter the picker | The existing searchable picker already handles catalogue providers; cross-region entries are distinguishable by id/name; no extra filtering that could hide a valid model. |

**Spec amendment**: the connection test now uses `bedrock:ListFoundationModels`; the spec's assumption and FR-011/FR-011a were adjusted on 2026-10-28 to match, and the guide's policy includes both actions.
