# Tasks: AWS Bedrock Provider

**Input**: Design documents from `/specs/034-aws-bedrock-provider/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md

**Tests**: Offline tests are included inside the implementation tasks because the plan's Testing section and the project constitution require them (`cargo test`, `cargo clippy`, WebUI `npm test`, `npm run typecheck`). **Live Bedrock is not run by the implementer** — there is no Bedrock account in-house. `quickstart.md` §5–§6 are packaged for a volunteer verifier on the special release (T047); they are not CI tasks.

**Organization**: Tasks are grouped by user story so each slice is independently implementable and testable. US1 and US2 are both P1; US2 extends the Converse dialect US1 introduces. US3 and US4 are independent capability slices but share `src/llm/media/bedrock.rs` as a serialization point. US5 is the operational slice over all of them.

**Before you start**: read [contracts/bedrock-wire.md](./contracts/bedrock-wire.md), [contracts/provider-credentials-api.md](./contracts/provider-credentials-api.md), [contracts/provider-layer.md](./contracts/provider-layer.md) and [data-model.md](./data-model.md) for the exact wire shapes, field rules, defaults and error mapping. Do not improvise the SigV4 canonicalisation, the Converse block shapes or the provider field names.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Include exact file paths in descriptions

## Path Conventions

Single Rust service with the existing layout: `src/`, `webui/app/`, `docs/src/`, `specs/034-aws-bedrock-provider/`. No new crate, no new package.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Provider identity, credential schema and the data-driven listing every story reads.

- [X] T001 [P] Add `ProviderId::AMAZON_BEDROCK = "amazon-bedrock"` in `src/config/provider.rs` and `DialectKind::Bedrock` in `src/llm/dialect/mod.rs` (serde/utoipa `snake_case`, dialect label `"bedrock"`).
- [X] T002 [P] Map `@ai-sdk/amazon-bedrock` to `DialectKind::Bedrock` in `src/catalogue/data.rs::dialect_for_npm`; replace the `bedrock` "unsupported provider" fixtures in `src/catalogue/data.rs` and `src/catalogue/mod.rs` with a provider that has no dialect (e.g. `galadriel`), and assert `amazon-bedrock` lists with `dialect == Some(DialectKind::Bedrock)` and its catalogue models.
- [X] T003 [P] Add `CredentialField::AccessKeyId`, `SecretAccessKey`, `SessionToken`, `Region` in `src/catalogue/builtin.rs`; add `access_key_id`, `secret_access_key`, `session_token`, `region` (all `Option<String>`, `serde(default, skip_serializing_if = "Option::is_none")`) to `ProviderCredentials` in `src/schema/provider.rs`, and include them in `is_empty()` and `merged_with()` (a non-empty incoming value replaces the stored one); add merge/is-empty tests.
- [X] T004 [P] Add the four field values to the WebUI `CredentialField` union in `webui/app/api/endpoints/providers.ts` and the `ProviderSummary.fields` union in `webui/app/api/endpoints/catalogue.ts`; add labels (`Access key ID`, `Secret access key`, `Session token (optional)`, `Region`) and mark `access_key_id`/`region` in `PLAIN_FIELDS` in `webui/app/lib/providers.ts`.
- [X] T005 Add the Bedrock listing rule in `src/catalogue/mod.rs`: `catalogue_listing` returns exactly `[AccessKeyId, SecretAccessKey, SessionToken, Region]` for `DialectKind::Bedrock` (no `api_key`), and a shared `extra_env` helper appends `AWS_SESSION_TOKEN` and `AWS_DEFAULT_REGION` to the catalogue's AWS env list. Add `amazon-bedrock` to `MEDIA_PROVIDERS` in `src/channels/http/api/v1/catalogue.rs`. Depends on T002 and T003.

**Checkpoint**: The provider is visible, the four fields exist end to end, and the listing is authoritative.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Signing, credentials, resolver, media transport and the chat dialect core. No user story can work without these.

**⚠️ CRITICAL**: Complete this phase before US1–US5 implementation.

- [X] T006 [P] Implement the pure SigV4 signer in `src/llm/sigv4.rs`: canonical request (method, already-encoded path, empty query, sorted lowercase headers, payload hash), `x-amz-date` = `YYYYMMDDTHHMMSSZ` UTC, scope `YYYYMMDD/{region}/bedrock/aws4_request`, the HMAC-SHA256 signing-key chain, and the `Authorization: AWS4-HMAC-SHA256 Credential=…/…, SignedHeaders=…, Signature=…` value. The canonical URI applies SigV4's double encode (`%` → `%25`). Signed headers: `content-type`, `host`, `x-amz-date`, plus `x-amz-security-token` when present. Test with the AWS SigV4 vectors (`get-vanilla`, `post-vanilla`, `post-vanilla-query`, session-token case) using a fixed clock and credentials.
- [X] T007 Add `SigV4Credential` and the default `async fn authorize_with_body(req, info, method, url, body)` method in `src/llm/auth.rs` (default delegates to `authorize`; `SigV4Credential::authorize` without a body returns `LlmError::Config`). In `src/llm/model.rs::send_once`, serialize the body **once**, set `content-type: application/json`, call `authorize_with_body`, and send those exact bytes (replace `.json(body)` on that path). Add `Authorization: AWS4-HMAC-SHA256 …` and `x-amz-security-token` masking to `src/llm/http.rs::redact` with a test. Depends on T006.
- [X] T008 [P] Implement `src/llm/bedrock.rs`: `aws_uri_encode` for one opaque model-id path segment (unreserved `A-Za-z0-9-._~` pass; `:`, `/`, `%`, `,` and other sub-delims become uppercase `%XX`), `model_path(model, "converse" | "invoke")`, region/endpoint helpers (`bedrock-runtime.{region}.amazonaws.com`, control plane `bedrock.{region}.amazonaws.com`), and `ListFoundationModels` (`GET /foundation-models`, count `modelSummaries`) signed with `SigV4Credential`; tests for versioned ids (`…-v1:0`), cross-region ids (`us.`, `eu.`, `apac.`, `global.`), ARNs, the endpoint strings and the model count. Depends on T006/T007.
- [X] T009 [P] Extend `src/provider_keys.rs` with explicit AWS mappings before the `contains("KEY")` heuristics: `AWS_ACCESS_KEY_ID` → `access_key_id`, `AWS_SECRET_ACCESS_KEY` → `secret_access_key`, `AWS_SESSION_TOKEN` → `session_token`, `AWS_REGION`/`AWS_DEFAULT_REGION` → `region`, `AWS_BEARER_TOKEN_BEDROCK` → `api_key` only so the "API keys unsupported" message can be produced. Reuse the `extra_env` helper from T005; add precedence tests (`AWS_REGION` before `AWS_DEFAULT_REGION`, saved row wins field by field).
- [X] T010 [P] Protect and decrypt `credentials.secret_access_key` and `credentials.session_token` in `src/encryption/fields.rs` with the existing record-bound AAD (`provider_config`, record id, field path); leave `access_key_id` and `region` plaintext. Add a round-trip test and a sibling-unchanged test.
- [X] T011 Extend `src/llm/resolve.rs` with the Bedrock rule: region `credentials.region` → `AWS_REGION` → `AWS_DEFAULT_REGION` (missing names the setting and both variables), region format `^[a-z]{2,4}(-[a-z0-9]+)+-\d+$` (invalid names the value and expected form), bearer-only (`api_key` set, no pair) refused with "Bedrock API keys are not supported; set an access key ID and secret access key (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY)", exactly one of `access_key_id`/`secret_access_key` names the missing field, `base_url = https://bedrock-runtime.{region}.amazonaws.com`, `credential = SigV4Credential`, empty headers/query, `stream_required = false`, and no `list_models` call in `fill_context_from_provider`. Add tests for every branch and update the old "bedrock unsupported" expectation. Depends on T001, T002, T006, T007, T009.
- [X] T012 [P] Add `Endpoint::post_json(path, body)` in `src/llm/media/openai_shaped.rs` (serialize once, `authorize_with_body`, send the same bytes, keep the 300 s timeout) and a Bedrock branch in `src/llm/media/mod.rs::endpoint` that resolves the same region/pair rules and builds a `SigV4Credential` against `https://bedrock-runtime.{region}.amazonaws.com`; every other provider keeps `media_base_url` + `api_key` behaviour. Depends on T007, T008, T009.
- [X] T013 Implement the chat core of `src/llm/dialect/bedrock.rs`: `SystemStyle::Field`, path `/model/{aws_uri_encode(model)}/converse`, `content-type`/`accept` headers, `system: [{text}]`, alternating `user`/`assistant` messages with consecutive same-role messages merged, text blocks, `inferenceConfig.maxTokens` (agent `max_tokens`, else catalogue output, omitted when neither), `usage` (`inputTokens`, `outputTokens`, `totalTokens`, cache read/write), stop reasons (`end_turn`/`stop_sequence` → `stop`, `tool_use` → `tool_call`, `max_tokens` → `length`, `content_filtered`/`guardrail_intervened` → `content_filter`, others lowercased), empty `end_turn` → empty text, and an error object in a 2xx body as a decode error. Add tests for multi-turn, usage, stop reasons and empty replies. Depends on T001, T008.
- [X] T014 Extend `src/channels/http/api/v1/providers/mod.rs`: add the four fields to `UpsertProviderRequest` (`#[serde(deny_unknown_fields)]`) and `sent()`/`into_credentials`/`apply`/`field_name`; expose `access_key_id`, `region`, `has_secret_access_key` and `has_session_token` on `ProviderResponse` (never the secrets) and include them in `connected()`; refuse a save that would leave exactly one of `access_key_id`/`secret_access_key` set, naming the missing field (setting only the region or a session token is allowed); refuse `api_key` for Bedrock through the existing `check_fields` path. Add save/mask/refusal tests. Depends on T003, T005, T009, T010.
- [X] T015 Regenerate the WebUI API types with `node scripts/openapi.mjs` (`webui/app/api/schema.d.ts`) after T014; add the new `CredentialField`/`ProviderResponse` fields to `webui/app/api/endpoints/providers.ts` if the generator leaves them hand-written, and run `cd webui && npm run typecheck`. Depends on T014.

