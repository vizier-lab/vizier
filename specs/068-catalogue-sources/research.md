# Research: Catalogue Sources (spec 068)

Facts were checked on 2026-11-27 against the code on `068-catalogue-sources` (from `38886697`) and
against the public endpoints named below.

## R1. What the catalogue is today

- `Catalogue` (`crates/vizier-core/src/catalogue/mod.rs`) wraps one `CatalogueData`, which is
  models.dev trimmed: providers with their connection hints (`npm`, `api`, `env`, `doc`, derived
  `dialect` and `kind`) **and** their models (`caps`, `limit`, `cost`, `status`, `route`). Every
  reader (pickers, `resolve.rs`, `agent_warnings`, `usage::Pricer`) goes through
  `provider()`, `model()`, `models()` and `find_model_by_id()`.
- `MediaCatalogue` (`catalogue/media.rs`, spec 028) is a second, separate store: OpenRouter's
  speech, transcription and image lists (ids, names, voices, aspect ratios, resolutions). It
  **drops prices**.
- `media_prices.rs` is a third: the shipped table `assets/media-prices.json` (24 entries, each
  with `source` and `as_of`), read only by `Pricer::priced` for media kinds.
- `Pricer` (`usage/mod.rs`) prices **text** with `catalogue.model(provider, model).cost` only. Media
  goes local → models.dev token price → table tokens → table units. Saved AWS inference-profile
  entries are asked first for an ARN.
- Text usage rows are always `local: false` (`agents/agent/model/mod.rs:110`), so Ollama,
  llama.cpp and dummyplug text is unpriced. `resolve_custom` already borrows `cost` from
  `find_model_by_id`, but nothing reads `ResolvedModel.cost` for a report.

**Decision**: the three stores become sources of one aggregate. Readers keep the method names
they call today. **Rationale**: FR-001 and FR-013 want one path, and keeping the method names keeps
the change out of roughly 15 call sites.

## R2. OpenRouter's lists (verified with `curl`; a WebFetch run returned a stripped 10-entry page)

| Endpoint | Entries | Prices |
|---|---|---|
| `GET /api/v1/models` | 477 | All 477 have `pricing`, as **strings in USD per token**: `prompt`, `completion`, `input_cache_read`, `input_cache_write`, `input_cache_write_1h`, `image`, `audio`, `audio_output`, `image_output`, `web_search`, `internal_reasoning`, plus `overrides: [{min_prompt_tokens, …}]` long-context tiers |
| `GET /api/v1/models?output_modalities=speech` | 43 | All priced, `prompt`/`completion` per token |
| `GET /api/v1/images/models` | 71 | **No** `pricing`. Each entry has `endpoints: "/api/v1/images/models/{id}/endpoints"`, which lists per-host `pricing: [{billable: "input_text"\|"input_image"\|"output_image", unit: "token"\|…, cost_usd}]` |

Other fields: `context_length`, `top_provider.max_completion_tokens`, `supported_parameters`,
`architecture.{input,output}_modalities`, `canonical_slug`, `expiration_date`. All are public, with
no key.

**Decisions**:
- **D1.** The OpenRouter source reads `/models` (text and audio models) and the three media lists.
  Prices are converted to USD per 1M tokens (× 1e6) at parse time, so they fit vizier's `Cost`.
  `overrides` tiers are dropped in this feature, and the base price is used (noted in the
  contract). Lists stay unauthenticated.
- **D2.** Image prices come from each image model's `endpoints` call. The **seed** script collects
  them at build time. A runtime refresh fetches them too, at most 4 at a time and only for ids
  that are new or whose price is older than the TTL. A failed endpoint keeps the model's last
  price. The cheapest host's price is used. Token-billed image prices go into `Cost` (input text →
  `input`, output image → `output`).
- **D3.** OpenRouter's list is authoritative **for `openrouter` only**. Its ids
  (`anthropic/claude-…`) are what vizier sends to OpenRouter, so they map unchanged to provider
  `openrouter`. They are never mapped onto the model's original maker (spec edge case "a
  reseller's list").

## R3. Venice's list (`GET https://api.venice.ai/api/v1/models?type=…`, public, no key)

- `type=image`: 52 models, all priced: `model_spec.pricing.generation.usd`, or
  `model_spec.pricing.resolutions.{1K,2K,4K}.usd`, optionally nested by `quality`
  (`low|medium|high`), plus `upscale.{2x,4x}`.
