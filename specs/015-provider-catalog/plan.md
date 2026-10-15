# Implementation Plan: Catalogue-Driven Providers and Models (and Removing rig)

**Branch**: `015-provider-catalog` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md) | **Issue**: #69

**Input**: Feature specification from `specs/015-provider-catalog/spec.md`

## Summary

Remove `rig-core` and replace it with an in-tree provider layer, `src/llm/`. It has:

- our own message, tool and usage types;
- a typed HTTP error, so `Retry-After` is honoured;
- five wire **dialects**: OpenAI chat, OpenAI Responses, Anthropic, Gemini and Ollama;
- curated media and embedding clients on the same transport.

Which providers and models exist stops being a hand-written list. It comes from **models.dev**. A snapshot is seeded at build time from a file committed to the repo (about 350 KB, plus about 300 KB of logos) and embedded in the binary. At runtime vizier refreshes **lazily**: when someone uses the catalogue and the copy is over a day old, a conditional (ETag) request runs in the background, and the result is cached in SQLite, so it stays visible offline. Agents never trigger a fetch. Each catalogue provider's `npm` hint, together with per-model route overrides, picks its dialect. That makes about 210 of the 230 catalogue providers usable with no per-provider code. Only quirks that can't be expressed as data are implemented as trait impls: Copilot's token exchange, ChatGPT's account auth, and Azure's URL template.

Around the catalogue:

- **Provider ids**: the `ProviderVariant` enum becomes a `ProviderId` string. Legacy names are mapped on read and rewritten once at startup. Galadriel and Mira are dropped.
- **Custom providers**: operators can define them (OpenAI-compatible or Anthropic protocol), with their own model lists.
- **Config file**: `providers` is removed from `.vizier.yaml`.
- **Runtime data**: agents get real context windows, output limits and capabilities. Images go straight to vision-capable models; others get a note.
- **WebUI**: the create wizard and the settings page become one form with a shared, logo-bearing provider and model picker. The providers page gains env-var detection, "used by" counts, reachability checks, custom providers and inline connect.
- **Usage view**: shows estimated cost from catalogue prices.

## Technical Context

**Language/Version**: Rust, edition 2024; TypeScript with React 19 (React Router 7, Tailwind 4) for the WebUI.

**Primary Dependencies**:
- Removed: `rig-core`.
- Used directly: `reqwest` 0.13 (turn on its `json` and `multipart` features), `serde`, `serde_json`, `schemars`, `zip` (already used for bundle export), `parking_lot`, `axum` (also used for the transport tests), `fastembed`, `rmcp`.
- New crates: none.

**Storage**: SQLite.
- `provider_config`: re-keyed by `ProviderId`, holding a generic `ProviderCredentials`.
- `custom_provider`: a new table.
- `agent_config`: rewritten once.
- `catalogue_snapshot` and `catalogue_logo`: new tables caching the refreshed catalogue.

**Testing**: `cargo test` covers:
- dialect `encode`/`decode` against fixtures;
- transport against an in-process axum server;
- catalogue parsing and validation;
- the `ProviderId` legacy map;
- custom-model merge;
- context-window resolution;
- golden fixtures recorded from rig 0.39 before the cutover, so wire formats are checked without keys.

End to end: dummyplug agent steps plus the stdlib `fixtures/stub_llm.py` in [quickstart.md](quickstart.md). Live-key steps are marked.

**Target Platform**: unchanged: Linux, macOS, Windows, and the musl and cross targets in `Cross.toml`. Removing rig also removes its dependency tree from them.

**Project Type**: single Rust binary with an embedded WebUI.

**Performance Goals**:
- The catalogue loads in under 200 ms at startup.
- Picker endpoints answer from memory in under 50 ms.
- An OpenRouter model search (380+ models) filters on the client in under 2 s (SC-008).
- A refresh never blocks a request or a turn: it is a single background task (stale-while-revalidate), and readers clone an `Arc`.

**Constraints**:
- Works fully offline from the bundled snapshot.
- No network access during `cargo build`.
- History, tool names, the dummyplug protocol and Anthropic prompt caching are unchanged.
- Embedding vectors are identical to rig's for the same provider, model and input (FR-020).

**Scale/Scope**:
- About 230 catalogue providers and about 8,900 models.
- Rust: about 60 files touched. About 2,000 lines are deleted: `model/provider.rs`, most of `model/mod.rs`, 30-odd adapter files and the `ProviderEntryConfig` matches. About 4,000 are added: `src/llm/` and `src/catalogue/`.
- WebUI: about 3,300 lines of duplicated form are deleted; the picker, providers page and custom-provider form are added.