**Checkpoint**: Signing, credentials, resolver, media transport and chat translation are ready; US1 can start.

---

## Phase 3: User Story 1 - Connect AWS Bedrock and talk to a model (Priority: P1) 🎯 MVP

**Goal**: Amazon Bedrock is connectable with IAM keys, the form teaches the setup, Test works without model access, and an agent can hold a multi-turn chat.

**Independent Test**: Offline: `GET /api/v1/catalogue/providers` lists Bedrock with four fields and the guide; `PUT` masks the secrets; the resolver returns the expected region/pair errors; dialect fixtures decode a Converse reply; a `dummyplug` agent still answers. Live: the volunteer follows `quickstart.md` §5.2–§5.4.

### Implementation for User Story 1

- [X] T016 [P] [US1] Add `src/catalogue/guide.rs` with the static, serializable `ProviderGuide { intro, steps: [{title, body}], policy: {language: "json", code}, links: [{label, url}] }` for `amazon-bedrock`: the five steps, the exact policy `["bedrock:InvokeModel","bedrock:ListFoundationModels"]` with `Resource: "*"`, and the four links (AWS IAM users, Bedrock model access, AWS supported models, `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock`). Register the module in `src/catalogue/mod.rs` and expose it as `ProviderSummary.guide: Option<ProviderGuide>` in `src/channels/http/api/v1/catalogue.rs` (`null` for every other provider); add a listing test asserting fields and guide.
- [X] T017 [US1] Render the guide in `webui/app/screens/providers/ConnectForm.tsx`: steps, a `<pre>` policy with a copy button, and the links; open for a new connection, collapsed while editing, and still reachable after a failed Test. Add the `ProviderGuide` type and `guide?: ProviderGuide | null` to `webui/app/api/endpoints/catalogue.ts`; add a component test that the policy text is exact and the guide survives a failed save/test. No provider-id branch. Depends on T016.
- [X] T018 [P] [US1] Add the Bedrock Test branch in `src/channels/http/api/v1/providers/usage.rs::check`: use `crate::llm::bedrock::list_foundation_models` with the resolved `SigV4Credential`, a 10–15 s timeout and one attempt (no retries); success returns `TestResult { ok: true, model_count: Some(n), .. }`, failure uses `TestResult::failed` with AWS's status/body and keeps saved credentials. Test the count/error shaping without network (helper/fixture).
- [X] T019 [P] [US1] Add Converse chat fixtures in `src/llm/dialect/bedrock.rs` and/or `src/llm/dialect/golden_tests.rs`: system + multi-turn text, empty `end_turn`, usage totals, `tool_use`/`max_tokens`/`guardrail_intervened` stop reasons, and an error object in a 2xx body.
- [X] T020 [P] [US1] Add the **Amazon Bedrock** section to `docs/src/configuration/providers.md` with anchor `#amazon-bedrock`: the five setup steps, the exact policy, IAM-only authentication, the `AWS_*` variables and precedence, the required region and single-region limitation, cross-region ids, the credential/region fields, and the note that Bedrock TTS/STT are not offered (name Amazon Polly/Transcribe). Keep the text identical to the guide.
- [X] T021 [US1] Wire and verify the offline connect/chat slice: add listing + guide tests in `src/channels/http/api/v1/catalogue.rs`, save/mask/refusal tests in `src/channels/http/api/v1/providers/mod.rs`, run the `quickstart.md` §2 form/API walkthrough, and record any failure in the task notes. Depends on T016–T018.

