# Quickstart: Catalogue-Driven Providers and Models (and Removing rig)

**Feature**: `015-provider-catalog` | **Date**: 2026-10-15

## Build and static checks

```sh
just install
cargo build
cargo test          # dialect encode/decode fixtures, transport tests (axum), catalogue parse/validation,
                    # ProviderId legacy map, custom-model merge, context-window resolution, golden fixtures recorded from rig 0.39
cargo clippy
cd webui && npm run typecheck
cargo tree | grep -c rig-core     # must print 0 from Phase 2 (cutover) on (SC-007)
cargo package --allow-dirty --no-verify --list | grep -c '^assets/catalogue/api.json$'   # must print 1
python3 -I scripts/catalogue_update.py --keep-on-failure   # with network: refreshes assets/catalogue; offline: warns, exit 0, files untouched
# Release dry run: run release-and-publish.yml with dry_run: true and check the prepare-release log shows the catalogue step
```

## Fixtures

- **The `dummyplug` provider** (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`): the agent loop, with no network.
- **`fixtures/stub_llm.py`**: a stdlib-only HTTP stub that needs only `python3`.
  - **Run it**: `python3 -I specs/015-provider-catalog/fixtures/stub_llm.py --port 18080`.
  - **Model list**: `GET /v1/models` returns `stub-tools` and `stub-vision`.
  - **OpenAI chat**: `POST /v1/chat/completions` replies with the text `ECHO <json of the last user message parts>`. If the last user text starts with `CALL `, it answers with a tool call instead: `CALL list_tool_functions {}` calls that tool.
  - **Anthropic**: `POST /v1/messages` behaves the same way, in Anthropic format.
  - **Errors**: `X-Stub-Status: 429` and `X-Stub-Retry-After: 2` on a custom provider's headers make the stub return that error first.
- **No network**: run the offline steps with outbound traffic blocked, e.g. `unshare -rn` or a firewall rule, or at least with the catalogue refresh off and no keys set.

```sh
env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY -u GEMINI_API_KEY VIZIER_CATALOGUE_REFRESH=false just run
```

## 1. Upgrading keeps agents working (US1, FR-014, FR-015, FR-016a, FR-026, FR-028)

**Setup**: on `master`, create these agents and saved provider entries, then run `just shutdown` and switch to this branch.

| Agent | provider / model | Extra |
|---|---|---|
| `dp-old` | `dummyplug` / `dummyplug` | |
| `gem-old` | `gemini` / `gemini-2.5-flash` | STT `gemini`, embeddings `gemini`. Saved provider entry `gemini` with a dummy key |
| `cus-old` | `custom` / `stub-tools` | saved `custom` entry: `base_url = http://127.0.0.1:18080/v1` |
| `gal-old` | `galadriel` / `llama3.1:8b` | |
| `moon-old` | `moonshot` / `moonshot-v1-8k` | saved `moonshot` entry with `base_url = https://api.moonshot.cn/v1` |

Also leave a `providers:` section with an `openrouter` key in the config file.

Start this branch with the stub running.

| Check | Expect |
|---|---|
| startup log | one `info` line per rewritten agent or provider row (`gemini → google`, `moonshot → moonshotai-cn`, `custom → custom provider`); a `warn` that `providers.openrouter` is ignored, naming `OPENROUTER_API_KEY`; no other errors |
| `GET /api/v1/agents/health` | `dp-old`, `gem-old`, `cus-old`, `moon-old` running; `gal-old` `Failed`, with `provider "galadriel" is not available; pick another provider` |
| `vizier agent ps` | the same states |
| `GET /api/v1/agents/gem-old` | `provider: "google"`; STT and embedding settings unchanged |
| `GET /api/v1/providers` | `google` (`migrated_from: "gemini"`, `used_by.chat = 1, stt = 1, embedding = 1`) and `moonshotai-cn`; no `openrouter` |
| `GET /api/v1/providers/custom` | `custom`, OpenAI-compatible, `base_url` from the old entry |
| restart again | no migration log lines (idempotent) |
| `PUT /api/v1/agents/gem-old` with `"provider": "gemini"` | accepted; reads back as `google` (old-client edge case) |

**Dummyplug regression** (SC-007): re-run the 008 and 014 quickstart dummyplug scripts against `dp-old`, unchanged. Every expected reply must match.

| Agent | Send | Expect |
|---|---|---|
| `dp-old` | `tools` | the 4 code-mode tools (unchanged from 014) |
| `dp-old` | `{"tool": "execute_python", "arguments": {"intent": "check", "code": "memory_list(limit=1)"}}` | `ok: true` |
| `dp-old` | `context` | the context block (unchanged) |

## 2. Catalogue pickers and data, offline (US2, US3, FR-001–FR-013, FR-031)

| Check | Expect |
|---|---|
| `GET /api/v1/catalogue` | `source: "bundled"`, `provider_count` ≥ 200 |
| `GET /api/v1/catalogue/providers` | includes every pre-feature provider under its new id, plus e.g. `cerebras`, `fireworks-ai`; excludes `amazon-bedrock` and `google-vertex` (unsupported); `dummyplug (testing)` listed |
| `GET /api/v1/catalogue/providers/anthropic/models` | tool-capable, non-deprecated models only, newest first; first has `default: true`; each has `limit`, `caps`, `cost`, `logo` |
| `…/models?all=true` | adds deprecated models |
| `GET /api/v1/catalogue/providers/openrouter/models?all=true` | ≥ 380 models (SC-008 data) |
| `GET /api/v1/catalogue/logos/anthropic.svg` | `200 image/svg+xml`, with the CSP header |
| create agent `ant-1` on `anthropic` / the default model, no `context_window` | `GET /agents/health` shows `context_window` equal to the catalogue's `limit.input ?? limit.context` (SC-004) |
| update `ant-1` with `context_window: 50000` | health shows `50000` (FR-012) |
| create an agent on a model whose catalogue entry says `tool_call: false` | `201`, with `warnings[0].code = "no_tool_calling"` |
| create an agent on `anthropic` / `my-unlisted-model` | `201`, with `warnings[0].code = "unknown_model"` (FR-011) |

## 3. Capability handling through the real HTTP path (US3 scenario 4, FR-013, FR-033)

**Setup**: create the custom provider `stub` (OpenAI-compatible, `http://127.0.0.1:18080/v1`), fetch its models, and set `stub-tools` to `image_input: false` and `stub-vision` to `image_input: true`. Create agent `img-no` on `stub-tools` and agent `img-yes` on `stub-vision`, both with read-image enabled and no vision model.

| Agent | Send | Expect |
|---|---|---|
| `img-yes` | a message with a PNG attached | the reply `ECHO …` contains an `image` part: the image went straight to the model |
| `img-no` | the same | the turn succeeds; the reply `ECHO …` has no image part, and has a text note that an image was attached but cannot be viewed |
| `img-no` | update with a read-image vision model of `stub` / `stub-vision`, then send the image again | the reply contains the vision model's description, as today |

## 4. Custom providers (US6, FR-022–FR-027)

| Step | Expect |
|---|---|
| `POST /providers/custom/test` with base URL `http://127.0.0.1:18080/v1` | `{ok: true, model_count: 2}` |
| the same, with port 18099 (nothing listening) | `{ok: false, error: "…refused…"}`; saving still allowed |
| `POST /providers/custom` with `id: "openai"` | `409`, naming the clash |
| create `home` (OpenAI-compatible), then `POST /providers/custom/home/fetch-models` | 2 models; `stub-tools` has no catalogue match, so its limits are empty |
| edit `stub-tools` with `context = 32768`, add model `extra`, fetch again | `stub-tools` keeps `32768` (`edited: true`); `extra` kept |
| `PUT /providers/custom/home` with `id: "home2"` | `400` (the id is immutable) |
| `GET /providers/custom` | `has_api_key`, and header **names** only; no secret values |
| agent `c1` on `home` / `stub-tools`; send `CALL list_tool_functions {}` | the tool runs, and the stub echoes the tool result on the next completion (custom provider through the agent loop) |
| create `home-anth` with `protocol: anthropic`, same stub; agent `c2`; send `hello` | the reply `ECHO …` arrived via `/v1/messages` |
| `PUT /providers/anthropic` with a `base_url` | `400` (FR-027) |
| `DELETE /providers/custom/home` | `409`, listing `c1`; with `?confirm=true` → `c1` becomes `Failed` |

## 5. Retry on `Retry-After` (FR-019)

Create custom provider `flaky` with headers `X-Stub-Status: 429` and `X-Stub-Retry-After: 2`, which makes the stub fail the first request of each turn. Create agent `r1` on it and send `hello`.

**Expect**:
- the log shows one retry, with `wait_ms ≥ 2000`;
- the reply arrives;
- the turn is not reported as an error.

## 6. Catalogue refresh (US4, FR-002–FR-004)

| Step | Expect |
|---|---|
| started with `VIZIER_CATALOGUE_REFRESH=false` and outbound traffic blocked | no request to `models.dev` (check the network log); `POST /catalogue/refresh` returns `409` |
| restart with refresh on and agents running, but no WebUI or API use for 2 minutes | still no request to `models.dev`: agents never trigger a check |
| `GET /catalogue/providers` | answers at once, from the seed; one background request appears in the log; then `GET /catalogue` shows `source: "cached"` with a newer `fetched_at` and a `catalogue_snapshot` row in SQLite |
| `GET /catalogue/providers` again right away | no request (inside the freshness window) |
| set `VIZIER_CATALOGUE_TTL=1s`, restart, read twice | the second check is a conditional request; the log shows `304`; `checked_at` advances, and `fetched_at` doesn't |
| set `catalogue.url` to `http://127.0.0.1:18080/broken` (the stub returns `{}`); `POST /catalogue/refresh` | `502`; `refresh.last_error` set; `source` still `cached` with the previous `fetched_at`; pickers unchanged; further reads within the hour make no request |
| restart offline | `source: "cached"`, loaded from SQLite; models newer than the seed still listed |

**Live-network steps**: rows 3–5 need internet access.

## 7. WebUI (US2, US5, US7, FR-005a, FR-010a–c, FR-029–FR-035)

These steps are manual, in a browser at `http://localhost:9999`.

- **One form** (FR-029): `/agents/new` and `/agents/dp-old/settings` render the same form. Settings adds Sharing and Danger Zone, and has one sticky "Save changes" bar that appears after an edit.
- **Picker**:
  - logos are shown;
  - connected providers come first, with custom providers in their own group;
  - searching "cereb" finds Cerebras;
  - the OpenRouter model search responds within 2 seconds (SC-008);
  - "Show all models" reveals deprecated models.
- **Defaults**: picking Anthropic in the wizard preselects the newest suitable model. Context window and max output are empty, sit under Advanced, and show the catalogue values as placeholders. Checkpoint shows `≈ N tokens`.
- **Read-image**: on a vision model, no vision picker is shown. Switching to `stub-tools` shows the warning and an image-capable-only picker.
- **Inline connect** (FR-034): picking Cerebras with no key shows the connect form, and saving it lists Cerebras under Settings › Providers. For a user without `settings:providers`, the form instead says to ask an admin.
- **Review**: the step shows the model card and warnings.
- **Providers page**:
  - with `ANTHROPIC_API_KEY` set and no saved key, Anthropic shows as connected, labelled `env ANTHROPIC_API_KEY`;
  - `used_by` counts are shown;
  - deleting a provider that is in use shows the agent list;
  - `home` shows reachable, and with the stub stopped, unreachable.
- **Usage view** (US8, FR-021): after a few `stub` turns on a model with a price set via catalogue match, or a live model, the cost is shown. Dummyplug and older entries show `—`, not `$0`.

## 8. Live-provider steps (marked: needs keys)

These need real keys and give nondeterministic output. For each provider that has a key set, run one text turn, one tool call and one tool result, using `cargo test -- --ignored llm_smoke` and the WebUI:

- openai (Responses)
- anthropic: the second turn must report `cached_input_tokens > 0`
- google
- deepseek, including the reasoning round-trip in a tool loop
- ollama (local)
- one catalogue provider vizier didn't support before (US5)

Each dialect that isn't run live is listed in the PR as "parity-tested only".
