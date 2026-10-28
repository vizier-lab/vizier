# Tasks: Standalone Dummyplug Test Server

**Input**: Design documents from `specs/030-standalone-dummyplug-server/`

**Prerequisites**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, and `quickstart.md`

**Tests**: Included because the specification explicitly requires legacy conformance, SDK compatibility, provider integration, failure/retry, streaming, and deployment evidence.

**Organization**: Tasks are grouped by the four user stories in `spec.md`. The standalone server repository must be published before Vizier pins its gitlink.

## Phase 1: Setup

**Purpose**: Establish the independently maintained server and record its source revision.

- [X] T001 Create or confirm the standalone Node.js package and `start`/`test` scripts in `dummyplug/package.json` and `dummyplug/package-lock.json`
- [X] T002 [P] Configure Node.js ignore rules in `dummyplug/.gitignore`
- [X] T003 [P] Add local and container startup instructions to `dummyplug/README.md` and `dummyplug/Dockerfile`

## Phase 2: Foundational

**Purpose**: Define request-local normalization and response representation shared by legacy and OpenAI surfaces.

- [X] T004 Implement OpenAI chat request normalization and legacy command parsing in `dummyplug/src/protocol.mjs`
- [X] T005 Implement request-local canonical completions and JSON/SSE serialization in `dummyplug/src/protocol.mjs`
- [X] T006 Add the standalone HTTP server, configurable bind address, structured logging, and disconnect cancellation in `dummyplug/src/server.mjs`

## Phase 3: User Story 1 - Preserve Existing Dummyplug Behavior (Priority: P1)

**Goal**: Keep the current Vizier dummyplug protocol behavior available from the standalone server.

**Independent Test**: Run the legacy §1–§6 cases against the server and the old implementation's contract, checking deterministic replies exactly and lorem/tool IDs by documented shape; then run a dummyplug agent through Vizier.

### Tests for User Story 1

- [X] T007 [P] [US1] Add legacy protocol conformance cases for command precedence, normalization, tool listing/description, JSON tool requests, errors, and `tool_choice: none` in `dummyplug/test/legacy-protocol.test.mjs`
- [X] T008 [P] [US1] Add statelessness and multi-tool-result echo cases in `dummyplug/test/request-isolation.test.mjs`

### Implementation for User Story 1

- [X] T009 [US1] Port legacy reply precedence, tool listing/description, and sample JSON generation to `dummyplug/src/protocol.mjs`
- [X] T010 [US1] Port valid tool-call parsing, malformed/unknown request text, and trailing tool-result echo to `dummyplug/src/protocol.mjs`
- [X] T011 [US1] Preserve lorem response shape, default zero usage, and historical tool-call behavior when `tool_choice` is `none` in `dummyplug/src/protocol.mjs`
- [X] T012 [US1] Ensure each request discards normalized input and generated scenario data after response completion in `dummyplug/src/server.mjs`
- [ ] T013 [US1] Run the existing Vizier dummyplug flow against the standalone server and record deterministic comparisons in `specs/030-standalone-dummyplug-server/acceptance-status.md` (a basic live `tools` command smoke passed; full quickstart/tool-result comparison remains open)

## Phase 4: User Story 2 - OpenAI-Compatible Standalone Use (Priority: P2)

**Goal**: Allow a stock OpenAI-compatible client to check readiness, list the model, and make whole or streamed chat requests.

**Independent Test**: Start only dummyplug; use an OpenAI-compatible SDK and raw HTTP to check health, model listing, text, tool call/result, whole response, and SSE reassembly.

### Tests for User Story 2

- [X] T014 [P] [US2] Add HTTP contract tests for health, model listing, whole completions, accepted authorization headers, and malformed payloads in `dummyplug/test/openai-http.test.mjs`
- [X] T015 [P] [US2] Add SSE tests for role/content/tool-call deltas, optional usage chunk, finish reason, `[DONE]`, cancellation, and whole/stream equivalence in `dummyplug/test/openai-stream.test.mjs`
- [X] T016 [P] [US2] Add a stock OpenAI-compatible SDK conformance client in `dummyplug/test/sdk-conformance.test.mjs`

