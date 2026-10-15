# PR notes (015-provider-catalog) — collected during implementation

## Baseline (T001, on 30a1d42 before any change)

- `cargo build`: ok
- `cargo test`: 303 passed, 0 failed
- `cargo clippy`: 98 warnings, 0 errors (pre-existing; mostly `collapsible_if`)
- `cd webui && npm run typecheck`: ok
- `cargo tree -e normal | wc -l`: 1989

## Deviations from tasks.md (with reasons)

- **T028 `type ProviderVariant = ProviderId` alias**: not possible — dozens of `match` arms use
  `ProviderVariant::openai`-style enum patterns, and a string newtype can't be a const pattern.
  `ProviderId` was added beside the enum in Phase 2 and the enum is deleted at the cutover (T049).
- **T013 `CredentialSource`**: gained a `RequestInfo` argument (Copilot's `X-Initiator` and
  `copilot-vision-request` depend on the request) and a `base_url()` hook (a Copilot token names
  its own API host). `invalidate()` returns whether a cached token was dropped.
- **T014**: `complete_until(req, deadline)` is a `CompletionModel` trait method (default ignores the
  deadline; `RetryingModel` overrides it), so `VizierModel` can stay a plain `Arc<dyn CompletionModel>`.
- **T018 Anthropic caching**: block-level `cache_control` on the last tool, the system prompt and the
  last message block, as the task specifies. rig 0.39's `with_automatic_caching` sent a single
  top-level `cache_control` instead; block-level breakpoints work on every Anthropic-compatible
  endpoint (MiniMax, opencode, Azure Foundry).
- **T020 Ollama tool-call ids**: the function name, as rig did — not a synthesised `call_<n>`. Ollama
  answers tool results by `tool_name`, so a synthesised id would break the round trip.
- **T023 Cohere embeddings**: `/v1/embed` with `texts`/`input_type`, which is what rig 0.39 actually
  sent (the task said `/v2/embed`); behaviour unchanged.
- **OpenAI chat**: system and assistant text are sent as plain strings, not part arrays (several
  OpenAI-compatible servers, DeepSeek among them, accept only strings). Single-text user messages
  were already strings.
- **T031 order**: the golden capture was written and run before the media clients (T022/T023) so the
  dialects could be built against rig's real wire output; media was captured afterwards, still with
  rig present.

## Golden fixtures: intentional differences from rig 0.39

Each is a named normaliser in `src/llm/dialect/golden_tests.rs` or noted in
`src/llm/media/golden_tests.rs`; everything else matches rig byte-for-byte as JSON values.

| Dialect | Difference | Why |
|---|---|---|
| OpenAI chat | system/assistant content as strings | compatibility (above) |
| DeepSeek | `tool_choice: "none"` is sent | rig's DeepSeek client dropped it, so answer-only turns could still call tools |
| Anthropic | one joined system block; block-level cache breakpoints | see T018 |
| Anthropic, Gemini | a later system message becomes `<system>…</system>` user text | contract S8; rig hoisted it to the top, which also defeats caching |
| Gemini | `maxOutputTokens` sent when `max_tokens` is set | rig only sent it when a generation config was already present |
| Gemini | key in `x-goog-api-key` header, not `?key=` | keeps the key out of URLs and logs |
| Ollama | output cap as `options.num_predict` | rig sent a top-level `max_tokens`, which Ollama ignores |
| Responses | leading system → `instructions`, later → developer messages; `store: false`; function calls carry only `call_id` | contract S5/S8; with nothing stored an item id can't be resolved (rig's replay of vizier history also failed with "call_id is required") |
| Hugging Face | requests go to `/hf-inference/models/{model}` | rig dropped the inference route segment |
| all | `null` and absent fields are equivalent | |

## Cutover (T052)

- `cargo tree | grep -c rig-core`: 0; `grep -rn "rig_core\|rig::" src`: empty.
- `cargo tree -e normal | wc -l`: 1989 → 1912.
- `cargo test`: 383 passed (the two ignored capture tests were deleted with `golden_capture.rs`).
- `cargo clippy`: 0 errors.
- Migration markers live in the `state` table (`migration.provider_ids_v1`, plus
  `migration.provider_ids_v1.map` for `migrated_from`), like vizier's other migrations, rather
  than in `global_config`, whose rows are typed.
- The provider-id migration also matches a saved base URL by host when no catalogue `api` is
  equal (`api.minimax.cn/v1` → `minimax-cn`), as the contract's examples need.

## Quickstart §1 and §5 (T053), with `fixtures/e2e/upgrade.py` and `retry.py`

