# Contract: Bedrock Wire (Converse, InvokeModel, SigV4)

**Feature**: `034-aws-bedrock-provider` | **Plan**: [plan.md](../plan.md)

This is the external contract between Vizier and Amazon Bedrock. It is not a new public API; it fixes what the dialect, signer and media clients must put on the wire.

## 1. Endpoint and request path

| Capability | Method | Path | Host |
|---|---|---|---|
| Chat | `POST` | `/model/{modelId}/converse` | `bedrock-runtime.{region}.amazonaws.com` |
| Embeddings | `POST` | `/model/{modelId}/invoke` | `bedrock-runtime.{region}.amazonaws.com` |
| Images | `POST` | `/model/{modelId}/invoke` | `bedrock-runtime.{region}.amazonaws.com` |
| Connection test | `GET` | `/foundation-models` | `bedrock.{region}.amazonaws.com` |

`modelId` is one opaque path segment. Encoding, applied once and sent:

- bytes `A-Z a-z 0-9 - . _ ~` pass through;
- every other byte (notably `:`, `/`, `%`, `,`) becomes uppercase `%XX`;
- the sent path is exactly this encoded string; the signer uses the same path and applies SigV4's double-encode step (`%` → `%25`).

Examples: `anthropic.claude-haiku-4-5-20251001-v1:0` → `anthropic.claude-haiku-4-5-20251001-v1%3A0`; a cross-region id keeps its dots and encodes the colon. The model id is never rewritten beyond this encoding.

## 2. SigV4 (service `bedrock`)

Inputs: method, encoded path, host, query (empty for these calls), headers, exact body bytes, access key ID, secret access key, optional session token, region, UTC time.

1. `x-amz-date = YYYYMMDDTHHMMSSZ` (UTC).
2. Payload hash = lowercase hex SHA-256 of the exact bytes sent.
3. Canonical request:
   ```
   {method}\n
   {double-encoded path}\n
   \n
   {canonical headers}\n
   {signed headers}\n
   {payload hash}
   ```
   Signed headers (sorted, lowercase): `content-type`, `host`, `x-amz-date`, and `x-amz-security-token` when a session token exists. Values are trimmed; multi-values are comma-joined.
4. Scope: `{YYYYMMDD}/{region}/bedrock/aws4_request`.
5. String to sign: `AWS4-HMAC-SHA256\n{amz-date}\n{scope}\n{sha256(canonical request)}`.
6. Signing key: `HMAC("AWS4"+secret, date)` → region → `bedrock` → `aws4_request`.
7. `Authorization: AWS4-HMAC-SHA256 Credential={id}/{scope}, SignedHeaders={list}, Signature={hex}`.
8. Add `x-amz-date`, `Authorization`, and `x-amz-security-token` when present. Do not send a bearer token.

A missing pair is a configuration error before signing. A 403 with `SignatureDoesNotMatch` means the canonicalisation is wrong; the request is not retried.

## 3. Converse request

Headers: `content-type: application/json`, `accept: application/json`.

```json
{
  "system": [{"text": "system prompt"}],
  "messages": [
    {"role": "user", "content": [{"text": "hello"}]},
    {"role": "assistant", "content": [{"text": "hi"}, {"toolUse": {"toolUseId": "t1", "name": "memory_search", "input": {"query": "x"}}}]},
    {"role": "user", "content": [{"toolResult": {"toolUseId": "t1", "content": [{"text": "result"}]}}]}
  ],
  "inferenceConfig": {"maxTokens": 1024},
  "toolConfig": {
    "tools": [{"toolSpec": {"name": "memory_search", "description": "…", "inputSchema": {"json": {"type": "object", "properties": {}}}}}],
    "toolChoice": {"auto": {}}
  }
}
```

Rules:

- Roles alternate. Consecutive same-role messages are merged into one message with concatenated content blocks; consecutive `toolResult` blocks are grouped into one user message.
- `system` is a block array; `inferenceConfig.maxTokens` is the agent's `max_tokens`, else the catalogue's output limit, and is omitted when neither is known.
- Images: `{"image": {"format": "png|jpeg|gif|webp", "source": {"bytes": "<base64>"}}}`. A model whose capabilities exclude images never receives one (`prepare` replaces it with a note).
- `ToolChoice::None` has no Converse equivalent; omit `toolChoice` (auto) and keep tools defined. Document the limitation.
- Do not send `temperature`/`topP` from this feature; the agent has no such fields today.
- Reasoning blocks are re-sent only when they carry a signature; otherwise dropped.