## Constitution Check

*GATE: checked before Phase 0 research, and again after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | ✅ The layer is deliberately not a port of rig's API: it covers only vizier's slice, with ten simplifications (contract S1–S10). Net removal: rig (a 43k-line dependency), about 50 near-identical provider match arms and builder impls, the 29-variant `ProviderEntryConfig`, the duplicate agent form, and the hard-coded WebUI model lists. No new crates: SSE is parsed by hand (R6), zip handling and the transport-test server are already dependencies. No streaming API and no speculative abstractions. Each trait has at least two real implementations: `CompletionModel` ×3, `Dialect` ×5, `CredentialSource` ×3. No duplicate media trait layer: the media clients implement the existing `Vizier*Model` traits (S9). |
| **II. DRY via Trait-Based Extensibility** | ✅ Wire formats are pure `Dialect` impls behind one shared `HttpModel<D>`, so HTTP, auth and errors are written once (S1). Capability handling and system-message normalisation live in one `prepare` step (S6, S8). Providers are **data** (catalogue rows plus a small base-URL table). Adding a provider means a catalogue update, or a custom provider in the UI, with no code. Quirks are `CredentialSource` impls and endpoint rules, never `if provider == …` in dialect code (R5). Media providers stay curated enums with one `credentials()` mapping table (data-model §8); their media-specific dispatch is unchanged in this feature. |
| **III. Self-Contained Runtime** | ✅ **The catalogue and logos are embedded**, so the binary works offline and on first start. **Refresh is on by default, but lazy.** Only a person using the WebUI or catalogue API triggers it, never an agent, so a headless install never phones home (R9). The project owner ruled on 2026-10-15 that this complies (R13): it fetches metadata only, is never required, and degrades silently to local data, so local-first behaviour is unaffected. It can be turned off with `catalogue.refresh: false`. The Distribution constraint's literal wording ("MUST be gated behind explicit user configuration") reads more strictly than that ruling. A PATCH clarification to `constitution.md` is recommended as a follow-up so reviewers don't flag it again. Logos are served from the binary, so the WebUI makes no third-party requests. |
| **IV. Portability** | ✅ Pure Rust over `reqwest`/rustls, and no OS-specific code. Catalogue files go through the existing workspace path helpers (`utils::build_path`). The update script is stdlib Python, used at release time only. |
| **V. Unified Errors & Observability** | ✅ `LlmError` converts into `VizierError`, and the `ErrorKind` prefixes are kept. Migration, refresh and validation outcomes are logged with `tracing`, and keys are redacted from request debug logs. The `expect("failed to create … client")` calls in today's media constructors are removed. |
| **Gate: dummyplug e2e** | ✅ The quickstart scripts upgrade, capabilities, custom providers, retry and refresh as dummyplug or stub-server steps, with no keys. The 008 and 014 dummyplug scripts must pass unchanged. Live-provider steps are marked (§8). |
| **Gate: clippy / test / typecheck** | ✅ Required at every phase boundary. |
| **Gate: manual run for startup and migration changes** | ✅ Quickstart §1 runs a real upgrade from `master`. |
| **Gate: conventional commits** | ⚠ This feature has breaking changes: Galadriel and Mira are dropped, and `providers` is removed from YAML. Commit them as `feat!:` with `[**breaking**]`. The form merge is `refactor:`, and snapshot refreshes are `chore:`. |

**Result (pre-research)**: PASS. **Result (post-design)**: PASS. The design added no dependency or service and no type-branching. Nothing goes in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/015-provider-catalog/
├── spec.md
├── plan.md                      # this file
├── research.md                  # R1–R15
├── data-model.md
├── quickstart.md
├── rig-removal-notes.md         # pre-spec draft (superseded where it disagrees)
├── contracts/
│   ├── http-api.md              # /catalogue, /providers, /providers/custom, agent + usage deltas
│   ├── provider-id-migration.md # legacy name → ProviderId map, base_url rule, YAML removal
│   ├── provider-layer.md        # src/llm types, errors, traits, dialect table
│   └── catalogue-snapshot.md    # build-time seed, SQLite cache, lazy conditional refresh, logos
├── fixtures/
│   └── stub_llm.py              # created in implementation (quickstart §Fixtures)
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
assets/catalogue/{meta.json,api.json.gz,logos/}   # NEW build-time seed, embedded with rust-embed
scripts/catalogue_update.py               # NEW release-time snapshot builder (stdlib only)
justfile                                  # + catalogue-update recipe

