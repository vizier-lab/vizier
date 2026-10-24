# 2.12 Observability

Vizier can export **traces** and **metrics** over OpenTelemetry (OTLP). The spans follow the [OpenTelemetry GenAI semantic conventions](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/gen-ai), so any backend that understands them (Jaeger, Grafana Tempo, Honeycomb, Langfuse, …) shows model calls, tool calls and agent turns without extra setup.

Export is **off by default**. Nothing is sent, and no exporter is built, until you set an endpoint.

## Enabling export

| Env var | Effect |
|---------|--------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Turns export on. The OTLP/HTTP base URL, e.g. `http://localhost:4318`. |
| `OTEL_METRICS_EXPORTER=none` | Export traces only. Use it when the backend takes no metrics (Jaeger). |
| `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true` | Also record prompts, replies, tool arguments and tool results. See [Content capture](#content-capture). |
| `OTEL_RESOURCE_ATTRIBUTES` | Extra resource attributes, e.g. `deployment.environment=staging`. |

The other standard `OTEL_EXPORTER_OTLP_*` variables (per-signal endpoints, headers, timeout) are read by the exporter itself.

- The protocol is **OTLP over HTTP (protobuf)**, usually port `4318`. The gRPC port `4317` is not supported.
- The service is reported as `service.name=vizier`, with `service.version` set to the binary's version.
- Spans pass through the same filter as logs. They are emitted at `info` level, so a `RUST_LOG` that hides `vizier` at `info` (e.g. `RUST_LOG=warn`) hides the spans too. With `RUST_LOG` unset, the default `vizier=debug` keeps them.
- In Docker, set these variables on the container; `docker-entrypoint.sh` passes the environment through unchanged. The sample `docker-compose.yaml` runs Jaeger next to Vizier with export already set up (UI at `http://localhost:16686`).

```sh
OTEL_EXPORTER_OTLP_ENDPOINT=http://collector:4318 vizier run
```

## What a trace looks like

One trace covers a conversation turn **and the work it starts**. Here a person's message makes the agent run a script that waits for two subagents:

```
invoke_agent otel                         ← the turn
  chat gpt-5                              ← one request to the provider
  execute_tool execute_python             ← a tool call the model made
    python_exec
      execute_tool parallel_subagents    ← a tool called from inside the script
        invoke_agent otel               ← subagent 1's turn
          chat gpt-5
        invoke_agent otel               ← subagent 2's turn
          chat gpt-5
  chat gpt-5                              ← the turn's final answer
```

`consult_agent` and `delegate_agent` nest the same way: the other agent's turn appears under the tool call that started it.

A message from a person (WebUI, Discord, Telegram), a scheduled task run and a dream cycle each start a **new trace**. To see a whole conversation, filter on its `gen_ai.conversation.id` (below).

## Spans

### `invoke_agent {agent}`: one agent turn

| Attribute | Value |
|-----------|-------|
| `gen_ai.operation.name` | `invoke_agent` |
| `gen_ai.agent.id` | the agent id |
| `gen_ai.conversation.id` | the full session slug, e.g. `otel__discord__123456789` |
| `vizier.channel.type` | `discord`, `telegram`, `http`, `task`, `inter_agent`, `subagent`, `dream`, `system` |
| `vizier.channel.id` | Discord channel id, Telegram chat id, or the WebUI channel id |
| `vizier.http.user` | WebUI/API user (`http` only) |
| `vizier.task.slug` | the scheduled task (`task` only) |
| `vizier.inter_agent.participants` | comma-separated agent ids (`inter_agent` only) |
| `vizier.dream.stage` | `extraction` or `consolidation` (`dream` only) |
| `vizier.dream.source` | the conversation being reflected on (`dream` only) |
| `vizier.topic` | the conversation topic, when there is one |
| `vizier.request.kind` | `chat`, `prompt`, `silent_read`, `unattended`, `command`, `audio_chat`, `audio_prompt`, `background_report` |
| `vizier.background_depth` | how many background hops led to this turn (0 for a person's message) |
| `vizier.queue.wait_ms` | time spent queued before the turn started: behind a running turn, or while the agent restarted |
| `vizier.dream` | `true` for a dream-cycle turn |

### `chat {model}`: one request to a provider

A turn makes one `chat` call per model step, and each retry attempt is its own span. The span kind is `CLIENT`.

| Attribute | Value |
|-----------|-------|
| `gen_ai.operation.name` | `chat` |
| `gen_ai.provider.name` | Vizier's provider id (`openai`, `anthropic`, `google`, a custom provider's id, …) |
| `gen_ai.request.model` | the model id |
| `gen_ai.request.max_tokens` | the agent's `max_tokens`, when set |
| `server.address`, `server.port` | the provider's host and port |
| `gen_ai.response.id` | the provider's id for the response (not for Ollama, which gives none) |
| `gen_ai.response.model` | the model that answered, as the provider names it: often a dated version of the requested alias |
| `gen_ai.response.finish_reasons` | why generation stopped: `stop`, `length` (hit the token limit), `tool_call` or `content_filter`; any other provider reason as it came, lower-cased |
| `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens` | token usage |
| `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_creation.input_tokens` | prompt-cache usage |
| `error.type` | on failure: the HTTP status (`429`, `503`, …), or `transport`, `timeout`, `decode`, `config`, `unsupported` |

### `execute_tool {tool}`: one tool call

| Attribute | Value |
|-----------|-------|
| `gen_ai.operation.name` | `execute_tool` |
| `gen_ai.tool.name` | the tool name |
| `gen_ai.tool.type` | `function` |
| `gen_ai.tool.call.id` | the model's tool call id; absent for calls made from inside a script |
| `error.type` | `tool_error`, or `timeout` when the call ran past the agent's tool timeout |

Tools the model calls directly (`execute_python`, `list_tool_functions`, …) and tools a script calls (`memory_search`, MCP tools, …) both get this span. Script calls nest under the script's `python_exec` span.

### `background_job {kind}`: a background job

It opens when `background_subagents` or `delegate_agent` launches the job and closes when the job is reported or cancelled, so its duration is the job's. The blocking `parallel_subagents` tool has no background-job span: its pieces are children of the tool call itself.

| Attribute | Value |
|-----------|-------|
| `vizier.job.id` | the job id, as shown in the WebUI's jobs tray |
| `vizier.job.kind` | `batch` or `delegation` |
| `vizier.job.pieces` | the number of pieces |
| `vizier.job.timeout_secs` | the per-piece time limit |

### `python_exec`

The sandbox run inside `execute_python`, with `agent_id`, `session` and `duration_ms`.

## Content capture

By default spans carry **no message content**, only metadata. Prompts contain the system prompt, CORE.md, memory passages and what people wrote, so content is opt-in, as the GenAI conventions require:

```sh
OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
```

With it set:

| Span | Attribute | Content |
|------|-----------|---------|
| `chat` | `gen_ai.system_instructions` | the system prompt |
| `chat` | `gen_ai.input.messages` | the conversation sent: history, the per-turn context block, tool results |
| `chat` | `gen_ai.output.messages` | the reply, including any tool calls |
| `chat` | `gen_ai.tool.definitions` | the tools offered to the model |
| `execute_tool` | `gen_ai.tool.call.arguments` | the call's arguments. For `execute_python`, the script and its stated intent. |
| `execute_tool` | `gen_ai.tool.call.result` | what the model gets back. For `execute_python`, stdout, the result, any error with its traceback, and the tools the script called. |

Attachments are described by type, name and size; their bytes are never recorded.

> **Keep content capture to backends you control.** Every prompt is sent in full, so spans get large (several KB per model call, far more with long histories), and they contain your users' messages. Leave it off when exporting to a shared or third-party backend.

## Metrics

| Metric | Type | Unit | Attributes |
|--------|------|------|------------|
| `gen_ai.client.token.usage` | histogram | `{token}` | `gen_ai.operation.name`, `gen_ai.provider.name`, `gen_ai.request.model`, `server.address`, `gen_ai.token.type` (`input` / `output`) |
| `gen_ai.client.operation.duration` | histogram | `s` | the same, without `gen_ai.token.type`, plus `error.type` on failure |

Both also carry `gen_ai.response.model` when the provider reports it.

Both are recorded per provider request, with the bucket boundaries the conventions recommend, and exported every 60 seconds (`OTEL_METRIC_EXPORT_INTERVAL`, in milliseconds, changes it).

## Filtering in Jaeger

Jaeger's **Tags** box takes `key=value` pairs separated by spaces, and all of them must match:

| To find | Tags |
|---------|------|
| everything from one Discord channel | `vizier.channel.type=discord vizier.channel.id=123456789` |
| one WebUI user's turns | `vizier.http.user=alice` |
| runs of one scheduled task | `vizier.task.slug=daily-digest` |
| one conversation | `gen_ai.conversation.id=otel__discord__123456789` |
| replies cut off at the token limit | `gen_ai.response.finish_reasons="[\"length\"]"` (see below) |
| rate-limited model calls | `error.type=429` |
| tool calls that timed out | `error.type=timeout` |

`gen_ai.response.finish_reasons` is an array, and Jaeger matches it as the text `["length"]`, not by element, so the value has to be quoted as shown.

Tag search matches individual spans. The channel attributes are on `invoke_agent`, so they find the trace a turn belongs to, but they don't narrow a search to, say, the failing `chat` spans of one channel.

## Local development with Jaeger

The Justfile runs a local Jaeger and points Vizier at it:

```sh
just jaeger        # Jaeger all-in-one in Docker; UI at http://localhost:16686
just run-otel      # `just run` with traces exported to it and content capture on
just dev-otel      # the same, with hot reload
just jaeger-down   # remove the container
```

`run-otel` sets `OTEL_METRICS_EXPORTER=none`, because Jaeger stores traces only. To see the metrics, point `OTEL_EXPORTER_OTLP_ENDPOINT` at a collector or backend that accepts them, such as an OpenTelemetry Collector feeding Prometheus, or the `grafana/otel-lgtm` image.

## Limitations

- **Only the first candidate is read.** Vizier asks for one reply per request, so `gen_ai.response.finish_reasons` always holds one value.
- **`gen_ai.provider.name` is Vizier's provider id**, not the conventions' well-known values (e.g. `google` rather than `gcp.gemini`).
- **Retries are not grouped.** Each attempt is its own `chat` span, with nothing tying the attempts or their backoff waits together.
- **Memory retrieval, checkpoints and MCP connections have no spans of their own.**
