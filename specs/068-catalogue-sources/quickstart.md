# Quickstart: verifying catalogue sources

The constitution's end-to-end gate: steps 1–5 run against the real binary with an agent on the
offline `dummyplug` provider. Each check reads the usage report:

```sh
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:9999/api/v1/agents/$AGENT/usage?start_date=$(date -u +%FT00:00:00Z)" | jq '.by_model, .unpriced, .sources'
```

## Setup

```sh
just run
```

Turn the catalogue refresh off for the offline steps (`VIZIER_CATALOGUE_REFRESH=false`), so every
source answers from its shipped seed. Create agent `prices` on provider `dummyplug`, model
`dummyplug`.

## Steps

1. **Local text is priced.** Send `hello` in a WebUI topic, and the reply arrives. In the report,
   `by_model` row `dummyplug/dummyplug` has `kind: text`, `price_source: "local"`,
   `cost_usd: 0`, and `unpriced.models` is empty. *(US2, FR-014, SC-001)*
2. **Past usage re-prices.** Using a workspace from before this branch with dummyplug usage, start
   this build and open the report. The old rows read `local` too, and no stored row was rewritten
   (`usage_event` row count and `updated` columns unchanged). *(US2 scenario 3, FR-018)*
3. **Borrowed estimate.** Create a custom provider `my-proxy` (OpenAI-compatible, base URL
   `http://127.0.0.1:9998/v1`, i.e. dummyplug's server) with model `gpt-4o-mini`, and point a
   second agent at it. Send `hello`. Its row reads `price_source: "borrowed"`, `borrowed_from:
   "openai"`, and `cost_usd > 0`. *(US4, FR-016)*
4. **Unpriced stays unpriced.** Add model `mystery-1` to `my-proxy`, switch the agent to it, and
   send `hello`. Its row reads `price_source: "none"`, `cost_usd: null`, and `unpriced.models`
   lists it. *(FR-017)*
5. **Sources are listed.** `GET /api/v1/catalogue` has `sources[]` for `openrouter`, `venice` and
   `curated`, each `origin: "bundled"`, `model_count > 0`, and `refresh.enabled: false`. The report's
   `sources[]` names every source label it uses. *(FR-012, FR-013)*
6. **No request without a person.** With refresh on (`VIZIER_CATALOGUE_REFRESH=true`) and the agent
   chatting, `RUST_LOG=vizier_core::catalogue=debug` logs no fetch until someone opens Settings ›
   Providers. Then one check per stale source runs. *(FR-011)*

## Unit checks (no binary)

`cargo test -p vizier-core catalogue::`:

- each source's committed seed parses, with a known model's price asserted (Venice `venice-sd35` =
  $0.01/image; one OpenRouter text model per 1M = string × 1e6);
- the authority order: an `openrouter` model priced by both OpenRouter and models.dev takes
  OpenRouter's price and lists models.dev as an alternative when it differs (US1, US3);
- OpenRouter's list never prices an `anthropic` model (reseller rule);
- **SC-005**: the aggregate built without `models_dev` still prices and describes every OpenRouter
  and Venice seed model;
- **SC-002**: every default OpenRouter and Venice media model is priced with no `curated` entry, and
  `KNOWN_UNPRICED` no longer needs OpenRouter or Venice rows;
- **FR-021**: every `(provider, model, kind)` priced by the old `Pricer` over the committed seeds
  gets the same `cost` from the new walk, except rows whose new source is `local`, `subscription`
  or a provider's own list. A golden list of those exceptions is checked in.

## Live provider (marked: needs a Venice key, costs about $0.01)

7. Create agent `venice-img` on dummyplug chat with `image_gen` on provider `venice`, model
   `qwen-image-2-pro`. Send code `image_generate(prompt="a fox")`. The report's image row reads
   `price_source: "venice"` with Venice's published price for the recorded variant.
