# Research: Standalone Dummyplug

## R0 — Server runtime and packaging

**Decision**: Implement the standalone server as a Node.js 22 package using native JavaScript ES modules, Node's built-in HTTP server and `node:test`. Commit `package-lock.json`, expose `npm start` and `npm test`, and use `npm ci` in fresh checkouts and CI. Vizier remains Rust; its normal Cargo build does not build or start this package.

**Rationale**: The chosen Node.js runtime keeps the model server independently runnable and permits a small dependency set. The HTTP contract, deterministic legacy behavior and stateless boundary do not depend on the implementation language.

**Alternatives considered**: The Rust/Cargo server layout suggested in issue #95; superseded by the team's Node.js decision. Embedding the Node.js process in Vizier; rejected because the server must remain independently runnable.

## R1 — Compatibility boundary

**Decision**: Treat [legacy §1–§6](../008-dummyplug-provider/contracts/dummyplug-protocol.md) and the [context supplement](../013-reaction-awareness/contracts/dummyplug-context.md) as golden behavior. Port all current capabilities first, including text normalization, tool-list/sample formatting, JSON validation, tool-result echo, context and random lorem. Preserve the default tool call even for `tool_choice: none`; a test needing a different response uses its own proxy.

**Rationale**: The constitution makes existing dummyplug quickstarts a release gate. The current `DummyplugModel` parses prepared message parts, strips frontmatter and attached-file trailers, and recognizes fences and a separate `# Context` part. Server inputs travel through OpenAI Chat encoding, which can alter that representation; parity needs tests at both boundaries. In particular `joined_text` in `src/llm/dialect/openai_chat.rs` emits an omitted-attachment marker for tool-result attachments, whereas the current model renders attachment parts as `[image]`. Capture the old output before removal and address this difference without a new per-provider dispatch branch.

**Alternatives considered**: Approximate user-visible text from the issue; rejected because it misses edge cases and creates test regressions. Keep the in-process model as fallback; rejected because it bypasses the transport path and hides missing-server failures.

## R2 — Provider integration and URL resolution

**Decision**: Use the existing `OpenAiChat` dialect with base URL `http://127.0.0.1:9998/v1`. Resolve `DUMMYPLUG_URL` into a BaseUrl credential through the generic builtin field/env mechanism. Expose normal provider GET/PUT configuration so WebUI can edit `base_url`. Keep the static `dummyplug` model fallback and accept arbitrary agent model strings.

**Rationale**: `src/agents/agent/model/mod.rs` currently constructs `DummyplugModel` directly; `src/llm/resolve.rs` rejects dummyplug; `src/catalogue/builtin.rs` has no dialect or configurable fields; provider HTTP API explicitly hides it. All four exclusions must change together. `src/provider_keys.rs` currently reads builtin env names only for secret-like fields, so adding `DUMMYPLUG_URL` to catalogue metadata alone will not work. The generic credential resolution needs a field-aware fix and tests. `OpenAiChat` already sends chat completion requests, parses whole JSON replies, and probes `/models`.

**Alternatives considered**: New dummyplug-only HTTP client; rejected as duplicate provider plumbing. Hardcode env override in resolver; rejected under trait-based extensibility. Make Vizier consume SSE immediately; not required because its current OpenAI Chat dialect expects whole JSON, while external SDKs can use server SSE.

## R3 — Server protocol and deterministic response generation

**Decision**: Generate one canonical completion from the normalized request, then serialize it as whole JSON or SSE deltas. Include standard id/model/finish_reason/usage fields; emit `data: [DONE]` and an optional usage chunk when `stream_options.include_usage` is requested. Send tool-call arguments as JSON-encoded strings in function-call deltas. Use the same canonical answer for both modes. No previous request affects generation.

**Rationale**: This makes whole/stream equivalence testable for deterministic legacy commands and avoids two response engines. Random lorem and generated tool-call IDs are compared by contract shape across separate requests. The current Vizier OpenAI Chat decoder expects a first-choice message with content or tool calls. The [official OpenAI Chat Completions reference](https://platform.openai.com/docs/api-reference/chat) describes the wire envelope, SSE stream and optional usage chunk.

**Alternatives considered**: Generate SSE token-by-token with separate random calls; rejected because one completion could be internally inconsistent. Introduce a new protocol version; rejected because contract tests cover compatibility and there is one target client dialect.

## R4 — Stateless model, stateful harness

**Decision**: Keep dummyplug stateless and always independently runnable. Do not add test-specific commands, headers, scenario keys, request logs, or control endpoints. Tests needing ordered failures, delays, synthetic completions, or request inspection use a separate harness-owned HTTP proxy with its own isolation, redaction and cleanup policy. Dummyplug's chat endpoint remains keyless.

**Rationale**: A model-like server should map each request to an answer without knowing it is part of a test. Retry counters, injected failures, and captured prompts are properties of a test, so the test harness owns them. This also keeps private prompt logs out of the keyless dummyplug process. A dropped request cancels proxy delay/hold work and remaining server stream output.

**Alternatives considered**: Session-scoped scenarios, per-request `DUMMYPLUG` control commands or headers, and bounded in-memory logs inside dummyplug; rejected after architecture review because they make the model aware of test orchestration. Durable SQLite/Redis state or a process-global sequence would add the same responsibility and introduce cross-test coupling.

## R5 — Delivery and gates

**Decision**: Keep the server in `vizier-lab/dummyplug`, pin it as a submodule, and merge its PR before the dependent Vizier PR. The e2e harness explicitly starts the server; `just run` starts only Vizier. Compose uses an internal service URL through `DUMMYPLUG_URL`. CI tests the pinned server, a stock SDK, and Vizier's dummyplug agent path. Update submodule cascade automation to know `dummyplug → vizier` without disturbing the existing design/webui chain.

**Rationale**: An independent release artifact proves the server runs without Vizier. The gitlink makes CI reproducible. Existing `scripts/client-tools-e2e.mjs` and `scripts/durable-turns-e2e.mjs` assume a running Vizier/JWT/dummyplug agent; the harness must provision those dependencies and handle Vizier restart tests. The server's protocol and behavior remain usable from other clients even if Vizier's architecture changes.

**Alternatives considered**: A Vizier workspace member or supervised child process; rejected because the normal build/startup would depend on dummyplug. Floating submodule branch; rejected because CI and review would test different server revisions.