**Checkpoint**: US1 is independently demonstrable offline; the live chat check is the volunteer's §5.

---

## Phase 4: User Story 2 - Tools and image input (Priority: P1)

**Goal**: Tool calls, multiple tool results, tool choice and vision work through the same Converse dialect; a text-only model gets the note.

**Independent Test**: Dialect fixtures: `toolUse` decodes to `AssistantPart::ToolCall`, consecutive `toolResult`s merge into one user message, an image encodes as an `image` block, and a text-only model's image is replaced by the shared note. A tool-loop fixture decodes a `toolUse`, feeds the result back, encodes the next request, and proves no call is duplicated. Live: `quickstart.md` §5.5–§5.6.

### Implementation for User Story 2

- [X] T022 [US2] Extend `src/llm/dialect/bedrock.rs` with `toolUse`/`toolResult` blocks (consecutive results grouped into one user message), `toolConfig`/`toolChoice` (send `{"auto": {}}`; `ToolChoice::None` omits `toolChoice` and keeps tools defined), image blocks (`png`/`jpeg`/`gif`/`webp`, base64 `source.bytes`), unsupported-image notes, assistant reasoning blocks (only when a signature exists; otherwise dropped), unknown blocks ignored, and several calls per round; add tests for several calls/results, tool choice and images. Depends on T013 and T019 (same file).
- [X] T023 [P] [US2] Add `prepare` capability tests for Bedrock in `src/llm/prepare.rs`: an image is kept when `caps.image_input() != Some(false)` and replaced by the shared note when `Some(false)`; a non-image attachment is replaced by the note.
- [X] T024 [US2] Add a tool-loop fixture test in `src/llm/dialect/bedrock.rs` (or `src/llm/dialect/golden_tests.rs`): decode a `toolUse`, feed the tool result back, encode the next request, and assert every call appears once and the result is a user `toolResult` block.