### Implementation for User Story 2

- [X] T017 [US2] Implement `GET /healthz`, `GET /v1/models`, and `POST /v1/chat/completions` in `dummyplug/src/server.mjs`
- [X] T018 [US2] Implement OpenAI Chat Completions JSON and tool-result wire mapping in `dummyplug/src/protocol.mjs`
- [X] T019 [US2] Implement SSE serialization from the canonical completion and stop production on client disconnect in `dummyplug/src/server.mjs`
- [X] T020 [US2] Run raw HTTP and SDK conformance checks and record results in `specs/030-standalone-dummyplug-server/acceptance-status.md`

## Phase 5: User Story 3 - Harness-Owned Provider Behavior Tests (Priority: P2)

**Goal**: Test retries, injected responses, usage, streaming failures, and request introspection through an isolated test-owned proxy while dummyplug remains stateless.

**Independent Test**: Route Vizier through the test proxy; verify a two-failures-then-success sequence, exhausted retries, usage accounting, cancellation, and two isolated concurrent scenarios. Confirm dummyplug exposes no test controls or request logs.

### Tests for User Story 3

- [X] T021 [P] [US3] Add proxy tests for delay/hold, injected status plus `Retry-After`, substituted completions, cancellation, redaction, cleanup, and scenario isolation in `scripts/dummyplug-test-proxy.test.mjs`
- [X] T022 [P] [US3] Add server-surface tests proving test-specific commands, headers, scenario endpoints, and request-log endpoints are absent in `dummyplug/test/public-surface.test.mjs`

### Implementation for User Story 3

- [X] T023 [US3] Implement a test-owned forwarding proxy with per-test response plans, cancellation, redacted request capture, and cleanup in `scripts/dummyplug-test-proxy.mjs`
- [X] T024 [US3] Add harness fixtures for retry sequences, nonzero/cached usage, reasoning, multiple tool calls, malformed arguments, finish reasons, and truncated SSE in `scripts/dummyplug-vizier-e2e.mjs`
- [ ] T025 [US3] Run retry, failure, usage, streaming-cancellation, and parallel-isolation scenarios and record evidence in `specs/030-standalone-dummyplug-server/acceptance-status.md`

## Phase 6: User Story 4 - Vizier Integration and Reliable Project Workflow (Priority: P3)

**Goal**: Route existing dummyplug agents through ordinary HTTP provider handling and make local, Compose, documentation, and submodule updates reliable.

**Independent Test**: From a fresh recursive checkout, run the server and Vizier locally and through Compose; verify existing agent configuration works, missing dummyplug does not block Vizier startup, and normal Vizier build/tests do not build the server.

### Tests for User Story 4

- [X] T026 [US4] Add provider resolution tests for the default URL, configured `base_url`, and `DUMMYPLUG_URL` precedence in `src/provider_keys.rs`
- [X] T027 [US4] Add API tests for dummyplug provider GET/PUT and catalogue behavior in `src/channels/http/api/v1/providers/mod.rs`
- [ ] T028 [P] [US4] Add Vizier HTTP-provider integration checks for legacy commands, tool results, missing-server behavior, and arbitrary model strings in `scripts/dummyplug-vizier-e2e.test.mjs`

### Implementation for User Story 4

