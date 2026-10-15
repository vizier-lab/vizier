# Research: Catalogue-Driven Providers and Models (and Removing rig)

**Feature**: [spec.md](spec.md)

**Date**: 2026-10-15

**Inputs**:
- the live `https://models.dev/api.json`, fetched 2026-10-15: 230 providers and 8,870 models;
- every provider logo at `https://models.dev/logos/{id}.svg`;
- the pinned rig 0.39 source in `~/.cargo/registry`;
- the current code.

The earlier pre-spec draft is [rig-removal-notes.md](rig-removal-notes.md). The plan here supersedes it: rig is removed in one cut after the new layer is built, with no shim.

Each entry gives the **Decision**, then the **Rationale**, then the **Alternatives** considered.

---

## R1. What the catalogue looks like, and which parts we use

**Decision**: Read the catalogue as a map of provider id to provider entry.

| Provider field | Present on | Notes |
|---|---|---|
| `id`, `name`, `env[]`, `npm`, `doc` | all providers | |
| `api` | 205 of 230 | base URL |

Each model carries:

- `id`, `name`, `family`;
- `attachment`, `reasoning`, `tool_call`, `temperature`;
- `modalities.{input,output}`;
- `limit.{context,input?,output}`;
- `cost.{input,output,cache_read?,cache_write?}`;
- `status?` (`deprecated` / `beta` / `alpha`);
- `release_date`, `last_updated`, `knowledge?`;
- `canonical_model_id?`;
- `provider?`, a per-model override of `npm`, `api` and `shape`.

We keep only those fields. The snapshot drops `description`, `reasoning_options`, `structured_output`, `open_weights` and `interleaved`, and is gzipped: **about 350 KB**, against 5.9 MB raw.

**Rationale**: These are exactly the fields the spec consumes (FR-001, FR-012, FR-013, FR-021, FR-031). Trimming keeps the binary's size growth under 1 MB, logos included (R11).

**Alternatives**:
- *Embed the raw JSON*: 6 MB of mostly unused text.
- *Keep only supported-protocol providers*: dropping unsupported ones would lose model data that custom providers can borrow by matching model id (FR-024), so they are kept and only marked unsupported.

## R2. Deciding a provider's wire protocol ("dialect")

**Decision**: Derive the dialect from the catalogue's `npm` hint, **per model**. A model's `provider.npm`, `provider.api` and `provider.shape` override the provider-level values; 410 models carry such an override. The mapping:

| `npm` | Dialect | Notes |
|---|---|---|
| `@ai-sdk/openai-compatible` (190), `@openrouter/ai-sdk-provider`, `@ai-sdk/groq`, `@ai-sdk/mistral`, `@ai-sdk/xai`, `@ai-sdk/togetherai`, `@ai-sdk/deepinfra`, `@ai-sdk/cerebras`, `@ai-sdk/perplexity` | `openai_chat` | the latter group have no `api`, so vizier supplies a default base URL (R3) |
| `@ai-sdk/cohere` | `openai_chat` via `https://api.cohere.ai/compatibility/v1` | the catalogue itself uses this URL in a per-model override |
| `@ai-sdk/openai` | `openai_responses` | used by 32 opencode models and 17 github-copilot models, among others |
| `shape: "responses"` | `openai_responses` | |
| `@ai-sdk/anthropic` | `anthropic` | minimax, zenmux, opencode Claude models, azure Claude models |
| `@ai-sdk/google` | `gemini` | |
| `@ai-sdk/azure` | `openai_chat` with Azure auth and URL rules (R5) | |
| anything else (bedrock, vertex, `@ai-sdk/gateway`, gitlab, sap, watsonx, …) | unsupported | the provider is hidden from pickers (FR-008), but its models stay readable for id matching |

**Rationale**:
- The per-model override is not optional. Without it, opencode's Claude models would be sent as OpenAI chat and rejected, and Copilot's Codex-class models would hit `/chat/completions`.
- rig 0.39 hard-coded Copilot's routing between chat and Responses by model name. The catalogue now carries that, so we don't port it.