**Checkpoint**: US1 + US2 both work; the live tool/vision checks are the volunteer's §5.5–§5.6.

---

## Phase 5: User Story 3 - Memory embeddings through Bedrock (Priority: P2)

**Goal**: Bedrock is selectable for embeddings; Titan V2 is the default, Titan V1 and Cohere v3 work; memory save/search is unchanged; a dimension mismatch fails clearly.

**Independent Test**: `src/llm/media/bedrock.rs` tests pin Titan/Cohere bodies and batching; `src/indexer/sqlite.rs` tests pin the dimension error; offline `memory_search` cannot call AWS, so the live recall check is the volunteer's §6.1–§6.3.

### Implementation for User Story 3

- [X] T025 [P] [US3] Add `EmbeddingProvider::Bedrock` in `src/schema/agent.rs` with `credentials() == Some(ProviderId::AMAZON_BEDROCK)`, `display_name() == "bedrock"`, and the documented default `amazon.titan-embed-text-v2:0`; add schema tests for credentials/default/display.
- [X] T026 [US3] Implement `BedrockEmbeddings` in `src/llm/media/bedrock.rs`: Titan V2 (`{"inputText","normalize":true}`), Titan V1 (`{"inputText"}`), Cohere v3 (`{"texts","input_type":"search_document"}`, chunks of 96 with one vector per input); family detection strips an optional `us.`/`eu.`/`apac.`/`global.` prefix; an unknown id errors naming the model and the supported families; `embed_text` uses the single-text path. All supported models return 1024 dimensions; no dimension setting. Add tests for request bodies, response parsing, wrong vector counts and batch length. Depends on T008/T012.
- [X] T027 [US3] Wire `src/embedding/mod.rs`: empty model → Titan V2, build `BedrockEmbeddings` through `media::endpoint`, and surface the family error at embedder build time. Depends on T026.
- [X] T028 [US3] Add the index dimension guard in `src/indexer/sqlite.rs::SqliteIndexer::new`: after the probe, read the existing `document_index` DDL (sqlite_master) and extract `float[N]`; when `N != probe.len()` return an error naming the index's dimension, the model's dimension and "the memory index must be rebuilt/reindexed before this embedding model can be used"; when the table is absent create it with the probe dimension as today; write no vector when the check fails. Test with a pre-created mismatched table. Depends on T027.
- [X] T029 [P] [US3] Add `bedrock` to `EMBEDDING_PROVIDERS`, the model hint/default (`amazon.titan-embed-text-v2:0`) and any labels in `webui/app/lib/agentForm.ts`; add the agent-form test.
- [X] T030 [P] [US3] Add the Bedrock embedding families, the documented default and the 1024-dimension rule to `docs/src/configuration/tools-embedding.md`, cross-linking the provider section.