## 4. Converse response

```json
{
  "output": {"message": {"role": "assistant", "content": [{"text": "…"}, {"toolUse": {"toolUseId": "t1", "name": "…", "input": {}}}]}},
  "stopReason": "end_turn",
  "usage": {"inputTokens": 30, "outputTokens": 12, "totalTokens": 42, "cacheReadInputTokens": 0, "cacheWriteInputTokens": 0}
}
```

Mapping:

| Converse | Vizier |
|---|---|
| `output.message.content[].text` | `AssistantPart::Text` |
| `output.message.content[].toolUse` | `AssistantPart::ToolCall { id: toolUseId, name, arguments: input }` |
| `output.message.content[].reasoningContent` | `AssistantPart::Reasoning` when text/signature exists |
| `output.message.content[].citationsContent`, unknown | ignored |
| `stopReason: end_turn / stop_sequence` | `finish_reason: "stop"` |
| `stopReason: tool_use` | `"tool_call"` |
| `stopReason: max_tokens` | `"length"` |
| `stopReason: content_filtered / guardrail_intervened` | `"content_filter"` |
| other stop reasons | lowercased as returned |
| `usage.*` | `Usage { input_tokens, output_tokens, total_tokens, cached_input_tokens, cache_creation_input_tokens }` |
| `output.message` absent / empty with `end_turn` | empty text part (never a crash) |

`model` is the resolved model; Converse does not echo it. No response id is required.

## 5. Embeddings (`InvokeModel`)

Headers: `content-type: application/json`, `accept: application/json`.

| Family | Request | Response |
|---|---|---|
| Titan V2 (`amazon.titan-embed-text-v2:0`) | `{"inputText": "…", "normalize": true}` | `{"embedding": [float, …], "inputTextTokenCount": n}` |
| Titan V1 (`amazon.titan-embed-text-v1`) | `{"inputText": "…"}` | `{"embedding": [float, …], "inputTextTokenCount": n}` |
| Cohere v3 (`cohere.embed-english-v3`, `cohere.embed-multilingual-v3`) | `{"texts": ["…"], "input_type": "search_document"}` | `{"embeddings": [[float, …]], "id": "…", "response_type": "embeddings_floats"}` |

Rules: empty model → Titan V2; unknown model → error naming supported families; Titan one text per call, Cohere up to 96 texts per call; a response with the wrong number of vectors is an error; vectors default to 1024 dimensions; `embed_text` uses the same path with one text.

## 6. Images (`InvokeModel`)

Headers: `content-type: application/json`, `accept: application/json`.

```json
// Request (Nova Canvas and Titan Image Generator V2)
{
  "taskType": "TEXT_IMAGE",
  "textToImageParams": {"text": "a red fox"},
  "imageGenerationConfig": {"numberOfImages": 1, "quality": "standard", "width": 1024, "height": 1024, "cfgScale": 8.0}
}
// Response
{"images": ["<base64>"], "error": "optional RAI message"}
```

Rules: empty model → `amazon.nova-canvas-v1:0`; `size` `WxH` → width/height, absent → 1024×1024, malformed → error naming the form; the provider rejects unsupported dimensions (Nova: 320–4096, multiples of 16, aspect 1:4–4:1, < 4,194,304 px; Titan: documented size table). A non-empty `error` is a tool error and no file is written. Decode the first image, sniff PNG/JPEG/WEBP, return the bytes and MIME to the existing tool.

## 7. Errors

| AWS | HTTP | Surface |
|---|---|---|
| `AccessDeniedException` | 403 | policy/model-access message; no retry |
| `ValidationException` | 400 | provider message; no retry |
| `ResourceNotFoundException` | 404 | model/region message; no retry |
| `ThrottlingException` | 429 | retry/backoff |
| `ModelNotReadyException` | 429 | retry/backoff |
| `ServiceUnavailableException`, `InternalServerException` | 503/500 | retry/backoff |
| `ModelTimeoutException` | 408 | retry/backoff |
| expired session token | 403 | names expiry; no retry |
| `SignatureDoesNotMatch` | 403 | names signing; no retry; implementation bug |

The body is surfaced (truncated to the existing limit) so the person sees AWS's `message`/`Message`.