- `type=text`: all priced: `model_spec.pricing.{input,output,cache_input,cache_write}.usd`, plus
  an `extended` long-context tier with `context_token_threshold`. Also `context_length`,
  `availableContextTokens`, `maxCompletionTokens` and `capabilities.{supportsFunctionCalling,
  supportsReasoning, supportsVision}`.
- `type=tts`: 11 models, `model_spec.pricing.input.usd`.
- A `diem` value always equals `usd`, and the list states no units. Venice's pricing page gives
  text as **USD per 1M tokens**, images as **USD per image** and speech as **USD per 1M
  characters**. These units are written into the parser and checked against `venice-sd35`
  ($0.01, already in the shipped table from Venice's page).

**Decisions**:
- **D4.** The Venice source reads `text`, `image` and `tts`. An image price with resolution tiers
  becomes `per_image.variants` keyed `1K|2K|4K` (`quality` → `{res}:{quality}`), with
  `generation`, or else the lowest tier, as `default`. `upscale` is not used: vizier never upscales.
- **D5.** It is authoritative for `venice` only. Its ids are what vizier sends, so they map
  unchanged.
- Venice's `variant` (the size vizier records for an image) has to name the tier. `VeniceImages`
  already maps a size setting onto Venice's `resolution` (`venice.rs:58`). The recorded variant is
  that tier. To check during tasks.

## R4. Other providers' model lists

Checked by reputation only. A provider is a new source only after its list is fetched and
verified like R2/R3.

- OpenAI, Anthropic, Google, Mistral, Groq, xAI and DeepSeek `/models`: no prices (and need a key).
- Together and DeepInfra publish prices but need a key. Out of scope (spec: public lists only,
  since a refresh runs with no stored credential).

**Decision D6**: OpenRouter and Venice only. The source trait makes each further one a single
file (FR-010).

## R5. Curated table versus models.dev

Every table entry that models.dev also prices has the **same** figures. The OpenAI embeddings
`text-embedding-3-small/large` and `ada-002` both read 0.02/0.13/0.10 per 1M. The xAI speech
wildcard applies to `kind: speech` only, while models.dev's xAI prices are text. So moving the
table **ahead** of models.dev (FR-004 step 4 before 5) changes no figure (FR-021).

**Decision D7**: keep `assets/media-prices.json` as the curated source, with its format
unchanged, and allow `kind: "text"` entries. Lookups stay kind-scoped and support prefix ids,
which is why the curated source answers lookups rather than adding rows to a merged model map.
The file keeps its name, since renaming it would buy nothing.

## R6. How to merge

Two different questions get different shapes:

- **Model facts** (name, caps, limits, status, kind) feed the pickers and the resolver, are read
  on every agent build, and are keyed by `(provider, model)`. **Decision D8**: precompute one
  merged `CatalogueData` whenever any source's data is swapped, field by field in authority order.
  Keep a side table recording which source gave each fact. 10k models merge in milliseconds, and
  readers stay lock-free (`Arc` swap, as today).
- **Prices** are kind-scoped, can match by prefix (curated), and can be rules (local,
  subscription). **Decision D9**: answer on demand by walking the sources in order with
  `price(provider, model, kind)`. `Pricer` keeps its per-report cache, so the walk runs once per
  `(provider, model, kind)` per report.

**Alternatives rejected**: merging prices into the model map (it cannot hold prefix entries or
kind-scoped prices); a generic "fact" enum with dynamic fields (more code than the five fields
need).

## R7. Authority and the rules

**Decision D10**: an `Authority` per `(source, provider)`: `Own` (a provider's own list for that
provider), `Curated`, `General` (models.dev), or `None`. Order: rules → inference-profile entry →
`Own` → `Curated` → `General` in source order → borrowed.

- **Rules** live on the provider definition, not in a data source: `BuiltinProvider` gains
  `billing: Billing::{Metered, Local, Subscription}`. Ollama, llama.cpp and dummyplug are `Local`,
  ChatGPT is `Subscription`. A models.dev provider whose `kind` is `Local` is `Local`.
  `Catalogue::billing(provider)` answers, and the aliased SSO id follows its source.
- **Inference-profile entries** stay where they are (stored per connection, read by `Pricer`
  through `saved_entries`). They are not a catalogue source, since they belong to one connection
  rather than to a model. Their place in the order is kept by `Pricer` asking them first, as today
  (FR-020).
- **Borrowed** is computed by the aggregate as the last step. It looks for the same model id at
  any other provider with a price from a non-borrowed source. It never runs for `Local` or
  `Subscription` billing. The preference is: the model's **maker** first, then the asking
  provider's dialect, then the provider id in `BTreeMap` order (deterministic, FR-016). The maker
  is the provider named by the `canonical_model_id` prefix that models.dev gives the same id
  elsewhere (8,618 of 10,022 models have one).
  Without the maker step, `gpt-4o-mini` would borrow from `abacus`, the first OpenAI-compatible
  reseller by id. `openai` itself has no `canonical_model_id`, but its resellers point at
  `openai/gpt-4o-mini`, which names it. **The spec's edge case ("same protocol first") is refined
  by this. The spec is updated to match.**

## R8. Disagreements

**Decision D11**: the price walk keeps going after the winner and records each later source that
has a price that **differs**, up to 3. Two token prices are equal when every rate matches to 1e-9.
The report row carries `price_alternatives: [{source, cost_usd}]`, each priced on the row's own
tokens and units. Only priced sources are compared, never borrowed.

## R9. Refresh and storage

- The models.dev refresh (`refresh.rs`) and the media one (`media.rs`) are two copies of the same
  state machine (TTL, 1 h failure backoff, `in_progress`, `refresh_now` waiting). **Decision D12**:
  one `SourceRefresh` per source implements it once. `maybe_refresh(deps)` and `refresh_now(deps)`
  fan out to every source. The existing call sites (people's reads only) do not change, so FR-011
  holds by construction. This collapses the third copy before it is written (constitution,
  Principle II).
- **Decision D13**: storage. models.dev keeps `catalogue_snapshot`, and the OpenRouter media lists
  keep `media_catalogue_snapshot` (schema bumped to 2, adding `price`; an older row is ignored and
  the seed answers). The new payloads (OpenRouter `/models`, Venice) go into one new table,
  `catalogue_source_snapshot(source TEXT PRIMARY KEY, schema, fetched_at, checked_at, last_error,
  failed_at, data BLOB)`, created by SQLite init and PostgreSQL `0017_catalogue_sources.sql`. No
  data moves, so nothing is added to #82.
- **Seeds**: `scripts/catalogue_update.py` writes `assets/catalogue/openrouter_models.json`
  (trimmed to the kept fields, about 150 KB) and `assets/catalogue/venice.json`, and adds `price`
  to `openrouter_media.json`. `build.rs` checks their syntax like `openrouter_media.json`. The
  release job's `--keep-on-failure` covers them.

## R10. The wire label

`PriceSource` is `catalogue | table | local | none` today, and only the WebUI reads it.
**Decision D14**: `price_source` becomes a source id string (`models_dev`, `openrouter`, `venice`,
`curated`, `inference_profile`, `borrowed`, `local`, `subscription`, `none`), and the report gains
`sources: [{id, name}]` for display names. Adding a source then changes no schema (FR-010).
`catalogue` is renamed `models_dev`, and `table` is renamed `curated`. This is a breaking change
to a computed, unstored field. It is marked `feat!` and noted in the API docs. No compatibility
alias is kept, because nothing stores the old value.

## R11. Pickers

**Decision D15**: FR-022 applies to the **chat** model pickers (`/catalogue/providers/{id}/models`),
which list the merged text models. Media pickers stay spec 028's OpenRouter lists. A Venice image
picker is not in this feature. A merged model carries `kind`, so media models never appear in a
chat picker (`models()` keeps `kind == Text`).

## R12. Verification

- Offline: dummyplug text is `Local` → `price_source: local`. A custom provider pointed at
  dummyplug with a models.dev model id gets `borrowed`. SC-005 is a unit test that builds the
  aggregate without models.dev and checks every OpenRouter and Venice seed model is still priced
  and described.
- Seeds are data: unit tests parse the committed seeds and assert SC-002 (every default OpenRouter
  and Venice media model is priced with no curated entry). The `every_default_media_model_is_priced_or_local`
  test is narrowed accordingly, since its `KNOWN_UNPRICED` entries for OpenRouter and Venice should
  disappear.