**Checkpoint**: Embedding selection, wire clients and the index guard are complete; live recall is the volunteer's §6.1–§6.3.

---

## Phase 6: User Story 4 - Image generation through Bedrock (Priority: P2)

**Goal**: Bedrock is selectable for image generation; Nova Canvas is the default, Titan Image Generator V2 works; size, MIME and RAI errors behave.

**Independent Test**: `src/llm/media/bedrock.rs` tests pin the Nova/Titan request bodies, strict size parsing, base64 decode, RAI `error` and MIME sniff; the live image check is the volunteer's §6.4–§6.5.

### Implementation for User Story 4

- [X] T031 [P] [US4] Add `ImageGenProvider::Bedrock` in `src/schema/agent.rs` with `credentials() == Some(ProviderId::AMAZON_BEDROCK)` and the documented default `amazon.nova-canvas-v1:0`; add schema tests for credentials/default.
- [X] T032 [US4] Implement `BedrockImages` in `src/llm/media/bedrock.rs`: `{"taskType":"TEXT_IMAGE","textToImageParams":{"text":…},"imageGenerationConfig":{"numberOfImages":1,"quality":"standard","width":…,"height":…,"cfgScale":8.0}}`; absent size → 1024×1024; strict `WxH` parse (malformed names the expected form); Nova Canvas and Titan Image Generator V2 detected from the id (optional cross-region prefix stripped), unknown ids name the supported families; decode the first base64 image, a non-empty `error` is a tool error and writes no file; sniff PNG/JPEG/WEBP and return the MIME with the bytes. Add tests for every branch, including `100x100` surfacing the provider's validation error. Depends on T012 and shares the file with T026.
- [X] T033 [US4] Wire `src/image_generation/mod.rs` to build `BedrockImages` with the shared saved connection and region (empty model → provider default); add a factory test or compile-time match coverage. Depends on T032.
- [X] T034 [P] [US4] Add `bedrock` to `IMAGE_GEN_PROVIDERS`, the default model (`amazon.nova-canvas-v1:0`) and any labels in `webui/app/lib/agentForm.ts`; add the agent-form test.
- [X] T035 [P] [US4] Add the Bedrock image families, default model, default size 1024×1024 and size rules to `docs/src/configuration/tools-embedding.md`.

