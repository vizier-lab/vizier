# Implementation Plan: AWS Bedrock Provider

**Branch**: `034-aws-bedrock-provider` | **Date**: 2026-10-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/034-aws-bedrock-provider/spec.md`

## Summary

Make Amazon Bedrock a first-class provider for **chat**, **memory embeddings** and **image generation**, with one IAM access-key connection, the catalogue's Bedrock chat models, and an in-form setup guide that walks a person through creating the key, attaching the minimal policy and enabling model access.

Five decisions carry the plan (details in [research.md](./research.md)):

1. **Bedrock becomes a dialect, not a provider special case** (R4). `DialectKind::Bedrock` implements the existing `Dialect` trait and translates to the unified **Converse** API: system blocks, alternating messages, grouped tool results, image blocks, `toolConfig`, `inferenceConfig` and the response's content/usage/stop reason. Chat works for every Bedrock model the Converse API supports; no per-family branching in the agent loop.
2. **SigV4 is one `CredentialSource` implementation over one pure signer** (R3). `src/llm/sigv4.rs` signs a method, URL, headers and exact body bytes with `ring`'s HMAC-SHA256 (already a direct dependency) and `chrono`; `SigV4Credential` plugs into the existing auth seam through a new body-aware default method (`authorize_with_body`). The HTTP path serializes the JSON once, signs those bytes and sends those bytes; media requests reuse the same helper. No AWS SDK, no new crate.
3. **The resolver and catalogue stay data-driven** (R5). `@ai-sdk/amazon-bedrock` maps to the new dialect; the regional endpoint is derived from the saved (or `AWS_REGION`/`AWS_DEFAULT_REGION`) region; the catalogue's 200 Bedrock chat models, limits, capabilities and prices are used unchanged. Missing keys, missing regions, bearer tokens and incomplete pairs each get their own actionable error.
4. **Embeddings and images are two small media clients behind the existing traits** (R6, R7). `BedrockEmbeddings` serves Titan Text Embeddings V2/V1 and Cohere Embed v3 (default Titan V2, 1024 dimensions) through `InvokeModel`; `BedrockImages` serves Nova Canvas (default) and Titan Image Generator V2 with the existing size field and a MIME-sniffed artifact. The existing `Endpoint` gains one body-aware POST helper; the existing embedder/image factories gain one match arm each. A wrong-sized embedding model against an existing index is caught with a clear error instead of writing corrupt vectors.
5. **The setup guide is provider data, the form stays generic** (R10). A static `ProviderGuide` for `amazon-bedrock` (steps, copyable policy, AWS links, docs anchor) rides on the provider listing; `ConnectForm` already renders provider metadata, so the new block is one component and no Bedrock branch. The same content is the new **Amazon Bedrock** section of the provider docs.

## Technical Context

**Language/Version**: Rust edition 2024 (single `vizier` binary, tokio multi-thread); TypeScript 5.9 / React 19 / React Router 7 (SPA) / Tailwind v4 in `webui/`; no WebUI or design-system change beyond the provider form and labels.

**Primary Dependencies**: No new crate. Reuse `reqwest`, `serde`/`serde_json`, `chrono`, `base64`, `ring` (HMAC-SHA256 and SHA-256), `axum`, `utoipa`, `tracing`, `parking_lot`. WebUI reuses `@vizier/design` `Button`, `TextField`, `Notice` and existing typography; no new design component.

**Storage**: Embedded SQLite, no schema change. The four credential fields live in the existing `provider_config` JSON blob; `secret_access_key` and `session_token` are added to the existing field-level encryption path, while `access_key_id` and `region` stay readable identifiers. Existing rows load with the new fields absent. Agent configuration needs no migration: the embedding and image provider enums gain a variant; existing values keep their meaning. No `#82` migration entry.

**Testing**: `cargo test` (SigV4 vectors and canonicalisation, dialect encode/decode, resolver precedence and errors, credential API/masking, encryption of the new fields, embedding/image bodies and parsing, index-dimension guard); `cargo clippy`; `cd webui && npm test` and `npm run typecheck`; offline end-to-end: the form/API/error/regression checks in [quickstart.md](./quickstart.md) §1–§4, including a dummyplug agent and a no-AWS-traffic check. **Live Bedrock is not available in-house**: §5–§6 are the volunteer verifier's checklist on the special release (chat, tools, vision, embeddings, images, usage, credential/region errors). No CI job or maintainer run touches AWS.

**Target Platform**: Linux, macOS and Windows via the single binary; modern evergreen browsers for the WebUI. No OS-specific code; SigV4 and the media clients use the existing `reqwest` transport.