src/llm/                                  # NEW — agent-agnostic provider layer (replaces rig)
├── mod.rs                                # CompletionModel trait; build(resolved) → VizierModel
├── types.rs                              # Message (incl. ToolResult), Part::Attachment, AssistantPart, ToolDefinition, Usage, request/response
├── error.rs                              # LlmError (+ From for VizierError)
├── http.rs                               # shared reqwest client, send_json/send_multipart, status→LlmError, redaction
├── sse.rs                                # whole-body SSE parser (ChatGPT)
├── auth.rs                               # CredentialSource: StaticKey, CopilotExchange, ChatGptAccount
├── resolve.rs                            # (ProviderId, model, overrides) → ResolvedModel
├── model.rs                              # HttpModel<D: Dialect>, RetryingModel<M> (retry logic moves here from agents/…/retry.rs)
├── prepare.rs                            # attachments by capability (FR-013) + normalize_system (3 styles)
├── dialect/{mod,openai_chat,openai_responses,anthropic,gemini,ollama}.rs + fixtures/   # pure encode/decode
└── media/{openai_shaped,gemini,cohere,ollama,hyperbolic,huggingface}.rs   # implement Vizier*Model traits directly

src/catalogue/                            # NEW
├── mod.rs                                # Catalogue handle (Arc<RwLock<Arc<CatalogueData>>>), queries for pickers
├── data.rs                               # CatalogueData/Provider/Model, parse + trim, dialect derivation (R2), base-URL table (R3)
├── builtin.rs                            # ollama, llama-cpp, chatgpt, dummyplug, elevenlabs, voyageai, hyperbolic
├── seed.rs                               # rust-embed seed; newest-of(seed, cache) at startup
├── cache.rs                              # catalogue_snapshot / catalogue_logo storage (in the sqlite backend)
└── refresh.rs                            # lazy trigger (TTL, 1h failure backoff, single-flight), conditional GET, validate, swap

src/config/provider.rs                    # ProviderVariant → ProviderId (+ legacy map); ProviderConfig (YAML) removed
src/config/mod.rs                         # VizierConfig.providers removed; catalogue settings added
src/provider_keys.rs                      # collapses to credential lookup: saved → catalogue env[] → legacy env
src/schema/provider.rs                    # ProviderEntryConfig → ProviderCredentials; CustomProvider, CustomModel
src/storage/provider.rs, storage/sqlite/provider.rs   # ProviderId keys; + CustomProviderStorage, custom_provider table
src/dependencies.rs                       # + catalogue load, http client; provider-id migration; migrate_providers removed
src/agents/agent/model/                   # mod.rs: VizierModel = Arc<dyn CompletionModel> via llm::resolve + llm::build;
                                          #   provider.rs and retry.rs DELETED (retry → llm::model::RetryingModel);
                                          #   registry.rs kept as last fallback; dummyplug implements CompletionModel
src/schema/{request,response,history}.rs, agents/process.rs, agents/agent/mod.rs,   # history.rs: tool-result grouping deleted (1:1 mapping)
src/agents/agent/system_prompt/context.rs, agents/tools/{mod,read_image/mod}.rs,
src/agents/mcp/mod.rs, sandbox/docs.rs     # rig types → llm::types; FR-013 image note; FR-033 read-image rules
src/embedding/, stt/, tts/, image_generation/  # ~30 per-provider rig adapter files DELETED; factories build src/llm/media clients; credentials() table
src/schema/response.rs, storage/sqlite/history.rs, schema/metrics.rs   # stats provider/model; estimated_cost_usd
src/channels/http/api/v1/catalogue.rs     # NEW /catalogue routes + logo serving
src/channels/http/api/v1/providers/mod.rs # ProviderId routes, used_by, reachable, test, custom providers
src/channels/http/api/v1/agents/mod.rs    # warnings on save
src/cli/onboard.rs                        # stops writing providers to YAML
src/main.rs                               # drop rig log directive
Cargo.toml                                # − rig-core; reqwest features json, multipart
CLAUDE.md                                 # Providers/Storage/Config sections updated

