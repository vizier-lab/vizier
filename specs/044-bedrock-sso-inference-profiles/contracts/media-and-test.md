# Contract: Test action and media on the SSO provider

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](../plan.md) | **Spec**: FR-012a, FR-012b, FR-033a, FR-033b, SC-016, SC-018

Two behaviours that differ from every other provider and therefore need their own contract: the connection **test is a real invoke**, and an inference-profile ARN is **chat only**.

---

## §1 The Test action (`POST /api/v1/providers/{id}/test`)

### Request

Body optional; when present it carries credentials to try before saving (existing shape).

```json
{ "region": "us-east-1", "model": "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123def456" }
```

`model` is **required** for this provider: it is the target of the invoke, and may be a catalogue id or any saved entry's ARN in either resource form. When omitted, the endpoint answers `400` with
`the test needs a model or inference profile to invoke; choose one first` and sends **no** AWS request (SC-018).

### Behaviour

1. Require a readable session (`SsoError::SessionExpired`/`SessionUnreadable` otherwise — a test never reports a session problem as a test failure).
2. Renew role credentials if needed (§5 of the wire contract).
3. Build the model exactly as a turn would (`llm::resolve::resolve_with` with the SSO id and the given `model`).
4. Send **one** `Converse` request: `Message::user_text("ping")`, `max_tokens: Some(1)`, `tools: []` — the same shape the Anthropic arm uses today (`crates/vizier-core/src/service/usage.rs:204-235`).
5. Map the outcome:

| Outcome | Response |
|---|---|
| 2xx | `200 { "ok": true, "model": "<the target>", "billing": "one billable inference call" }` |
| 403 `AccessDenied` naming an action | `200 { "ok": false, "error": "the role is not allowed to perform bedrock:InvokeModel; …" }` |
| 403 model access not enabled | `200 { "ok": false, "error": "model \"…\" is not enabled for this account in us-east-1" }` |
| 404 profile not found | `200 { "ok": false, "error": "inference profile \"…\" was not found; it may have been deleted" }` |
| 429 throttled | `200 { "ok": false, "error": "AWS throttled the test; try again shortly" }` |
| timeout (15 s) | `200 { "ok": false, "error": "request timed out" }` |

The `200 { ok: false }` shape is the existing `TestResult` (`crates/vizier-core/src/service/usage.rs:161`); a failed test is an answer, not an HTTP error.

### What it proves, and what it does not

- Proves: the session can mint role credentials, the role may invoke **this** target, and the model/profile exists.
- Does not prove: `bedrock:ListFoundationModels` (deliberately not required — FR-012b), that the account has any other model enabled, or anything about the entry's price. The test neither reads nor reports `pricing`: an unpaired or stale-priced entry is tested exactly like a priced one (FR-034a.5).

### Wording before the button is pressed (SC-018)

The guide's step 6 and the button's helper text both say: **"This sends one small billable inference call to the model you selected."** The WebUI renders that from the guide data — no provider branch in the form (FR-038).

---

## §2 Media: an ARN is a chat resource (FR-033a)

| Call | Today (`amazon-bedrock`) | With an ARN on `amazon-bedrock-sso` |
|---|---|---|
| chat | model string → `/model/{id}/converse` | ARN → `/model/{arn}/converse`, unchanged (FR-027) |
| embedding | family detected from the model id; unsupported family refused | **ARN refused first**, before the family check |
| image generation | same | **ARN refused first** |

Both resource types (`inference-profile` and `application-inference-profile`) are refused identically: the ban is on an ARN as a media model, not on one ARN form (FR-033a). The refusal names the capability and the foundation models that work, and never suggests a pairing — the entry's `model` field prices the entry and does **not** supply an embedding or image model (FR-034a.6).

Refusal (one message, both capabilities):

```text
an inference-profile ARN is a chat model; it cannot be used for
memory embeddings. Use one of the foundation models this connection supports
(amazon.titan-embed-text-v2:0, amazon.titan-embed-text-v1, cohere.embed-english-v3,
cohere.embed-multilingual-v3), or leave the field empty for the default.
```
For images the same sentence names `amazon.nova-canvas-v1:0` and `amazon.titan-image-generator-v2:0`. The wording says **inference-profile ARN**, not "application inference profile": both resource types are refused here, and a message that named only the application form would leave a person holding a system-defined profile unsure whether the rule applied to them.

Placement: the guard goes **before** `EmbeddingFamily::detect`/`ImageFamily::detect` in `crates/vizier-core/src/llm/media/bedrock.rs`, so no request is built and no index is written (the spec's Edge Case: "so a memory index is never written with a vector from a call that could not have produced one").

### The pickers

| Picker | Offers | Never offers |
|---|---|---|
| chat model | catalogue Bedrock models + saved entries (§5, provider API contract) | — |
| embedding model | the supported embedding foundation models + "default" | any ARN |
| image model | the supported image foundation models + "default" | any ARN |

### Defaults (FR-033b, unchanged)

| Capability | Default when the field is empty |
|---|---|
| embedding | `amazon.titan-embed-text-v2:0` (`crates/vizier-core/src/llm/media/bedrock.rs:16`) |
| image | `amazon.nova-canvas-v1:0` (`:18`) |

An SSO-only setup therefore reaches memory recall and image generation with no extra configuration — provided the account's model access covers those foundation models. When it does not, AWS's own refusal is passed through naming the model (spec Consequence note, Q1).

---

## §3 What an agent may select (per capability)

| Agent field | Accepted on this provider |
|---|---|
| `provider` | `amazon-bedrock-sso` |
| chat `model` | a catalogue Bedrock id, or a saved entry's ARN |
| `embedding.model` | a supported embedding foundation model (or empty) |
| `image_gen.model` | a supported image foundation model (or empty) |

An ARN in `embedding.model` or `image_gen.model` is refused at **build** time (agent start / settings save), not only at call time, so the mistake is visible in the UI rather than in a failed turn.