**Alternatives**:
- *Hand-maintain a provider → dialect table*: it recreates the hard-coded list the feature exists to remove.
- *Probe endpoints at runtime*: slow and unreliable.

## R3. Base URLs the catalogue leaves out

**Decision**: Keep a small static table of default base URLs, keyed by provider id, for supported providers that have no `api`:

| Provider | Default base URL |
|---|---|
| `openai` | `https://api.openai.com/v1` |
| `anthropic` | `https://api.anthropic.com` |
| `google` | `https://generativelanguage.googleapis.com/v1beta` |
| `groq` | `https://api.groq.com/openai/v1` |
| `mistral` | `https://api.mistral.ai/v1` |
| `xai` | `https://api.x.ai/v1` |
| `togetherai` | `https://api.together.xyz/v1` |
| `perplexity` | `https://api.perplexity.ai` |
| `cerebras` | `https://api.cerebras.ai/v1` |
| `deepinfra` | `https://api.deepinfra.com/v1/openai` |
| `cohere` | `https://api.cohere.ai/compatibility/v1` |

Azure is handled by R5. A supported provider with neither an `api` nor a table entry is treated as unsupported and logged at debug level.

**Rationale**: There are 11 rows, all for long-stable endpoints, and each was checked against the matching rig 0.39 client constant during research.

**Alternatives**: Read the URLs out of the `@ai-sdk/*` packages at build time. That would add a JavaScript build dependency for 11 strings.

## R4. Provider ids, built-ins and the legacy migration

**Decision**: Replace the `ProviderVariant` enum with `ProviderId(String)`, a validated newtype: lowercase `[a-z0-9._-]`, 1–64 characters.

- Its `Deserialize` maps legacy enum names to new ids. That keeps old agent JSON, old API clients and old provider rows readable (FR-015, edge case "an old client").
- A one-time startup migration in `dependencies.rs` rewrites the stored `agent_config` and `provider_config` rows and logs each change, so the mapping isn't needed forever.
- The full map is in [contracts/provider-id-migration.md](contracts/provider-id-migration.md).

Built-in providers, which are not in the catalogue, are defined in code with the same shape as catalogue providers:

| Built-in | Kind | Dialect |
|---|---|---|
| `ollama` | local | native `ollama` |
| `llama-cpp` | local | `openai_chat` |
| `chatgpt` | subscription | `openai_responses` over SSE |
| `dummyplug` | offline test provider | its own `CompletionModel` |
| `elevenlabs`, `voyageai`, `hyperbolic` | credentials only, used for media and embeddings (FR-016a) | none, never offered for chat |

`galadriel` and `mira` map to nothing. Agents that use them fail with a clear message (Story 1, scenario 4).

A saved legacy `base_url` decides the target when it matches a catalogue provider's `api`. For example, Moonshot with `api.moonshot.cn` maps to `moonshotai-cn`, and an `openai` entry pointed at `token-plan-sgp.xiaomimimo.com` maps to `xiaomi-token-plan-sgp`. A saved base URL that matches nothing becomes a **custom provider**, because catalogue base URLs are fixed (FR-027).

**Rationale**:
- A string id is the only shape that can name 230 catalogue providers plus operator-defined ones.
- Normalising at deserialisation means a missed row never breaks an agent, and the explicit rewrite still gives the logged once-only migration the spec asks for.

**Alternatives**:
- *Keep the enum and add a `Catalogue(String)` variant*: two spellings for one thing, and every `match` stays.
- *Rewrite only, with no read-time mapping*: one missed code path or old client breaks agents.

## R5. Provider quirks that are not data

**Decision**: Quirks are keyed by provider id and expressed as trait impls, not as branches at call sites (Principle II). There are two seams:

- **`CredentialSource`** (trait): turns stored credentials into request auth. It has three implementations:
  - `StaticKey`: a bearer token, or a named header such as `x-api-key` / `api-key`, or a query key;
  - `CopilotExchange`: trades the GitHub token at `api.github.com/copilot_internal/v2/token` for a short-lived token, caches it until expiry, and re-fetches on a 401. Ported from rig `copilot/auth/native.rs`;
  - `ChatGptAccount`: an access token plus the `chatgpt-account-id` header.
- **`EndpointRule`** (data on the provider definition): a URL template, plus extra fixed headers. Two cases need it:
  - **Azure**: `https://{resource}.openai.azure.com/openai/deployments/{model}/chat/completions?api-version=…`, using `AZURE_RESOURCE_NAME` or a saved endpoint. The `${AZURE_RESOURCE_NAME}` placeholder already appears in catalogue `api` values and is expanded the same way.
  - **opencode-go**: an `x-opencode-session` header, minted once per client. This keeps today's behaviour from `model/provider.rs`.

Anthropic automatic prompt caching is a behaviour of the `anthropic` dialect, so it applies wherever that dialect is used. That includes MiniMax and opencode's Claude models, which is a small improvement.

**Rationale**:
- These are the only places rig 0.39 did something beyond the wire format for the providers we keep.
- Expressing them as auth and endpoint seams keeps dialect code free of provider names.

**Alternatives**: `if provider == "github-copilot"` checks inside the dialects. This is rejected by Principle II and its review gate.

## R6. Our own HTTP and error layer

**Decision**: Use `reqwest` 0.13, which is already a dependency, with its `json` and `multipart` features turned on.

- One shared `reqwest::Client` is held on `VizierDependencies`.
- Every non-2xx response becomes `LlmError::Http { status, retry_after: Option<Duration>, body }`. Connection resets, refusals and socket timeouts become `LlmError::Transport`, and timeouts become `LlmError::Timeout`.
- `retry.rs::classify` decides from the typed status first. `classify_text` is kept only for providers that report an error as HTTP 200 with an error body.
- A `Retry-After` header (in seconds or as an HTTP date) sets the wait to `max(backoff, retry_after)`, capped by the turn deadline (FR-019).
- `LlmError` converts into `VizierError`. The text prefixes `ErrorKind::classify` relies on (`RATE_LIMITED`, `PROVIDER_UNAVAILABLE`, `PROMPT_TIMED_OUT`) are unchanged.

**SSE**: only ChatGPT's Codex backend requires streaming. rig 0.39 reads the whole SSE body as text and then parses it (`parse_sse_completion_body`), so we do the same: no `stream` feature and no SSE crate, just a small parser for `event:`/`data:` lines.

**Rationale**: This is the classification gap `retry.rs` documents today, and the smallest dependency footprint.

**Alternatives**:
- *`eventsource-stream`*: a new crate for something we parse once, in full.
- *`async-openai`*: it covers one dialect and would add a second HTTP stack.

## R7. Our own message types, and the reasoning round-trip

**Decision**: Write `src/llm/types.rs` around how vizier actually calls the model, rather than mirroring rig's types. The shapes, and the ten simplifications S1–S10, are in [contracts/provider-layer.md](contracts/provider-layer.md). In short:

- **One `messages` list**, with no separate "prompt" message (S3).
- **`Message::ToolResult` as its own kind** (S4). This matches the one-entry-per-call shape of `SessionHistoryContent`, so `history.rs` loses its grouping logic.
- **One tool-call id** (S5).
- **One `Part::Attachment { mime, data }`** (S6), handled by capability in a single `prepare` step.
- **`tool_choice` on the request**, replacing the `answer_only` method (S7).
- **System messages allowed anywhere**, normalised once per dialect style (S8).

`ToolDefinition` JSON and `Usage` field names stay identical to rig's on purpose: the sandbox docs, the 014 fixtures and token accounting depend on them.

Persisted history still drops reasoning, as it does today. Within a turn, `AssistantPart::Reasoning { text, provider_data }` stays in memory, and each dialect sends it back:

- **Anthropic**: thinking signatures.
- **DeepSeek**: `reasoning_content`.
- **Responses**: encrypted reasoning items.
- **Gemini**: `thoughtSignature`.

