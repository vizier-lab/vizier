# Implementation Plan: Standalone Dummyplug Test Server

**Branch**: `030-standalone-dummyplug-server` | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: [Issue #95](https://github.com/vizier-lab/vizier/issues/95), the approved feature specification, and the existing [dummyplug protocol](../008-dummyplug-provider/contracts/dummyplug-protocol.md).

## Summary

Extract dummyplug into an independent, stateless Node.js server in the `vizier-lab/dummyplug` repository, pinned by a `dummyplug/` git submodule. Port all existing §1–§6 behavior with compatibility tests, then expose OpenAI-compatible chat/model endpoints. The consuming test harness owns response injection, scenario sequencing, and request logs. Route Vizier's `dummyplug` provider through its existing OpenAI Chat HTTP dialect, preserving agent provider/model values and keeping the optional server out of Vizier's normal build and startup.

## Technical Context

**Language/Version**: Rust 2024 edition for Vizier; standalone server uses Node.js 22 and native JavaScript ES modules in its own npm package.

**Primary Dependencies**: Vizier's existing `reqwest`, `serde`, `serde_json`, `tokio`, `tracing`, provider catalogue and `OpenAiChat` dialect; standalone server uses Node.js built-in HTTP and test APIs, adding npm packages only when justified.

**Storage**: Vizier's existing SQLite is unchanged. Dummyplug stores no test state or request records. A test harness may keep temporary scenario state and logs outside dummyplug, with its own cleanup and redaction policy.

**Testing**: Server `npm ci` and `npm test` with `node:test`; Vizier `cargo test` and `cargo clippy`; stock OpenAI-compatible SDK conformance; server-independent legacy golden cases; [quickstart.md](quickstart.md) scripts the mandatory dummyplug-agent end-to-end gate. Run the documented checks locally; this feature adds no GitHub Actions workflow.

**Target Platform**: Local macOS/Linux development and Linux CI/container; do not add a dependency to Vizier's cross-build targets.

**Project Type**: Optional standalone HTTP test service plus a provider-catalogue/integration change in Vizier.

**Performance Goals**: Default local request completes without deliberate test delay; `/healthz` and `/v1/models` answer promptly enough for provider discovery; the test-owned proxy cancels its delayed work on disconnect.

**Constraints**: Preserve all legacy protocol cases and any model string; no API key for chat; no dummyplug-owned scenario or inspection API; no dummyplug process spawned by `just run`; no required service for Vizier startup or non-dummyplug agents.

**Scale/Scope**: One independent model server and a separate test-owned proxy for injected responses and multi-attempt scenarios; one new submodule, provider wiring, e2e and documentation updates. Media generation and embeddings are outside this feature.

## Constitution Check

| Gate | Design and verification |
|---|---|
| I. Lean by Default | Reuse Vizier's OpenAI Chat dialect and HTTP client. Keep the standalone Node.js service small; justify each npm dependency in its own PR. |
| II. DRY via Trait-Based Extensibility | Register dummyplug as an ordinary builtin provider with the existing dialect. Remove the `DummyplugModel` shortcut and resolver refusal instead of adding another provider branch. Share environment field handling rather than special-casing `DUMMYPLUG_URL`. |
| III. Self-Contained, Zero-Dependency Runtime | Dummyplug remains an optional model service. A normal Vizier binary, startup, and non-dummyplug agent run without the dummyplug Node.js service or its npm dependencies. `just run` starts Vizier only; test/Compose paths explicitly start dummyplug. |
| IV. Portability by Default | No OS-specific child process in Vizier; server binds a configurable address. Keep Vizier cross builds and single-binary release path unchanged. |
| V. Unified Errors & Observability | Vizier uses its existing `VizierError` conversion and `tracing` for provider failures. The separate server uses consistent HTTP error bodies and structured, credential-safe logging. |
| Quality gates | Run server `npm test`, Vizier `cargo test` and `cargo clippy`, server conformance, SDK cases, live Vizier dummyplug agent e2e and runtime startup checks. Test missing server as an ordinary provider error. |

**Pre-research gate**: Pass. **Post-design gate**: Pass. The optional server does not violate the single-binary runtime requirement; the implementation must prove this with an absent-server startup check.

## Implementation Sequence

1. **Independent server and compatibility baseline**: Create/obtain `vizier-lab/dummyplug`; implement the protocol and HTTP server as an independent Node.js package. Capture golden tests from current §1–§6 implementation, including frontmatter, context part, attachments, fences, tool schema ordering, invalid JSON, tool result echo, lorem shape, and default `tool_choice: none` behavior. Port these before adding the test-owned proxy.
2. **OpenAI wire contract**: Add `/healthz`, `/v1/models`, and `/v1/chat/completions` for whole and SSE replies. Normalize text strings/parts, tool calls/results, token limit and tool choice; compare whole/stream output for deterministic legacy commands. Test with a stock SDK as well as raw HTTP.
3. **Test harness**: Put failure sequences, delays/holds, substituted completions, usage, reasoning, malformed responses, and request inspection in a separate test-owned proxy. Add no test command, control header, scenario, request-log, reset, or release API to dummyplug.
4. **Vizier provider conversion**: Remove the in-process model and resolver refusal; set dummyplug's catalogue dialect to `OpenAiChat`, default base URL `http://127.0.0.1:9998/v1`, and optional `DUMMYPLUG_URL`. Generalize environment-to-provider-field resolution so the URL actually overrides. Permit GET/PUT provider configuration with editable `base_url`; retain the static model fallback and arbitrary model strings. Validate attachment and tool-result normalization across the HTTP boundary.
5. **Harness and documentation**: Add `dummyplug/` submodule, a `just dummyplug` recipe that runs `npm start` in that package, Node.js server checks, Compose service and service URL, and a test-owned proxy that sequences failures and captures requests when local e2e checks need state. Keep the checks runnable through the documented local harness; this feature does not add a GitHub Actions workflow. Revise protocol/provider API/quickstarts/docs and update submodule cascade order to `dummyplug → vizier`, while preserving design → webui → vizier for other changes.
6. **Review/merge order**: Land the independent dummyplug PR first. Pin that reviewed, fetchable commit in a Vizier PR containing provider conversion and harness/docs. A local-only commit is insufficient for recursive checkout or CI.

## Project Structure

### Documentation (this feature)

```text
specs/030-standalone-dummyplug-server/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── openai-chat.md
│   ├── test-controls.md
│   └── vizier-integration.md
└── checklists/
    └── requirements.md
```

### Source Code (two repositories)

```text
vizier-lab/dummyplug/          # separate repository, pinned at vizier/dummyplug/
├── package.json             # npm start and npm test scripts
├── package-lock.json        # reproducible standalone install
├── src/protocol.mjs          # legacy behavior, normalization, response generation
├── src/server.mjs            # HTTP routes and SSE; request-local work only
└── test/                     # golden protocol, SDK and HTTP conformance

vizier/
├── dummyplug/                # git submodule; outside the Vizier Cargo workspace
├── src/catalogue/builtin.rs
├── src/provider_keys.rs
├── src/llm/resolve.rs
├── src/agents/agent/model/mod.rs
├── src/channels/http/api/v1/providers/mod.rs
├── Justfile
├── docker-compose.yaml       # optional dummyplug service and internal URL
├── scripts/                  # e2e and submodule automation
└── docs/                     # setup and protocol docs
```

**Structure Decision**: A separate Node.js package keeps dummyplug out of Vizier's Cargo workspace and release binary. The gitlink pins the tested server revision. The HTTP and protocol contracts remain separate even if the package uses a flat source layout. This supersedes the Rust/Cargo packaging suggested in issue #95 while retaining its standalone server and HTTP contract goals.
