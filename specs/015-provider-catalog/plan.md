# Implementation Plan: Catalogue-Driven Providers and Models (and Removing rig)

**Branch**: `015-provider-catalog` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md) | **Issue**: #69

**Input**: Feature specification from `specs/015-provider-catalog/spec.md`

## Summary

Remove `rig-core` and replace it with an in-tree provider layer, `src/llm/`. It has:

- our own message, tool and usage types;
- a typed HTTP error, so `Retry-After` is honoured;
- five wire **dialects**: OpenAI chat, OpenAI Responses, Anthropic, Gemini and Ollama;
- curated media and embedding clients on the same transport.

Which providers and models exist stops being a hand-written list. It comes from a **models.dev catalogue snapshot** bundled in the binary (about 350 KB, plus about 300 KB of logos) and refreshed in the background. Each catalogue provider's `npm` hint, together with per-model route overrides, picks its dialect. That makes about 210 of the 230 catalogue providers usable with no per-provider code. Only quirks that can't be expressed as data are implemented as trait impls: Copilot's token exchange, ChatGPT's account auth, and Azure's URL template.

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
- Catalogue files under `{workspace}/.runtime/catalogue/`.

**Testing**: `cargo test` covers:
- dialect `encode`/`decode` against fixtures;
- transport against an in-process axum server;
- catalogue parsing and validation;
- the `ProviderId` legacy map;
- custom-model merge;
- context-window resolution;
- a temporary rig-parity harness.

End to end: dummyplug agent steps plus the stdlib `fixtures/stub_llm.py` in [quickstart.md](quickstart.md). Live-key steps are marked.

**Target Platform**: unchanged: Linux, macOS, Windows, and the musl and cross targets in `Cross.toml`. Removing rig also removes its dependency tree from them.

**Project Type**: single Rust binary with an embedded WebUI.

**Performance Goals**:
- The catalogue loads in under 200 ms at startup.
- Picker endpoints answer from memory in under 50 ms.
- An OpenRouter model search (380+ models) filters on the client in under 2 s (SC-008).
- A refresh never blocks a turn, because readers clone an `Arc`.

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
| **I. Lean by Default** | ✅ Net removal: rig (a 43k-line dependency), about 50 near-identical provider match arms and builder impls, the 29-variant `ProviderEntryConfig`, the duplicate agent form, and the hard-coded WebUI model lists. No new crates: SSE is parsed by hand (R6), zip handling and the transport-test server are already dependencies. No streaming API and no speculative abstractions; each trait has at least two real implementations (`CompletionProvider` ×5 dialects plus dummyplug, `CredentialSource` ×3). |
| **II. DRY via Trait-Based Extensibility** | ✅ Wire formats are `CompletionProvider` impls, and providers are **data** (catalogue rows plus a small base-URL table). Adding a provider means a catalogue update, or a custom provider in the UI, with no code. Quirks are `CredentialSource` impls and endpoint rules, never `if provider == …` in dialect code (R5). Media providers stay curated enums with one `credentials()` mapping table (data-model §8); their media-specific dispatch is unchanged in this feature. |
| **III. Self-Contained Runtime** | ✅ **The catalogue and logos are embedded**, so the binary works offline and on first start. **Background refresh is on by default.** The project owner ruled on 2026-10-15 that it complies (R13): it fetches metadata only, is never required, and degrades silently to local data, so local-first behaviour is unaffected. It can be turned off with `catalogue.refresh: false`. The Distribution constraint's literal wording ("MUST be gated behind explicit user configuration") reads more strictly than that ruling. A PATCH clarification to `constitution.md` is recommended as a follow-up so reviewers don't flag it again. Logos are served from the binary, so the WebUI makes no third-party requests. |
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
│   └── catalogue-snapshot.md    # zip layout, trimming, refresh validation, logo rendering
├── fixtures/
│   └── stub_llm.py              # created in implementation (quickstart §Fixtures)
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
assets/catalogue/catalogue.zip            # NEW bundled snapshot (api.json + logos + meta.json)
scripts/catalogue_update.py               # NEW release-time snapshot builder (stdlib only)
justfile                                  # + catalogue-update recipe

src/llm/                                  # NEW — agent-agnostic provider layer (replaces rig)
├── mod.rs                                # traits: CompletionProvider, CredentialSource, Embedding/Stt/Tts/ImageGenProvider
├── types.rs                              # Message, parts, ToolDefinition, ToolChoice, Usage, request/response
├── error.rs                              # LlmError (+ From for VizierError)
├── http.rs                               # shared reqwest client, send_json/send_multipart, status→LlmError, redaction
├── sse.rs                                # whole-body SSE parser (ChatGPT)
├── resolve.rs                            # (ProviderId, model, overrides) → ResolvedModel
├── auth.rs                               # StaticKey, CopilotExchange, ChatGptAccount
├── dialect/{openai_chat,openai_responses,anthropic,gemini,ollama}.rs + fixtures/
├── embed/{openai,gemini,cohere,ollama}.rs
├── audio/{openai_stt,gemini_stt,hf_stt,openai_tts,hyperbolic_tts}.rs
└── image/{openai,hyperbolic,hf}.rs