**Rationale**:
- History never stored rig types, so no data changes.
- Anthropic and DeepSeek require the reasoning round-trip in tool loops with thinking enabled.
- Shaping the types to vizier's own history removes conversion code instead of moving it.

**Alternatives**:
- *Port rig's types one-to-one*: keeps the `OneOrMany`, tool-results-inside-`User` and `id`/`call_id` complexity that vizier never needed.
- *Keep rig's types and remove only its transport*: leaves the dependency in place, which is the point of #69.

## R8. Effective context window and max output (FR-012, FR-032)

**Decision**:

- **Context window**: the agent's explicit `context_window`, else the catalogue's `limit.input` if present, else `limit.context`, else the provider's model listing (Ollama `/api/show`, OpenRouter), else `registry::detect_context_window`. A value of `0` or a missing one counts as unknown.
- **Max output**: sent only if the agent sets `max_tokens`, or if the dialect requires it (Anthropic). In the second case the value is the catalogue's `limit.output`, else rig's fallback of 2,048.

**Rationale**:
- `limit.input` is the real prompt budget where it differs from the context window. OpenAI's `gpt-5` has a context of 400k but input of 272k, and checkpoints must trigger before the input limit.
- rig 0.39 already sends Anthropic models their full synchronous output limit (`default_max_tokens_for_model`: 64k or 128k), so using the catalogue's output limit is the same behaviour, generalised to every model.

**Alternatives**: Always send `limit.output`. That changes behaviour for OpenAI-style providers, which don't need it, and can trip per-account output caps.

## R9. Where the catalogue lives, and how it refreshes

**Decision**: Three layers, detailed in [contracts/catalogue-snapshot.md](contracts/catalogue-snapshot.md).

1. **Seed at build time, from a committed file.** `assets/catalogue/{meta.json, api.json, logos/*.svg}` is committed. `api.json` is trimmed, sorted, one-model-per-line plain JSON, so git stores small deltas and refresh diffs can be reviewed. `build.rs` gzips it into `OUT_DIR` (`flate2`, already in the tree, as a build-dependency). The binary embeds about 650 KB. `Cargo.toml` `include` gains `assets/catalogue` so the crates.io package builds. **The seed is downloaded once per release**, in `release.yml`'s `prepare-release` job, before the version-bump commit and tag. Every binary, the crates.io package and the Docker image therefore ship the same snapshot. If models.dev is down, the committed snapshot is kept and the run warns; a release is never blocked. There is no scheduled job between releases; developers run `just catalogue-update`. `cargo build` never accesses the network.
2. **Cache in SQLite.** The single-row `catalogue_snapshot` table holds the gzipped trimmed JSON, plus `etag`, `fetched_at`, `checked_at`, `last_error` and `failed_at`. A `catalogue_logo` table holds logos. Writes happen in one transaction.
3. **Memory.** At startup, the newer of the seed and the cache is parsed into `CatalogueData`, held as `Arc<RwLock<Arc<…>>>` (`parking_lot`). Every query reads memory.

**Refresh is lazy and conditional (stale-while-revalidate).** A check starts only when a person reads the catalogue: a `/catalogue/*` endpoint, `GET /providers`, or the "Refresh now" button.

- The check runs when the cached data is older than `catalogue.ttl` (default 24h), no check failed in the last hour, and no check is already running. It is one background task, and the caller is served at once from memory.
- The request is `GET` with `If-None-Match`:
  - a `304` only updates `checked_at`;
  - a `200` is validated (same rules as before), saved and swapped in.
- **Agent builds, turns, the scheduler and the dream cycle never trigger a check** (owner decision, 2026-10-15). A headless install never contacts models.dev.

**Measured on 2026-10-15**: models.dev returns an `ETag`, and `If-None-Match` gets a `304` with no body. A full download is about 560 KB gzipped, so an unchanged catalogue costs one tiny request.

