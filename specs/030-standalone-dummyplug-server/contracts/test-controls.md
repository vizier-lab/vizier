# Contract: Test Harness Controls

Dummyplug has no test-control API. It exposes only `GET /healthz`, `GET /v1/models`, and `POST /v1/chat/completions` and preserves the legacy `tools`, `context`, exact tool-name, JSON tool-request, tool-result, and lorem behavior. It does not recognize new `DUMMYPLUG` commands or `X-Dummyplug-*` control headers.

## Harness-owned response injection

Tests that need behavior beyond dummyplug's ordinary responses place a test-owned HTTP proxy at Vizier's configured `DUMMYPLUG_URL`. The proxy can record outgoing requests, delay or hold a response, return a selected HTTP failure with `Retry-After`, forward a request to dummyplug, or substitute a Chat Completion with fixed text, usage, reasoning, tool calls, malformed arguments, or a chosen finish reason. A streaming test can produce or truncate SSE at the proxy. These are test fixtures, not dummyplug features.

For a two-failures-then-success test, the proxy owns the attempt counter, returns failures for attempts one and two, forwards attempt three, and records request timing. Each concurrent test has isolated proxy state. The proxy controls retention, redaction, and cleanup of captured prompts and credentials. Dummyplug receives no test ID, scenario plan, retry count, or inspection request.

## Cancellation and compatibility

A disconnected client cancels proxy delay or hold work and dummyplug streaming work promptly. A truncated stream omits normal finish and `[DONE]`. A test that does not opt into proxy behavior is forwarded unchanged and follows dummyplug's legacy behavior, including default zero usage and the historical `tool_choice: none` handling.
