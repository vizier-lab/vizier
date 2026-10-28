# Feature Specification: Standalone Dummyplug Test Server

**Feature Branch**: `030-standalone-dummyplug-server`

**Created**: 2026-10-25

**Status**: Ready for implementation; acceptance pending

**Input**: [Vizier issue #95](https://github.com/vizier-lab/vizier/issues/95) and subsequent architecture feedback: preserve the existing protocol, run dummyplug as an independent stateless server, preserve legacy `tool_choice: none` behavior, and put all test orchestration and response injection in the consuming test harness. Media stubs remain out of scope.

## Context

Dummyplug is Vizier's offline test model. It currently runs inside Vizier, so its tests bypass the provider connection, request transport, response parsing, and failure handling used by external models. A separately runnable dummyplug lets Vizier's offline tests exercise that same path and lets another client use the test model. The replacement must preserve every existing dummyplug command and response rule. Existing agents retain their provider and model values without a data migration. Each chat response is generated from the supplied request, with no conversation memory, test-run state, request log, or new test-specific commands and headers. A test harness owns response injection, sequencing, inspection, and cleanup. This intentionally narrows issue #95's proposed server-side test controls in response to later architecture feedback.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Keep every existing dummyplug test working (Priority: P1)

A Vizier contributor runs an existing dummyplug agent against the new server. The agent still lists its available tools, describes a named tool, calls a tool from a valid JSON request, echoes the result, reveals the per-request context, and answers ordinary prompts as before. Existing quickstarts and tests work after the contributor starts the server.

**Why this priority**: The current protocol is the project's offline test harness. Losing any current behavior would make the replacement unsafe, regardless of its new features.

**Independent Test**: Run the existing §1–§6 protocol cases against the old model and the new server with equivalent requests, then run an existing dummyplug agent through Vizier. Compare exact output wherever the old result is deterministic and check the documented form of random output.

**Acceptance Scenarios**:

1. **Given** the same tool definitions, **When** a user sends `tools` or an exact tool name, **Then** the available-tool list and sample request match the current contract, including ordering, descriptions, arguments, and formatting.
2. **Given** a valid JSON tool request, **When** dummyplug handles it, **Then** Vizier receives one actionable tool call, and a later request containing its tool result returns the current echo format.
3. **Given** malformed JSON, invalid arguments, or an unknown tool, **When** dummyplug handles the request, **Then** it returns the current assistant-text error and runs no tool.
4. **Given** a request containing channel frontmatter, an attached-files trailer, a fenced command, and a separate context block, **When** dummyplug reads it, **Then** command matching and `context` behave exactly as in the existing protocol.
5. **Given** a message that is not a recognized command, **When** dummyplug answers, **Then** it retains the existing lorem-ipsum response form; tests check its documented shape rather than exact random words.
6. **Given** an existing agent configured with provider `dummyplug` and any model string, **When** its server is available, **Then** it works without editing or migrating that agent.
7. **Given** a request that disallows tool calls, **When** a legacy JSON tool command is sent, **Then** dummyplug retains its existing tool-call behavior.

---

### User Story 2 - Use dummyplug from another client (Priority: P2)

A developer starts dummyplug without starting Vizier, checks that it is ready, lists its model, and sends a chat request using a standard OpenAI-compatible client. The client can receive a whole response or streamed pieces representing the same answer.

**Why this priority**: Independent use and the real provider connection are the reason for extracting dummyplug.

**Independent Test**: Start only dummyplug and run a conformance script with a stock OpenAI-compatible SDK. Check model listing, ordinary text, a tool call and its result, and streaming.

**Acceptance Scenarios**:

1. **Given** dummyplug is running, **When** a client checks readiness and lists models, **Then** it receives a healthy response and the `dummyplug` model.
2. **Given** a chat request with text as a string or as text parts, **When** a client requests a whole response, **Then** it receives a valid completion with model, finish reason, message, and usage fields.
3. **Given** a deterministic legacy request, **When** a client requests streaming, **Then** the ordered chunks contain a role, content or tool-call deltas, a final finish reason, an optional usage result when requested, and an end marker; reassembled content equals the whole response.
4. **Given** a tool-call result after a prior assistant call, **When** a client sends both messages, **Then** dummyplug resolves the tool name and returns the documented echo.
5. **Given** no authorization header or an arbitrary authorization header, **When** a client uses the chat surface, **Then** neither requires a credential.

---

### User Story 3 - Exercise provider behavior offline (Priority: P2)

A test author uses dummyplug's ordinary chat API for legacy behavior. To test delay, failure, retry, nonzero usage, reasoning, or unusual tool calls, the test harness places a proxy in front of dummyplug and changes that test's HTTP responses. Dummyplug has no test-specific behavior to configure.

**Why this priority**: The current in-process model cannot test retries, transport failures, streaming, nonzero usage, or the exact outgoing request.

**Independent Test**: Without Vizier, send legacy requests before and after unrelated requests and verify command behavior does not depend on history. With Vizier, use a harness-owned proxy to record outgoing requests and inject an attempt sequence; prove retries, usage, cancellation, and parallel-test isolation.

**Acceptance Scenarios**:

1. **Given** a legacy request, **When** dummyplug handles it, **Then** it follows only the legacy protocol and reports default zero usage.
2. **Given** an equivalent legacy request before and after unrelated traffic or restart, **When** dummyplug handles it, **Then** deterministic commands match exactly and random lorem retains its documented shape.
3. **Given** a harness-delayed or held response, **When** Vizier disconnects or aborts, **Then** the harness stops pending work promptly and does not send a later response.
4. **Given** a harness plan for two failures followed by success, **When** Vizier retries through the harness proxy, **Then** the harness returns the specified failures for the first two attempts and forwards the third to dummyplug; retry timing can be checked from harness records.
5. **Given** a harness-injected nonzero usage response, **When** Vizier handles it, **Then** its turn data reports the specified input, output, and cached values.
6. **Given** two concurrent test runs, **When** their harness plans advance, **Then** each run selects its own next response; dummyplug retains no session counter or script.
7. **Given** a harness proxy records Vizier's requests, **When** the author inspects its log, **Then** it shows messages, tools, tool choice, token limit, and relevant non-secret headers; dummyplug exposes no inspection log.
8. **Given** harness-generated reasoning, multiple tool calls, malformed arguments, or a specified finish reason, **When** Vizier handles the response, **Then** the chosen case is represented correctly without changing dummyplug's protocol.

---

### User Story 4 - Run and maintain the two projects reliably (Priority: P3)

A contributor installs Vizier's dependencies, starts the test server and Vizier locally, or uses the sample container setup. The documented local checks cover the server on its own and Vizier against that server. A maintainer can review and merge the server change before the Vizier change that depends on it.

**Why this priority**: A replacement that cannot be started or tested consistently would weaken the project's mandatory offline gate.

**Independent Test**: Follow documented setup from a fresh checkout, run both local and container paths, and run the server's conformance tests plus Vizier's existing dummyplug end-to-end checks.

**Acceptance Scenarios**:

1. **Given** a fresh recursive checkout, **When** a contributor follows the setup instructions, **Then** the test server is available without building it as part of Vizier's normal build.
2. **Given** the test server is unavailable, **When** a dummyplug agent tries to use it, **Then** Vizier reports an ordinary local-provider availability failure rather than silently using a private fallback.
3. **Given** the sample container setup, **When** both services start, **Then** Vizier can reach dummyplug at the configured service address.
4. **Given** a contributor runs the documented local checks, **When** they test the pinned server version, **Then** standalone server checks and Vizier end-to-end checks can be run together without a GitHub Actions workflow added by this feature.
5. **Given** separate changes to the server and Vizier, **When** maintainers review them, **Then** the dependency and merge order are documented and the Vizier change pins the reviewed server revision.

### Edge Cases

- Several trailing tool results are echoed in order; an unresolved tool name falls back to its call identifier.
- User text mixed with a separate context part, frontmatter, attachments, or a surrounding code fence is normalized before command matching.
- A malformed or unknown tool request remains a successful chat response containing an error explanation; only injected transport failures use error status codes.
- Test harness behavior cannot change a request outside the specific test that enabled it.
- One harness-managed test must not consume another test's steps; an unmodified request to dummyplug always follows the legacy default behavior.
- An interrupted or intentionally truncated stream must not be mistaken for a successful complete answer.
- Harness request inspection can contain private prompt content; the harness must protect and clean up its records.
- A stalled or missing server must produce a visible provider failure and must not prevent Vizier itself from starting.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Dummyplug MUST preserve every currently documented §1–§6 command, normalization, tool-call, tool-result, error-text, and default-response rule as its default behavior.
- **FR-002**: Existing agents using provider `dummyplug` and any model string MUST continue to work without a data migration when the test server is running.
- **FR-003**: Dummyplug MUST always be an independently runnable server, with no Vizier library, process, workspace, or runtime dependency, and be usable by a standard OpenAI-compatible chat client without a required API key.
- **FR-004**: The public chat surface MUST accept whole and streamed completions, tool definitions, tool results, text strings and text parts, tool choice, token limits, and optional streamed usage.
- **FR-005**: Every successful whole response MUST include a valid model identifier, finish reason, message, and usage; streamed responses for deterministic legacy requests MUST reassemble to the same content as whole responses.
- **FR-006**: Dummyplug MUST expose a model listing containing `dummyplug` and a readiness check.
- **FR-007**: Vizier MUST use its ordinary provider connection and response handling for dummyplug; there MUST be no remaining in-process dummyplug model or fallback.
- **FR-008**: Vizier MUST use a local default dummyplug address and allow the operator to override it for local, remote, and container setups.
- **FR-009**: The test harness MUST be able to inject delay or hold, failures with optional `Retry-After`, retry sequences, and nonzero usage without adding controls to dummyplug.
- **FR-010**: The test harness MUST be able to supply reasoning content, multiple tool calls, malformed arguments, and a chosen finish reason for one response without adding controls to dummyplug.
- **FR-011**: The test harness MUST own multi-request scenarios and request inspection, including message content, tool definitions, tool choice, token limit, and relevant headers. Dummyplug MUST NOT persist scenario progress or request records.
- **FR-012**: Dummyplug MUST expose only its public model/readiness/chat API. It MUST NOT add test-specific commands, headers, scenario, request-log, reset, or release endpoints.
- **FR-013**: Dummyplug MUST handle each request without consulting prior requests or retaining test-session state. Deterministic legacy commands MUST produce the same answer regardless of request order, concurrent clients, or process restart; random lorem MUST retain its documented shape.
- **FR-014**: The harness MUST protect any captured prompts and credentials. Dummyplug's public chat surface MUST remain keyless; it MUST NOT store or expose captured requests.
- **FR-015**: Streaming work in dummyplug and delayed or held work in the harness MUST stop when the requesting client disconnects or cancels.
- **FR-016**: Legacy behavior that may call a tool despite `tool_choice: none` MUST remain unchanged. A test may exercise alternative behavior through a harness-generated response, not a dummyplug control.
- **FR-017**: Standalone conformance checks MUST cover model listing, whole and streamed text, tool calls, tool-result echo, and all legacy protocol cases without requiring Vizier.
- **FR-018**: Vizier's existing dummyplug quickstarts and end-to-end checks MUST run with the server; the documented local harness MUST start the server and support checks across both projects. This feature does not add a GitHub Actions workflow.
- **FR-019**: Documentation MUST explain local and container startup, the separate-server requirement, address override, harness-owned test behavior, all legacy commands, and the reviewed server-to-Vizier update order; stale tool examples MUST be corrected.
- **FR-020**: The normal Vizier build and test MUST remain independent of building the dummyplug server, and Vizier MUST remain runnable without the server when no dummyplug agent is used.

### Key Entities

- **Chat request**: The messages, tool definitions, model name, and output controls supplied by a client.
- **Chat completion**: The assistant's text or tool calls, finish reason, model, and usage, delivered whole or in pieces.
- **Test scenario**: A sequence of replies or failures selected and tracked by the consuming test harness, outside dummyplug.
- **Request record**: A temporary inspection copy captured and owned by the test harness, outside dummyplug.
- **Provider configuration**: The address Vizier uses to reach dummyplug; existing agent provider and model values remain unchanged.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of existing documented §1–§6 cases pass against the standalone server, with exact matches for deterministic cases and the documented response shape for random cases.
- **SC-002**: 100% of existing Vizier dummyplug quickstarts and end-to-end checks pass with the server running; no test requires a live model credential.
- **SC-003**: A stock OpenAI-compatible SDK completes model listing, whole chat, streamed chat, a tool call, and tool-result echo against dummyplug without Vizier.
- **SC-004**: For every deterministic legacy conformance case, reassembling streamed content yields exactly the same answer as a whole completion; random cases satisfy the documented shape.
- **SC-005**: The documented test harness demonstrates a temporary failure followed by success, a failed turn, nonzero usage, a cancellable hold, and two isolated parallel scenarios without dummyplug test controls or state.
- **SC-006**: A contributor following the documented local setup can start both programs and complete a dummyplug conversation without editing an existing agent.
- **SC-007**: The normal Vizier build and tests complete without building dummyplug; Vizier starts successfully when the optional dummyplug server is absent and no dummyplug agent is used.

## Completion Gate

The feature is complete only when the following evidence is recorded against the *same* dummyplug commit pinned by Vizier. A passing replacement harness alone does not substitute for the existing checks named below.

| Gate | Required observable evidence |
|---|---|
| Legacy compatibility | Run the documented §1–§6 cases, including normalization, malformed commands, multiple trailing tool results, and `tool_choice: none`, against the standalone server. Compare deterministic output exactly and random lorem by documented shape. |
| Existing Vizier flows | Run `scripts/client-tools-e2e.mjs` and `scripts/durable-turns-e2e.mjs` with a Vizier agent using the HTTP dummyplug provider. Run the existing dummyplug quickstart conversation, including a state-changing tool and verification of its result. Record any intentionally skipped restart step separately; a skip is not a pass for that step. |
| Provider failures and retries | Through a test-owned HTTP proxy, return two failures and forward the third request to dummyplug. Observe three proxy-recorded attempts for one Vizier turn and the final successful answer. Separately exhaust Vizier's retry policy and observe a failed turn while `/api/v1/ping` remains healthy. Verify nonzero and cached usage in the proxy's response and Vizier's recorded turn data; direct dummyplug usage stays at its legacy zero default. |
| Cancellation and streaming | While the harness holds or delays a response, disconnect Vizier before release; prove the harness sends no later completion. Abort a dummyplug SSE client mid-stream and verify production stops. A harness-truncated stream must omit both the final finish chunk and `[DONE]`. Check whole/stream equivalence for deterministic text and tool-call content. |
| Statelessness and isolation | Send deterministic request A, then unrelated requests, then A again; compare exact responses. Repeat with concurrent clients and after restart; check random cases by shape. Confirm dummyplug has no test commands, headers, scenario or log endpoints, or request-record store. Run two parallel harness plans and prove each captures only its own attempts; protect and clean up captured prompts in the harness. |
| Deployment and publication | Push the server commit to `vizier-lab/dummyplug` before pinning it in a Vizier PR. From a fresh recursive checkout, resolve the pinned gitlink, run the documented local path and Compose path, and reach dummyplug from Vizier at the service URL. Run standalone checks and Vizier gates locally against that gitlink. The normal Vizier build and a non-dummyplug startup must succeed without building or starting dummyplug. |

Record each command, commit SHA, result, and any environmental limitation in the implementation PR. A gate that has not run remains **unverified**, even if the corresponding code is present.

## Assumptions

- Issue #95 defines the legacy protocol and the required acceptance boundary; this spec prioritizes complete migration of current behavior before additive controls.
- Dummyplug is a test provider. Media-generation and embedding stubs are separate work.
- The server runs as a separate process during development and in container deployments. The end-to-end harness starts it automatically.
- The default address is local; operators can override it for other deployment layouts.
- The test harness, not dummyplug, owns scenario progress, retry counters, and request inspection. Its storage and cleanup policy belong to the test implementation.
- Random lorem-ipsum text and generated tool-call identifiers are not compared byte-for-byte across requests; their formatting and dispatch rules remain compatible.
- Tests and contracts, rather than a separate protocol-version number, govern compatibility of Vizier-specific input framing.