A pre-015 workspace was simulated by planting legacy rows (tagged `ProviderEntryConfig` JSON
under legacy names; agents on `gemini`, `custom`, `galadriel`, `moonshot` + `api.moonshot.cn`;
a YAML `providers.openrouter`) and clearing the migration marker. 19/19 and 3/3 checks passed:

- log lines `provider 'gemini' → 'google'`, `'moonshot' → 'moonshotai-cn'`, `custom` → custom
  provider, galadriel credentials deleted, per-agent rewrites, and
  `providers.openrouter is ignored; set OPENROUTER_API_KEY or add it in Settings › Providers`;
- health: `gal-old` failed with `provider "galadriel" is not available; pick another provider`;
  `dp-old`, `cus-old`, `moon-old` alive. `gem-old` was refused at startup by Google's real API
  ("API key not valid") when its Gemini embeddings were built with the planted dummy key — the
  new request path working end to end;
- `GET /providers` lists `google` and `moonshotai-cn`, not `openrouter` or `galadriel`; the
  `custom` custom provider holds the old base URL;
- a second restart logs no migration lines (idempotent);
- `PUT /agents/gem-old` with `"provider": "gemini"` reads back `google` (old client);
- dummyplug (008/014): `tools` lists the 4 code-mode tools, the `execute_python` request
  returns `"ok": true`, `context` echoes the context block;
- §5: an agent on a custom provider whose headers make the stub answer `429` with
  `Retry-After: 2` waited ≥ 2 s (`wait_ms=2000+` in the log) and answered;
- a custom-provider agent ran a full tool loop through the stub over HTTP (`CALL …` → tool
  runs → `ECHO tool-result …`).

## US3 (T054–T058), with `fixtures/e2e/capabilities.py` — 8/8

- `ant-1` on `anthropic/claude-sonnet-4-5` with no override: health `model.context_window`
  200000, `context_source: catalogue`; with `context_window: 50000`: 50000, `override`.
- Saving an agent on `openai/gpt-3.5-turbo` (catalogue `tool_call: false`) returns
  `warnings: [no_tool_calling]`; on `anthropic/my-unlisted-model`, `[unknown_model]`.