**Checkpoint**: Image generation is complete offline; live generation is the volunteer's §6.4–§6.5.

---

## Phase 7: User Story 5 - Credentials, health and cost (Priority: P2)

**Goal**: Rotation, removal, region changes, usage/cost and agent health behave like every other provider; secrets stay masked; one change affects only Bedrock agents.

**Independent Test**: API/encryption tests cover masking and rotation; a usage fixture prices a Converse reply from the catalogue; the restart path is the existing provider-change path; error classification tests cover 400/403/404/429/503 and retry exhaustion. Live rotation/error checks are the volunteer's §5/§6.7.

### Implementation for User Story 5

- [X] T036 [US5] Extend `src/channels/http/api/v1/providers/usage.rs` `features`/`used_by_all` so a Bedrock-configured chat, embedding and image agent counts under `amazon-bedrock`, and add a used-by test; verify `restart_users` (the existing provider-change path) is the only restart trigger and that other providers' agents are untouched. Depends on US1's Test/API and US3/US4 selections.
- [X] T037 [P] [US5] Add a usage/cost fixture test in `src/usage/` for a Converse reply: input/output tokens, catalogue price, no cost estimate when the model is unpriced, and embeddings/images staying out of chat token accounting.
- [X] T038 [P] [US5] Update `webui/app/lib/providers.ts::credentialText` for Bedrock (`Saved keys` / `Keys from AWS_ACCESS_KEY_ID`, region shown, never a secret) and add a test.
- [X] T039 [P] [US5] Add `secret_access_key` and `session_token` to the protected provider fields in `docs/src/configuration/encryption.md`; add credential lifecycle, rotation, removal, region-change and error explanations to the Amazon Bedrock section of `docs/src/configuration/providers.md`.
- [X] T040 [US5] Add API tests in `src/channels/http/api/v1/providers/mod.rs` for a complete save, masked response (`access_key_id`/`region` visible, `has_secret_access_key`/`has_session_token` booleans), invalid region, incomplete pair, bearer-only rejection, `api_key`/`base_url` refusal and deletion; cover encryption round-trip for the new fields in `src/encryption/fields.rs` if T010 did not already.
- [X] T041 [P] [US5] Add Bedrock error-classification tests in `src/llm/model.rs` (or the existing retry test module): 403 `AccessDeniedException` and 404 `ResourceNotFoundException` are not retried; 400 `ValidationException` is not retried; 429 `ThrottlingException`/`ModelNotReadyException`, 503 `ServiceUnavailableException`, 500 `InternalServerException` and 408 `ModelTimeoutException` are retried; an exhausted budget surfaces `RateLimited`/`ProviderUnavailable` with AWS's message, and `Retry-After` is honoured.

**Checkpoint**: Operational behaviour is covered offline; live rotation/error checks are the volunteer's §5/§6.7.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Schema/docs consistency, the offline gate and the volunteer release package.