webui/app/
├── components/AgentForm.tsx              # single form (create + edit); Advanced section; auto read-image; Review card
├── routes/agent-settings.tsx             # thin wrapper: AgentForm mode="edit" + Sharing + Danger tabs
├── components/ProviderModelPicker.tsx    # NEW shared picker (logos, badges, groups, search, inline connect)
├── components/ProviderLogo.tsx           # NEW <img> + initials fallback
├── components/providers/                 # NEW providers page pieces: ConnectedList, AddProvider, ConnectForm,
│                                         #   CustomProviderForm (2-step), ModelsDrawer, DeleteWarning
├── routes/settingsRoot.tsx               # providers section uses the above; hard-coded ALL_VARIANTS removed
├── routes/usage.tsx                      # cost column
├── services/vizier.tsx                   # catalogue + custom provider API calls
└── interfaces/types.ts                   # CHAT_PROVIDERS / *_MODELS / DEFAULT_MODELS removed (media lists kept)
```

**Structure decision**: a single project. Two new agent-agnostic modules, `src/llm/` and `src/catalogue/`, sit beside `src/sandbox/`. They depend on nothing in `agents/`, and `agents/` depends on them.

## Phases

**rig is removed in one cut, and the provider layer is built first.**

- In Phase 1, the new `src/llm/` and `src/catalogue/` modules are written alongside the untouched rig code. Nothing is wired to them yet, so there is no shim and no conversion layer.
- Phase 2 is a single cutover. Every call site moves to the new layer, and rig is deleted in the same change. From the end of Phase 2 on, `cargo tree | grep rig-core` prints nothing (SC-007).
- Every phase compiles and passes clippy, tests and typecheck. The branch is merged as a whole, so no release ever contains both.

| # | Phase | Delivers | Key risks |
|---|---|---|---|
| 0 | **Research** ✅ | [research.md](research.md) | — |
| 1 | **Build the provider layer** (new code only; rig untouched and nothing wired) | `src/llm/`: types, errors, HTTP, SSE, auth (`StaticKey`, `CopilotExchange`, `ChatGptAccount`), the five completion dialects, the media and embedding clients, and `resolve`. `src/catalogue/`: data, R2 dialect derivation, R3 base-URL table, built-ins, the seed plus `scripts/catalogue_update.py` and `assets/catalogue/`. `ProviderId` with its legacy map. **Golden fixtures**: rig 0.39 is run once against a local capture server to record its request bodies and parsed responses for every dialect and media path, plus embedding vectors. These become committed fixtures that the new layer must match. Axum transport tests cover `Retry-After`, 5xx, timeouts, SSE and Copilot's refresh | Wire drift (cache control, schema sanitising, reasoning round-trip): the golden fixtures are the check |
| 2 | **Cutover: rig deleted** (one change) | Every call site moves to `llm::types` (schema, history, process, agent, context, tools, MCP, sandbox, read-image, dummyplug). `VizierModel::new` becomes resolve, dialect, retry. `retry.rs` moves to `LlmError` and honours `Retry-After`. Context and output limits follow R8. FR-013 image note and FR-033 read-image rules. Media and embedding wrappers use `credentials()`. `ProviderCredentials` and the `custom_provider` table. The startup migration (contract), YAML `providers` removed, onboarding updated. Deleted: `rig-core`, `model/provider.rs`, all rig adapters, the rig log directive. Dummyplug 008 and 014 scripts pass unchanged | A large diff: it is mechanical, but must land at once. Quickstart §1 (a real upgrade from `master`) and §3–§5 run before moving on |
| 3 | **WebUI form merge** (`refactor:`) | `agent-settings.tsx` becomes `AgentForm mode="edit"` plus Sharing and Danger, with a sticky save bar. No behaviour change | Settings-only fields lost in the merge: walk every field, old against new |
| 4 | **Catalogue and providers API** | `/catalogue` read API and logos; `/providers` (`used_by`, `reachable`, `test`); `/providers/custom` (CRUD, fetch, test); agent `warnings` | Secrets leaking through responses: the contract tests check every response |
| 5 | **WebUI pickers and providers page** | `ProviderModelPicker`, `ProviderLogo`, the providers page (connected list, search and connect, env detection, `used_by`, delete warning, reachability), the two-step custom provider form, inline connect, the Advanced section, automatic read-image, the Review card. Hard-coded chat lists removed | Picker performance on OpenRouter: client-side filtering plus a virtualised list |
| 6 | **Refresh and cost** | The `catalogue_snapshot`/`catalogue_logo` cache; lazy conditional refresh wired into the catalogue and provider read endpoints; lazy logo fetch; `POST /catalogue/refresh`; release workflow runs `just catalogue-update`; stats `provider`/`model`; `estimated_cost_usd`; usage cost column; `CLAUDE.md` updated | Refresh validation too strict or too loose |
| 7 | **Verification** | The whole quickstart (§1–§7), and live smoke tests on the main five dialects (§8) | — |

## Complexity Tracking

No violations. The Principle III refresh default is recorded in the Constitution Check as an owner ruling, not a deviation.