**Rationale**:
- Lazy plus conditional is the least network use that still keeps the catalogue current for the people looking at it.
- The DB cache keeps newer data visible offline and across restarts, which matches the user's direction.
- The committed seed keeps builds offline and reproducible.
- It removes the scheduler task and the workspace files.

**Alternatives**:
- *Periodic refresh in `VizierScheduler`*: the previous design; it contacts models.dev even when nobody uses the catalogue.
- *Fetching in `build.rs`*: breaks offline builds, `cargo install` from crates.io, network-less Docker builds and reproducible builds.
- *Cache as workspace files*: workable, but the DB is already the place for cached state, and it is transactional.
- *Normalised tables, one row per model*: about 8,900 row writes per refresh, and a schema migration whenever models.dev adds a field, with no query benefit, since all queries run in memory.

## R10. Custom providers and stored credentials

**Decision**: Store custom providers in a new `custom_provider` table (`id TEXT PRIMARY KEY, data TEXT`). The data is JSON:

```
{name, protocol: openai_compatible|anthropic, base_url, api_key?, headers{}, models[{id, context?, output?, tool_call?, image_input?, reasoning?, edited: bool}]}
```

Credentials for catalogue and built-in providers stay in `provider_config`. The key becomes the `ProviderId` string, and `data` becomes a generic `ProviderCredentials{api_key?, endpoint?, base_url?, access_token?, account_id?}`, which replaces the 29-variant tagged `ProviderEntryConfig`.

- **Keys and header values**: stored as today, in plain text in SQLite, like existing keys. They are never returned by the API; responses carry only `has_api_key` and header names.
- **Fetch merge**: models fetched again are merged by id. Rows with `edited: true` keep the operator's values.
- **Ids**: a custom id that collides with a catalogue or built-in id is refused. If a later refresh adds a catalogue provider with the same id, the custom provider wins and the catalogue entry is hidden (spec edge case).

**Rationale**:
- Custom providers have structure (a list of models) that a credentials row doesn't.
- The generic credentials struct removes a large per-variant `match` in `provider_keys.rs` and `providers/mod.rs`.

**Alternatives**:
- *One table with a `kind` column*: mixes two shapes.
- *Encrypting keys at rest*: out of scope, and unchanged from today.

## R11. Logos

**Decision**: Bundle every catalogue provider's and lab's SVG in the build-time seed (about 1.3 MB raw). Logos for providers added after the build are fetched once, the first time they're requested, and cached in `catalogue_logo`. Serve them all at `GET /api/v1/catalogue/logos/{id}.svg` with headers that stop any embedded script running:

- `Content-Type: image/svg+xml`
- `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`
- `X-Content-Type-Options: nosniff`

The WebUI shows them only through `<img src>`, never inline in the page. A CSS `filter`/`color-scheme` rule keeps the 221 logos that use `currentColor` legible in dark mode; the 10 full-colour marks are shown as they are.