src/catalogue/                            # NEW
├── mod.rs                                # Catalogue handle (Arc<RwLock<Arc<CatalogueData>>>), queries for pickers
├── data.rs                               # CatalogueData/Provider/Model, parse + trim, dialect derivation (R2), base-URL table (R3)
├── builtin.rs                            # ollama, llama-cpp, chatgpt, dummyplug, elevenlabs, voyageai, hyperbolic
├── snapshot.rs                           # zip read/write, bundled vs refreshed selection
└── refresh.rs                            # fetch, validate, atomic write; driven by VizierScheduler

src/config/provider.rs                    # ProviderVariant → ProviderId (+ legacy map); ProviderConfig (YAML) removed
src/config/mod.rs                         # VizierConfig.providers removed; catalogue settings added
src/provider_keys.rs                      # collapses to credential lookup: saved → catalogue env[] → legacy env
src/schema/provider.rs                    # ProviderEntryConfig → ProviderCredentials; CustomProvider, CustomModel
src/storage/provider.rs, storage/sqlite/provider.rs   # ProviderId keys; + CustomProviderStorage, custom_provider table
src/dependencies.rs                       # + catalogue load, http client; provider-id migration; migrate_providers removed
src/agents/agent/model/                   # mod.rs shrinks to resolve→dialect→retry; provider.rs DELETED;
                                          #   retry.rs on LlmError (+Retry-After); registry.rs kept as last fallback; dummyplug on new types
src/schema/{request,response,history}.rs, agents/process.rs, agents/agent/mod.rs,
src/agents/agent/system_prompt/context.rs, agents/tools/{mod,read_image/mod}.rs,
src/agents/mcp/mod.rs, sandbox/docs.rs     # rig types → llm::types; FR-013 image note; FR-033 read-image rules
src/embedding/, stt/, tts/, image_generation/  # per-provider rig adapters DELETED → thin wrappers over src/llm; credentials() table
src/scheduler/mod.rs                      # runs catalogue refresher
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

Each phase compiles, passes clippy, tests and typecheck, and leaves vizier working. rig stays until Phase 8, so any unported path can still use it.

| # | Phase | Delivers | Key risks |
|---|---|---|---|
| 0 | **Research** ✅ | [research.md](research.md) | — |
| 1 | **WebUI form merge** (`refactor:`) | `agent-settings.tsx` → `AgentForm mode="edit"` plus Sharing and Danger; sticky save bar. No behaviour change | Settings-only fields lost in the merge → walk every field, old against new |
| 2 | **Own the types** | `src/llm/types.rs`; all non-model call sites on our types; one `rig_shim.rs` at the model boundary. Dummyplug 008 and 014 scripts pass | Large mechanical diff → land it alone |
| 3 | **Transport and errors** | `llm/{http,error,sse,auth}.rs`; axum transport tests; `retry.rs` on `LlmError` with `Retry-After` | Copilot refresh edge cases |
| 4 | **Catalogue core** | `src/catalogue/`, the bundled snapshot plus `scripts/catalogue_update.py`, `ProviderId` with its legacy map, `ProviderCredentials`, the `custom_provider` table, the startup migration, YAML `providers` removed, the `/catalogue` read API and logos | The migration's base-URL rule; agents left on removed providers |
| 5 | **Completion dialects** | `openai_chat`, then `anthropic`, `ollama`, `gemini`, `openai_responses` (OpenAI, then ChatGPT), each with fixtures and a parity test; `llm::resolve`; `VizierModel::new` → resolve, dialect, retry; context and output from R8; FR-013 image note; FR-033 read-image rules; `model/provider.rs` deleted | Wire drift (cache control, schema sanitising, reasoning round-trip) → parity harness plus live smoke tests |
| 6 | **Media and embeddings** | `llm/{embed,audio,image}`; curated enums use `credentials()`; vector-parity test; constructor `expect`s removed | Embedding vectors differ → index drift |
| 7 | **Providers API and WebUI** | `/providers` (`used_by`, `reachable`, `test`), `/providers/custom` (CRUD, fetch, test), agent `warnings`; `ProviderModelPicker`, `ProviderLogo`, providers page, custom provider form, inline connect, Advanced section, auto read-image, Review card | Picker performance on OpenRouter → client-side filtering and a virtualised list |
| 8 | **Refresh, cost, rig removal** | Refresher in `VizierScheduler`; `POST /catalogue/refresh`; stats `provider`/`model` and `estimated_cost_usd`; usage cost column; rig, shim and parity tests removed; `main.rs` directive; `CLAUDE.md` | Refresh validation too strict or too loose |
| 9 | **Verification** | the whole quickstart (§1–§7), and live smoke tests on the main five dialects (§8) | — |

## Complexity Tracking

No violations. The Principle III refresh default is recorded in the Constitution Check as an owner ruling, not a deviation.
