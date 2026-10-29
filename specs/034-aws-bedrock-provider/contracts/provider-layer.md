# Contract: Provider Layer Seams

**Feature**: `034-aws-bedrock-provider` | **Plan**: [plan.md](../plan.md)

These are internal contracts between the modules that already exist. They are deliberately small: Bedrock must add implementations and data, not new dispatch branches.

## 1. `Dialect`

`DialectKind` gains `Bedrock`; `src/llm/dialect/bedrock.rs` implements:

```rust
impl Dialect for Bedrock {
    fn system_style(&self) -> SystemStyle { SystemStyle::Field }
    fn path(&self, m: &ResolvedModel) -> String;       // /model/{aws_uri_encode(model)}/converse
    fn encode(&self, req: &PreparedRequest, m: &ResolvedModel) -> Value;
    fn decode(&self, body: Value) -> Result<CompletionResponse, LlmError>;
    fn headers(&self) -> Vec<(&'static str, String)>;  // content-type, accept
}
```

Invariants:

- The dialect is pure: no I/O, no provider-id checks, no signing. It receives the resolved model and the prepared request only.
- `encode` preserves Vizier's message order while merging consecutive same-role messages; tool results become user content blocks.
- `decode` never panics; unknown blocks are ignored and an empty `end_turn` is empty text.
- `path` uses the shared `aws_uri_encode` so the exact path string is also the one the signer sees.
- `ToolChoice::None` is not representable in Converse; the dialect omits `toolChoice` and documents the limitation.

## 2. `CredentialSource`

The trait gains a body-aware method with a default, so every existing implementation keeps working:

```rust
#[async_trait]
pub trait CredentialSource: Send + Sync {
    async fn authorize(&self, req: RequestBuilder, info: &RequestInfo) -> Result<RequestBuilder, LlmError>;

    async fn authorize_with_body(
        &self,
        req: RequestBuilder,
        info: &RequestInfo,
        method: &str,
        url: &str,
        body: &[u8],
    ) -> Result<RequestBuilder, LlmError> {
        self.authorize(req, info).await
    }

    async fn base_url(&self) -> Result<Option<String>, LlmError> { Ok(None) }
    async fn invalidate(&self) -> bool { false }
}
```

`SigV4Credential`:

- fields: `access_key_id`, `secret_access_key`, `session_token: Option<String>`, `region`, `service: &'static str` (`"bedrock"`);
- `authorize_with_body` calls `sigv4::sign` and adds `x-amz-date`, `authorization`, `x-amz-security-token` when present;
- `authorize` (without a body) returns `LlmError::Config("SigV4 signing requires the request body; use the body-aware path")` so an unsigned request is impossible;
- it never logs secrets; `Debug` redacts them.

`HttpModel::send_once` and `Endpoint::post_json` are the only callers that must use `authorize_with_body`. `list_models` keeps `authorize` (Bedrock does not implement it). Other providers take the default method and behave exactly as before.

## 3. Transport body bytes

`HttpModel::send_once`:

```rust
let bytes = serde_json::to_vec(body).map_err(...)?;
let req = self.http.post(url)
    .header(CONTENT_TYPE, "application/json")
    .body(bytes.clone())
    .header(ACCEPT, "application/json"); // from the dialect headers
let req = self.resolved.credential
    .authorize_with_body(req, info, "POST", url, &bytes).await?;
http::send(req).await
```

Invariant: the bytes signed are the bytes sent. Do not use `.json(body)` on this path. The non-Bedrock providers see the same JSON bytes and headers.

`Endpoint::post_json(&self, path, body) -> Result<HttpResponse, LlmError>` follows the same rule for media calls and reuses the endpoint's 300 s timeout. Existing `Endpoint::post` remains for the unchanged clients.

## 4. Resolver (`llm/resolve.rs`)

The resolver remains the only place that reads the catalogue, credentials and endpoint rules. The Bedrock rule runs after limits/caps:

1. `dialect == Bedrock` is selected by `dialect_for_npm("@ai-sdk/amazon-bedrock")`.
2. Region from credentials → `AWS_REGION` → `AWS_DEFAULT_REGION`; missing/invalid is `LlmError::Config`.
3. Bearer-only (`api_key` set, no pair) is refused with the "Bedrock API keys are not supported" message.
4. `access_key_id` + `secret_access_key` required; the missing half is named.
5. `base_url = https://bedrock-runtime.{region}.amazonaws.com`; `credential = SigV4Credential`; `headers`/`query` empty; `stream_required = false`.
6. Limits/caps/cost come from the catalogue entry, as for every provider; the agent's overrides win.
7. A custom provider with the same id still wins (existing rule); a custom provider is the escape hatch for a different endpoint.

`provider_env` adds `AWS_SESSION_TOKEN` and `AWS_DEFAULT_REGION` for Bedrock. `fill_context_from_provider` does not call a listing for Bedrock; catalogue limits are used.

## 5. Media factories

- `EmbeddingProvider::Bedrock` → `EmbeddingProvider::credentials() == Some(ProviderId::AMAZON_BEDROCK)`; `VizierEmbedder::from_agent_settings` builds `BedrockEmbeddings` with the endpoint from `media::endpoint`.
- `ImageGenProvider::Bedrock` → `ImageGenProvider::credentials() == Some(ProviderId::AMAZON_BEDROCK)`; `VizierImageGen::new` builds `BedrockImages` the same way.
- `media::endpoint` gains one Bedrock branch: same credential resolution as the resolver (region/pair), same `SigV4Credential`, base URL `https://bedrock-runtime.{region}.amazonaws.com`. Other providers keep `media_base_url` + `api_key` behaviour. The branch is one place, not per client.
- `BedrockEmbeddings`/`BedrockImages` implement the existing traits; family detection is a small enum at construction and an error naming supported families otherwise.
- The existing `image_generate` tool already derives the filename extension from the returned MIME and saves the session file; no change is needed. Embeddings feed the existing indexer; the new dimension check is the only indexer change.

## 6. Index dimension guard

`SqliteIndexer::new` after the probe embedding:

- Read the existing `document_index` DDL (or its sqlite_master row) and extract `float[N]`.
- If `N != probe.len()`, return `anyhow::Error` with: the index's dimension, the model's dimension, and "the memory index must be rebuilt/reindexed before this embedding model can be used".
- If the table does not exist, create it with the probe dimension as today.
- No vector is written when the check fails.

This is provider-neutral and applies to every embedding provider; Bedrock is the first place the spec requires it.

## 7. Test action

`providers/usage.rs::check` gains a `DialectKind::Bedrock` branch:

- build a `SigV4Credential` from the resolved model (or a small `bedrock::connection` helper);
- `GET https://bedrock.{region}.amazonaws.com/foundation-models` with the same signer, 10–15 s timeout, one attempt;
- success → `TestResult { ok: true, model_count: Some(n), .. }`;
- failure → the existing `TestResult::failed`, including status/body; credentials kept.

This requires `bedrock:ListFoundationModels`; the guide policy includes it. No model access is needed for the test.