- [X] T029 [US4] Register dummyplug as a local `OpenAiChat` provider with `BaseUrl`, `DUMMYPLUG_URL`, and the local default in `src/catalogue/builtin.rs` and `src/provider_keys.rs`
- [X] T030 [US4] Remove the in-process dummyplug model branch and resolver refusal in `src/agents/agent/model/mod.rs` and `src/llm/resolve.rs`
- [X] T031 [US4] Make provider API GET/PUT expose the effective base URL while refusing unsupported fields in `src/channels/http/api/v1/providers/mod.rs`
- [X] T032 [US4] Add the `dummyplug/` git submodule and local server/test recipes in `.gitmodules` and `Justfile`
- [X] T033 [US4] Add the optional Compose service and internal `DUMMYPLUG_URL` in `docker-compose.yaml`
- [X] T034 [US4] Add dummyplug to submodule PR discovery and document the `dummyplug → vizier` merge order in `scripts/submodule-prs.mjs` and `docs/src/getting-started/contributing.md`
- [X] T035 [US4] Update provider docs, dummyplug quickstarts, and stale tool examples for separate-server startup in `docs/src/configuration/providers.md`, `docs/src/getting-started/quick-start.md`, and `specs/008-dummyplug-provider/`
- [X] T036 [US4] Verify normal Vizier test suite and non-dummyplug startup without the optional server; record results in `specs/030-standalone-dummyplug-server/acceptance-status.md`
- [ ] T037 [US4] Complete local and Compose setup, recursive-checkout, e2e, failure/retry, cancellation, and pinned-revision evidence in `specs/030-standalone-dummyplug-server/acceptance-status.md`

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; establish the independent server repository first.
- **Foundational (Phase 2)**: Depends on setup; defines types, normalization, response representation, and server bootstrap.
- **US1 (Phase 3)**: Depends on foundational protocol and server skeleton; establishes the P1 compatibility baseline.
- **US2 (Phase 4)**: Depends on US1's canonical legacy completion behavior; exposes it over OpenAI HTTP/SSE.
- **US3 (Phase 5)**: Proxy implementation is independent of dummyplug internals, but its full Vizier scenarios depend on US2 and the provider wiring from US4.
- **US4 (Phase 6)**: Provider wiring can proceed after the server API contract (US2) is fixed; full integration/e2e and publication depend on US1–US3 and the fetchable server commit.
- **Polish/completion evidence**: US4 T036–T037 close the cross-cutting build, startup, documentation, deployment, and acceptance gates.

### User Story Dependencies

- **US1 (P1)**: First deliverable; establishes legacy behavior independently of Vizier's HTTP integration.
- **US2 (P2)**: Builds the public API over the US1 reply implementation.
- **US3 (P2)**: Harness proxy can be developed separately; end-to-end provider assertions require US2 and US4.
- **US4 (P3)**: Integration and release path depend on the server's public contract; this story completes the two-project workflow.

### Parallel Opportunities

- T002 and T003 can run in parallel after repository initialization.
- T007 and T008 can be authored in parallel; T014–T016 can be authored in parallel after request/response contracts are fixed.
- T021 and T022 can be authored in parallel; the proxy does not require dummyplug-owned state.
- T028 can be authored in parallel with provider implementation because it uses a separate test file; T026–T027 precede the implementation tasks that edit their respective source files.

## Implementation Strategy

### MVP First (User Story 1)

1. Complete setup and foundational request/response types.
2. Implement and verify the legacy protocol in the standalone server.
3. Run the existing Vizier dummyplug flow against it.
4. Stop and validate the compatibility gate before adding public API and harness features.

### Incremental Delivery

1. Land the independent dummyplug server with the legacy protocol and OpenAI API (US1 + US2).
2. Land the Vizier provider conversion against that reviewed, fetchable server revision (US4 core).
3. Add harness-owned injection and request inspection (US3), then run the complete integration and deployment gates.

## Notes

- All task descriptions identify an owning repository path; `dummyplug/...` paths refer to the standalone submodule repository, implemented as Node.js ES modules.
- `[P]` marks tasks that can be implemented in parallel without editing the same files or depending on incomplete work.
- Tests are included because the spec requires conformance and records explicit acceptance evidence.
- Do not mark a completion gate passed until command, server commit SHA, Vizier commit SHA, result, and any limitation are recorded in `acceptance-status.md`.