- [X] T042 [P] Regenerate `webui/app/api/schema.d.ts` with `node scripts/openapi.mjs` after the API/guide types settle, and run `cd webui && npm run typecheck`.
- [X] T043 [P] Check the three docs pages agree (`docs/src/configuration/providers.md`, `docs/src/configuration/tools-embedding.md`, `docs/src/configuration/encryption.md`): the guide steps, policy, defaults, protected fields, `#amazon-bedrock` anchor and the TTS/STT absence; fix drift.
- [X] T044 Run the full offline gate from the repository root: `cargo test`, `cargo clippy`, `cd webui && npm test && npm run typecheck`; fix warnings/errors. Inputs: `Cargo.toml`, `webui/package.json`.
- [X] T045 Run `specs/034-aws-bedrock-provider/quickstart.md` §1–§4 on a fresh workspace (`--data-dir /tmp/vizier-bedrock-quickstart`), including the `dummyplug` regression (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`) and the no-startup-AWS-traffic check; record results in the task notes.
- [X] T046 [P] Verify no new dependency and no migration/legacy entry: inspect `Cargo.toml`/`Cargo.lock`, confirm `ring`/`chrono` only, and confirm no `#82` item is needed.
- [X] T047 Package the volunteer verification: finalize `specs/034-aws-bedrock-provider/quickstart.md` §5–§6 and the Verifier report, attach it to the special-release notes/tracking issue, and state that live failures are fixed before stable. Do not run the live steps.
- [X] T048 [P] Add the Bedrock provider/dialect/credential notes to the Providers section of `CLAUDE.md` and check that `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock` resolves to the new section.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup; blocks all user stories.
- **US1 (Phase 3)**: depends on Foundational (T006–T015).
- **US2 (Phase 4)**: depends on US1's dialect (T013, T019) and extends it; can follow US1 immediately.
- **US3 (Phase 5)**: depends on Foundational (T008, T012, T014) and can run in parallel with US2/US4, but shares `src/llm/media/bedrock.rs` with US4.
- **US4 (Phase 6)**: depends on Foundational (T008, T012, T014); shares `src/llm/media/bedrock.rs` with US3.
- **US5 (Phase 7)**: depends on US1 (T014/T018) and the US3/US4 selections being present; can run after US1 and alongside late US3/US4 work.
- **Polish (Phase 8)**: depends on all selected stories.

### User Story Dependencies

- **US1 (P1)**: independent after Foundational — the MVP.
- **US2 (P1)**: needs US1's dialect; no separate provider or credential work.
- **US3 (P2)**: needs Foundational only; independent of US2/US4 except the shared media file.
- **US4 (P2)**: needs Foundational only; independent of US3 except the shared media file.
- **US5 (P2)**: needs US1's Test/API and the capability selections; integration-only.

### Within Each User Story

- Dialect core before tool/image extensions.
- Media client before factory wiring.
- Schema variant before WebUI/default/docs.
- Offline tests before the story checkpoint.
- Live checks are packaged (T047), never run by the implementer.

### Parallel Opportunities

- Setup: T001–T004; T005 after T002/T003.
- Foundational: T006, T009, T010 can start together; T007/T008 after T006; T011 after T006–T009; T012 after T007–T009; T013 after T008; T014 after T003/T005/T009/T010; T015 after T014.
- US1: T016 (guide), T018 (Test), T019 (fixtures), T020 (docs) can run in parallel; T017 after T016; T021 after T016–T018.
- US2: T023 can run alongside T022; T024 after T022.
- US3: T025, T028, T029, T030 can run in parallel; T026 → T027; T026 and T032 are serialized on `src/llm/media/bedrock.rs`.
- US4: T031, T034, T035 can run in parallel; T032 → T033.
- US5: T037, T038, T039, T041 can run in parallel; T036/T040 after US1/US3/US4.

---

## Parallel Example: User Story 1

```bash
# Backend guide, Test, fixtures and docs can start together after Foundational:
Task: "Add src/catalogue/guide.rs and ProviderSummary.guide in src/channels/http/api/v1/catalogue.rs"
Task: "Add the Bedrock Test branch in src/channels/http/api/v1/providers/usage.rs"
Task: "Add Converse chat fixtures in src/llm/dialect/bedrock.rs"
Task: "Add the Amazon Bedrock section to docs/src/configuration/providers.md"
```

## Parallel Example: User Story 3

```bash
Task: "Add EmbeddingProvider::Bedrock in src/schema/agent.rs"
Task: "Add the index dimension guard in src/indexer/sqlite.rs"
Task: "Add bedrock to EMBEDDING_PROVIDERS in webui/app/lib/agentForm.ts"
Task: "Add Bedrock embedding docs to docs/src/configuration/tools-embedding.md"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Setup + Foundational.
2. Complete US1 (connect, guide, Test, chat fixtures).
3. **STOP and VALIDATE** offline: listing/guide/API mask/resolver errors + dialect fixtures.
4. US2 is the second P1 slice and should ship in the same special release; then US3/US4, then US5.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 → offline validation → volunteer can run §5.2–§5.4.
3. US2 → offline fixtures → volunteer can run §5.5–§5.6.
4. US3 → offline fixture/index tests → volunteer can run §6.1–§6.3.
5. US4 → offline fixture tests → volunteer can run §6.4–§6.5.
6. US5 → offline operational tests → volunteer can run §5/§6.7.
7. T047 hands the whole checklist to the volunteer; live failures are fixed before stable.

### Parallel Team Strategy

- One developer: Setup → Foundational → US1 → US2 → US3 → US4 → US5 → Polish.
- Two developers: after Foundational, A takes US1→US2 (dialect), B takes US3→US4 (media); coordinate `src/llm/media/bedrock.rs`; US5 after both.
- US1's guide/Test/docs and US3/US4's WebUI/docs tasks can be split further.

---

## Notes

- `[P]` means different files and no incomplete dependency; the shared `src/llm/media/bedrock.rs` and `src/llm/dialect/bedrock.rs` are serialization points.
- Live AWS checks are not run by the implementer; T047 packages them for the volunteer on the special release.
- Tests are offline only: SigV4 vectors, dialect fixtures, resolver/credentials/encryption, media bodies, index guard, WebUI tests/typecheck, and the dummyplug regression.
- Each task should be committed as a logical change; the final gate is T044 + T045.

---

## Verification notes (offline)

Recorded on 2026-10-29 while implementing this task list.

- **T001–T015**: `cargo test --bin vizier` passes (821 passed, 7 ignored); the SigV4 suite
  matches the AWS `get-vanilla`, `post-vanilla`, `post-vanilla-query`, IAM `ListUsers` and
  Bedrock session-token vectors, and the Converse encode/decode, resolver, credential API,
  encryption and media fixtures pass. `cargo clippy --all-targets` reports no warning in the
  new or changed code (the repository has pre-existing warnings elsewhere).
- **T016–T021**: with a fresh workspace (`--data-dir /tmp/opencode/vizier-034 --port 9876`),
  `GET /api/v1/catalogue/providers` lists `amazon-bedrock` with the four fields, 200 models,
  dialect `bedrock`, the six env names and the five-step guide; `PUT` refuses an incomplete
  pair, a malformed region, `api_key` and `base_url`; a complete save returns
  `access_key_id`/`region` and the two booleans without a secret; `DELETE` removes it. The
  persisted row stores the secret as an `enc:v1:` envelope and keeps the access key id and
  region readable.
- **T022–T024**: dialect fixtures cover several calls and results, tool choice, images,
  unsupported-image notes, signed-only reasoning, unknown blocks, empty `end_turn`, error
  objects in 2xx bodies, and a full tool-loop round trip with no duplicate call.
- **T025–T030**: embedding families/defaults, strict image sizes, request bodies, MIME sniffing
  and the index-dimension guard pass offline; the WebUI `npm test` (503 tests) and
  `npm run typecheck` pass.
- **T031–T035**: Nova/Titan family detection, the image request body and the size rules pass;
  `IMAGE_GEN_PROVIDERS`/`EMBEDDING_PROVIDERS` and the documented defaults are pinned by the
  agent-form tests.
- **T036–T041**: usage pricing fixtures, used-by coverage for chat/embedding/image,
  `credentialText`, encryption docs and the 400/403/404/408/429/5xx retry classification pass.
- **T042–T044**: `webui/app/api/schema.d.ts` regenerated from the running server; `cargo test`,
  `cargo clippy`, `npm test` and `npm run typecheck` all pass.
- **T045**: the dummyplug regression (`hello` and `tools`) answered through the real WebSocket
  turn path; the server log held no `bedrock`/`amazonaws` line after startup, a catalogue read
  and an agent build, and a Bedrock agent with no connection was refused at create time with
  the actionable missing-region error.
- **T046**: `Cargo.toml`/`Cargo.lock` are untouched (only `ring`, `chrono`, `base64` and
  `serde_json`, already direct dependencies); no migration, legacy shim or `#82` entry is added.
- **T047**: `quickstart.md` §5–§6 and its Verifier report are the volunteer package; posting it
  to the special-release tracking issue is an operational step at release time. Live AWS was
  not run (no Bedrock account in-house).
- **T048**: `CLAUDE.md` describes the Bedrock dialect, signer, credentials, media clients and
  guide; `docs/src/configuration/providers.md` has the `#amazon-bedrock` anchor the guide links
  to.