- Custom provider `stub` with `stub-vision` (`image_input: true`) and `stub-tools` (`false`),
  both agents with read-image on and no vision model: the image reaches `stub-vision`; for
  `stub-tools` the turn succeeds and the model gets "[image attached: dot.png, but this model
  cannot view images]"; `img-no` health warns `read_image: main model cannot view images;
  configure a vision model`; with `stub/stub-vision` as the vision model, the description comes
  back as before.
- **Found and fixed while testing**: since code mode (014) the model can only call
  `read_image_file` from an `execute_python` script, and the loop only forwarded images when the
  outer tool was `read_image_file` itself — so images loaded from a script were filed as session
  files and never shown to the model. `read_image_file` now hands its image to the turn through
  `ToolContext.loaded_images`, which the loop drains after each round, whichever way it was called.

## Catalogue API and lazy refresh (T059, T060, T082–T084, T086, T089), with `fixtures/e2e/catalogue.py` — 27/27

- §2 offline: `GET /catalogue` is `bundled` with 230 providers; `/catalogue/providers` lists every
  pre-feature id under its new name plus `cerebras` and `fireworks-ai`, hides `amazon-bedrock`
  and `google-vertex`, lists `dummyplug (testing)` as connected, shows a saved key only as
  `{status: connected, source: saved}`; `?q=cereb` finds Cerebras; the Anthropic default list is
  tool-capable, non-deprecated, newest first with the first marked `default`; `?all=true` adds
  deprecated models; OpenRouter has ≥ 380; an unlisted model is `404 "not in catalogue"`; logos
  are served without auth with `image/svg+xml`, CSP and `nosniff`; unknown logos 404.
- §6: with refresh off, `POST /catalogue/refresh` is 409 and nothing is requested. With refresh
  on, running and chatting agents make no request; a person's read answers at once and starts
  one background request; then `source: cached` with an ETag row in `catalogue_snapshot`; a second
  read within the TTL makes no request; with `ttl: 1s` the next check sends `If-None-Match` and a
  304 advances `checked_at` only; a broken URL gives 502, sets `last_error`, keeps the cached data,
  and further reads within the hour make no request; an offline restart loads the cache from SQLite.
- The refresh is exercised end to end against a local ETag-aware catalogue server rather than
  axum unit tests: `check()` needs the full `VizierDependencies`. `trim` and validation have unit tests.
- `catalogue.refresh`/`catalogue.ttl` are also `--catalogue-refresh`/`--catalogue-ttl` flags and the
  `VIZIER_CATALOGUE_REFRESH`/`VIZIER_CATALOGUE_TTL` env vars (read directly too, not only by the
  Docker entrypoint).

## Custom providers and the providers API (T066–T072, T085), with `fixtures/e2e/custom_providers.py` — 24/24

- §4: `POST /providers/custom/test` against the stub lists 2 models; a closed port fails with
  "refused"; `id: "openai"` is 409 naming the catalogue; a second `home` is 409 naming the custom
  provider; fetch-models adds both stub models with empty limits (no catalogue match); edited
  rows (`context = 32768`) and an added `extra` row survive a second fetch; a different `id` on
  `PUT` is 400; `GET /providers/custom` shows `has_api_key` and header names, never values.
- Both protocols through the agent loop: `c1` on the OpenAI-compatible provider runs a tool and
  the stub echoes the result; `c2` on `protocol: anthropic` answers via `/v1/messages`.
- Providers API: `base_url` on `anthropic` is 400 (FR-027); an unused field is 400; a saved key
  shows `source: saved`; `used_by.chat` counts main/dream/read-image; dummyplug and custom providers
  are never in `GET /providers`; `POST /providers/{id}/test` works for a custom provider; deleting a
  used provider (catalogue or custom) is 409 listing the agents; with `?confirm=true` the agent is
  `Failed` with "not available".
- **Added while testing**: an agent built with credentials that are then changed or removed kept
  running on the old ones until a restart. Saving or deleting a provider's credentials (or editing
  or deleting a custom provider) now restarts the agents that use it through a new
  `AgentCommand::Restart`; a build that fails leaves the agent `Failed`, as at boot.

## WebUI (T061–T065, T069–T081, T087, T092), checked in Chrome against a scratch server

- One form: `/agents/<id>/settings` is `AgentForm` in edit mode with Sharing and Danger Zone as
  extra tabs (`agent-settings.tsx` 3286 → 493 lines). The sticky "Save changes" bar appears only
  after an edit (a bug where it showed on load was fixed), saving clears it, Discard reloads.
- Picker: grouped providers (connected / custom / all) with logos inverted on the dark theme;
  choosing Anthropic (not connected) opened the inline connect form, saving it moved Anthropic to
  CONNECTED and loaded its models (a stale-cache bug that left "Loading…" was fixed); the model list
  shows lab logos, context, 🔧👁💭 and prices; choosing Sonnet 5.5 shows its card, the checkpoint
  line "≈ 800,000 tokens of 1,000,000" and Advanced placeholders "1,000,000 · from catalogue" /
  "128,000 · from catalogue".
- Read-image: "✓ Claude Sonnet 5.5 can see images. They are sent to it directly."
- Settings › Providers: catalogue line (models.dev, date, bundled, counts), Refresh disabled with
  a tooltip when refresh is off, connected rows with source badge, model count, "Used by: chat
  (1 agent)" and reachability; the custom provider form ran Test (✓ 2 models) → Create → Fetch
  from server → Save.
- Usage: an "Estimated Cost" card shows "—" with no priced replies; per-day (chart tooltip) and
  per-channel-type costs read "$x.xx est." or "—".

## Polish (T093–T097)

- `CLAUDE.md` and the mdBook (`docs/src/configuration/{providers,main-config,index,cli,
  storage-shell,agents,tools-embedding}.md`, `api-integration/rest-api.md`, quick start) describe
  the catalogue, custom providers, `catalogue.*` / `VIZIER_CATALOGUE_*`, and no YAML `providers`.
- `src/llm/smoke_tests.rs`: six `#[ignore]` `llm_smoke_*` tests (openai Responses, anthropic with a
  cache-hit assertion, google, deepseek, ollama, and `LLM_SMOKE_EXTRA`, default `cerebras`), each
  skipping without its key.
- `cargo test`: 395 passed, 6 ignored. `cargo clippy`: 0 errors, 87 warnings (baseline 98); none in
  the new providers/catalogue code. `npm run typecheck`: ok.
- T097: all five e2e scripts in one run on the final build — upgrade 19/19, retry 3/3,
  capabilities 8/8, catalogue 27/27, custom providers 24/24.

## Remaining

- **T098**: live runs of quickstart §8 need provider keys (`llm_smoke_*`). Until they run, every
  dialect is golden-fixture-tested only.
- **T099**: `release-and-publish.yml` with `dry_run: true` must be dispatched on GitHub. Locally,
  `cargo package --allow-dirty --no-verify --list` contains `assets/catalogue/api.json`.
