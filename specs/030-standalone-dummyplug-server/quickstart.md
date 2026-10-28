# Quickstart and Acceptance: Standalone Dummyplug

These commands and checks are the acceptance script for the implementation PR. The `dummyplug/` submodule and `just dummyplug` recipe are planned interfaces; they are not part of this spec-only branch.

Use the [completion gate](spec.md#completion-gate) for required evidence. Run the implementation checks locally against the exact dummyplug commit pinned by the Vizier gitlink; publish that commit before testing a fresh recursive checkout.

## 1. Start both processes locally

From a recursive Vizier checkout with Node.js 22 installed, run in separate terminals:

```sh
just install
npm ci --prefix dummyplug
just dummyplug
```

```sh
env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY just run
```

Expect `curl http://127.0.0.1:9998/healthz` to be healthy and `curl http://127.0.0.1:9998/v1/models` to include `dummyplug`. `just run` starts Vizier only. Stop dummyplug, restart Vizier, and verify Vizier still starts; a dummyplug turn then reports a provider connection failure. Restart dummyplug for the remaining checks.

## 2. Check the public API without Vizier

```sh
curl -sS http://127.0.0.1:9998/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"dummyplug","messages":[{"role":"user","content":"tools"}]}'
```

Expect a Chat Completion with assistant text, model, finish reason and zero usage. Repeat the deterministic `tools` request: its content must match. Repeat with `"stream":true` and `"stream_options":{"include_usage":true}`; content reconstructed from SSE deltas equals the whole `tools` answer, the stream ends with `[DONE]`, and the optional usage chunk has empty `choices`. For random lorem, compare the documented shape. Also run the same model-list/chat/stream/tool-call/tool-result sequence with a stock OpenAI-compatible SDK pointed at this base URL and a dummy API key if its constructor requires one.

## 3. Run the constitutional dummyplug-agent gate

In WebUI, create or reuse an agent with provider `dummyplug` and model `dummyplug` (an existing agent needs no edits). Send the following in one topic:

| Send | Expected reply or effect |
|---|---|
| `hello` | 1–3 nonempty lorem paragraphs. |
| `tools` | Sorted, formatted available-tool list. |
| An exact tool name from that list, such as `think` | Full description and fenced sample JSON. |
| Paste the generated JSON request | One actionable tool call, tool history entry, then `**Tool result** (...)` echo. |
| `{"tool":"nope"}` | Unknown-tool assistant text; no tool runs. |
| `{"tool":` | Parse-error assistant text; no tool runs. |
| `context` | The verbatim current `# Context` block or `(no context block)`. |

Repeat the JSON step with an available state-changing action through `execute_python`; verify its result through the existing UI. Run the established dummyplug quickstarts and `scripts/client-tools-e2e.mjs` and `scripts/durable-turns-e2e.mjs` through the new harness. Compare all legacy §1–§6 goldens, including frontmatter, attached-file markers, fenced JSON, multiple trailing tool results, tool schema ordering, and `tool_choice: none`. No live model credential is required.

## 4. Exercise harness behavior and server isolation

Send `tools`, then unrelated requests, then `tools` again. The two answers must match exactly, including after a dummyplug restart. Verify dummyplug has no test-specific commands, control headers, scenario API, or request-log endpoint. For a Vizier retry test, place a test-owned HTTP proxy at `DUMMYPLUG_URL`: the proxy returns two selected failures, forwards the third attempt to dummyplug, and records requests and timing. Run two proxy plans concurrently and verify their counters and records remain isolated. The proxy must not retain Authorization or other credentials in its logs.

Have the proxy hold a response, cancel the Vizier client, and verify no later response or lingering proxy task. Test a proxy-truncated SSE stream: it must lack a normal finish and `[DONE]`. Have the proxy inject nonzero usage for one request and verify Vizier records it; a direct unmodified dummyplug request still reports zero usage. A plain `HOLD` or `DUMMYPLUG FAIL` sent directly to dummyplug follows legacy fallback behavior.

## 5. Run project and deployment gates

The optional Compose profile starts the server and gives Vizier its internal URL:

```sh
docker compose --profile dummyplug up -d
```

Without that profile, Vizier does not start or require dummyplug.

```sh
npm ci --prefix dummyplug
npm test --prefix dummyplug
```

From Vizier's root:

```sh
cargo test
cargo clippy
node --test scripts/*.test.mjs
```

Run the local e2e harness against the pinned submodule commit and run Vizier with `DUMMYPLUG_URL=http://127.0.0.1:9998/v1`. Use `docker-compose.yaml` with the dummyplug service and verify Vizier reaches `http://dummyplug:9998/v1` inside Compose. Build and start the normal Vizier release without building or starting dummyplug; non-dummyplug agents must still work. This feature does not add a GitHub Actions workflow.

## 6. Review order

Review and merge the standalone `vizier-lab/dummyplug` PR first. Then update the `dummyplug/` gitlink in the Vizier PR, rerun all gates against that exact pinned commit, and review the provider, harness, documentation and cascade changes together.