**Project Type**: Web application inside the existing single-binary Rust service (Rust backend + `webui` submodule on the `design` submodule) plus the docs site.

**Performance Goals**: SigV4 adds well under 1 ms per request (two SHA-256 and four HMAC operations over a small body); chat latency is the provider's. Embeddings are bounded by the provider: Titan is one text per invocation, Cohere batches up to 96 texts per invocation; a memory save embeds its passages sequentially with no unbounded concurrency. Image generation inherits the existing 300 s media timeout. No polling, background refresh or startup work is added.

**Constraints**: zero AWS request at startup, provider/agent build, scheduler or dream (Constitution III, SC-013); one region per saved connection; no streaming; no Bedrock API keys/bearer tokens; no AWS credential chain; embedding and image calls stay outside chat token accounting; secrets never logged; the connect form must work without adding a Bedrock-specific branch; the minimal policy includes `bedrock:ListFoundationModels` because the Test action uses it (spec Assumptions, amended 2026-10-28).

**Scale/Scope**: one provider, three capabilities; 200 catalogue chat models (all regions' entries, cross-region inference profiles included); three embedding model families and two image model families; one new dialect, one signer, two media clients; one provider guide; one docs section; WebUI field/label additions and one guide block.

## Verification & Release Gate

The project has no Bedrock account. CI proves the wire translation, signing, credentials, encryption, UI and the existing agent loop; a maintainer runs the offline form/API/error checks (§1–§4); a volunteer with Bedrock access runs the live checklist (§5–§6) on the special release. The live layer is a **release gate, not a CI gate**: the provider ships as volunteer-verified, and any failure reported there is fixed before it is declared stable. The constitution's dummyplug e2e gate is met for the agent-observable non-Bedrock behaviour (regression and startup), while the real-provider path is the marked live step the constitution allows where behaviour depends on real model output. The limitation is recorded in the spec Assumptions, [research.md](./research.md) R12 and [quickstart.md](./quickstart.md).

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after design below.*

| Principle | Assessment |
|---|---|
| **I. Lean by default** | PASS. No new dependency: `ring` and `chrono` already sign/hash and timestamp; `reqwest` already sends. The change is one dialect, one signer, two media clients and metadata. No AWS SDK, no provider plugin framework, no per-model-family chat clients: Converse covers the chat families with one implementation. |
| **II. DRY via traits** | PASS. Bedrock plugs into the existing `Dialect` and `CredentialSource` seams; media plugs into `VizierEmbeddingModel`/`VizierImageGenModel`; the resolver remains the one place endpoint/auth rules live; the media `Endpoint` is reused. Family differences (Titan/Cohere, Nova/Titan Image) are one enum and one match in the media module, not scattered provider checks. The setup guide is data on the listing, not a form branch. |
| **III. Self-contained runtime** | PASS. Signing is in-process; no AWS SDK, sidecar or external service. The connection is opt-in; a fresh install makes no AWS call and needs no AWS configuration. The only network calls are a person's Test, an agent turn, a memory indexing operation or an image call. |
| **IV. Portability** | PASS. No OS, filesystem or architecture assumptions. `ring` and `chrono` already build for every target in `Cross.toml`; no new native library. |
| **V. Unified errors & observability** | PASS. Rust paths return `crate::Result`/`LlmError`; provider errors keep AWS's status/body and map into the existing retry classifier. Logging is `tracing`; the new secret fields and the SigV4 `Authorization` header are added to redaction. No `unwrap` outside tests. |
| **Workflow gates** | PASS with a recorded verification exception. `cargo clippy`, `cargo test`, `npm test`, `npm run typecheck` and the offline quickstart checks are in the task list. The dummyplug regression and no-startup-traffic checks run offline; the live Bedrock path is a volunteer release gate because the project has no Bedrock account (see Verification & Release Gate and Complexity Tracking). Docs are required by FR-024. No migration/legacy shim is added, so no `#82` entry. Conventional commits. |

**Post-design re-check**: no violations. Watch items: (a) SigV4 canonicalisation must match the AWS SDK's label encoding plus double-encoded canonical path — pinned by vectors and a live check; (b) the Test action now uses the control-plane listing, so the documented policy carries `bedrock:ListFoundationModels`; (c) `ToolChoice::None` has no Converse equivalent, so it is sent as the provider default (auto) and documented; (d) the embedding dimension guard is a new cross-provider safety check, so its error text must name the index's size, the model's size and the reindex path.

## Project Structure

### Documentation (this feature)

```text
specs/034-aws-bedrock-provider/
├── plan.md                          # this file
├── research.md                      # R1–R12 decisions and evidence
├── data-model.md                    # credential fields, guide, resolved connection, model families
├── quickstart.md                    # offline + live verification
├── contracts/
│   ├── bedrock-wire.md              # Converse/InvokeModel shapes, SigV4, errors, stop reasons
│   ├── provider-credentials-api.md  # fields, masking, validation, Test, guide payload
│   └── provider-layer.md            # dialect/signer/resolver/media seam contracts
├── checklists/requirements.md
└── tasks.md                         # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── config/provider.rs                 # + ProviderId::AMAZON_BEDROCK
├── llm/
│   ├── sigv4.rs                       # NEW: pure SigV4 canonical request/signature + vectors
│   ├── auth.rs                        # + SigV4Credential; authorize_with_body default
│   ├── model.rs                       # serialize body once, sign exact bytes, send them
│   ├── bedrock.rs                     # NEW: model-id encoding, connection fields, ListFoundationModels
│   ├── resolve.rs                     # + Bedrock dialect mapping, region/endpoint/auth rules
│   ├── http.rs                        # redact AWS Authorization / x-amz-security-token
│   ├── media/
│   │   ├── mod.rs                     # Bedrock endpoint branch; Endpoint::post_json
│   │   ├── openai_shaped.rs           # Endpoint::post_json helper
│   │   └── bedrock.rs                 # NEW: BedrockEmbeddings, BedrockImages + families
│   └── dialect/
│       ├── mod.rs                     # + DialectKind::Bedrock
│       ├── bedrock.rs                 # NEW: Converse encode/decode
│       └── golden_tests.rs            # Bedrock fixtures
├── embedding/mod.rs                   # + EmbeddingProvider::Bedrock; default model/family
├── image_generation/mod.rs            # + ImageGenProvider::Bedrock; default model/family
├── indexer/sqlite.rs                  # existing vec-table dimension must match the embedder
├── schema/
│   ├── agent.rs                       # + Bedrock variants, image default model, tests
│   └── provider.rs                    # + access_key_id/secret_access_key/session_token/region
├── provider_keys.rs                   # explicit AWS env mapping, extra env names
├── encryption/fields.rs               # protect/decrypt the two new secrets
├── catalogue/
│   ├── data.rs                        # + Bedrock dialect for @ai-sdk/amazon-bedrock
│   ├── mod.rs                         # Bedrock credential fields, media listing, guide
│   └── guide.rs                       # NEW: static Amazon Bedrock setup guide
├── channels/http/api/v1/
│   ├── providers/mod.rs               # fields in PUT/GET, masked booleans, sent/apply
│   ├── providers/usage.rs             # Bedrock Test via ListFoundationModels
│   └── catalogue.rs                   # ProviderSummary.guide; MEDIA_PROVIDERS
└── agents/tools/image_gen/mod.rs      # unchanged: it already picks the extension from the MIME

webui/app/
├── api/endpoints/catalogue.ts         # ProviderGuide type
├── api/endpoints/providers.ts         # new CredentialField values and booleans
├── api/schema.d.ts                    # regenerated from the OpenAPI
├── lib/providers.ts                   # labels + plain fields + credential summary
├── lib/agentForm.ts                   # + bedrock in embedding/image lists and defaults
├── screens/providers/ConnectForm.tsx  # render guide (steps, copyable policy, links)
└── screens/providers/*.test.tsx       # guide/fields rendering (new or existing)

docs/src/configuration/
├── providers.md                       # new "Amazon Bedrock" section (the form's stable link)
├── encryption.md                      # new protected provider fields
└── tools-embedding.md                 # Bedrock embedding/image providers and defaults
```

**Structure Decision**: single Rust service with the existing module layout, plus the `webui` submodule and docs. The new backend code lives beside the seams it implements (`llm/dialect`, `llm`, `llm/media`, `catalogue`), not in a new crate or a provider module; the media client is one file because both families share the same endpoint, signer and JSON transport.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Workflow-gate exception: the live Bedrock path is not exercised in-house | The maintainers have no Bedrock account; a volunteer with access verifies the special release against real AWS (§5–§6 of the quickstart) | Blocking the feature indefinitely on a credential, or shipping without any live verification; a mock cannot validate AWS signing, model access or real provider output. The offline suite and fixtures cover everything else. |

No other violations. No new project, dependency, abstraction layer or migration is added; the one new module (`catalogue/guide.rs`) is static provider metadata.