- **A model's logo** comes from the lab named in its `canonical_model_id` prefix, else its hosting provider's logo.
- **No logo**: the generic icon (models.dev's fallback, detected by its hash at snapshot time and recorded as "no logo"), or initials for built-in and custom providers.

**Rationale**:
- Served from the binary, so it works offline and page views never reach models.dev (FR-005a).
- `<img>` plus CSP neutralises scripts inside SVGs.

**Alternatives**:
- *Hot-linking models.dev*: it is a third-party request on every view and breaks offline installs.
- *Inlining the SVGs*: an XSS risk.

## R12. Cost estimates need the model on each response

**Decision**: Add `provider: Option<ProviderId>` and `model: Option<String>` to `VizierResponseStats` (with `serde(default)`), filled in by `VizierAgent::chat`. `aggregate_usage` multiplies tokens by the catalogue price of the recorded provider and model. Older entries have no model recorded, so they show no cost, rather than a cost computed from the agent's current model, which may differ.

**Rationale**: Agents change models over time, so pricing old usage at today's model would be wrong. FR-021 requires that cost be left out, never shown as zero, where it is unknown.

**Alternatives**: Price everything at the agent's current model. That is simple but wrong after any model switch.

## R13. Constitution, Principle III, and the default refresh

**Decision**: Catalogue refresh is **on by default and lazy**: it is triggered only when a person uses the catalogue, never by agents (R9). On 2026-10-15 the project owner ruled that this complies with Principle III, for four reasons:

1. It fetches metadata only.
2. The bundled snapshot is complete, so the refresh is never required.
3. With no network, vizier shows what is available locally, with no error on the default path.
4. Local-first behaviour is unaffected.

`catalogue.refresh: false` (or `VIZIER_CATALOGUE_REFRESH=false`) turns it off for air-gapped installs. The literal wording of the Distribution constraint ("features that fetch something at runtime MUST be gated behind explicit user configuration") reads more strictly than this ruling. A PATCH-level clarification to `constitution.md` is suggested, so that future reviews don't re-litigate it. It is not required for this feature.

**Rationale**: The project owner made the call.

**Alternatives**: Off by default (rejected by the owner), or a Complexity Tracking deviation (unnecessary, given the ruling).

## R14. The WebUI agent form merge

**Decision**: The settings route (`agent-settings.tsx`, 3,286 lines) becomes a thin wrapper that renders `AgentForm` in `mode="edit"`, plus two extra tabs, Sharing and Danger Zone, passed in as extra tabs. `AgentForm` already has `mode: 'create' | 'edit'`, but nothing uses the edit mode today.

The merge lands first, as its own `refactor:` commit with no behaviour change. It is checked by `npm run typecheck` and by a manual walk through every setting, comparing old and new.

Then a shared `ProviderModelPicker` component replaces the six provider and model pickers (main, dream and read-image, in both files). It reads `GET /catalogue/providers` and `GET /catalogue/providers/{id}/models`. The hard-coded `CHAT_PROVIDERS`, `CHAT_PROVIDER_MODELS` and `CHAT_PROVIDER_DEFAULT_MODELS` in `types.ts` are deleted.

**Rationale**: Doing the merge first halves every later WebUI change (FR-029).

**Alternatives**: Build the picker and drop it into both forms. The user rejected this.

## R15. Testing without keys

**Decision**: Testing happens at four layers:

1. **Pure dialect tests**: `encode(CompletionRequest) -> serde_json::Value` and `decode(Value) -> CompletionResponse` for each of the five dialects, against JSON fixtures in `src/llm/dialect/fixtures/`.
2. **Transport tests**: an `axum` server on `127.0.0.1:0` (axum is already a dependency) checks:
   - 429 with `Retry-After`;
   - 5xx and 529 responses;
   - a 400 that fails at once;
   - a refused connection;
   - an attempt timeout;
   - SSE aggregation;
   - Copilot's token refresh after a 401.
3. **Golden fixtures recorded from rig**: before the cutover (plan Phase 1), run the rig 0.39 clients once against a local capture server, with the same inputs we use for our own tests. Record each request body, the response used, and rig's parsed result, and commit them as fixtures. The new dialects must produce semantically equal request bodies and identical parsed results. Embedding fixtures record rig's vectors. After the cutover, the fixtures stay as plain regression tests, with no rig involved.
4. **End to end**: the dummyplug quickstart, plus a stdlib-only Python stub server (`specs/015-provider-catalog/fixtures/stub_llm.py`). It speaks OpenAI chat (including `/models`) and Anthropic `/v1/messages`, and echoes what it received. That drives custom providers, capability handling (FR-013) and the agent loop over the real HTTP path, with no key.

Steps that need a live key are marked as such in `quickstart.md`.

**Rationale**: This covers the issue's open question ("how to verify each provider without keys") and the constitution's dummyplug gate.

**Alternatives**:
- *`wiremock`*: a new crate that duplicates what axum already gives us.
- *Live side-by-side parity tests with a compatibility shim*: rejected by the owner (2026-10-15), who chose to remove rig in one cut once the new layer is built. Recorded fixtures keep the parity check without keeping rig around.
